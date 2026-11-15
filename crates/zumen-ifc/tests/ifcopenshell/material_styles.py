"""材料の文脈・表示の参照と元値を外部実装から照らす。材料の割り当てと描画は未評価。"""

import json
import sys
from pathlib import Path

import ifcopenshell


def style(value):
    assert value.is_a() == "IfcSurfaceStyle" and len(value.Styles) == 1
    shading = value.Styles[0]
    colour = shading.SurfaceColour
    return {"id": value.id(), "shading": shading.id(), "colours": [colour.id()],
            "name": value.Name, "side": value.Side,
            "rgb": [colour.Red, colour.Green, colour.Blue],
            "transparency": getattr(shading, "Transparency", None)}


def item(value):
    assert value.is_a() == "IfcStyledItem" and value.Item is None
    assignments, styles = [], []
    for assignment in value.Styles:
        if assignment.is_a() == "IfcPresentationStyleAssignment":
            assignments.append(assignment.id())
            styles.extend(assignment.Styles)
        else:
            styles.append(assignment)
    return {"id": value.id(), "name": value.Name, "assignments": assignments,
            "styles": [style(s) for s in styles]}


def compare(path):
    expected = json.loads(path.read_text(encoding="utf-8"))
    model = ifcopenshell.open(str(path.with_suffix(".ifc")))
    material = model.by_id(expected["material"])
    assert len(material.HasRepresentation) == 1
    definition = material.HasRepresentation[0]
    actual = {"material": material.id(), "definition": definition.id(), "name": definition.Name,
              "description": definition.Description, "representations": []}
    for rep in definition.Representations:
        assert rep.is_a() == "IfcStyledRepresentation"
        actual["representations"].append({"id": rep.id(), "identifier": rep.RepresentationIdentifier,
                                         "type": rep.RepresentationType, "context": rep.ContextOfItems.id(),
                                         "items": [item(i) for i in rep.Items]})
    assert actual == expected, path.name
    print(f"{path.name}: 材料の表示 {len(definition.Representations)} 文脈、差異 0")


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert [p.stem for p in paths] == ["IFC2X3", "IFC4", "IFC4X3_ADD2", "public_material"]
    for path in paths:
        compare(path)
    print(f"IfcOpenShell {ifcopenshell.version}: 合成 9 文脈・公開 1 文脈、差異 0")
