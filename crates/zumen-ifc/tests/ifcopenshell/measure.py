"""公開 2 試料の 11 室で、形からの水平面積を IFC に記録された面積と別に照合する。"""

import argparse
import json
import os
from pathlib import Path
import subprocess

import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.util.unit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[4] / "samples"
    settings = ifcopenshell.geom.settings()
    settings.set(settings.USE_WORLD_COORDS, True)
    for name, count in [("LargeBuilding.ifc", 8), ("TallBuilding.ifc", 3)]:
        relative = f"ifc/bim-whale/{name}"
        model = ifcopenshell.open(str(root / relative))
        length_scale = ifcopenshell.util.unit.calculate_unit_scale(model)
        area_scale = ifcopenshell.util.unit.calculate_unit_scale(model, "AREAUNIT")
        differences = {"GrossFloorArea": [], "NetFloorArea": []}
        computed = 0.0
        expected = 0.0
        spaces = model.by_type("IfcSpace")
        assert len(spaces) == count
        for space in spaces:
            values = {}
            for relation in space.IsDefinedBy:
                if not relation.is_a("IfcRelDefinesByProperties"):
                    continue
                group = relation.RelatingPropertyDefinition
                if not group.is_a("IfcElementQuantity"):
                    continue
                for quantity in group.Quantities:
                    if quantity.is_a("IfcQuantityArea") and quantity.Name in differences:
                        assert quantity.Unit is None, "明示された単位の照合は未対応"
                        assert quantity.Name not in values
                        values[quantity.Name] = quantity.AreaValue * area_scale
            assert set(values) == set(differences)
            # 切る高さだけを外部の形の範囲から選ぶ。外部の面積は計算結果へ混ぜない。
            shape = ifcopenshell.geom.create_shape(settings, space)
            vertices = shape.geometry.verts
            low = [min(vertices[i::3]) for i in range(3)]
            high = [max(vertices[i::3]) for i in range(3)]
            origin = [(low[0] + high[0]) / 2, (low[1] + high[1]) / 2,
                      low[2] + min(1.0, (high[2] - low[2]) / 2)]
            command = [str(args.executable.resolve()), relative, space.GlobalId,
                       *[str(v / length_scale) for v in origin], "shape", "auto"]
            result = subprocess.run(command, env=dict(os.environ, ZUMEN_SAMPLES=str(root)),
                                    capture_output=True, check=True, timeout=60)
            area = json.loads(result.stdout)["area_m2"]
            assert area is not None
            computed += area
            expected += values["NetFloorArea"]
            for key, value in values.items():
                difference = abs(area - value)
                assert difference <= 0.00001
                differences[key].append(difference)
        print(json.dumps({"sample": name, "spaces": count, "computed_m2": computed,
                          "stored_net_m2": expected,
                          "maximum_difference_m2": {k: max(v) for k, v in differences.items()}},
                         ensure_ascii=False))


if __name__ == "__main__":
    main()
