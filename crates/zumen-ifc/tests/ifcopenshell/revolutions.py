"""共有写像を通して取得した回転体の元属性を、合成3版で照らす。

外部実装はバイナリーだけを使う。形や体積の評価は比較しない。
"""
import importlib.util
import json
import sys
from pathlib import Path
import ifcopenshell


def own_module(name):
    spec = importlib.util.spec_from_file_location("revolution_" + name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


curves = own_module("curves")
profiles = own_module("profiles")
sections = own_module("sections")


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
    units = {u.UnitType: sections.unit(u) for u in model.by_type("IfcProject")[0].UnitsInContext.Units}
    kinds = {"IfcRectangleProfileDef": "rectangle", "IfcCircleProfileDef": "circle",
             "IfcIShapeProfileDef": "i-shape", "IfcRectangleHollowProfileDef": "rectangle-hollow",
             "IfcArbitraryClosedProfileDef": "arbitrary-closed"}
    for item in data["items"]:
        assert item["instance"] in leaves, "要素の表現から到達できない"
        solid = model.by_id(item["instance"])
        assert solid.is_a() == "IfcRevolvedAreaSolid"
        profile = solid.SweptArea
        assert profile.ProfileType == "AREA"
        actual = {
            "instance": solid.id(), "profile": profile.id(), "profile_name": profile.ProfileName,
            "profile_kind": kinds[profile.is_a()],
            "profile_position": profiles.position(getattr(profile, "Position", None)),
            "position": profiles.position(solid.Position),
            "axis_location": list(solid.Axis.Location.Coordinates),
            "axis_direction": list(solid.Axis.Axis.DirectionRatios), "angle": solid.Angle,
            "angle_unit": units["PLANEANGLEUNIT"], "length_unit": units["LENGTHUNIT"],
            "boundary": [curves.curve(profile.OuterCurve)] if profile.is_a() == "IfcArbitraryClosedProfileDef" else None,
        }
        failures = list(curves.differences(item, actual, str(solid.id())))
        assert not failures, failures[:20]
    print(f"{path.name}: 回転体 {len(data['items'])} 件、差異0")
    return len(data["items"])


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 3, "合成3版が必要"
    count = sum(compare(path) for path in paths)
    print(f"IfcOpenShell {ifcopenshell.version}: 回転体 {count} 件、差異0")
