"""自作 DXF から新規に書いた P21/JWW を ezsxf 0.3.7・ezjww 0.7.6 と照らす。

convert_cad_case --output <フォルダー> の出力だけを使う。
P21 の参照と数値は ezsxf の解析値を使う。文字の STEP エスケープは検証キットの補助関数で戻す。
実機での表示と SXF 全体の適合認定は含めない。
"""
import json
import math
import re
import sys
from importlib.metadata import version
from pathlib import Path

import ezjww
import ezsxf

from check_ezsxf import Step, p21_text, records


def equal(actual, expected):
    if isinstance(expected, (list, tuple)):
        assert len(actual) == len(expected), (actual, expected)
        for a, e in zip(actual, expected):
            equal(a, e)
    elif isinstance(expected, float):
        assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), (actual, expected)
    else:
        assert actual == expected, (actual, expected)


def check_jww(path):
    doc = ezjww.read_document(str(path))
    assert not doc.get("diagnostics"), doc.get("diagnostics")
    assert not ezjww.audit(str(path)).get("issue_codes"), ezjww.audit(str(path))
    header = doc["header"]
    equal(header["palette"]["extended_colors"][0], 255)
    equal(header["layer_groups"][0]["layers"][0]["name"], "図形")
    entities = doc["entities"]
    equal([e["type"] for e in entities], ["LINE", "CIRCLE", "ARC", "LINE", "LINE", "TEXT", "POINT"])
    for at, expected in [(0, [1.0, 2.0, 11.0, 12.0]), (3, [0.0, 20.0, 10.0, 30.0]),
                         (4, [10.0, 30.0, 20.0, 20.0])]:
        equal([entities[at][k] for k in ("start_x", "start_y", "end_x", "end_y")], expected)
    equal([entities[1][k] for k in ("center_x", "center_y", "radius")], [20.0, 30.0, 4.0])
    equal([entities[2][k] for k in ("center_x", "center_y", "radius", "start_angle", "arc_angle")],
          [40.0, 50.0, 5.0, math.radians(10), math.radians(120)])
    text = entities[5]
    equal([text[k] for k in ("content", "font_name", "start_x", "start_y", "size_x", "size_y", "angle")],
          ["変換 Test", "Arial", 30.0, 40.0, 3.0, 3.0, 15.0])
    equal([entities[6][k] for k in ("x", "y")], [60.0, 70.0])
    for entity in entities:
        equal(entity["base"]["layer_group"], 0)
        equal(entity["base"]["layer"], 0)
        equal(entity["base"]["pen_color"], 100)
    for entity in entities[:5]:
        equal(entity["base"]["pen_style"], 30)
        equal(entity["base"]["pen_width"], 35)
    # 生のヘッダーを外部実装が返す表で確かめる。独自の座標リーダーは作らない。
    return header


def check_p21(path):
    parsed = ezsxf.parse_p21(path.read_bytes(), strict=True)
    assert not parsed["warnings"], parsed["warnings"]
    statements = {}
    records(path.read_bytes().decode("latin-1"), [], statements=statements)
    step = Step(parsed, statements)
    parts = step.parts
    sheet_id = next(i for i in step.order if step.has(i, "DRAWING_SHEET_REVISION"))
    sheet = step.refs(step.part(sheet_id, "DRAWING_SHEET_REVISION")[1])
    occurrences = [i for i in sheet if any(k.endswith("_OCCURRENCE") for k in parts[i])]
    equal(len(occurrences), 6)
    layer_id = next(i for i in step.order if step.has(i, "PRESENTATION_LAYER_ASSIGNMENT"))
    layer = step.part(layer_id, "PRESENTATION_LAYER_ASSIGNMENT")
    equal(step.text(layer_id, "PRESENTATION_LAYER_ASSIGNMENT"), "図形")
    assert set(occurrences) <= set(step.refs(layer[2]))
    frame = next(i for i in sheet if step.has(i, "PLANAR_BOX"))
    equal(step.part(frame, "PLANAR_BOX")[1:3], [420.0, 297.0])
    equal(step.shape(occurrences[0]), {"shape": "line", "start": [1.0, 2.0], "end": [11.0, 12.0]})
    for at, center, radius in [(1, [20.0, 30.0], 4.0), (2, [40.0, 50.0], 5.0)]:
        _, ref = step.styled(occurrences[at])
        if at == 2:
            trim = step.get(ref, "TRIMMED_CURVE")
            equal(trim[2][0]["parameters"], [math.radians(10)])
            equal(trim[3][0]["parameters"], [math.radians(130)])
            ref = trim[1]
        circle = step.get(ref, "CIRCLE")
        placement = step.get(circle[1], "AXIS2_PLACEMENT_2D")
        equal(step.point(placement[1]), center)
        equal(circle[2], radius)
    _, ref = step.styled(occurrences[3])
    equal([step.point(p) for p in step.get(ref, "POLYLINE")[1]],
          [[0.0, 20.0], [10.0, 30.0], [20.0, 20.0]])
    _, ref = step.styled(occurrences[4])
    literal = step.get(ref, "TEXT_LITERAL_WITH_EXTENT")
    quoted = re.findall(r"'((?:[^']|'')*)'", statements[step.ref(ref)])
    equal(p21_text(quoted[1]), "変換 Test")
    placement = step.get(literal[2], "AXIS2_PLACEMENT_2D")
    equal(step.point(placement[1]), [30 - 1.5 * math.sin(math.radians(15)),
                                    40 + 1.5 * math.cos(math.radians(15))])
    equal(step.get(literal[6], "PLANAR_EXTENT")[1:], [13.5, 3.0])
    font = step.get(literal[5], "EXTERNALLY_DEFINED_TEXT_FONT")[0]
    equal(font["parameters"], ["Arial"])
    equal(step.shape(occurrences[5])["position"], [60.0, 70.0])
    _, ref = step.styled(occurrences[5])
    symbol = step.get(ref, "DEFINED_SYMBOL")
    equal(step.get(symbol[1], "PRE_DEFINED_POINT_MARKER_SYMBOL"), ["dot"])
    for occurrence in occurrences[:4]:
        style, _ = step.styled(occurrence)
        assignment = step.get(style[0], "PRESENTATION_STYLE_ASSIGNMENT")[0]
        curve_style = step.get(assignment[0], "CURVE_STYLE")
        equal(step.get(curve_style[3], "COLOUR_RGB")[1:], [1.0, 0.0, 0.0])
        font = step.get(curve_style[1], "CURVE_STYLE_FONT")
        equal(step.get(font[1][0], "CURVE_STYLE_FONT_PATTERN"), [2.0, 1.0])
        width = curve_style[2]
        if width.get("kind") == "reference":
            width = step.get(width, "LENGTH_MEASURE_WITH_UNIT")[0]
        equal(width["parameters"], [0.35])


def main():
    equal(version("ezjww"), "0.7.6")
    equal(version("ezsxf"), "0.3.7")
    folder = Path(sys.argv[1])
    check_jww(folder / "converted.jww")
    check_p21(folder / "converted.p21")
    for name in ["sxf", "jww"]:
        report = json.loads((folder / f"{name}-estimate.json").read_text(encoding="utf-8"))["result"]["estimate"]
        assert any(c["kind"] == "spline" and c["fidelity"] == "dropped" and c["count"] == 1 for c in report["counts"])
    print("P21/JWW: 外部実装の数値・文字・色・線種・線幅・レイヤの参照が一致")


if __name__ == "__main__":
    main()
