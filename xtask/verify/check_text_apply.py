"""文字・レイヤの適合テストを書き出し、ezjww 0.7.6・ezsxf 0.3.7 と照らす。

使い方: python check_text_apply.py <zumen の動的ライブラリ> <出力フォルダー>
合成の 5 ケースだけを使う。P21 は ezsxf の実体解析結果の参照をたどって値を比べる。
"""

import io
import json
import math
import sys
import zipfile
from importlib.metadata import version
from pathlib import Path

import ezjww
import ezsxf

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "bindings/python"))
from zumen import ZumenError, load  # noqa: E402


def equal(actual, expected):
    if isinstance(expected, float):
        assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), (actual, expected)
    else:
        assert actual == expected, (actual, expected)


def drawing_bytes(ext, data):
    if ext not in ("sfz", "p2z"):
        return data
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        equal(archive.read("keep.txt"), b"untouched attachment")
        return archive.read("drawing.sfc" if ext == "sfz" else "drawing.p21")


def check_jww(path, stage):
    doc = ezjww.read_document(str(path))
    warnings = set(ezjww.audit(str(path)).get("issue_codes", []))
    warnings.update(str(d.get("code", d)) for d in doc.get("diagnostics", []))
    if stage == -1:
        return warnings
    layers = doc["header"]["layer_groups"][0]["layers"]
    equal([layers[i]["name"] for i in (0, 1)], ["Renamed", "Added"])
    equal(len(doc["entities"]), 1 if stage == 0 else 2)
    line = doc["entities"][0]
    equal([line[key] for key in ("start_x", "start_y", "end_x", "end_y")], [0, 0, 100, 50])
    if stage > 0:
        text = doc["entities"][1]
        x, y = (1.0, 2.0) if stage == 1 else (1.2, 2.4) if stage < 4 else (1.3, 2.6)
        for key, value in {"type": "TEXT", "content": "Initial" if stage == 1 else "Revised",
                           "start_x": x, "start_y": y, "size_x": 3.0 if stage == 1 else 4.0,
                           "size_y": 3.0 if stage == 1 else 4.0, "angle": 0.0 if stage == 1 else 90.0,
                           "spacing": 0.0, "font_name": "NewFont"}.items():
            equal(text[key], value)
        equal(text["base"]["layer"], 1 if stage < 3 else 0)
        equal(text["base"]["pen_color"], 2)
    return warnings


def check_sfc(data, stage):
    parsed = ezsxf.parse_sfc(data, strict=False)
    warnings = {warning["code"] for warning in parsed["warnings"]}
    if stage == -1:
        return warnings
    features = parsed["typed_features"]
    equal([f["name"] for f in features if f["kind"] == "layer"], ["Renamed", "B", "Added"])
    texts = [f for f in features if f["kind"] == "text"]
    equal(len(texts), 2 if stage == 0 else 3)
    for text in texts[:2]:
        equal(text["text"], "old")
        equal(text["anchor"], {"x": 1, "y": 1})
        equal(text["spacing"], 0.25)
    if stage > 0:
        text = texts[-1]
        x, y = (100, 200) if stage == 1 else (120, 240) if stage < 4 else (130, 260)
        for key, value in {"text": "Initial" if stage == 1 else "Revised", "anchor": {"x": x, "y": y},
                           "height": 3.0 if stage == 1 else 4.0, "width": 10.5 if stage == 1 else 14.0,
                           "spacing": 0.0, "angle_deg": 0.0 if stage == 1 else 90.0,
                           "slant_deg": 0.0, "base_point": 1, "direction": 1}.items():
            equal(text[key], value)
        equal(text["style"]["layer_code"], 3 if stage < 3 else 1)
        equal(text["style"]["color_code"], 2)
        fonts = [f for f in features if f["kind"] == "text_font"]
        equal(fonts[text["style"]["font_code"] - 1]["name"], "NewFont")
    return warnings


def check_p21(data, stage):
    parsed = ezsxf.parse_p21(data, strict=False)
    warnings = {warning["code"] for warning in parsed["warnings"]}
    if stage == -1:
        return warnings
    entities = {entity["id"]: entity for entity in parsed["entities"]}

    def part(number, keyword):
        entity = entities[number]
        records = entity.get("records", [entity.get("record")])
        return next(record["parameters"] for record in records if record["keyword"] == keyword)

    def ref(value):
        equal(value["kind"], "reference")
        return value["value"]

    layers = {part(n, "PRESENTATION_LAYER_ASSIGNMENT")[0]: part(n, "PRESENTATION_LAYER_ASSIGNMENT")[2]
              for n, e in entities.items() if e.get("record", {}).get("keyword") == "PRESENTATION_LAYER_ASSIGNMENT"}
    equal(set(layers), {"Renamed", "B", "Added"})
    sheet = [ref(value) for value in part(100, "DRAWING_SHEET_REVISION")[1]]
    equal(sheet[:2], [10, 11])
    equal(len(sheet), 2 if stage == 0 else 3)
    equal(part(9, "TEXT_LITERAL_WITH_EXTENT")[1], "old")
    for number in (10, 11):
        equal(ref(part(number, "ANNOTATION_TEXT_OCCURRENCE")[2]), 9)
    if stage > 0:
        number = sheet[-1]
        style_refs, literal_ref = part(number, "STYLED_ITEM")
        literal = part(ref(literal_ref), "TEXT_LITERAL_WITH_EXTENT")
        equal(literal[1], "Initial" if stage == 1 else "Revised")
        placement = part(ref(literal[2]), "AXIS2_PLACEMENT_2D")
        point = part(ref(placement[1]), "CARTESIAN_POINT")[1]
        # baseline left の基点は文字高の半分だけ上にある。90 度では左へずれる。
        expected = (100.0, 201.5) if stage == 1 else (118.0, 240.0) if stage < 4 else (128.0, 260.0)
        for actual, wanted in zip(point, expected):
            equal(actual, wanted)
        extent = part(ref(literal[6]), "PLANAR_EXTENT")
        equal(extent[1:], [10.5, 3] if stage == 1 else [14, 4])
        font = part(ref(literal[5]), "EXTERNALLY_DEFINED_TEXT_FONT")[0]
        equal(font["parameters"], ["NewFont"])
        assignment = part(ref(style_refs[0]), "PRESENTATION_STYLE_ASSIGNMENT")[0]
        style = ref(assignment[0])
        box = {v["keyword"]: v["parameters"][0] for v in part(style, "TEXT_STYLE_WITH_BOX_CHARACTERISTICS")[0]}
        equal(box["BOX_HEIGHT"], 3.0 if stage == 1 else 4.0)
        equal(box["BOX_WIDTH"], 10.5 if stage == 1 else 14.0)
        equal(box["BOX_ROTATE_ANGLE"], 0.0 if stage == 1 else math.pi / 2)
        appearance = ref(part(style, "TEXT_STYLE")[1])
        color = ref(part(appearance, "TEXT_STYLE_FOR_DEFINED_FONT")[0])
        equal(part(color, "DRAUGHTING_PRE_DEFINED_COLOUR"), ["red"])
        layer = "Added" if stage < 3 else "Renamed"
        assert number in [ref(value) for value in layers[layer]]
    # 空レイヤを保持するための補助点は用紙に置かない。
    for member in layers["Added"]:
        if entities[ref(member)].get("record", {}).get("keyword") == "CARTESIAN_POINT":
            assert ref(member) not in sheet
    return warnings


def main():
    library, folder = sys.argv[1:]
    output = Path(folder)
    output.mkdir(parents=True, exist_ok=True)
    core = load(str(Path(library).resolve()))
    for ext in ("jww", "sfc", "p21", "sfz", "p2z"):
        case = json.loads((REPO / f"bindings/conformance/cases/apply_text_{ext}.json").read_text(encoding="utf-8"))
        session = core.open()
        stage = -1
        baseline = None
        for step in case["steps"]:
            data = None
            if "push" in step:
                data = bytes.fromhex(step["push"]["hex"])
                session.push(step["push"]["name"], data)
            elif "call" in step:
                try:
                    response = session.call(step["call"])
                    equal(step.get("status", "ok"), "ok")
                except ZumenError as error:
                    equal(step.get("status"), "failed")
                    response = error.response
                for pointer, wanted in step.get("expect", {}).items():
                    actual = response
                    for key in pointer.strip("/").split("/"):
                        actual = actual[int(key)] if isinstance(actual, list) else actual[key]
                    equal(actual, wanted)
            elif "take" in step:
                data = session.take(step["take"])
                stage += 1
                reopened = core.open()
                name = f"text.{ext}"
                reopened.push(name, data)
                reopened.call({"v": 1, "op": "open", "input": name})
                request = {"v": 1, "op": "entities", "doc": name,
                           "fields": ["id", "kind", "layer", "hash", "geometry"]}
                equal(reopened.call(request)["result"], session.call(request)["result"])
            if data is not None:
                path = output / f"text-{stage + 1}.{ext}"
                path.write_bytes(data)
                raw = drawing_bytes(ext, data)
                if ext == "jww":
                    warnings = check_jww(path, stage)
                elif ext in ("sfc", "sfz"):
                    warnings = check_sfc(raw, stage)
                else:
                    warnings = check_p21(raw, stage)
                if baseline is None:
                    baseline = warnings
                else:
                    assert warnings <= baseline, (ext, stage, warnings - baseline)
        equal(stage, 4)
        print(f"{ext}: 文字・レイヤの 5 回の書き戻しを照合")
    print(f"ezjww {version('ezjww')} / ezsxf {version('ezsxf')}")


if __name__ == "__main__":
    main()
