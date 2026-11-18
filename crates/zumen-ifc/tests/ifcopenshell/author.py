"""変更だけで作った 2 階建てと、それを直した IFC を IfcOpenShell で開いて照らす（#898）。

使い方: python author.py <フォルダー>

フォルダーには、`author.rs` の `writes_the_authored_files_for_other_readers` が、環境変数 `ZUMEN_IFC_AUTHORED` の
フォルダーに書いた `<スキーマ>.built.ifc` と `<スキーマ>.changed.ifc` がある。

- IfcOpenShell の検査（スキーマと EXPRESS の規則）で誤りが出ない。
- 種類ごとの数、階の高さと配置、要素の所属、開口と建具の関係、壁の材料の層が、足した値と直した値のとおりに読める。
- 開口を引いた壁の形の体積が、芯の長さ・高さ・層の厚さと開口の大きさから求めた値になる。
"""

import sys
from pathlib import Path

import ifcopenshell
import ifcopenshell.geom
import ifcopenshell.guid
import ifcopenshell.util.element
import ifcopenshell.util.placement
import ifcopenshell.util.shape
import ifcopenshell.validate

SCHEMAS = ["IFC2X3", "IFC4", "IFC4X3_ADD2"]

COUNTS = {
    "IfcBuilding": 1,
    "IfcBuildingStorey": 2,
    "IfcGrid": 1,
    "IfcWall": 8,
    "IfcSlab": 2,
    "IfcColumn": 2,
    "IfcBeam": 2,
    "IfcOpeningElement": 2,
    "IfcDoor": 1,
    "IfcWindow": 1,
}

# 状態 → 2F の高さ、1F の最初の壁の層、壁 → 開口を引いた体積（m³）
EXPECTED = {
    "built": {
        "elevation": 3000.0,
        "layers": [150.0, 50.0],
        "volumes": {2000: (6.0 * 2.8 - 0.9 * 2.1) * 0.2, 3000: (6.0 * 2.8 - 1.2 * 1.2) * 0.2},
    },
    "changed": {
        "elevation": 3200.0,
        "layers": [200.0, 50.0, 20.0],
        "volumes": {2000: (6.5 * 3.0 - 0.9 * 2.1) * 0.27, 3000: (6.0 * 2.8 - 1.6 * 1.0) * 0.2},
    },
}


def gid(n):
    # GlobalId::from_uuid の 16 バイトの大きい端から並べた n を、IFC の 22 文字に縮めたもの
    return ifcopenshell.guid.compress(f"{n:032x}")


def errors(model):
    logger = ifcopenshell.validate.json_logger()
    ifcopenshell.validate.validate(model, logger, express_rules=True)
    return sorted(
        f"{s.get('attribute')}: {str(s.get('message')).splitlines()[0]}"
        for s in logger.statements
        if s.get("level") == "error"
    )


def check(path, state):
    failures = []
    model = ifcopenshell.open(str(path))
    for error in errors(model):
        failures.append(f"検査の誤り: {error}")
    for kind, n in COUNTS.items():
        found = len(model.by_type(kind))
        if found != n:
            failures.append(f"{kind} が {found} 個（期待は {n}）")
    expected = EXPECTED[state]
    upper = model.by_guid(gid(1002))
    # IFC4.3 は Elevation を非推奨にしたので書かず、配置だけで高さを持つ
    if model.schema_identifier == "IFC4X3_ADD2":
        if upper.Elevation is not None:
            failures.append(f"2F に非推奨の Elevation {upper.Elevation} がある")
    elif abs(upper.Elevation - expected["elevation"]) > 1e-9:
        failures.append(f"2F の Elevation が {upper.Elevation}")
    z = ifcopenshell.util.placement.get_local_placement(upper.ObjectPlacement)[2][3]
    if abs(z - expected["elevation"]) > 1e-9:
        failures.append(f"2F の配置の z が {z}")
    for level, storey in [(1, 1001), (2, 1002)]:
        base = 1000 * (level + 1)
        for n in [base, base + 1, base + 2, base + 3, base + 10, base + 11, base + 12, base + 21]:
            container = ifcopenshell.util.element.get_container(model.by_guid(gid(n)))
            if container is None or container.GlobalId != gid(storey):
                failures.append(f"{n} の所属が {container and container.GlobalId}")
        opening = model.by_guid(gid(base + 20))
        hosts = [rel.RelatingBuildingElement.GlobalId for rel in opening.VoidsElements]
        if hosts != [gid(base)]:
            failures.append(f"開口 {base + 20} の母体が {hosts}")
        fillings = [rel.RelatedBuildingElement.GlobalId for rel in opening.HasFillings]
        if fillings != [gid(base + 21)]:
            failures.append(f"開口 {base + 20} の建具が {fillings}")
    material = ifcopenshell.util.element.get_material(model.by_guid(gid(2000)))
    layers = [layer.LayerThickness for layer in material.ForLayerSet.MaterialLayers]
    if layers != expected["layers"]:
        failures.append(f"壁 2000 の層が {layers}")
    settings = ifcopenshell.geom.settings()
    for n, volume in expected["volumes"].items():
        shape = ifcopenshell.geom.create_shape(settings, model.by_guid(gid(n)))
        found = ifcopenshell.util.shape.get_volume(shape.geometry)
        if abs(found - volume) > 1e-6:
            failures.append(f"壁 {n} の体積が {found}（期待は {volume}）")
    return failures


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    folder = Path(sys.argv[1])
    failures = []
    for schema in SCHEMAS:
        for state in EXPECTED:
            name = f"{schema}.{state}.ifc"
            failures += [f"{name}: {f}" for f in check(folder / name, state)]
    for failure in failures:
        print(failure)
    if failures:
        sys.exit(1)
    print(f"IfcOpenShell {ifcopenshell.version}: 作った IFC と直した IFC を 3 版で照らした")


if __name__ == "__main__":
    main()
