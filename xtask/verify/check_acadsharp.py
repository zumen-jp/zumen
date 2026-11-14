"""検証キットの DWG を ACadSharp で読んだ値と、zumen が読んだ値を照らす（#503、決定 0254）。

使い方: 先に acadsharp/ のツールでキットの DWG を読み（acadsharp-dump.json を作る）、次に
        python check_acadsharp.py <キットのフォルダー>

DWG のファイルごとに、次を確かめる。

- ACadSharp が読める。書き戻したファイルの読みで、入力にない通知（警告・誤り）が出ない。
- zumen が読んだ画層の名前が、ACadSharp の画層の表と同じ。
- zumen の要素の出所のハンドルの図形が ACadSharp にあり、種類・画層と、モデル空間では形の種類と値（端点・中心・
  半径・角度・位置・文字・尺度・格子・属性など）が同じ。形のフィールドは両向きに比べる。比べられる種類のモデル空間の
  図形は、zumen の要素が形を持つ。
- 要素にならないモデル空間の図形の数が、種類ごとに、移さなかったことを言う zumen の診断（compare.py の
  LIFTED_AWAY。kind は図形の種類。kind を持たない code は、その code が言いうる種類（KINDLESS）に足す）の件数を
  超えない。DWG の診断の場所はバイトの範囲で、ACadSharp は図形の
  バイトの位置を返さないので、図形ごとには結ばない。そのため、ブロックの定義の中など、ほかの所の同じ種類の図形を
  言う診断と区別しない。
- 書き戻したファイルでは、変更で決めた形と画層（manifest.json の expect）と、足した画層の色・変えた色・点の
  表し方が、ACadSharp で読んだ値と同じ。
"""

import json
import math
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare import ATTACHMENTS, KINDLESS, check_handles, lifted_away, run, usage  # noqa: E402

# 既知の食い違い: (ファイルの道筋の初め, 食い違いの文の初め, issue, 当たるファイルの数)。直ったら消す
KNOWN = []
# ACadSharp で照らさない入力。適合テストの合成の DWG（open_dwg.json）は、扱えない型と壊れたオブジェクトをわざと含み、
# ACadSharp は読めない。直す見込みのない食い違いなので KNOWN に入れず、照らす所から外す。変換した DXF は ezdxf で照らす（#690）
SKIPPED = {"input/conformance-open_dwg-a.dwg"}
MODEL = "*model_space"
# ヘッダーの変数のうち、HANDSEED より後ろに書くもの（crates/zumen-acad/src/dwg/header.rs の並び）。CLAYER は ACadSharp が
# 名前で返すので、入力が既定の画層 0 なら、ハンドルが画層に当たらないずれは見分けにくい（推定）。R2010 からのハンドルの
# ストリームのずれは、画層・図形の照らしと通知の数でも見つかる
AFTER_SEED = ("INSUNITS", "PLIMMIN", "PLIMMAX", "CLAYER")


def caret(text):
    """DWG の生の制御文字を、zumen・ASCII DXF と同じ `^` 表記に直す（crates/zumen-acad/src/db.rs の caret）"""
    out = []
    for ch in text:
        code = ord(ch)
        if code < 0x20:
            out.append("^" + chr(code + 0x40))
        elif ch == "^":
            out.append("^ ")
        else:
            out.append(ch)
    return "".join(out)


def geometry(d):
    """ACadSharp の図形の値を、図面モデルの形の値（entities の geometry）の形にする。比べない種類は None。
    ACadSharp の角度はラジアン。値の意味は DXF Reference の同じ名前の図形と同じ"""
    t = d["type"]
    if t == "LINE":
        return {"shape": "line", "start": d["start"], "end": d["end"]}
    if t == "CIRCLE":
        return {"shape": "circle", "center": d["center"], "radius": d["radius"]}
    if t == "ARC":
        return {
            "shape": "arc",
            "center": d["center"],
            "radius": d["radius"],
            "start_angle": math.degrees(d["start_angle"]),
            "end_angle": math.degrees(d["end_angle"]),
        }
    if t == "ELLIPSE":
        # 長軸は中心から長軸の端への向き。図面モデルの major は長軸の端の点
        return {
            "shape": "ellipse",
            "center": d["center"],
            "major": [d["center"][0] + d["major_axis"][0], d["center"][1] + d["major_axis"][1]],
            "ratio": d["ratio"],
            "start_angle": math.degrees(d["start_param"]),
            "end_angle": math.degrees(d["end_param"]),
        }
    if t == "LWPOLYLINE":
        if d.get("has_width", False):
            return None
        g = {"shape": "polyline", "points": [[p[0], p[1]] for p in d["points"]], "closed": d["closed"]}
        if any(p[2] for p in d["points"]):
            g["bulges"] = [p[2] for p in d["points"]]
        return g
    if t == "POINT":
        return {"shape": "point", "position": d["location"], "rotation": math.degrees(d["rotation"])}
    if t == "TEXT":
        width_factor = d.get("width_factor", 1.0)
        if not isinstance(width_factor, (int, float)) or not math.isfinite(width_factor) or width_factor <= 0.0:
            return None
        g = {"shape": "text", "content": caret(d["text"]), "height": d["height"], "rotation": math.degrees(d["rotation"]), "style": d["style"]}
        # 横の寄せ方が 3（aligned）・5（fit）なら 2 点の間に置き、ほかで寄せ方が 0・0 でなければ 2 点目に寄せる
        h, v = d["halign"], d["valign"]
        if h in (3, 5):
            if d["insert"] == d["align_point"]:
                return None
            g["position"], g["end"] = d["insert"], d["align_point"]
            g["fit"] = "aligned" if h == 3 else "fit"
            g["rotation"] = math.degrees(math.atan2(g["end"][1] - g["position"][1], g["end"][0] - g["position"][0]))
        else:
            g["position"] = d["insert"] if h == 0 and v == 0 else d["align_point"]
            if h_name := {1: "center", 2: "right", 4: "middle"}.get(h):
                g["horizontal"] = h_name
            if v_name := {1: "bottom", 2: "middle", 3: "top"}.get(v):
                g["vertical"] = v_name
        if width_factor != 1.0:
            g["width_factor"] = width_factor
        oblique = math.degrees(d.get("oblique", 0.0))
        if oblique != 0.0:
            g["oblique"] = oblique
        mirror = d.get("mirror", 0)
        if mirror & 2:
            g["backward"] = True
        if mirror & 4:
            g["upside_down"] = True
        return g
    if t == "MTEXT":
        g = {
            "shape": "formatted_text",
            "content": caret(d["text"]),
            "position": d["insert"],
            "height": d["height"],
            "rotation": math.degrees(d["rotation"]),
            "attachment": ATTACHMENTS[d["attachment"] - 1],
            "style": d["style"],
        }
        if d["width"] > 0:
            g["width"] = d["width"]
        return g
    if t == "INSERT":
        g = {
            "shape": "symbol",
            "name": d["name"],
            "position": d["insert"],
            "scale": [d["xscale"], d["yscale"]],
            "rotation": math.degrees(d["rotation"]),
        }
        if d["columns"] > 1 or d["rows"] > 1:
            g["grid"] = {
                "columns": d["columns"],
                "rows": d["rows"],
                "column_spacing": d["column_spacing"],
                "row_spacing": d["row_spacing"],
            }
        if d["attributes"]:
            g["attributes"] = d["attributes"]
        return g
    return None


def view(dump):
    """共有の照らし方（compare.check_handles）に渡す ACadSharp の読み"""
    entities = dump["entities"]
    block_names = {b.lower() for b in dump.get("blocks", []) if b}
    by_owner = {}
    for d in entities.values():
        by_owner.setdefault((d.get("owner") or "").lower(), []).append(d)

    def entity(handle):
        d = entities.get(handle)
        if d is None:
            return None
        # owner は、図形を持つブロックの名前（記号の定義の中の要素を、定義の名前と照らす）
        out = {"type": d["type"], "layer": d["layer"], "color": d["color"], "line_type": d["line_type"], "owner": d["owner"], "geometry": geometry(d)}
        if "block" in d:
            out["part"] = {
                "block": d["block"] or "",
                "measurement": d["measurement"],
                "text_position": d["text_midpoint"],
                "override": caret(d.get("text", "")),
            }
        return out

    def block(name):
        if name.lower() not in block_names:
            return None
        return [{"type": d["type"], "layer": d["layer"], "geometry": geometry(d)} for d in by_owner.get(name.lower(), [])]

    return {
        "tool": "ACadSharp",
        "layers": set(dump["layers"]),
        "layer_colors": {name.lower(): abs(color) for name, color in dump["layers"].items() if color is not None},
        "entity": entity,
        "block": block,
        "header": (dump["header"]["PDMODE"], dump["header"]["PDSIZE"]),
    }


def check_file(dumps, entry, reading, baseline, counts):
    """1 つの DWG のファイルの食い違い。比べた数を counts に足す"""
    if entry["path"] in SKIPPED:
        return [], None
    dump = dumps.get(entry["path"])
    if dump is None:
        return ["ACadSharp の読みがない（acadsharp-dump.json を作り直す）"], None
    if "error" in dump:
        return [f"ACadSharp が読めない: {dump['error']}"], None
    failures = []
    notes = sorted({f"{n['type']}: {n['message']}" for n in dump["notifications"]})
    if entry["role"] == "apply" and baseline is not None:
        added = [n for n in notes if n not in baseline]
        if added:
            failures.append(f"ACadSharp の読みで、入力にない通知が出た: {added[:3]}")
    if reading is None:
        return failures, notes

    found, seen = check_handles(entry, reading, counts, view(dump))
    failures += found

    # HANDSEED の大きさ（バイト）が増える数の図形を足したファイル（#701）では、ファイルの HANDSEED（ACadSharp でヘッダー
    # だけを読んだ値）が、足した最後の図形の次になる。HANDSEED より後ろに書く変数（AFTER_SEED）は入力のまま
    if entry.get("recipe") == "handseed":
        seed = dump["header"].get("HANDSEED")
        last = max(int(handle, 16) for handle in dump["entities"])
        source = dumps.get(entry["from"], {}).get("header", {})
        for name in AFTER_SEED:
            if name not in source or dump["header"].get(name) != source[name]:
                failures.append(f"HANDSEED の後ろの {name} が {dump['header'].get(name)!r}（入力は {source.get(name)!r}）")
        if seed != last + 1 or last <= 0xFFFF:
            failures.append(f"HANDSEED が {seed}（期待は、0xFFFF より大きい最後の図形のハンドル {last} の次）")

    # 黙って捨てない: 要素にならないモデル空間の図形の数は、種類ごとに、移さなかったことを言う zumen の診断（kind は
    # 図形の種類）の件数を超えない。DWG の診断の場所はバイトの範囲なので、図形ごとには結ばない
    away, kindless = Counter(), Counter()
    for d in lifted_away(entry, reading):
        kind, n = d.get("params", {}).get("kind"), d.get("count", len(d.get("at", [])))
        if kind:
            away[kind.upper()] += n
        else:
            kindless[d["code"]] += n
    dropped = Counter(d["type"].upper() for handle, d in dump["entities"].items() if d["owner"].lower() == MODEL and handle not in seen)
    missing = {kind: n - away[kind] for kind, n in sorted(dropped.items()) if n > away[kind]}
    # kind を持たない code は、その code が言いうる種類（KINDLESS）の残りに足す。種類の組は code どうしで重ならない
    for code, n in kindless.items():
        for kind in sorted(KINDLESS.get(code, set()) & missing.keys()):
            covered = min(n, missing[kind])
            missing[kind] -= covered
            n -= covered
    missing = {kind: n for kind, n in missing.items() if n}
    if missing:
        failures.append(f"要素にならず、移さなかったことを言う診断もないモデル空間の図形: {missing}")
    return failures, notes


def main():
    folder = usage("check_acadsharp.py")
    if folder is None:
        return 2
    dump = json.loads((folder / "acadsharp-dump.json").read_text(encoding="utf-8"))
    return run(folder, "acadsharp", dump["acadsharp"], {"dwg"}, lambda _, *rest: check_file(dump["files"], *rest), KNOWN)


if __name__ == "__main__":
    sys.exit(main())
