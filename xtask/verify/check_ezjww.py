"""確認の束の JWW を ezjww で読み、zumen が読んだ値と照らす（#503、決定 0254）。

使い方: python check_ezjww.py <束のフォルダー>

JWW のファイルごとに、次を確かめる。

- ezjww が読める。ezjww の監査（ezjww.audit）と読みの診断で、書き戻したファイルに入力にない項目が出ない。
- 図形の一覧の図形は、どれも zumen の要素になるか、移さなかったことを言う zumen の診断（compare.py の
  LIFTED_AWAY）がそのレコードを指す（黙って捨てない）。zumen のレコードの番号は、見出しを 0 とし、図形の
  一覧の i 番目（0 から）を i + 1 とする（決定 0133）。zumen が描かずに拡張に持つ文字（(0, -1000) の設定と、
  `^@BM` でない `^@` で始まる文字。docs/formats/jww.md）は数えず、要素の出所にあれば食い違いにする。束の JWW には
  zumen が移さない図形も描かない文字もないので、診断と結ぶ枝と描かない文字の枝は、受け取った図面でだけ働く（#558）。
- zumen の要素の出所の図形の種類・レイヤと、線と点では形の値が同じ（形を返さない要素は、種類を比べない。#558）。
  形の欄は両向きに比べ、線と点の要素は形を持つ。ezjww の座標は用紙の mm なので、図形のレイヤグループの縮尺の分母を掛けて、zumen の実寸の座標と比べる
  （docs/formats/jww.md）。
- 書き戻したファイルでは、変更で決めた形・レイヤ・ペンの番号（manifest.json の expect）が、ezjww で読んだ図形と
  同じ。
"""

import re
import sys
from collections import Counter
from importlib.metadata import version
from pathlib import Path

import ezjww

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare import differences, lifted_away_records, run, usage  # noqa: E402

# 図面モデルの形 → ezjww の図形の種類
TYPES = {
    "line": {"LINE"},
    "circle": {"CIRCLE", "ARC"},
    "arc": {"ARC"},
    "ellipse": {"ARC", "CIRCLE"},
    "point": {"POINT"},
    "text": {"TEXT"},
    "symbol": {"BLOCK"},
    "dimension": {"DIMENSION"},
}
# 既知の食い違い: (ファイルの道筋の初め, 食い違いの文の初め, issue, 当たるファイルの数)。直ったら消す
KNOWN = []
# 記号の点の線種（docs/formats/jww.md）
MARKER_PEN = 100
# 設定を書いた文字の位置と、`名前 = 値` の名前（ASCII の英字の後に英数字か _。crates/zumen-jww/src/drawing.rs の
# setting）
SETTING_AT = (0.0, -1000.0)
SETTING = re.compile(r"[A-Za-z][A-Za-z0-9_]* = ")


def undrawn(e):
    """zumen が描かずに拡張に持つ文字か。(0, -1000) の `名前 = 値` は設定（jww.setting）、`^@BM` でない `^@` で
    始まる文字は制御（jww.control）とする（docs/formats/jww.md）"""
    if e["type"] != "TEXT":
        return False
    content = e["content"]
    if (e["start_x"], e["start_y"]) == SETTING_AT and SETTING.match(content):
        return True
    return content.startswith("^@") and not content.startswith("^@BM")


def scale(header, group):
    """レイヤグループの縮尺の分母。正でなければ 1"""
    groups = header.get("layer_groups", [])
    value = groups[group]["scale"] if group < len(groups) else 1.0
    return value if value > 0 else 1.0


def layer_name(header, base):
    """zumen のレイヤの名前。名前が空なら「グループ-レイヤ」（16 進）"""
    group, layer = base["layer_group"], base["layer"]
    try:
        name = header["layer_groups"][group]["layers"][layer]["name"]
    except (IndexError, KeyError):
        name = ""
    return name or f"{group:x}-{layer:x}"


def geometry(header, e):
    """ezjww の図形を、図面モデルの実寸の形の値にする。比べない種類は None"""
    s = scale(header, e["base"]["layer_group"])
    if e["type"] == "LINE":
        return {"shape": "line", "start": [e["start_x"] * s, e["start_y"] * s], "end": [e["end_x"] * s, e["end_y"] * s]}
    if e["type"] == "POINT":
        g = {"shape": "point", "position": [e["x"] * s, e["y"] * s], "rotation": 0.0}
        if e["base"]["pen_style"] == MARKER_PEN:
            g["rotation"] = e["angle"]
            g["marker"] = {"namespace": "jww.point", "code": str(e["code"]), "scale": e["scale"]}
        if e["is_temporary"]:
            g["construction"] = True
        return g
    return None


def audit(path, doc):
    """ezjww の監査と読みの診断の code"""
    report = ezjww.audit(str(path))
    codes = list(report.get("issue_codes", [])) + [str(d.get("code", d)) for d in doc.get("diagnostics", [])]
    return sorted(Counter(codes).items())


def check_file(folder, entry, reading, baseline, counts):
    """1 つの JWW のファイルの食い違い。比べた数を counts に足す"""
    path = folder / entry["path"]
    try:
        doc = ezjww.read_document(str(path))
        found = audit(path, doc)
    except Exception as error:  # noqa: BLE001 ezjww が読めないことそのものを結果にする
        return [f"ezjww が読めない: {type(error).__name__}: {error}"], None
    failures = []
    if entry["role"] == "apply" and baseline is not None:
        before = dict(baseline)
        added = {code: n for code, n in found if n > before.get(code, 0)}
        if added:
            failures.append(f"ezjww の監査で、入力より項目が増えた: {added}")
    if reading is None:
        return failures, found

    header, entities = doc["header"], doc["entities"]
    seen = set()
    for element in reading["entities"]:
        record = element.get("source", {}).get("record")
        # 図形の一覧の図形だけを結ぶ（ブロックの定義の中の図形は、一覧の後ろのレコード）
        if record is None or not 1 <= record <= len(entities) or not element["id"].startswith("space/"):
            continue
        seen.add(record)
        e = entities[record - 1]
        where = element["id"]
        # 描かずに拡張に持つ文字は、要素にならない
        if undrawn(e):
            failures.append(f"{where}: 描かない文字（TEXT のレコード {record}）が要素になった")
            continue
        shape = element.get("geometry", {}).get("shape")
        # 表にない形は、出所の種類を比べられないので知らせる（TYPES に足す）
        if shape is not None and shape not in TYPES:
            failures.append(f"{where}: 形 {shape} と図形の種類の対応がない（check_ezjww.py の TYPES に足す）")
            continue
        if shape in TYPES and e["type"] not in TYPES[shape]:
            failures.append(f"{where}: 形 {shape} の出所が {e['type']}")
            continue
        if element.get("layer") != layer_name(header, e["base"]):
            failures.append(f"{where}: レイヤが {element.get('layer')!r}、ezjww は {layer_name(header, e['base'])!r}")
        counts["elements"] += 1
        got = geometry(header, e)
        # 形を比べる種類（線と点）の要素は、形を持つ
        if shape is None and got is not None:
            failures.append(f"{where}: {e['type']} のレコード {record} の要素が形を持たない")
        if shape in TYPES and got is not None:
            counts["shapes"] += 1
            for d in differences(element["geometry"], got, both=True):
                failures.append(f"{where}（{e['type']} のレコード {record}）{d}")

    # 黙って捨てない: 移さなかったことを言う診断が、そのレコードを指す。kind はクラスの名前で、ezjww の種類の名前と
    # 違うので、レコードだけで結ぶ。描かずに拡張に持つ文字は数えない
    told = {record for _, record in lifted_away_records(entry, reading)}
    dropped = Counter(e["type"] for i, e in enumerate(entities) if i + 1 not in seen and i + 1 not in told and not undrawn(e))
    if dropped:
        failures.append(f"要素にならず、移さなかったことを言う診断もない図形: {dict(dropped)}")

    # 書き戻した値
    if entry["role"] == "apply":
        by_id = {e["id"]: e for e in reading["entities"]}
        for step in entry["steps"]:
            for want in step.get("expect", []):
                record = by_id.get(want["id"], {}).get("source", {}).get("record")
                if record is None or not 1 <= record <= len(entities):
                    failures.append(f"変更 {want['edit']} の要素 {want['id']} を、ezjww の図形と結べない")
                    continue
                e = entities[record - 1]
                got = geometry(header, e)
                if got is None:
                    failures.append(f"変更 {want['edit']} の要素 {want['id']} を ezjww で比べられない（{e['type']}）")
                    continue
                counts["expected"] += 1
                for d in differences(want["geometry"], got):
                    failures.append(f"変更 {want['edit']}（{want['id']}）{d}")
                if "layer" in want and want["layer"] != layer_name(header, e["base"]):
                    failures.append(f"変更 {want['edit']}（{want['id']}）のレイヤが {layer_name(header, e['base'])!r}（期待は {want['layer']!r}）")
                # 足した図形の色は、ペンの番号（docs/formats/jww.md）
                pen = want.get("color", {}).get("jww")
                if pen is not None and e["base"]["pen_color"] != pen:
                    failures.append(f"変更 {want['edit']}（{want['id']}）のペンが {e['base']['pen_color']}（期待は {pen}）")
    return failures, found


def main():
    folder = usage("check_ezjww.py")
    if folder is None:
        return 2
    return run(folder, "ezjww", version("ezjww"), {"jww"}, check_file, KNOWN)


if __name__ == "__main__":
    sys.exit(main())
