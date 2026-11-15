"""統合した取得の共有グラフ・直接の表示参照と代表の元属性を照らす。形は評価しない。"""

import json
import sys
from pathlib import Path

import ifcopenshell


def sample(value):
    kind = value.is_a()
    if kind == "IfcExtrudedAreaSolid":
        return value.Depth
    if kind == "IfcRevolvedAreaSolid":
        return value.Angle
    if kind == "IfcSweptDiskSolid":
        return value.Radius
    if kind == "IfcMappedItem":
        return value.MappingTarget.id()
    if value.is_a("IfcHalfSpaceSolid"):
        return value.AgreementFlag
    return None


def graph(root):
    pending, found = [root], {}
    while pending:
        value = pending.pop()
        if value.id() in found:
            continue
        found[value.id()] = value
        kind = value.is_a()
        if kind == "IfcProductDefinitionShape":
            pending.extend(value.Representations)
        elif kind == "IfcShapeRepresentation":
            pending.extend(value.Items)
        elif kind == "IfcMappedItem":
            pending.append(value.MappingSource)
        elif kind == "IfcRepresentationMap":
            pending.append(value.MappedRepresentation)
        elif kind in ("IfcGeometricSet", "IfcGeometricCurveSet"):
            pending.extend(value.Elements)
        elif value.is_a("IfcBooleanResult"):
            pending.extend([value.FirstOperand, value.SecondOperand])
    return found


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = ifcopenshell.open(str(path.parent / data["file"]))
    nodes = graph(model.by_id(data["product"]).Representation)
    assert set(nodes) == {row["instance"] for row in data["nodes"]}
    for row in data["nodes"]:
        value = nodes[row["instance"]]
        assert row["kind"] == value.is_a().upper()
        assert row["sample"] == sample(value)
    layers = model.by_type("IfcPresentationLayerAssignment")
    for row in data["styles"]:
        item = nodes[row["item"]]
        rep = nodes.get(row["representation"])
        if rep:
            assert item in rep.Items
        styled = list(item.StyledByItem)
        assert len(styled) <= 1
        assert row["styled_item"] == (styled[0].id() if styled else None)
        assert sorted(row["layers"]) == sorted(v.id() for v in layers
                                               if item in v.AssignedItems or rep in v.AssignedItems)
    return len(nodes), len(data["styles"])


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 5, "合成 3 版と公開 2 例が必要"
    counts = [compare(path) for path in paths]
    print(f"IfcOpenShell {ifcopenshell.version}: 形の項目 {sum(v[0] for v in counts)}、"
          f"表示を使う位置 {sum(v[1] for v in counts)}、差異 0")
