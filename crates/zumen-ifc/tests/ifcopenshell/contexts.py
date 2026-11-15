"""文脈の座標系・親・表示尺度の元属性を実行 API で照らす。座標変換は評価しない。"""

import json
import sys
from pathlib import Path

import ifcopenshell


def direction(value):
    return list(value.DirectionRatios) if value else None


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = ifcopenshell.open(str(path.parent / data["file"]))
    for row in data["contexts"]:
        context = model.by_id(row["id"])
        is_child = context.is_a("IfcGeometricRepresentationSubContext")
        base = context.ParentContext if is_child else context
        assert row["parent"] == (base.id() if is_child else None)
        assert row["identifier"] == base.ContextIdentifier
        assert row["kind"] == base.ContextType
        assert row["dimension"] == base.CoordinateSpaceDimension
        assert row["precision"] == base.Precision
        assert row["north"] == direction(base.TrueNorth)
        world = base.WorldCoordinateSystem
        assert row["world"] == {
            "location": list(world.Location.Coordinates),
            "axis": direction(world.Axis) if world.is_a("IfcAxis2Placement3D") else None,
            "direction": direction(world.RefDirection),
        }
        assert row["view"] == ({
            "identifier": context.ContextIdentifier,
            "kind": context.ContextType,
            "scale": context.TargetScale,
            "target": context.TargetView,
            "name": context.UserDefinedTargetView,
        } if is_child else None)
        units = [u for u in model.by_type("IfcProject")[0].UnitsInContext.Units
                 if getattr(u, "UnitType", None) == "LENGTHUNIT"]
        assert len(units) == 1 and units[0].is_a("IfcSIUnit")
        unit = units[0]
        assert row["unit"] == [unit.UnitType, unit.Prefix, unit.Name, None]
    return len(data["contexts"])


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 5, "合成 3 版と公開 2 例が必要"
    count = sum(compare(path) for path in paths)
    print(f"IfcOpenShell {ifcopenshell.version}: 表現の文脈 {count} 件、差異 0")
