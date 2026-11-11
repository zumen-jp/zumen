"""要素から取得した未評価の矩形・円・任意断面と押し出しの元属性を照らす。

合成3版と公開の床・家具・柱。配置の適用、単位換算や形の評価は行わない。
診断・予算・出所はRustの検査とnative/WASIの一致で確認する。
"""
import importlib.util
import json
import sys
from pathlib import Path
import ifcopenshell

# 共有するのは自分たちの比較コードだけ。外部実装のソースは読まない。
spec = importlib.util.spec_from_file_location("profile_curves", Path(__file__).with_name("curves.py"))
curves = importlib.util.module_from_spec(spec)
spec.loader.exec_module(curves)


def position(entity):
    if entity is None:
        return None
    result = {"location": list(entity.Location.Coordinates),
              "reference_direction": list(entity.RefDirection.DirectionRatios) if entity.RefDirection else None}
    if entity.is_a() == "IfcAxis2Placement3D":
        result["axis"] = list(entity.Axis.DirectionRatios) if entity.Axis else None
    return result


def graph(root):
    # この取得経路の任意断面は、既存getterで閉鎖を確認する単独の輪だけ。
    return [curves.curve(root)]


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
    exponents = ("LengthExponent", "MassExponent", "TimeExponent", "ElectricCurrentExponent",
                 "ThermodynamicTemperatureExponent", "AmountOfSubstanceExponent", "LuminousIntensityExponent")
    assert tuple(getattr(length.Dimensions, key) for key in exponents) == (1, 0, 0, 0, 0, 0, 0)
    if length.is_a() == "IfcSIUnit":
        unit = {"kind": length.UnitType, "name": length.Name, "prefix": length.Prefix, "scale": None}
    else:
        assert length.is_a() == "IfcConversionBasedUnit"
        factor = length.ConversionFactor
        base = factor.UnitComponent
        assert base.is_a() == "IfcSIUnit" and base.UnitType == "LENGTHUNIT"
        assert base.Name == "METRE"
        assert base.Prefix in (None, "MILLI"), "この照合が使う接頭辞だけ"
        unit = {"kind": length.UnitType, "name": length.Name, "prefix": None,
                "scale": factor.ValueComponent.wrappedValue * (0.001 if base.Prefix == "MILLI" else 1.0)}
    for item in data["items"]:
        assert item["instance"] in leaves, "要素の表現から到達できない"
        solid = model.by_id(item["instance"])
        assert solid.is_a() == "IfcExtrudedAreaSolid"
        profile = solid.SweptArea
        assert profile.ProfileType == "AREA"
        kind = profile.is_a()
        if kind == "IfcRectangleProfileDef":
            kind, dims = "rectangle", [profile.XDim, profile.YDim]
        elif kind == "IfcCircleProfileDef":
            kind, dims = "circle", [profile.Radius]
        else:
            assert kind == "IfcArbitraryClosedProfileDef"
            kind, dims = "arbitrary-closed", []
        actual = {"instance": solid.id(), "profile": profile.id(), "profile_name": profile.ProfileName,
                  "profile_kind": kind, "dimensions": dims,
                  "profile_position": position(getattr(profile, "Position", None)),
                  "position": position(solid.Position), "direction": list(solid.ExtrudedDirection.DirectionRatios),
                  "depth": solid.Depth, "unit": unit,
                  "boundary": graph(profile.OuterCurve) if kind == "arbitrary-closed" else None}
        expected = {key: item[key] for key in actual}
        failures = list(curves.differences(expected, actual, str(solid.id())))
        assert not failures, failures[:20]
    print(f"{path.name}: 断面と押し出し {len(data['items'])} 件、差異0")
    return len(data["items"])


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 6, "合成3版と公開3要素が必要"
    count = sum(compare(path) for path in paths)
    print(f"IfcOpenShell {ifcopenshell.version}: 断面と押し出し {count} 件、差異0")
