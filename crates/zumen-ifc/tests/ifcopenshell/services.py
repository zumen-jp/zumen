"""設備の最小入力を IfcOpenShell の実行 API で照らす。実装・スキーマは読まない。"""

import json
import sys
from pathlib import Path

import ifcopenshell
import ifcopenshell.util.element


def check(path):
    expected = json.loads(path.read_text(encoding="utf-8"))
    model = ifcopenshell.open(str(path.with_suffix(".ifc")))
    assert {e.id() for e in model.by_type("IfcObjectDefinition")} == {e["source"] for e in expected}
    relations, properties = 0, 0
    for item in expected:
        entity = model.by_id(item["source"])
        name = entity.is_a()
        assert entity.GlobalId == item["id"], (path.stem, name, "GlobalId")
        assert name == item["class"], (path.stem, name, "class")
        predefined = getattr(entity, "PredefinedType", None)
        user_defined = getattr(entity, "ElementType" if name.endswith("Type") else "ObjectType", None)
        if name == "IfcElectricDistributionPoint":
            predefined = entity.DistributionPointFunction
            user_defined = entity.UserDefinedFunction
        elif name == "IfcTransportElement" and path.stem == "IFC2X3":
            predefined = entity.OperationType
        assert predefined == item["predefined"], (path.stem, name, "predefined")
        assert user_defined == item["user_defined"], (path.stem, name, "user_defined")
        # get_type は型自身にはその型を返すため、関係を持つ要素だけで使う。
        target = None if name.endswith("Type") else ifcopenshell.util.element.get_type(entity)
        assert (target.id() if target else None) == item["type"], (path.stem, name, "type")
        relations += target is not None
        sets = getattr(entity, "HasPropertySets", None)
        value = None
        if sets:
            assert len(sets) == 1 and sets[0].Name == "Pset_ServiceTest"
            props = sets[0].HasProperties
            assert len(props) == 1 and props[0].Name == "Probe"
            value = props[0].NominalValue.wrappedValue
            properties += 1
        assert value == item["property"], (path.stem, name, "property")
    assert relations == 23
    print(f"{path.stem}: 要素 {len(expected)}、型との関係 {relations}、型のプロパティ {properties} が一致")
    return len(expected), relations, properties


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert [p.stem for p in paths] == ["IFC2X3", "IFC4", "IFC4X3_ADD2"]
    results = [check(path) for path in paths]
    counts = [sum(result[i] for result in results) for i in range(3)]
    print(f"IfcOpenShell {ifcopenshell.version}: 合成の要素 {counts[0]}、関係 {counts[1]}、プロパティ {counts[2]} が一致")
