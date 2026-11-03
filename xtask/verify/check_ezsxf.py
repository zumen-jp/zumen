"""確認の束の SFC・P21 を ezsxf で読み、zumen が読んだ値と照らす（#503、決定 0254）。

使い方: python check_ezsxf.py <束のフォルダー>

ファイルごとに、次を確かめる。

- ezsxf が読める（strict にせず、警告を集める）。書き戻したファイルで、入力にない種類の警告が出ない。警告は code の
  種類で比べる（フィーチャごとの警告は、足したフィーチャの分だけ増えるため）。
- zumen の要素の出所のレコードの番号が、docs/formats/sxf.md の「レコードの区切り」で数えた番号と同じ。
- SFC: zumen の要素の出所のインスタンスのフィーチャが ezsxf にあり、種類・レイヤと、線・円・点マーカーでは形の
  値が同じ（形の欄は両向きに比べる）。どの要素も用紙（drawing_sheet_feature）の中にある。ezsxf がフィーチャを型で
  読まない SFC（印が `/*SXF3` など）は、インスタンスがあることとキーワードだけを比べ、結果の notes に書く。make.py が
  作った SFC（made-features）とその書き戻しを ezsxf が型で読まなければ、失敗にする。
- P21: zumen の要素の出所のインスタンスがあり、形に合う表記の実体（点は ANNOTATION_SYMBOL_OCCURRENCE、線などの曲線は
  ANNOTATION_CURVE_OCCURRENCE、文字は ANNOTATION_TEXT_OCCURRENCE）である。レイヤ（PRESENTATION_LAYER_ASSIGNMENT）と、
  用紙（DRAWING_SHEET_REVISION）の中にあることを比べる。参照をたどって、点の位置・回転・倍率と、線の端点を比べる
  （印の種類と色は比べない）。
- 黙って捨てない: 図形のフィーチャ（SFC。ezsxf が型で読まずに飛ばしたものも含む）と、用紙に置いた表記の実体
  （P21）は、どれも zumen の要素になるか、移さなかったことを言う zumen の診断（sxf.lift.dropped・sxf.lift.invalid）が
  そのレコードを指す。
- 書き戻したファイルでは、変更で決めた形・レイヤ（manifest.json の expect）と、SFC では色（既定義色の番号）が、
  ezsxf で読んだ値と同じ。
"""

import math
import re
import sys
from collections import Counter
from importlib.metadata import version
from pathlib import Path

import ezsxf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare import differences, lifted_away_records, run, usage  # noqa: E402

# 図面モデルの形 → ezsxf のフィーチャの種類（SFC）
KINDS = {
    "line": {"line"},
    "circle": {"circle"},
    "arc": {"arc"},
    "point": {"point_marker"},
    "polyline": {"polyline"},
    "text": {"text"},
}
# 図面モデルの形 → SFC のフィーチャの名前（ezsxf が型で読まない SFC で比べる）
KEYWORDS = {
    "line": {"line_feature"},
    "circle": {"circle_feature"},
    "arc": {"arc_feature"},
    "point": {"point_marker_feature"},
    "polyline": {"polyline_feature"},
    "text": {"text_string_feature"},
}
# 図形のフィーチャの名前。どれも zumen の要素になるか、移さなかったことを言う診断がある
DRAWN = {
    "point_marker_feature",
    "line_feature",
    "polyline_feature",
    "circle_feature",
    "arc_feature",
    "ellipse_feature",
    "ellipse_arc_feature",
    "text_string_feature",
    "spline_feature",
    "clothoid_feature",
}
# 図面モデルの形 → P21 の表記の実体（ISO 10303-46 の annotation_occurrence の下位の型）
OCCURRENCES = {
    "point": {"ANNOTATION_SYMBOL_OCCURRENCE"},
    "line": {"ANNOTATION_CURVE_OCCURRENCE"},
    "circle": {"ANNOTATION_CURVE_OCCURRENCE"},
    "arc": {"ANNOTATION_CURVE_OCCURRENCE"},
    "ellipse": {"ANNOTATION_CURVE_OCCURRENCE"},
    "polyline": {"ANNOTATION_CURVE_OCCURRENCE"},
    "text": {"ANNOTATION_TEXT_OCCURRENCE"},
}
# SFC のフィーチャの印と閉じ（docs/formats/sxf.md の「レコードの区切り」。長いものから比べる）
MARKS = [("/*SXF3.1", "SXF3.1*/"), ("/*SXF3", "SXF3*/"), ("/*SXF", "SXF*/")]
# make.py が、ezsxf がフィーチャを型で読むように作った SFC
MADE = "input/made-features.sfc"
# 既知の食い違い: (ファイルの道筋の初め, 食い違いの文の初め, issue, 当たるファイルの数)。直ったら消す
KNOWN = []


def records(text):
    """インスタンスの番号 → zumen のレコードの番号。docs/formats/sxf.md の「レコードの区切り」のとおり、`;` で終わる文と、
    データの節の `/*SXF` から閉じまでを、ファイルの初めから 0 で数える。間のコメントは数えない"""
    found, n, i, data = {}, 0, 0, False
    end = len(text)
    while i < end:
        if text[i].isspace():
            i += 1
            continue
        if text.startswith("/*", i):
            mark = next((m for m in MARKS if data and text.startswith(m[0], i)), None)
            opening, closing = mark or ("/*", "*/")
            close = text.find(closing, i + len(opening))
            stop = end if close < 0 else close + len(closing)
            if mark:
                m = re.match(r"\s*#(\d+)", text[i + len(mark[0]) : stop])
                if m:
                    found[int(m.group(1))] = n
                n += 1
            i = stop
            continue
        start = i
        while i < end and text[i] != ";":
            if text[i] == "'":
                # 文字列の中の ; は区切りでない。'' は ' を 1 つ書いたもの（ISO 10303-21 の文字列）
                i += 1
                while i < end and (text[i] != "'" or text.startswith("''", i)):
                    i += 2 if text.startswith("''", i) else 1
            elif text.startswith("/*", i):
                close = text.find("*/", i + 2)
                i = end if close < 0 else close + 2
                continue
            i += 1
        statement = text[start:i].strip()
        i += 1
        m = re.match(r"#(\d+)\s*=", statement)
        if m:
            found[int(m.group(1))] = n
        if statement.upper() == "DATA":
            data = True
        elif statement.upper() == "ENDSEC":
            data = False
        n += 1
    return found


def xy(p):
    return [p["x"], p["y"]]


def feature_geometry(f):
    """ezsxf の SFC のフィーチャを、図面モデルの形の値にする。比べない種類は None"""
    if f["kind"] == "line":
        return {"shape": "line", "start": xy(f["start"]), "end": xy(f["end"])}
    if f["kind"] == "circle":
        return {"shape": "circle", "center": xy(f["center"]), "radius": f["radius"]}
    if f["kind"] == "point_marker":
        return {
            "shape": "point",
            "position": xy(f["position"]),
            "rotation": f["rotation_angle_deg"],
            "marker": {"namespace": "sxf.marker", "code": str(f["marker_code"]), "scale": f["scale"]},
        }
    return None


def instance_of(element):
    instance = element.get("source", {}).get("instance")
    return None if instance is None else int(instance)


def check_records(reading, at):
    """zumen の要素の出所のレコードの番号が、数えた番号と同じか"""
    failures = []
    for element in reading["entities"]:
        instance, record = instance_of(element), element.get("source", {}).get("record")
        if instance is not None and record is not None and at.get(instance) != record:
            failures.append(f"{element['id']}: #{instance} のレコードの番号が {record}、数えると {at.get(instance)}")
    return failures


def unlifted(found, at, entry, reading, what):
    """要素にならなかった図形のうち、移さなかったことを言う診断がレコードを指さないもの"""
    told = {record for _, record in lifted_away_records(entry, reading)}
    dropped = Counter(keyword for instance, keyword in found if at.get(instance) not in told)
    return [f"要素にならず、移さなかったことを言う診断もない{what}: {dict(dropped)}"] if dropped else []


def check_sfc(parsed, entry, reading, counts, notes, at):
    features = {f["id"]: f for f in parsed["typed_features"]}
    if not any(f["keyword"] in DRAWN for f in features.values()):
        notes.append("ezsxf がフィーチャを型で読まないので、インスタンスとキーワードだけを比べる")
        return check_raw_sfc(parsed, entry, reading, counts, at)
    model = parsed["model"]
    layers = {t["code"]: features.get(t["entity_id"], {}).get("name") for t in model["code_tables"]["layers"]}
    sheet = set((model.get("sheet") or {}).get("component_ids", []))
    failures, matched = [], {}
    for element in reading["entities"]:
        instance = instance_of(element)
        if instance is None:
            continue
        where = element["id"]
        f = features.get(instance)
        if f is None:
            failures.append(f"{where}: ezsxf に #{instance} のフィーチャがない")
            continue
        matched[where] = f
        shape = element.get("geometry", {}).get("shape")
        # 表にない形は、出所の種類を比べられないので知らせる（KINDS に足す）
        if shape is not None and shape not in KINDS:
            failures.append(f"{where}: 形 {shape} とフィーチャの種類の対応がない（check_ezsxf.py の KINDS に足す）")
            continue
        if shape in KINDS and f["kind"] not in KINDS[shape]:
            failures.append(f"{where}: 形 {shape} の出所が {f['kind']}")
            continue
        layer = layers.get((f.get("style") or {}).get("layer_code"))
        if element.get("layer") is not None and layer is not None and element["layer"] != layer:
            failures.append(f"{where}: レイヤが {element['layer']!r}、ezsxf は {layer!r}")
        if where.startswith("sheet/") and instance not in sheet:
            failures.append(f"{where}: #{instance} が ezsxf の用紙の中にない")
        counts["elements"] += 1
        got = feature_geometry(f)
        if shape in KINDS and got is not None:
            counts["shapes"] += 1
            for d in differences(element["geometry"], got, both=True):
                failures.append(f"{where}（{f['kind']} #{instance}）{d}")
    # 黙って捨てない。ezsxf が型で読まずに飛ばしたフィーチャ（引数の誤りなど）も、生のレコードの名前で数える
    seen = {instance_of(e) for e in reading["entities"]}
    raw = {e["id"]: e["record"]["keyword"].lower() for e in parsed["entities"] if e.get("body_type") == "simple"}
    failures += unlifted([(i, k) for i, k in raw.items() if k in DRAWN and i not in seen], at, entry, reading, "フィーチャ")
    out = {}
    for where, f in matched.items():
        style = f.get("style") or {}
        out[where] = (feature_geometry(f), layers.get(style.get("layer_code")), style.get("color_code"))
    return failures, out


def check_raw_sfc(parsed, entry, reading, counts, at):
    """ezsxf が型で読まない SFC: 要素の出所のインスタンスがあり、形に合うフィーチャの名前である"""
    raw = {e["id"]: e["record"]["keyword"].lower() for e in parsed["entities"] if e.get("body_type") == "simple"}
    failures, seen = [], set()
    for element in reading["entities"]:
        instance = instance_of(element)
        if instance is None:
            continue
        where = element["id"]
        seen.add(instance)
        keyword = raw.get(instance)
        if keyword is None:
            failures.append(f"{where}: ezsxf に #{instance} のフィーチャがない")
            continue
        shape = element.get("geometry", {}).get("shape")
        if shape is not None and shape not in KEYWORDS:
            failures.append(f"{where}: 形 {shape} とフィーチャの名前の対応がない（check_ezsxf.py の KEYWORDS に足す）")
            continue
        if shape in KEYWORDS and keyword not in KEYWORDS[shape]:
            failures.append(f"{where}: 形 {shape} の出所が {keyword}")
            continue
        counts["elements"] += 1
    failures += unlifted([(i, k) for i, k in raw.items() if k in DRAWN and i not in seen], at, entry, reading, "フィーチャ")
    return failures, {}


class Step:
    """P21 のインスタンスの参照をたどる。引数の位置は、ISO 10303-42（幾何）・-46（表示）・-101（製図）の実体の
    定義の属性の順（下の各所に書く）"""

    def __init__(self, parsed):
        self.records = {}
        for e in parsed["entities"]:
            if e.get("body_type") == "simple":
                self.records[e["id"]] = e["record"]

    def get(self, value, keyword=None):
        """参照の先のレコード。型が違えば None"""
        if isinstance(value, dict) and value.get("kind") == "reference":
            record = self.records.get(value["value"])
            if record is not None and (keyword is None or record["keyword"] == keyword):
                return record
        return None

    def refs(self, values):
        """参照の並びの、インスタンスの番号"""
        return [v["value"] for v in values or [] if isinstance(v, dict) and v.get("kind") == "reference"]

    def point(self, value):
        # cartesian_point (name, coordinates)（ISO 10303-42）
        record = self.get(value, "CARTESIAN_POINT")
        return [float(c) for c in record["parameters"][1]] if record else None

    def layers(self):
        """インスタンスの番号 → レイヤの名前。presentation_layer_assignment (name, description, assigned_items)
        （ISO 10303-46）"""
        out = {}
        for record in self.records.values():
            if record["keyword"] == "PRESENTATION_LAYER_ASSIGNMENT":
                for instance in self.refs(record["parameters"][2]):
                    out[instance] = record["parameters"][0]
        return out

    def sheet(self):
        """用紙に置いたインスタンスの番号。drawing_sheet_revision は representation (name, items, context_of_items) の
        下位の型（ISO 10303-101）"""
        return {i for r in self.records.values() if r["keyword"] == "DRAWING_SHEET_REVISION" for i in self.refs(r["parameters"][1])}

    def geometry(self, record, shape):
        """点と線の、図面モデルの形の値。たどれなければ None"""
        # annotation_occurrence は styled_item (name, styles, item)（ISO 10303-46）
        if shape == "point":
            # defined_symbol (name, definition, target)・symbol_target (name, placement, x_scale, y_scale)（ISO 10303-46）
            symbol = self.get(record["parameters"][2], "DEFINED_SYMBOL")
            target = symbol and self.get(symbol["parameters"][2], "SYMBOL_TARGET")
            # axis2_placement_2d (name, location, ref_direction)・direction (name, direction_ratios)（ISO 10303-42）
            placement = target and self.get(target["parameters"][1], "AXIS2_PLACEMENT_2D")
            if not placement:
                return None
            direction = self.get(placement["parameters"][2], "DIRECTION")
            dx, dy = (float(c) for c in direction["parameters"][1]) if direction else (1.0, 0.0)
            return {
                "shape": "point",
                "position": self.point(placement["parameters"][1]),
                "rotation": math.degrees(math.atan2(dy, dx)),
                "marker": {"scale": float(target["parameters"][2])},
            }
        if shape == "line":
            # trimmed_curve (name, basis_curve, trim_1, trim_2, sense_agreement, master_representation)（ISO 10303-42）
            curve = self.get(record["parameters"][2], "TRIMMED_CURVE")
            if not curve:
                return None
            start, end = (self.point(trim[0]) if trim else None for trim in curve["parameters"][2:4])
            return {"shape": "line", "start": start, "end": end}
        return None


def check_p21(parsed, entry, reading, counts, notes, at):
    step = Step(parsed)
    layers, sheet = step.layers(), step.sheet()
    failures, matched, seen = [], {}, set()
    for element in reading["entities"]:
        instance = instance_of(element)
        if instance is None:
            continue
        where = element["id"]
        seen.add(instance)
        record = step.records.get(instance)
        if record is None:
            failures.append(f"{where}: ezsxf に #{instance} のインスタンスがない")
            continue
        shape = element.get("geometry", {}).get("shape")
        # 表にない形は、出所の種類を比べられないので知らせる（OCCURRENCES に足す）
        if shape is not None and shape not in OCCURRENCES:
            failures.append(f"{where}: 形 {shape} と表記の実体の対応がない（check_ezsxf.py の OCCURRENCES に足す）")
            continue
        if shape in OCCURRENCES and record["keyword"] not in OCCURRENCES[shape]:
            failures.append(f"{where}: 形 {shape} の出所が {record['keyword']}")
            continue
        layer = layers.get(instance)
        if element.get("layer") is not None and element["layer"] != layer:
            failures.append(f"{where}: レイヤが {element['layer']!r}、ezsxf は {layer!r}")
        if where.startswith("sheet/") and instance not in sheet:
            failures.append(f"{where}: #{instance} が用紙（DRAWING_SHEET_REVISION）の中にない")
        counts["elements"] += 1
        got = step.geometry(record, shape)
        matched[where] = (got, layer, None)
        if got is not None:
            counts["shapes"] += 1
            want = dict(element["geometry"])
            # 印の種類（PRE_DEFINED_POINT_MARKER_SYMBOL の名前）は比べず、倍率だけを比べる
            if "marker" in want:
                want["marker"] = {"scale": want["marker"]["scale"]}
            for d in differences(want, got, both=True):
                failures.append(f"{where}（{record['keyword']} #{instance}）{d}")
    # 黙って捨てない: 用紙に置いた表記の実体
    placed = [(i, step.records[i]["keyword"]) for i in sorted(sheet) if i in step.records and step.records[i]["keyword"].endswith("_OCCURRENCE")]
    failures += unlifted([(i, k) for i, k in placed if i not in seen], at, entry, reading, "表記の実体")
    return failures, matched


def check_file(folder, entry, reading, baseline, counts):
    """1 つの SFC・P21 のファイルの食い違い。比べた数を counts に足す"""
    path = folder / entry["path"]
    parse = ezsxf.parse_sfc if entry["format"] == "sfc" else ezsxf.parse_p21
    try:
        parsed = parse(str(path), strict=False)
    except Exception as error:  # noqa: BLE001 ezsxf が読めないことそのものを結果にする
        return [f"ezsxf が読めない: {type(error).__name__}: {error}"], None
    found = sorted(Counter(w["code"] for w in parsed["warnings"]).items())
    failures, notes = [], []
    if entry["role"] == "apply" and baseline is not None:
        added = sorted({code for code, _ in found} - {code for code, _ in baseline["warnings"]})
        if added:
            failures.append(f"ezsxf の警告で、入力にない種類が出た: {added}")
    audit = {"warnings": found, "notes": notes}
    if reading is None:
        return failures, audit
    # 区切りの文字は ASCII なので、バイトをそのまま文字にして数える（Shift_JIS の 2 バイト目は ' と ; にならない）
    at = records(path.read_bytes().decode("latin-1"))
    failures += check_records(reading, at)
    check = check_sfc if entry["format"] == "sfc" else check_p21
    found_failures, matched = check(parsed, entry, reading, counts, notes, at)
    failures += found_failures
    # make.py の SFC は、ezsxf が型で読むように作った。読まなくなったら、SFC の形を照らせなくなる
    if notes and MADE in (entry["path"], entry.get("from")):
        failures.append("ezsxf が、型で読むように作った SFC のフィーチャを型で読まない")

    # 書き戻した値
    if entry["role"] == "apply" and not notes:
        for step in entry["steps"]:
            for want in step.get("expect", []):
                got, layer, color = matched.get(want["id"], (None, None, None))
                if got is None:
                    failures.append(f"変更 {want['edit']} の要素 {want['id']} を ezsxf で比べられない")
                    continue
                counts["expected"] += 1
                geometry = dict(want["geometry"])
                if entry["format"] == "p21" and "marker" in geometry:
                    geometry["marker"] = {"scale": geometry["marker"]["scale"]}
                for d in differences(geometry, got):
                    failures.append(f"変更 {want['edit']}（{want['id']}）{d}")
                if "layer" in want and layer is not None and want["layer"] != layer:
                    failures.append(f"変更 {want['edit']}（{want['id']}）のレイヤが {layer!r}（期待は {want['layer']!r}）")
                # 色は既定義色の番号（SFC のフィーチャの色の欄）
                code = want.get("color", {}).get("sxf")
                if entry["format"] == "sfc" and code is not None and color != code:
                    failures.append(f"変更 {want['edit']}（{want['id']}）の色が {color}（期待は既定義色 {code}）")
    return failures, audit


def main():
    folder = usage("check_ezsxf.py")
    if folder is None:
        return 2
    return run(folder, "ezsxf", version("ezsxf"), {"sfc", "p21"}, check_file, KNOWN)


if __name__ == "__main__":
    sys.exit(main())
