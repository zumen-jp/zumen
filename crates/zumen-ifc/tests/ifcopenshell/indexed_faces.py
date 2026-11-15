"""添字付き面集合の元属性を合成2版と公開の壁で照らす。形の評価は含めない。

外部実装はバイナリーだけを使い、スキーマやソースは読まない。
"""
import importlib.util
import json
import sys
from pathlib import Path
import ifcopenshell


def own_module(name):
    spec = importlib.util.spec_from_file_location("indexed_faces_" + name, Path(__file__).with_name(name + ".py"))
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
        tags = getattr(item.Coordinates, "TagList", None)
        actual = {
            "instance": item.id(), "coordinates": item.Coordinates.id(),
            "points": [list(row) for row in item.Coordinates.CoordList],
            "tags": list(tags) if tags is not None else None, "closed": item.Closed,
            "point_index": list(item.PnIndex) if item.PnIndex is not None else None,
            "length_unit": sections.unit(unit),
        }
        if item.is_a() == "IfcTriangulatedFaceSet":
            actual.update(kind="triangles", indices=[list(row) for row in item.CoordIndex],
                          normals=[list(row) for row in item.Normals] if item.Normals is not None else None)
            count += len(item.CoordIndex)
        else:
            assert item.is_a() == "IfcPolygonalFaceSet"
            unique = {face.id(): face for face in item.Faces}
            actual.update(kind="polygons", indices=[face.id() for face in item.Faces], faces=[
                {"instance": face.id(), "boundary": list(face.CoordIndex),
                 "holes": [list(row) for row in face.InnerCoordIndices] if face.is_a() == "IfcIndexedPolygonalFaceWithVoids" else []}
                for face in unique.values()])
            count += len(unique)
        failures = list(curves.differences(expected, actual, str(item.id())))
        assert not failures, failures[:20]
    print(f"{path.name}: 形 {len(data['items'])} 件・共有を除く面 {count} 件、差異0")
    return len(data["items"]), count


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 3, "合成2版と公開1要素が必要"
    counts = [compare(path) for path in paths]
    print(f"IfcOpenShell {ifcopenshell.version}: 形 {sum(c[0] for c in counts)} 件・共有を除く面 {sum(c[1] for c in counts)} 件、差異0")
