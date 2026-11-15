"""材料の継承と面の表示を IfcOpenShell の実行 API で照らす。

実装のソースは読まない。文脈をまたぐ選択の差も記録し、一致した範囲と分ける。
"""

import json
import sys
from pathlib import Path

import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.util.element


def compare(path):
    cases = json.loads(path.read_text(encoding="utf-8"))
    assert len(cases) == 12
    references, colours, differences = 0, 0, 0
    for case in cases:
        if case["case"] == "unsupported":
            continue
        # 未対応を調べる自作の実体は、外部の読み手への入力から除く。
        text = (path.parent / case["file"]).read_text(encoding="utf-8")
        text = text.replace("#640=IFCMATERIALUNKNOWN();", "")
        model = ifcopenshell.file.from_string(text)
        product = model.by_id(100)
        owner = product
        relations = [r for r in product.HasAssociations if r.is_a("IfcRelAssociatesMaterial")]
        if not relations:
            owner = ifcopenshell.util.element.get_type(product)
            relations = [r for r in owner.HasAssociations if r.is_a("IfcRelAssociatesMaterial")]
        assert owner.id() == case["owner"], case["file"]
        assert sorted(r.RelatingMaterial.id() for r in relations) == sorted(case["materials"]), case["file"]
        references += 1
        if case["case"] in ("multiple", "empty"):
            continue
        shape = ifcopenshell.geom.create_shape(ifcopenshell.geom.settings(), product)
        assert len(shape.geometry.materials) == 1 and set(shape.geometry.material_ids) == {0}, case["file"]
        colour = shape.geometry.materials[0].diffuse
        actual = [colour.r(), colour.g(), colour.b()]
        if case["case"] in ("unrelated", "exact"):
            # 外部実装の文脈の選択は、zumen の明示した規則と一致するとは限らない。
            different = actual != case["colour"]
            differences += int(different)
            print(f"{case['file']}: 文脈の選択 zumen={case['colour']}, IfcOpenShell={actual}, 差異={different}")
        else:
            assert actual == case["colour"], (case["file"], actual, case["colour"])
            colours += 1
    print(f"{path.name}: 割り当て {references} 件と面の色 {colours} 件が一致。別枠の文脈の選択 2 件中、差異 {differences} 件")
    return references, colours, differences


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert [p.stem for p in paths] == ["IFC2X3", "IFC4", "IFC4X3_ADD2"]
    results = [compare(p) for p in paths]
    counts = tuple(sum(r[i] for r in results) for i in range(3))
    print(f"IfcOpenShell {ifcopenshell.version}: 割り当て {counts[0]} 件・色 {counts[1]} 件の一致、別枠の文脈の選択の差異 {counts[2]} 件")
