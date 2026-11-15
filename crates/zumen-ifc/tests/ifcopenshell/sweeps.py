"""自作の 3 版の掃引円盤を IfcOpenShell の元属性と照らす。形は評価しない。"""
import importlib.util
import json
import sys
from pathlib import Path

import ifcopenshell

# 自作の曲線の比較コードだけを共有する。外部実装のソースは読まない。
spec = importlib.util.spec_from_file_location("sweep_curves", Path(__file__).with_name("curves.py"))
curves = importlib.util.module_from_spec(spec)
spec.loader.exec_module(curves)


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = curves.open_model(path.parent / data["file"])
    product = model.by_id(data["product"])
    leaves, seen, stack = set(), set(), list(product.Representation.Representations)
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
    length = next(u for u in model.by_type("IfcProject")[0].UnitsInContext.Units
                  if getattr(u, "UnitType", None) == "LENGTHUNIT")
    assert length.is_a() == "IfcSIUnit"
    unit = {"kind": length.UnitType, "name": length.Name, "prefix": length.Prefix, "scale": None}
    for expected in data["items"]:
        assert expected["instance"] in leaves
        solid = model.by_id(expected["instance"])
        assert solid.is_a() == "IfcSweptDiskSolid"
        graph, visited, pending = [], set(), [solid.Directrix]
        while pending:
            curve = pending.pop()
            if curve.id() in visited:
                continue
            visited.add(curve.id())
            graph.append(curves.curve(curve))
            if curve.is_a() == "IfcCompositeCurve":
                pending.extend(s.ParentCurve for s in reversed(curve.Segments))
            elif curve.is_a() == "IfcTrimmedCurve":
                pending.append(curve.BasisCurve)
        actual = {"instance": solid.id(), "radius": solid.Radius,
                  "inner_radius": solid.InnerRadius, "start": solid.StartParam, "end": solid.EndParam,
                  "directrix": solid.Directrix.id(), "curves": graph, "length_unit": unit}
        failures = list(curves.differences(expected, actual, str(solid.id())))
        assert not failures, failures[:20]
    print(f"{path.name}: 掃引円盤 {len(data['items'])} 件、差異 0")
    return len(data["items"])


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 3, "合成 3 版が必要"
    count = sum(compare(path) for path in paths)
    print(f"IfcOpenShell {ifcopenshell.version}: 掃引円盤 {count} 件、差異 0（合成入力）")
