"""面集合の元属性を、合成3版と公開2要素で照らす。幾何の評価は比較しない。

外部実装はバイナリーだけを使い、スキーマやソースは読まない。
"""
import importlib.util
import json
import sys
from pathlib import Path
import ifcopenshell


def own_module(name):
    spec = importlib.util.spec_from_file_location("faces_" + name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


curves = own_module("curves")
sections = own_module("sections")


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = curves.open_model(path.parent / data["file"])
    product = model.by_guid(data["global_id"])
    assert product.id() == data["product"]
    stack = list(product.Representation.Representations)
    leaves, seen = set(), set()
    while stack:
        item = stack.pop()
        if item.id() in seen:
            continue
        seen.add(item.id())
        if item.is_a() == "IfcShapeRepresentation":
            stack.extend(item.Items)
        elif item.is_a() == "IfcMappedItem":
            stack.append(item.MappingSource.MappedRepresentation)
        else:
            leaves.add(item.id())
    unit = next(u for u in model.by_type("IfcProject")[0].UnitsInContext.Units if u.UnitType == "LENGTHUNIT")
    count = 0
    for expected in data["items"]:
        assert expected["instance"] in leaves
        item = model.by_id(expected["instance"])
        if item.is_a() == "IfcFacetedBrep":
            kind, roots = "faceted-solid", [item.Outer]
        elif item.is_a() == "IfcShellBasedSurfaceModel":
            kind, roots = "shell-surface", item.SbsmBoundary
        elif item.is_a() == "IfcFaceBasedSurfaceModel":
            kind, roots = "face-surface", item.FbsmFaces
        else:
            raise AssertionError(item.is_a())
        actual = {"instance": item.id(), "kind": kind, "roots": [r.id() for r in roots], "length_unit": sections.unit(unit)}
        tables = {name: {} for name in ["sets", "faces", "bounds", "loops", "points"]}
        for root in roots:
            closed = {"IfcClosedShell": True, "IfcOpenShell": False}.get(root.is_a())
            tables["sets"].setdefault(root.id(), {"instance": root.id(), "closed": closed, "faces": [f.id() for f in root.CfsFaces]})
            for face in root.CfsFaces:
                assert face.is_a() == "IfcFace"
                tables["faces"].setdefault(face.id(), {"instance": face.id(), "bounds": [b.id() for b in face.Bounds]})
                for bound in face.Bounds:
                    polygon = bound.Bound
                    assert polygon.is_a() == "IfcPolyLoop"
                    tables["bounds"].setdefault(bound.id(), {"instance": bound.id(), "polygon": polygon.id(), "orientation": bound.Orientation, "outer": bound.is_a() == "IfcFaceOuterBound"})
                    tables["loops"].setdefault(polygon.id(), {"instance": polygon.id(), "points": [p.id() for p in polygon.Polygon]})
                    for point in polygon.Polygon:
                        tables["points"].setdefault(point.id(), {"instance": point.id(), "coordinates": list(point.Coordinates)})
        actual.update({name: list(table.values()) for name, table in tables.items()})
        failures = list(curves.differences(expected, actual, str(item.id())))
        assert not failures, failures[:20]
        count += len(actual["faces"])
    print(f"{path.name}: 形 {len(data['items'])} 件・面 {count} 件、差異0")
    return len(data["items"]), count


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 5, "合成3版と公開2要素が必要"
    counts = [compare(path) for path in paths]
    print(f"IfcOpenShell {ifcopenshell.version}: 形 {sum(c[0] for c in counts)} 件・面 {sum(c[1] for c in counts)} 件、差異0")
