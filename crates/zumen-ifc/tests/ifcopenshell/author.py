"""新しい IFC のひな形と、変更だけで作った 2 階建てと、それを直した IFC と、変換した IFC を IfcOpenShell で開いて
照らす（#898・#395）。

使い方: python author.py <フォルダー>

フォルダーには、`author.rs` の `writes_the_authored_files_for_other_readers` が、環境変数 `ZUMEN_IFC_AUTHORED` の
フォルダーに書いた `<スキーマ>.built.ifc` と `<スキーマ>.changed.ifc` と、`create.rs` の `writes_the_templates_for_other_readers` が書いた
`<スキーマ>.template.ifc` と、`convert.rs` の `writes_the_conversions_for_other_readers` が IFC4 の 2 階建てを
各バージョンへ変換した `<スキーマ>.converted.ifc` がある。

- ひな形は、検査で誤りが出ず、プロジェクトの GlobalId・名前、ミリメートルの長さの単位、`Body` の下位の文脈、
  所有履歴の作ったソフトと日時が、渡した値のとおりに読める。
- 変換した IFC は、検査で誤りが出ず、元の GlobalId・所属・開口と建具の関係・階の高さと配置・定義された種類が、
  元の値のとおりに読める。

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
import ifcopenshell.util.unit
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


# `create.rs` の `derive_global_id(b"namespace", b"project")`
TEMPLATE_PROJECT = "3pg1SZxHur0WpyDIEWpuL6"


def check_template(path):
    failures = []
    model = ifcopenshell.open(str(path))
    for error in errors(model):
        failures.append(f"検査の誤り: {error}")
    projects = model.by_type("IfcProject")
    if [p.GlobalId for p in projects] != [TEMPLATE_PROJECT]:
        failures.append(f"プロジェクトが {[p.GlobalId for p in projects]}")
        return failures
    project = projects[0]
    if project.Name != "新しい建物":
        failures.append(f"プロジェクトの名前が {project.Name}")
    scale = ifcopenshell.util.unit.calculate_unit_scale(model)
    if abs(scale - 0.001) > 1e-12:
        failures.append(f"長さの単位の大きさが {scale}")
    bodies = [c.ContextIdentifier for c in model.by_type("IfcGeometricRepresentationSubContext")]
    if bodies != ["Body"]:
        failures.append(f"下位の文脈が {bodies}")
    history = project.OwnerHistory
    application = history.OwningApplication.ApplicationFullName if history else None
    if application != "zumen" or history.CreationDate != 1_700_000_000:
        failures.append(f"所有履歴が {history}")
    return failures


# `convert.rs` の元の文書。種類ごとの数
CONVERTED = {
    "IfcSite": 1,
    "IfcBuilding": 1,
    "IfcBuildingStorey": 2,
    "IfcWall": 1,
    "IfcOpeningElement": 1,
    "IfcDoor": 1,
    "IfcSlab": 1,
    "IfcColumn": 1,
    "IfcMaterial": 5,
}


def check_converted(path):
    failures = []
    model = ifcopenshell.open(str(path))
    for error in errors(model):
        failures.append(f"検査の誤り: {error}")
    for kind, n in CONVERTED.items():
        found = len(model.by_type(kind))
        if found != n:
            failures.append(f"{kind} が {found} 個（期待は {n}）")
    project = model.by_type("IfcProject")[0]
    if (project.GlobalId, project.Name) != (gid(1), "元の建物"):
        failures.append(f"プロジェクトが {project.GlobalId} {project.Name}")
    upper = model.by_guid(gid(13))
    if model.schema_identifier == "IFC4X3_ADD2":
        if upper.Elevation is not None:
            failures.append(f"2F に非推奨の Elevation {upper.Elevation} がある")
    elif upper.Elevation != 3000.0:
        failures.append(f"2F の Elevation が {upper.Elevation}")
    z = ifcopenshell.util.placement.get_local_placement(upper.ObjectPlacement)[2][3]
    if abs(z - 3000.0) > 1e-9:
        failures.append(f"2F の配置の z が {z}")
    for n, storey in [(20, 12), (22, 12), (23, 13), (24, 13)]:
        container = ifcopenshell.util.element.get_container(model.by_guid(gid(n)))
        if container is None or container.GlobalId != gid(storey):
            failures.append(f"{n} の所属が {container and container.GlobalId}")
    opening = model.by_guid(gid(21))
    hosts = [rel.RelatingBuildingElement.GlobalId for rel in opening.VoidsElements]
    fillings = [rel.RelatedBuildingElement.GlobalId for rel in opening.HasFillings]
    if (hosts, fillings) != ([gid(20)], [gid(22)]):
        failures.append(f"開口の母体が {hosts}、建具が {fillings}")
    wall = ifcopenshell.util.placement.get_local_placement(model.by_guid(gid(20)).ObjectPlacement)
    if abs(wall[0][3] - 1000.0) > 1e-9:
        failures.append(f"壁の配置の x が {wall[0][3]}")
    door = ifcopenshell.util.placement.get_local_placement(model.by_guid(gid(22)).ObjectPlacement)
    if abs(door[0][3] - 1500.0) > 1e-9:
        failures.append(f"扉の配置の x が {door[0][3]}")
    if model.by_guid(gid(23)).PredefinedType != "FLOOR":
        failures.append("床の定義された種類が FLOOR でない")
    column = model.by_guid(gid(24))
    if model.schema_identifier != "IFC2X3" and column.PredefinedType != "COLUMN":
        failures.append(f"柱の定義された種類が {column.PredefinedType}")
    quantities = ifcopenshell.util.element.get_psets(model.by_guid(gid(20)), qtos_only=True)
    base = {k: v for k, v in quantities.get("Qto_WallBaseQuantities", {}).items() if k != "id"}
    counts = {k: v for k, v in quantities.get("Zumen_Counts", {}).items() if k != "id"}
    if (base, counts) != ({"Length": 3000.0, "NetSideArea": 7.5}, {"Count": 2}):
        failures.append(f"壁の数量が {base} {counts}")
    areas = [u for u in project.UnitsInContext.Units if getattr(u, "UnitType", None) == "AREAUNIT"]
    if [(u.is_a(), u.Name) for u in areas] != [("IfcSIUnit", "SQUARE_METRE")]:
        failures.append(f"面積の単位が {areas}")
    wall_type = ifcopenshell.util.element.get_type(model.by_guid(gid(20)))
    if wall_type is None or (wall_type.GlobalId, wall_type.PredefinedType) != (gid(25), "SHEAR"):
        failures.append(f"壁の型が {wall_type}")
    # IFC2X3 は扉の型を持たないので、変換で落とす
    door_type = ifcopenshell.util.element.get_type(model.by_guid(gid(22)))
    if model.schema_identifier == "IFC2X3":
        if door_type is not None:
            failures.append(f"IFC2X3 の扉に型 {door_type} がある")
    elif door_type is None or (
        door_type.GlobalId,
        door_type.PredefinedType,
        door_type.OperationType,
    ) != (gid(26), "NOTDEFINED", "NOTDEFINED"):
        failures.append(f"扉の型が {door_type}")
    failures += check_converted_materials(model)
    return failures


def check_converted_materials(model):
    """変換した材料。元は IFC4。IFC2X3 は層の名前と構成要素を持たないので、構成要素は並びになる"""
    failures = []
    later = model.schema_identifier != "IFC2X3"

    def material(n):
        return ifcopenshell.util.element.get_material(model.by_guid(gid(n)), should_inherit=False)

    wall = material(20)
    layers = [
        (layer.Material.Name, layer.LayerThickness, getattr(layer, "Name", None))
        for layer in getattr(wall, "MaterialLayers", [])
    ]
    expected = [("Concrete", 200.0, "Core" if later else None), ("Insulation", 50.0, None)]
    if not (wall and wall.is_a("IfcMaterialLayerSet")) or layers != expected:
        failures.append(f"壁の材料が {wall} {layers}")
    # IFC4 からは材料の並びが非推奨なので、名前のない構成要素になる
    door = material(22)
    if later:
        names = [c.Material.Name for c in getattr(door, "MaterialConstituents", None) or []]
        kind = "IfcMaterialConstituentSet"
    else:
        names = [m.Name for m in getattr(door, "Materials", [])]
        kind = "IfcMaterialList"
    if not (door and door.is_a(kind)) or names != ["Wood", "Glass"]:
        failures.append(f"扉の材料が {door} {names}")
    column = material(24)
    if not (column and column.is_a("IfcMaterial")) or column.Name != "Steel":
        failures.append(f"柱の材料が {column}")
    wall_type = material(25)
    if later:
        parts = [(c.Name, c.Material.Name) for c in getattr(wall_type, "MaterialConstituents", None) or []]
        if not (wall_type and wall_type.is_a("IfcMaterialConstituentSet")) or parts != [("Body", "Concrete")]:
            failures.append(f"壁の型の材料が {wall_type} {parts}")
    else:
        names = [m.Name for m in getattr(wall_type, "Materials", [])]
        if not (wall_type and wall_type.is_a("IfcMaterialList")) or names != ["Concrete"]:
            failures.append(f"IFC2X3 の壁の型の材料が {wall_type} {names}")
    return failures


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    folder = Path(sys.argv[1])
    failures = []
    for schema in SCHEMAS:
        name = f"{schema}.template.ifc"
        failures += [f"{name}: {f}" for f in check_template(folder / name)]
    for schema in SCHEMAS:
        for state in EXPECTED:
            name = f"{schema}.{state}.ifc"
            failures += [f"{name}: {f}" for f in check(folder / name, state)]
    for schema in SCHEMAS:
        name = f"{schema}.converted.ifc"
        failures += [f"{name}: {f}" for f in check_converted(folder / name)]
    for failure in failures:
        print(failure)
    if failures:
        sys.exit(1)
    print(f"IfcOpenShell {ifcopenshell.version}: ひな形と、作った IFC・直した IFC・変換した IFC を 3 バージョンで照らした")


if __name__ == "__main__":
    main()
