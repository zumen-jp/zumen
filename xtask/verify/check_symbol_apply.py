"""記号配置の合成出力を ezjww 0.7.6 と ezsxf 0.3.7 で照らす。

使い方: python check_symbol_apply.py <drawing_metadata_formats.json> <出力フォルダー>
適合入力の編集前・再読用のバイト列を使う。自前のリーダーの値には依存しない。
"""

import json
import math
import sys
from importlib.metadata import version
from pathlib import Path

import ezjww
import ezsxf


def close(actual, expected):
    assert math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12), (actual, expected)


def check_jww(before, after, output):
    files = []
    for label, data in [("before", before), ("after", after)]:
        path = output / f"symbol-{label}.jww"
        path.write_bytes(data)
        files.append(ezjww.read_document(str(path)))
    old, new = files
    assert len(new["entities"]) == len(old["entities"]) + 1
    assert new["entities"][:-1] == old["entities"]
    symbol = new["entities"][-1]
    assert symbol["type"] == "BLOCK"
    assert symbol["def_number"] == 1 and symbol["block_name"] == "未配置"
    assert symbol["base"]["layer_group"] == symbol["base"]["layer"] == 0
    for key, expected in [("ref_x", 0.1), ("ref_y", 0.2), ("scale_x", 0.02),
                          ("scale_y", 0.02), ("rotation", math.pi / 6)]:
        close(symbol[key], expected)


def check_sfc(before, after):
    old = ezsxf.parse_sfc(before, strict=False)
    new = ezsxf.parse_sfc(after, strict=True)
    # 入力には、一覧から選ぶための未配置の部品を 1 つだけ置く。
    assert [warning["code"] for warning in old["warnings"]] == ["sfc-unused-sfig-definition"]
    assert not new["warnings"], new["warnings"]
    symbols = [f for f in new["typed_features"] if f["kind"] == "sfig_locate"]
    assert len(symbols) == 1
    symbol = symbols[0]
    assert symbol["name"] == "Part" and symbol["style"]["layer_code"] == 1
    assert symbol["position"] == {"x": 10.0, "y": 20.0}
    close(symbol["angle_deg"], 30.0)
    close(symbol["ratio_x"], 2.0)
    close(symbol["ratio_y"], 2.0)
    assert new["model"]["sfig_references"] == [{"placement_id": symbol["id"], "definition_id": 30}]
    assert [f for f in new["typed_features"] if f["kind"] != "sfig_locate"] == old["typed_features"]


def check_p21(before, after):
    old = ezsxf.parse_p21(before, strict=True)
    new = ezsxf.parse_p21(after, strict=True)
    assert not old["warnings"] and not new["warnings"], (old["warnings"], new["warnings"])
    entities = {entity["id"]: entity for entity in new["entities"]}

    def part(number, name):
        entity = entities[number]
        records = entity.get("records", [entity.get("record")])
        return next(record["parameters"] for record in records if record["keyword"] == name)

    def ref(value):
        assert value["kind"] == "reference"
        return value["value"]

    sheet = part(16, "DRAWING_SHEET_REVISION")[1]
    assert len(sheet) == 1
    occurrence = ref(sheet[0])
    assert occurrence in [ref(value) for value in part(15, "PRESENTATION_LAYER_ASSIGNMENT")[2]]
    mapped = ref(part(occurrence, "STYLED_ITEM")[1])
    representation, target = map(ref, part(mapped, "MAPPED_ITEM"))
    assert ref(part(representation, "SYMBOL_REPRESENTATION_MAP")[1]) == 14
    placement = part(target, "SYMBOL_TARGET")
    assert placement[2:] == [2.0, 2.0]
    axis = part(ref(placement[1]), "AXIS2_PLACEMENT_2D")
    assert part(ref(axis[1]), "CARTESIAN_POINT")[1] == [10.0, 20.0]
    direction = part(ref(axis[2]), "DIRECTION")[1]
    close(direction[0], math.sqrt(3) / 2)
    close(direction[1], 0.5)
    # 定義・内部の線とスタイル・ほかの既存実体はそのまま残す。
    for entity in old["entities"]:
        if entity["id"] not in (15, 16):
            assert entities[entity["id"]] == entity


def main():
    assert version("ezjww") == "0.7.6" and version("ezsxf") == "0.3.7"
    case = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    inputs = [bytes.fromhex(step["push"]["hex"]) for step in case["steps"] if "push" in step]
    assert len(inputs) == 10
    output = Path(sys.argv[2])
    output.mkdir(parents=True, exist_ok=True)
    check_jww(inputs[4], inputs[5], output)
    check_sfc(inputs[6], inputs[7])
    check_p21(inputs[8], inputs[9])
    print("JWW・SFC・P21: 定義の参照・位置・倍率・回転・所属と未変更要素を照合")
    print("ezjww 0.7.6 / ezsxf 0.3.7")


if __name__ == "__main__":
    main()
