"""代表要素から得る I 形・中空矩形の元属性と単位を、合成3版で照らす。

輪郭や体積の評価は行わない。外部実装のソースやスキーマは読まない。
"""
import importlib.util
import json
import sys
from pathlib import Path
import ifcopenshell

# 再利用するのは自分たちの照合関数だけ。
spec = importlib.util.spec_from_file_location("section_curves", Path(__file__).with_name("curves.py"))
curves = importlib.util.module_from_spec(spec)
spec.loader.exec_module(curves)


def unit(value):
    if value.is_a() == "IfcSIUnit":
        return {"kind": value.UnitType, "name": value.Name, "prefix": value.Prefix, "scale": None}
    assert value.is_a() == "IfcConversionBasedUnit"
    factor = value.ConversionFactor
    base = factor.UnitComponent
    assert base.is_a() == "IfcSIUnit" and base.Name == "RADIAN" and base.Prefix is None
    assert base.UnitType == value.UnitType == "PLANEANGLEUNIT"
    exponents = ("LengthExponent", "MassExponent", "TimeExponent", "ElectricCurrentExponent",
                 "ThermodynamicTemperatureExponent", "AmountOfSubstanceExponent", "LuminousIntensityExponent")
    assert tuple(getattr(value.Dimensions, key) for key in exponents) == (0,) * 7
    return {"kind": value.UnitType, "name": value.Name, "prefix": None, "scale": factor.ValueComponent.wrappedValue}


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
    units = {u.UnitType: unit(u) for u in model.by_type("IfcProject")[0].UnitsInContext.Units}
    for item in data["items"]:
        assert item["instance"] in leaves, "要素の表現から到達できない"
        solid = model.by_id(item["instance"])
        assert solid.is_a() == "IfcExtrudedAreaSolid"
        profile = solid.SweptArea
        assert profile.ProfileType == "AREA"
        actual = {
            "instance": solid.id(), "profile": profile.id(), "name": profile.ProfileName,
            "profile_position": list(profile.Position.Location.Coordinates),
            "profile_direction": list(profile.Position.RefDirection.DirectionRatios),
            "position": list(solid.Position.Location.Coordinates),
            "axis": list(solid.Position.Axis.DirectionRatios),
            "reference_direction": list(solid.Position.RefDirection.DirectionRatios),
            "direction": list(solid.ExtrudedDirection.DirectionRatios), "depth": solid.Depth,
            "length_unit": units["LENGTHUNIT"],
        }
        if profile.is_a() == "IfcIShapeProfileDef":
            slope = getattr(profile, "FlangeSlope", None)
            actual.update({"kind": "i-shape",
                           "dimensions": [profile.OverallWidth, profile.OverallDepth, profile.WebThickness, profile.FlangeThickness],
                           "fillet_radius": profile.FilletRadius,
                           "edge_radius": getattr(profile, "FlangeEdgeRadius", None),
                           "slope": slope,
                           "angle_unit": units["PLANEANGLEUNIT"] if slope is not None else None})
        else:
            assert profile.is_a() == "IfcRectangleHollowProfileDef"
            actual.update({"kind": "rectangle-hollow",
                           "dimensions": [profile.XDim, profile.YDim, profile.WallThickness],
                           "inner_radius": profile.InnerFilletRadius, "outer_radius": profile.OuterFilletRadius})
        failures = list(curves.differences(item, actual, str(solid.id())))
        assert not failures, failures[:20]
    print(f"{path.name}: 断面と押し出し {len(data['items'])} 件、差異0")
    return len(data["items"])


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 3, "合成3版が必要"
    count = sum(compare(path) for path in paths)
    print(f"IfcOpenShell {ifcopenshell.version}: I 形・中空矩形 {count} 件、差異0")
