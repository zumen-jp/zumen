"""未適用の押し出し・断面・外輪の元属性を IfcOpenShell と照らす。

合成の 3 版は synthetic、既存公開サンプルの柱 1 件は external の範囲。
単位は SI の接頭辞・名前を比較し、配置・写像の適用や自己交差・面積・メッシュは評価しない。
診断・予算・出所の列は Rust 側で検査する。
"""

import importlib.util
import json
import sys
from pathlib import Path

import ifcopenshell

# -I でも自分たちの比較コードを、場所を指定して共有する。外部の実装のソースは読まない。
spec = importlib.util.spec_from_file_location("curve_comparison", Path(__file__).with_name("curves.py"))
curves = importlib.util.module_from_spec(spec)
spec.loader.exec_module(curves)


def position(entity):
    if entity is None:
        return None
    return {
        "location": list(entity.Location.Coordinates),
        "axis": list(entity.Axis.DirectionRatios) if entity.Axis is not None else None,
        "reference_direction": list(entity.RefDirection.DirectionRatios) if entity.RefDirection is not None else None,
    }


def length_unit(model):
    project = model.by_type("IfcProject")[0]
    unit = next(u for u in project.UnitsInContext.Units if u.UnitType == "LENGTHUNIT")
    assert unit.is_a("IfcSIUnit"), "この照合の単位は SI のみ"
    return {"kind": unit.UnitType, "name": unit.Name, "prefix": unit.Prefix, "scale": None}


def compare(path):
    data = json.loads(path.read_text(encoding="utf-8"))
    model = ifcopenshell.open(str(path.parent / data["file"]))
    for item in data["items"]:
        entity = model.by_id(item["instance"])
        assert entity.is_a() == "IfcExtrudedAreaSolid"
        profile = entity.SweptArea
        assert profile.is_a() == "IfcArbitraryClosedProfileDef" and profile.ProfileType == "AREA"
        expected = {k: item[k] for k in ("instance", "profile", "profile_name", "direction", "depth", "position", "unit", "boundary")}
        actual = {
            "instance": entity.id(), "profile": profile.id(), "profile_name": profile.ProfileName,
            "direction": list(entity.ExtrudedDirection.DirectionRatios), "depth": entity.Depth,
            "position": position(entity.Position), "unit": length_unit(model),
            "boundary": curves.curve(profile.OuterCurve),
        }
        failures = list(curves.differences(expected, actual, str(item["instance"])))
        assert not failures, failures[:20]
    print(f"{path.name}: 押し出し {len(data['items'])}、差異 0")
    return len(data["items"])


if __name__ == "__main__":
    paths = sorted(Path(sys.argv[1]).glob("*.json"))
    assert len(paths) == 4, "合成 3 版と公開サンプルが必要"
    total = sum(compare(path) for path in paths)
    print(f"IfcOpenShell {ifcopenshell.version}: 押し出し {total} 件、差異 0")
