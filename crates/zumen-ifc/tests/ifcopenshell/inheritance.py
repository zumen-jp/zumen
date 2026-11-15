"""写像のどの面の表示を使うか、合成の 3 版を IfcOpenShell の幾何エンジンと照合する。

外部の実装のソースは読まず、実行 API だけを使う。形や体積の一致率の検査ではない。
"""

import json
import sys
from pathlib import Path

import ifcopenshell
import ifcopenshell.geom


def compare(path):
    cases = json.loads(path.read_text(encoding="utf-8"))
    assert len(cases) == 6
    for case in cases:
        model = ifcopenshell.open(str(path.parent / case["file"]))
        shape = ifcopenshell.geom.create_shape(ifcopenshell.geom.settings(), model.by_id(100))
        geometry = shape.geometry
        materials = geometry.materials
        assert len(materials) == 1 and set(geometry.material_ids) == {0}, case["file"]
        colour = materials[0].diffuse
        actual = [colour.r(), colour.g(), colour.b()]
        assert actual == case["colour"], (case["file"], actual, case["colour"])
    print(f"{path.name}: 写像の面の表示 {len(cases)} 件、差異 0")


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert [p.stem for p in paths] == ["IFC2X3", "IFC4", "IFC4X3_ADD2"]
    for path in paths:
        compare(path)
    print(f"IfcOpenShell {ifcopenshell.version}: 合成 18 件、差異 0")
