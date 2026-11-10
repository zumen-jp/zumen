"""L1 の形状表現の参照と、保持した配置・写像の元属性を IfcOpenShell で照らす。

利用: python -I product_shapes.py <export のフォルダー>
曲線は未変換の点を比較する。配置・写像の適用、単位、幾何の評価と妥当性、診断と予算は比較しない。
自作した 3 IFC の照合は synthetic、公開サンプルの表現と写像の照合は external の範囲。
"""

import json
import math
import sys
from pathlib import Path

import ifcopenshell


def ref(entity):
    return entity.id() if entity is not None else None


def value(attribute):
    if isinstance(attribute, ifcopenshell.entity_instance):
        return {"ref": attribute.id()} if attribute.id() else value(attribute.wrappedValue)
    if isinstance(attribute, tuple):
        return [value(v) for v in attribute]
    return attribute


def node(entity):
    result = {"instance": entity.id(), "kind": entity.is_a().upper()}
    if entity.is_a("IfcProductDefinitionShape"):
        result["representations"] = [ref(r) for r in entity.Representations]
    elif entity.is_a("IfcShapeRepresentation"):
        result.update(
            context=ref(entity.ContextOfItems),
            identifier=entity.RepresentationIdentifier,
            representation_type=entity.RepresentationType,
            items=[ref(r) for r in entity.Items],
        )
    elif entity.is_a("IfcMappedItem"):
        result.update(source=ref(entity.MappingSource), target=ref(entity.MappingTarget))
    elif entity.is_a("IfcRepresentationMap"):
        result.update(origin=ref(entity.MappingOrigin), representation=ref(entity.MappedRepresentation))
    elif entity.is_a("IfcGeometricSet"):
        result["elements"] = [ref(r) for r in entity.Elements]
    return result


def equal(a, b):
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(equal(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(equal(x, y) for x, y in zip(a, b))
    if isinstance(a, (float, int)) and isinstance(b, (float, int)):
        return math.isclose(a, b, rel_tol=1e-12, abs_tol=1e-12)
    return a == b


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = ifcopenshell.open(str(path.parent / data["file"]))
    product = model.by_id(data["product"])
    assert data["placement"] == ref(product.ObjectPlacement), path.name
    assert data["definition"] == ref(product.Representation), path.name
    for expected in data["nodes"]:
        actual = node(model.by_id(expected["instance"]))
        assert equal(expected, actual), (path.name, expected, actual)
    for expected in data["records"]:
        entity = model.by_id(expected["instance"])
        actual = {"instance": entity.id(), "kind": entity.is_a().upper(), "args": [value(v) for v in entity]}
        assert equal(expected, actual), (path.name, expected, actual)
    for expected in data["curves"]:
        entity = model.by_id(expected["instance"])
        points = ([list(p.Coordinates) for p in entity.Points] if entity.is_a("IfcPolyline")
                  else [list(p) for p in entity.Points.CoordList])
        assert equal(expected["points"], points), (path.name, expected["instance"])
    if path.name == "public.json":
        # この 2 段は L1 の追跡例。立体と断面を型へ写した結果ではない
        assert model.by_guid("2q2CBrfQTCKPY0zxCdZygb") == product
        assert model.by_id(114).SweptArea.id() == 112
        assert model.by_id(112).OuterCurve.id() == 111
    print(f"{path.name}: 表現の項目 {len(data['nodes'])}、補助レコード {len(data['records'])}、曲線 {len(data['curves'])}、差異 0")


if __name__ == "__main__":
    folder = Path(sys.argv[1])
    paths = sorted(folder.glob("*.json"))
    assert len(paths) == 4, "合成 3 版と公開サンプルの JSON が必要"
    for path in paths:
        compare(path)
