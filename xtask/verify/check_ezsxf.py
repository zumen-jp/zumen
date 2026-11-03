"""確認の束の SFC・P21 を ezsxf で読み、zumen が読んだ値と照らす（#503、決定 0254）。

使い方: python check_ezsxf.py <束のフォルダー>

ファイルごとに、次を確かめる。

- ezsxf が読める（strict にせず、警告を集める）。書き戻したファイルで、入力にない種類の警告が出ない。警告は code の
  種類で比べる（フィーチャごとの警告は、足したフィーチャの分だけ増えるため）。
- SFC: zumen の要素の出所のインスタンスのフィーチャが ezsxf にあり、種類・レイヤと、線・円・点マーカーでは形の
  値が同じ。どの要素も用紙（drawing_sheet_feature）の中にある。ezsxf が型で読む図形のフィーチャは、どれも zumen の
  要素になるか、zumen が診断を返している。ezsxf がフィーチャを型で読まない SFC（印が `/*SXF3` など）は、要素を
  照らさない（結果の notes に書く）。
- P21: zumen の要素の出所のインスタンスがあり、点は ANNOTATION_SYMBOL_OCCURRENCE、線は ANNOTATION_CURVE_OCCURRENCE
  である。参照をたどって、点の位置・回転・倍率と、線の端点を比べる（印の種類は比べない）。レイヤ
  （PRESENTATION_LAYER_ASSIGNMENT）はたどらず、比べない。
- 書き戻したファイルでは、変更で決めた形とレイヤ（manifest.json の expect）が、ezsxf で読んだ値と同じ。
"""

import math
import sys
from collections import Counter
from importlib.metadata import version
from pathlib import Path

import ezsxf

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare import differences, run, usage  # noqa: E402

# 図面モデルの形 → ezsxf のフィーチャの種類（SFC）
KINDS = {
    "line": {"line"},
    "circle": {"circle"},
    "arc": {"arc"},
    "point": {"point_marker"},
    "polyline": {"polyline"},
    "text": {"text"},
}
# ezsxf が型で読む、図形のフィーチャの種類
DRAWN = {"line", "circle", "arc", "point_marker", "polyline", "text", "ellipse", "ellipse_arc", "spline", "clothoid"}
# 図面モデルの形 → P21 のインスタンスの型
OCCURRENCES = {"point": "ANNOTATION_SYMBOL_OCCURRENCE", "line": "ANNOTATION_CURVE_OCCURRENCE"}
# 既知の食い違い: (ファイルの道筋の初め, 食い違いの文の初め, issue, 当たるファイルの数)。直ったら消す
KNOWN = []


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


def check_sfc(parsed, entry, reading, counts, notes):
    features = {f["id"]: f for f in parsed["typed_features"]}
    if not any(f["kind"] in DRAWN for f in features.values()):
        notes.append("ezsxf がフィーチャを型で読まないので、要素を照らさない")
        return [], {}
    model = parsed["model"]
    layers = {t["code"]: features.get(t["entity_id"], {}).get("name") for t in model["code_tables"]["layers"]}
    sheet = set((model.get("sheet") or {}).get("component_ids", []))
    failures, matched = [], {}
    for element in reading["entities"]:
        instance = element.get("source", {}).get("instance")
        if instance is None:
            continue
        where = element["id"]
        f = features.get(int(instance))
        if f is None:
            failures.append(f"{where}: ezsxf に #{instance} のフィーチャがない")
            continue
        matched[where] = f
        shape = element.get("geometry", {}).get("shape")
        if shape in KINDS and f["kind"] not in KINDS[shape]:
            failures.append(f"{where}: 形 {shape} の出所が {f['kind']}")
            continue
        layer = layers.get((f.get("style") or {}).get("layer_code"))
        if element.get("layer") is not None and layer is not None and element["layer"] != layer:
            failures.append(f"{where}: レイヤが {element['layer']!r}、ezsxf は {layer!r}")
        if where.startswith("sheet/") and int(instance) not in sheet:
            failures.append(f"{where}: #{instance} が ezsxf の用紙の中にない")
        counts["elements"] += 1
        got = feature_geometry(f)
        if shape in KINDS and got is not None:
            counts["shapes"] += 1
            for d in differences(element["geometry"], got):
                failures.append(f"{where}（{f['kind']} #{instance}）{d}")
    # 黙って捨てない: 型で読む図形のフィーチャがあれば、要素になるか、zumen が診断を返している
    seen = {f["id"] for f in matched.values()}
    dropped = Counter(f["kind"] for f in features.values() if f["kind"] in DRAWN and f["id"] not in seen)
    if dropped and not reading["diagnostics"] + entry.get("open", []):
        failures.append(f"要素にならず、診断もないフィーチャ: {dict(dropped)}")
    return failures, {where: (feature_geometry(f), layers.get((f.get("style") or {}).get("layer_code"))) for where, f in matched.items()}


class Step:
    """P21 のインスタンスの参照をたどる"""

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

    def point(self, value):
        record = self.get(value, "CARTESIAN_POINT")
        return [float(c) for c in record["parameters"][1]] if record else None

    def geometry(self, record, shape):
        """点と線の、図面モデルの形の値。たどれなければ None"""
        if shape == "point":
            symbol = self.get(record["parameters"][2], "DEFINED_SYMBOL")
            target = symbol and self.get(symbol["parameters"][2], "SYMBOL_TARGET")
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
            curve = self.get(record["parameters"][2], "TRIMMED_CURVE")
            if not curve:
                return None
            start, end = (self.point(trim[0]) if trim else None for trim in curve["parameters"][2:4])
            return {"shape": "line", "start": start, "end": end}
        return None


def check_p21(parsed, entry, reading, counts, notes):
    step = Step(parsed)
    failures, matched = [], {}
    for element in reading["entities"]:
        instance = element.get("source", {}).get("instance")
        if instance is None:
            continue
        where = element["id"]
        record = step.records.get(int(instance))
        if record is None:
            failures.append(f"{where}: ezsxf に #{instance} のインスタンスがない")
            continue
        shape = element.get("geometry", {}).get("shape")
        if shape in OCCURRENCES and record["keyword"] != OCCURRENCES[shape]:
            failures.append(f"{where}: 形 {shape} の出所が {record['keyword']}")
            continue
        counts["elements"] += 1
        got = step.geometry(record, shape)
        matched[where] = (got, None)
        if got is not None:
            counts["shapes"] += 1
            want = dict(element["geometry"])
            # 印の種類（PRE_DEFINED_POINT_MARKER_SYMBOL の名前）は比べず、倍率だけを比べる
            if "marker" in want:
                want["marker"] = {"scale": want["marker"]["scale"]}
            for d in differences(want, got):
                failures.append(f"{where}（{record['keyword']} #{instance}）{d}")
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
    check = check_sfc if entry["format"] == "sfc" else check_p21
    found_failures, matched = check(parsed, entry, reading, counts, notes)
    failures += found_failures

    # 書き戻した値
    if entry["role"] == "apply" and not notes:
        for step in entry["steps"]:
            for want in step.get("expect", []):
                got, layer = matched.get(want["id"], (None, None))
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
    return failures, audit


def main():
    folder = usage("check_ezsxf.py")
    if folder is None:
        return 2
    return run(folder, "ezsxf", version("ezsxf"), {"sfc", "p21"}, check_file, KNOWN)


if __name__ == "__main__":
    sys.exit(main())
