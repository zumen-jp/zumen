//! 建物モデルから新しい IFC を書く変換（#395）。ファイルは、どれもテストの中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_building::{
    Building, ElementRef, FaceModelKind, GlobalId, Id, Literal, Local, MaterialKind, ParentKind,
    QuantityKind, Transform, TypedValue,
};
use zumen_ifc::{
    ConvertOptions, Converted, Edit, Header, Ifc, LengthUnit, NewConstituent, NewElement, NewFaces,
    NewMaterial, NewMaterialLayer, NewPlacement, NewQuantity, NewSpace, NewType, NewUnit,
    OwnerHistory, SpaceKind, Target, Template, Version, apply, convert, create,
};
use zumen_sdk::conversion::ConversionFidelity;
use zumen_sdk::{Codec, Lift};

const VERSIONS: [Version; 3] = [Version::Ifc2x3, Version::Ifc4, Version::Ifc4x3];

#[path = "support/shape.rs"]
mod shape;

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn global(n: u32) -> GlobalId {
    GlobalId::from_uuid(u128::from(n).to_be_bytes())
}

fn owner() -> OwnerHistory {
    OwnerHistory {
        person: "設計者".to_owned(),
        organization: "Example".to_owned(),
        developer: "zumen".to_owned(),
        application: "zumen".to_owned(),
        version: "0.3.0".to_owned(),
        identifier: "zumen".to_owned(),
        created: 1_700_000_000,
    }
}

fn options(version: Version) -> ConvertOptions {
    ConvertOptions {
        version,
        header: Header {
            name: "converted.ifc".to_owned(),
            time_stamp: "2026-11-18T09:00:00".to_owned(),
            originating_system: "zumen 0.3.0".to_owned(),
            ..Header::default()
        },
        owner: Some(owner()),
        namespace: None,
        limit: 1000,
    }
}

/// 読んで形のある建物モデルにする。人・組織・次元の指数を落とした診断と、形のない要素の診断のほかは出ないこと
fn lift(src: &[u8]) -> Option<Building> {
    let (file, diag) = with(|cx| Ifc::read(src, cx));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    let file = file.ok()?;
    let (building, diag) = with(|cx| file.lift_geometry(cx));
    let others: Vec<_> = diag
        .iter()
        .filter(|d| ![codes::IFC_ENTITY_DROPPED, codes::IFC_SHAPE_MISSING].contains(&d.code()))
        .collect();
    assert!(others.is_empty(), "{others:?}");
    building.ok()
}

fn at(x: f64, y: f64, z: f64, relative_to: Target) -> Option<NewPlacement> {
    Some(NewPlacement {
        relative_to: Some(relative_to),
        local: Transform::translation(x, y, z),
    })
}

fn element(
    class: &str,
    n: u32,
    predefined: Option<&str>,
    container: Option<Target>,
    placement: Option<NewPlacement>,
) -> Edit {
    Edit::Element(NewElement {
        class: class.to_owned(),
        predefined: predefined.map(str::to_owned),
        name: Some(format!("E{n}")),
        container,
        placement,
        global_id: Some(global(n)),
    })
}

fn space(
    kind: SpaceKind,
    n: u32,
    parent: Target,
    placement: Option<NewPlacement>,
    elevation: Option<f64>,
) -> Edit {
    Edit::Space(NewSpace {
        kind,
        name: Some(format!("S{n}")),
        parent,
        placement,
        elevation,
        global_id: Some(global(n)),
    })
}

fn element_type(class: &str, n: u32, predefined: Option<&str>) -> Edit {
    Edit::ElementType(NewType {
        class: class.to_owned(),
        predefined: predefined.map(str::to_owned),
        element_type: None,
        name: Some(format!("T{n}")),
        global_id: Some(global(n)),
    })
}

/// 敷地・建物・2 つの階・壁・開口・扉・床・柱と、壁と扉の型の文書を、書き手で作る。2F は高さ 3000 で、建物から
/// z 3000 に置く。IFC2X3 は扉の型（`IfcDoorType`）を持たないので、壁の型だけにする
fn source(version: Version) -> Option<Vec<u8>> {
    let template = Template {
        version,
        unit: LengthUnit::Millimeter,
        project: global(1),
        name: Some("元の建物".to_owned()),
        header: Header::default(),
        owner: Some(owner()),
    };
    let (bytes, _) = with(|cx| create(&template, cx));
    let bytes = bytes.ok()?;
    let (file, _) = with(|cx| Ifc::read(&bytes, cx));
    let file = file.ok()?;
    // IFC2X3 の柱は定義された種類を持たない
    let column = (version != Version::Ifc2x3).then_some("COLUMN");
    let mut edits = vec![
        space(
            SpaceKind::Site,
            10,
            Target::Instance(1),
            Some(NewPlacement {
                relative_to: None,
                local: Transform::IDENTITY,
            }),
            None,
        ),
        space(SpaceKind::Building, 11, Target::Added(0), None, None),
        space(SpaceKind::Storey, 12, Target::Added(1), None, Some(0.0)),
        space(
            SpaceKind::Storey,
            13,
            Target::Added(1),
            at(0.0, 0.0, 3000.0, Target::Added(1)),
            Some(3000.0),
        ),
        element(
            "IfcWall",
            20,
            None,
            Some(Target::Added(2)),
            at(1000.0, 0.0, 0.0, Target::Added(2)),
        ),
        element(
            "IfcOpeningElement",
            21,
            None,
            None,
            at(500.0, 0.0, 0.0, Target::Added(4)),
        ),
        element(
            "IfcDoor",
            22,
            None,
            Some(Target::Added(2)),
            at(0.0, 0.0, 0.0, Target::Added(5)),
        ),
        Edit::Void {
            host: Target::Added(4),
            opening: Target::Added(5),
        },
        Edit::Fill {
            opening: Target::Added(5),
            filler: Target::Added(6),
        },
        element(
            "IfcSlab",
            23,
            Some("FLOOR"),
            Some(Target::Added(3)),
            at(0.0, 0.0, 0.0, Target::Added(3)),
        ),
        element(
            "IfcColumn",
            24,
            column,
            Some(Target::Added(3)),
            at(2000.0, 2000.0, 0.0, Target::Added(3)),
        ),
        element_type("IfcWallType", 25, Some("SHEAR")),
        Edit::Type {
            element: Target::Added(4),
            type_of: Some(Target::Added(9)),
        },
    ];
    if version != Version::Ifc2x3 {
        edits.push(element_type("IfcDoorType", 26, None));
        edits.push(Edit::Type {
            element: Target::Added(6),
            type_of: Some(Target::Added(10)),
        });
    }
    // 形。壁は芯の線と矩形、床は穴のある多角形、柱は円の押し出し
    let [wall, slab, column] = shape::examples();
    edits.extend(
        [
            (4, wall),
            (7, slab),
            (8, column),
            (6, shape::door(version != Version::Ifc2x3)),
        ]
        .map(|(n, shape)| Edit::Shape {
            element: Target::Added(n),
            shape,
        }),
    );
    // 材料。壁は層、扉は並び（IFC4 からは非推奨なので名前のない構成要素）、柱は単一、壁の型は構成要素
    // （IFC2X3 は持たないので単一）。層の名前は IFC4 から
    let later = version != Version::Ifc2x3;
    let layer = |material: &str, thickness, name: Option<&str>| NewMaterialLayer {
        material: Some(material.to_owned()),
        thickness,
        name: name.filter(|_| later).map(str::to_owned),
    };
    edits.extend([
        Edit::Material {
            element: Target::Added(4),
            material: NewMaterial::Layers {
                layers: vec![
                    layer("Concrete", 200.0, Some("Core")),
                    layer("Insulation", 50.0, None),
                ],
                usage: None,
            },
        },
        Edit::Material {
            element: Target::Added(6),
            material: if later {
                NewMaterial::Constituents(
                    ["Wood", "Glass"]
                        .map(|material| NewConstituent {
                            material: material.to_owned(),
                            name: None,
                        })
                        .to_vec(),
                )
            } else {
                NewMaterial::List(vec!["Wood".to_owned(), "Glass".to_owned()])
            },
        },
        Edit::Material {
            element: Target::Added(8),
            material: NewMaterial::Single("Steel".to_owned()),
        },
        Edit::Material {
            element: Target::Added(9),
            material: if later {
                NewMaterial::Constituents(vec![NewConstituent {
                    material: "Concrete".to_owned(),
                    name: Some("Body".to_owned()),
                }])
            } else {
                NewMaterial::Single("Concrete".to_owned())
            },
        },
    ]);
    let (applied, diag) = with(|cx| apply(&file, &edits, b"seed", cx));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    let bytes = applied.ok()?.bytes;
    // 数量は文書にあるインスタンスの番号で指すので、書いた後に足す
    let (file, _) = with(|cx| Ifc::read(&bytes, cx));
    let file = file.ok()?;
    let wall = instance_of(&bytes, global(20))?;
    let quantity = |set: &str, name: &str, kind, value, formula: Option<&str>| Edit::Quantity {
        element: wall,
        set: set.to_owned(),
        method: set.starts_with("Qto_").then(|| "BaseQuantities".to_owned()),
        quantity: NewQuantity {
            name: name.to_owned(),
            kind,
            value,
            formula: formula.map(str::to_owned),
        },
    };
    const BASE: &str = "Qto_WallBaseQuantities";
    // IFC2X3 の数量は式を持たない
    let formula = (version != Version::Ifc2x3).then_some("L");
    let edits = [
        Edit::Unit(NewUnit {
            kind: "AREAUNIT".to_owned(),
            prefix: None,
            name: "SQUARE_METRE".to_owned(),
        }),
        quantity(BASE, "Length", QuantityKind::Length, 3000.0, formula),
        quantity(BASE, "NetSideArea", QuantityKind::Area, 7.5, None),
        // 個数は壁の基本の数量にない（ifc-gherkin-rules の QTY001）ので、別のセットに書く
        quantity("Zumen_Counts", "Count", QuantityKind::Count, 2.0, None),
    ];
    let (applied, diag) = with(|cx| apply(&file, &edits, b"seed2", cx));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    applied.ok().map(|a| a.bytes)
}

/// GlobalId が `global` のインスタンスの番号
fn instance_of(bytes: &[u8], global: GlobalId) -> Option<u64> {
    let text = core::str::from_utf8(bytes).ok()?;
    let needle = format!("('{global}'");
    let line = text.lines().find(|line| line.contains(&needle))?;
    line.strip_prefix('#')?.split('=').next()?.parse().ok()
}

/// 数量のセットの名前・測り方と、数量の名前・種類・値・式
type QuantitySetRow = (
    String,
    Option<String>,
    Vec<(String, QuantityKind, f64, Option<String>)>,
);

type MaterialRow = (
    MaterialKind,
    Vec<(Option<String>, Option<f64>, Option<String>)>,
    Option<String>,
);

/// 材料の構成の種類・構成ごとの材料の名前と厚さと名前・層の向き
fn materials(building: &Building, n: u32) -> Option<MaterialRow> {
    let e = building.find(Id::Global(global(n)))?;
    let material = building.components.material.get(e)?;
    let name = |m: Option<ElementRef>| {
        m.and_then(|m| building.components.identity.get(m))
            .and_then(|i| i.name.clone())
    };
    Some((
        material.kind,
        material
            .parts
            .iter()
            .map(|p| {
                (
                    name(p.material),
                    p.thickness.as_ref().map(|t| t.value),
                    p.name.clone(),
                )
            })
            .collect(),
        material.direction.clone(),
    ))
}

/// 数量のセットごとの値
fn quantities(building: &Building, n: u32) -> Vec<QuantitySetRow> {
    let Some(e) = building.find(Id::Global(global(n))) else {
        return Vec::new();
    };
    building
        .components
        .quantities
        .get(e)
        .map(|q| {
            q.sets
                .iter()
                .map(|set| {
                    (
                        set.name.clone(),
                        set.method.clone(),
                        set.quantities
                            .iter()
                            .map(|q| (q.name.clone(), q.kind, q.value.value, q.formula.clone()))
                            .collect(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn run(building: &Building, options: &ConvertOptions) -> (Option<Converted>, Vec<Code>) {
    let (result, diag) = with(|cx| convert(building, b"document", options, cx));
    (result.ok(), diag.iter().map(|d| d.code()).collect())
}

/// 要素の ID・クラス・定義された種類・名前・所属・配置・型
type Row = (
    Id,
    String,
    Option<String>,
    Option<String>,
    Option<(ParentKind, Id)>,
    Option<(Option<Id>, [[f64; 4]; 3])>,
    Option<Id>,
);

fn rows(building: &Building) -> Vec<Row> {
    let id = |e: ElementRef| building.id(e);
    let c = &building.components;
    let mut rows: Vec<Row> = building
        .elements()
        .iter()
        .enumerate()
        .filter_map(|(n, _)| {
            let e = ElementRef(u32::try_from(n).ok()?);
            // 材料は GlobalId を持たないので、構成の中で比べる（materials）
            let class = c.class.get(e).filter(|class| class.name != "IfcMaterial")?;
            let parent = c
                .parent
                .get(e)
                .and_then(|p| p.of.as_ref())
                .and_then(|of| Some((of.kind, id(of.target)?)));
            let placement = c.placement.get(e).and_then(|p| match &p.local {
                Local::Transform(t) => Some((p.relative_to.and_then(id), t.0)),
                Local::Unsupported(_) => None,
            });
            Some((
                id(e)?,
                class.name.clone(),
                class.predefined.clone(),
                c.identity.get(e).and_then(|i| i.name.clone()),
                parent,
                placement,
                c.r#type.get(e).and_then(|t| id(t.target)),
            ))
        })
        .collect();
    rows.sort_by_key(|row| row.0.to_string());
    rows
}

fn storey_elevation(building: &Building, n: u32) -> Option<f64> {
    let e = building.find(Id::Global(global(n)))?;
    building
        .components
        .spatial
        .get(e)?
        .elevation
        .as_ref()
        .map(|m| m.value)
}

#[test]
fn converts_the_structure_between_every_version() {
    for from in VERSIONS {
        let source = lift(&source(from).unwrap()).unwrap();
        for to in VERSIONS {
            let (converted, codes) = run(&source, &options(to));
            let converted = converted.unwrap();
            let text = String::from_utf8(converted.bytes.clone()).unwrap();
            assert!(
                text.contains(&format!("FILE_SCHEMA(('{}'));", to.file_schema())),
                "{from:?} → {to:?}"
            );
            let target = lift(&converted.bytes).unwrap();
            let mut expected = rows(&source);
            let found = rows(&target);
            // IFC2X3 の柱は定義された種類を持たないので落とし、近似として数える
            let column = Id::Global(global(24));
            // IFC2X3 は扉の型を持たないので落とし、扉の型を近似として数える
            let door = Id::Global(global(22));
            let door_type = Id::Global(global(26));
            let lost_column = from != Version::Ifc2x3 && to == Version::Ifc2x3;
            if lost_column {
                expected.retain(|row| row.0 != door_type);
                for row in &mut expected {
                    if row.0 == column {
                        row.2 = None;
                    }
                    if row.0 == door {
                        row.6 = None;
                    }
                }
            }
            assert_eq!(found, expected, "{from:?} → {to:?}");
            // 形は、書き手が省かずに書く配置のほかは保つ
            for n in [20, 22, 23, 24] {
                let of = |building: &Building| {
                    building
                        .find(Id::Global(global(n)))
                        .and_then(|e| shape::shape_of(building, e))
                };
                let wanted = of(&source).map(|s| shape::as_written(&s));
                assert!(wanted.is_some(), "{from:?} {n}");
                let found = of(&target);
                // IFC2X3 へは三角形の箱を、点を共有する多角形の閉殻にする
                if n == 22 && to == Version::Ifc2x3 && from != Version::Ifc2x3 {
                    let faces = found.map(|s| s.faces);
                    let Some([NewFaces::Polygons(graph)]) = faces.as_deref() else {
                        panic!("{from:?} → {to:?}: {faces:?}");
                    };
                    assert_eq!(graph.kind, FaceModelKind::FacetedSolid);
                    assert_eq!((graph.faces.len(), graph.points.len()), (12, 8));
                    continue;
                }
                assert_eq!(found, wanted, "{from:?} → {to:?} {n}");
            }
            // 数量は保つ。IFC2X3 へは式を落とす
            let mut wanted = quantities(&source, 20);
            if to == Version::Ifc2x3 {
                for (_, _, items) in &mut wanted {
                    for item in items {
                        item.3 = None;
                    }
                }
            }
            assert_eq!(quantities(&target, 20), wanted, "{from:?} → {to:?}");
            // 材料は保つ。IFC2X3 へは層の名前を落とし、構成要素を並びで書く。IFC4 からは並びを構成要素にする
            let mut materials_wanted = [20, 22, 24, 25].map(|n| materials(&source, n));
            for row in materials_wanted.iter_mut().flatten() {
                if to == Version::Ifc2x3 {
                    for part in &mut row.1 {
                        part.2 = None;
                    }
                    if row.0 == MaterialKind::Constituents {
                        row.0 = MaterialKind::List;
                    }
                } else if row.0 == MaterialKind::List {
                    row.0 = MaterialKind::Constituents;
                }
            }
            assert!(materials_wanted.iter().all(Option::is_some));
            assert_eq!(
                [20, 22, 24, 25].map(|n| materials(&target, n)),
                materials_wanted,
                "{from:?} → {to:?}"
            );
            assert_eq!(
                wanted.iter().map(|set| set.2.len()).collect::<Vec<_>>(),
                [2, 1]
            );
            assert_eq!(
                target.document.project_name.as_deref(),
                Some("元の建物"),
                "{from:?} → {to:?}"
            );

            // 2F の高さは、IFC4.3 では配置だけが持つ
            match to {
                Version::Ifc4x3 => assert_eq!(storey_elevation(&target, 13), None),
                Version::Ifc2x3 | Version::Ifc4 => {
                    assert_eq!(
                        storey_elevation(&target, 13),
                        Some(3000.0),
                        "{from:?} → {to:?}"
                    );
                }
            }

            let mut approximated: Vec<_> = converted
                .report
                .details
                .iter()
                .filter(|d| d.fidelity != ConversionFidelity::Native)
                .map(|d| (d.kind.as_str(), d.fidelity, d.reasons.clone()))
                .collect();
            approximated.sort_by(|a, b| a.0.cmp(b.0));
            let mut wanted = Vec::new();
            if lost_column {
                wanted.extend([
                    (
                        "IfcColumn",
                        ConversionFidelity::Approximated,
                        vec!["predefined".to_owned()],
                    ),
                    (
                        "IfcDoor",
                        ConversionFidelity::Approximated,
                        vec!["type".to_owned()],
                    ),
                    ("IfcDoorType", ConversionFidelity::Dropped, Vec::new()),
                ]);
            }
            // 長さの数量の式・層の名前・構成要素・三角形の面集合は IFC2X3 に書けない
            if from != Version::Ifc2x3 && to == Version::Ifc2x3 {
                if let Some(door) = wanted.iter_mut().find(|w| w.0 == "IfcDoor") {
                    door.2
                        .splice(0..0, ["material".to_owned(), "representation".to_owned()]);
                }
                wanted.extend([
                    (
                        "IfcWall",
                        ConversionFidelity::Approximated,
                        vec!["material".to_owned(), "quantities".to_owned()],
                    ),
                    (
                        "IfcWallType",
                        ConversionFidelity::Approximated,
                        vec!["material".to_owned()],
                    ),
                ]);
            }
            // IFC4 からは材料の並びが非推奨なので、構成要素にする
            if from == Version::Ifc2x3 && to != Version::Ifc2x3 {
                wanted.push((
                    "IfcDoor",
                    ConversionFidelity::Approximated,
                    vec!["material".to_owned()],
                ));
            }
            assert_eq!(approximated, wanted, "{from:?} → {to:?}");
            let values: usize = wanted
                .iter()
                .filter(|w| w.1 == ConversionFidelity::Approximated)
                .map(|w| w.2.len())
                .sum();
            let dropped = wanted.len()
                - wanted
                    .iter()
                    .filter(|w| w.1 == ConversionFidelity::Approximated)
                    .count();
            let count = |code| codes.iter().filter(|&&c| c == code).count();
            assert_eq!(
                (
                    count(codes::CONVERT_ELEMENT_UNSUPPORTED),
                    count(codes::CONVERT_ITEM_APPROXIMATED),
                    codes.len()
                ),
                (dropped, values, dropped + values),
                "{from:?} → {to:?}: {codes:?}"
            );
            // プロジェクト・空間 4 つ・要素 5 つ・型（IFC2X3 からは壁の型だけ）・材料 5 つ
            let types = if from == Version::Ifc2x3 { 1 } else { 2 };
            assert_eq!(converted.report.total, 15 + types, "{from:?} → {to:?}");
        }
    }
}

#[test]
fn writes_the_same_bytes_and_keeps_the_project_global_id() {
    let source = lift(&source(Version::Ifc4).unwrap()).unwrap();
    let (a, _) = run(&source, &options(Version::Ifc4x3));
    let (b, _) = run(&source, &options(Version::Ifc4x3));
    let a = a.unwrap();
    assert_eq!(Some(&a.bytes), b.as_ref().map(|b| &b.bytes));
    let text = String::from_utf8(a.bytes).unwrap();
    assert!(
        text.contains(&format!("#1=IFCPROJECT('{}'", global(1))),
        "{text}"
    );
    // 名前空間を変えると、書き手が足す関係の GlobalId が変わる
    let mut other = options(Version::Ifc4x3);
    other.namespace = Some(b"other".to_vec());
    let (c, _) = run(&source, &other);
    assert_ne!(Some(text.into_bytes()), c.map(|c| c.bytes));
}

/// プロジェクト・敷地・建物・階・部屋・壁（説明・型・プロパティ）・家具の IFC4。プロジェクトにもプロパティを付ける
fn rich() -> String {
    let g = |n: u32| global(n).to_string();
    let lines = [
        format!("#1=IFCPROJECT('{}',$,'P',$,$,$,$,$,#3);", g(1)),
        "#3=IFCUNITASSIGNMENT((#4,#5,#6,#7));".to_owned(),
        "#4=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);".to_owned(),
        "#5=IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.);".to_owned(),
        "#6=IFCSIUNIT(*,.TIMEUNIT.,$,.SECOND.);".to_owned(),
        "#7=IFCCONVERSIONBASEDUNIT(#8,.PLANEANGLEUNIT.,'degree',#9);".to_owned(),
        "#8=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);".to_owned(),
        "#9=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.0174532925199433),#15);".to_owned(),
        "#15=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);".to_owned(),
        format!(
            "#10=IFCSITE('{}',$,'Site',$,$,#11,$,$,.ELEMENT.,$,$,$,$,$);",
            g(10)
        ),
        "#11=IFCLOCALPLACEMENT($,#12);".to_owned(),
        "#12=IFCAXIS2PLACEMENT3D(#13,$,$);".to_owned(),
        "#13=IFCCARTESIANPOINT((0.,0.,0.));".to_owned(),
        format!("#14=IFCRELAGGREGATES('{}',$,$,$,#1,(#10));", g(14)),
        format!(
            "#30=IFCBUILDING('{}',$,'B',$,$,#31,$,$,.ELEMENT.,$,$,$);",
            g(30)
        ),
        "#31=IFCLOCALPLACEMENT(#11,#12);".to_owned(),
        format!("#32=IFCRELAGGREGATES('{}',$,$,$,#10,(#30));", g(32)),
        format!(
            "#40=IFCBUILDINGSTOREY('{}',$,'1F',$,$,#41,$,$,.ELEMENT.,0.);",
            g(40)
        ),
        "#41=IFCLOCALPLACEMENT(#31,#12);".to_owned(),
        format!("#42=IFCRELAGGREGATES('{}',$,$,$,#30,(#40));", g(42)),
        format!(
            "#50=IFCSPACE('{}',$,'Room',$,$,#51,$,$,.ELEMENT.,.INTERNAL.,$);",
            g(50)
        ),
        "#51=IFCLOCALPLACEMENT(#41,#12);".to_owned(),
        format!("#52=IFCRELAGGREGATES('{}',$,$,$,#40,(#50));", g(52)),
        format!(
            "#60=IFCWALL('{}',$,'W','説明',$,#61,$,$,.SOLIDWALL.);",
            g(60)
        ),
        "#61=IFCLOCALPLACEMENT(#41,#62);".to_owned(),
        "#62=IFCAXIS2PLACEMENT3D(#63,$,$);".to_owned(),
        "#63=IFCCARTESIANPOINT((1000.,0.,0.));".to_owned(),
        format!(
            "#64=IFCWALLTYPE('{}',$,'WT',$,$,(#69),$,$,$,.SHEAR.);",
            g(64)
        ),
        "#73=IFCPROPERTYSINGLEVALUE('Reference',$,IFCIDENTIFIER('W-1'),$);".to_owned(),
        format!(
            "#69=IFCPROPERTYSET('{}',$,'Pset_WallCommon',$,(#73));",
            g(69)
        ),
        format!("#65=IFCRELDEFINESBYTYPE('{}',$,$,$,(#60),#64);", g(65)),
        "#66=IFCPROPERTYSINGLEVALUE('IsExternal',$,IFCBOOLEAN(.T.),$);".to_owned(),
        format!(
            "#67=IFCPROPERTYSET('{}',$,'Pset_WallCommon',$,(#66));",
            g(67)
        ),
        format!(
            "#68=IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#60),#67);",
            g(68)
        ),
        format!("#70=IFCFURNITURE('{}',$,'F',$,$,#71,$,$,$);", g(70)),
        "#71=IFCLOCALPLACEMENT(#41,#12);".to_owned(),
        format!(
            "#72=IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,$,$,(#60,#70),#40);",
            g(72)
        ),
        "#80=IFCPROPERTYSINGLEVALUE('Phase',$,IFCLABEL('基本設計'),$);".to_owned(),
        "#81=IFCPROPERTYENUMERATEDVALUE('Status',$,(IFCLABEL('NEW')),$);".to_owned(),
        format!(
            "#82=IFCPROPERTYSET('{}',$,'Zumen_Project',$,(#80,#81));",
            g(82)
        ),
        format!("#83=IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#1),#82);", g(83)),
        "#90=IFCQUANTITYAREA('NetSideArea',$,$,7.5,$);".to_owned(),
        "#91=IFCQUANTITYTIME('Duration',$,$,60.,$);".to_owned(),
        "#92=IFCQUANTITYCOUNT('Count',$,$,2.5,$);".to_owned(),
        format!(
            "#93=IFCELEMENTQUANTITY('{}',$,'Qto_WallBaseQuantities',$,$,(#90,#91,#92));",
            g(93)
        ),
        format!(
            "#94=IFCRELDEFINESBYPROPERTIES('{}',$,$,$,(#60),#93);",
            g(94)
        ),
        "#95=IFCMATERIAL('Brick',$,$);".to_owned(),
        "#96=IFCMATERIALLAYER(#95,100.,$,'Outer',$,$,$);".to_owned(),
        "#97=IFCMATERIALLAYERSET((#96),$,$);".to_owned(),
        "#98=IFCMATERIALLAYERSETUSAGE(#97,.AXIS2.,.NEGATIVE.,5.,$);".to_owned(),
        format!("#99=IFCRELASSOCIATESMATERIAL('{}',$,$,$,(#60),#98);", g(99)),
        "#100=IFCMATERIAL('Unused',$,$);".to_owned(),
    ];
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('a.ifc','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
        lines.join("\n")
    )
}

fn lift_any(src: &[u8]) -> Option<Building> {
    let (file, _) = with(|cx| Ifc::read(src, cx));
    let file = file.ok()?;
    with(|cx| file.lift(cx)).0.ok()
}

#[test]
fn counts_the_elements_and_values_it_cannot_write() {
    let source = lift_any(rich().as_bytes()).unwrap();
    for to in VERSIONS {
        let (converted, codes) = run(&source, &options(to));
        let converted = converted.unwrap();
        let detail = |kind: &str| {
            converted
                .report
                .details
                .iter()
                .find(|d| d.kind == kind)
                .map(|d| (d.fidelity, d.reasons.clone()))
        };
        assert_eq!(
            detail("IfcSpace"),
            Some((ConversionFidelity::Dropped, Vec::new())),
            "{to:?}"
        );
        assert_eq!(
            detail("IfcFurniture"),
            Some((ConversionFidelity::Dropped, Vec::new())),
            "{to:?}"
        );
        let (fidelity, reasons) = detail("IfcWall").unwrap();
        assert_eq!(fidelity, ConversionFidelity::Approximated);
        assert!(
            reasons.iter().any(|r| r == "identity"),
            "{to:?}: {reasons:?}"
        );
        // 型と、単一の値のプロパティは書く
        for reason in ["type", "properties"] {
            assert!(!reasons.iter().any(|r| r == reason), "{to:?}: {reasons:?}");
        }
        // 個数 2.5 は、整数の IfcCountMeasure の IFC4X3 に書けない
        assert_eq!(
            reasons.iter().any(|r| r == "quantities"),
            to == Version::Ifc4x3,
            "{to:?}: {reasons:?}"
        );
        assert_eq!(
            detail("IfcWallType"),
            Some((ConversionFidelity::Native, Vec::new())),
            "{to:?}"
        );
        // 層の使い方の向きの正負は読んだ値に残らないので、材料は近似になる
        assert!(
            reasons.iter().any(|r| r == "material"),
            "{to:?}: {reasons:?}"
        );
        // 使う材料は書き、どの構成も使わない材料は落とす
        let mut materials: Vec<_> = converted
            .report
            .details
            .iter()
            .filter(|d| d.kind == "IfcMaterial")
            .map(|d| d.fidelity)
            .collect();
        materials.sort_by_key(|f| format!("{f:?}"));
        assert_eq!(
            materials,
            [ConversionFidelity::Dropped, ConversionFidelity::Native],
            "{to:?}"
        );
        // IFC2X3 の壁は定義された種類を持たない
        assert_eq!(
            reasons.iter().any(|r| r == "predefined"),
            to == Version::Ifc2x3,
            "{to:?}: {reasons:?}"
        );
        assert_eq!(
            detail("IfcProject"),
            // 列挙のプロパティは書かない
            Some((
                ConversionFidelity::Approximated,
                vec!["properties".to_owned(), "units".to_owned()]
            )),
            "{to:?}"
        );
        // 落とした要素ごとに 1 件、近似は落とした値ごとに 1 件
        let dropped = converted
            .report
            .details
            .iter()
            .filter(|d| d.fidelity == ConversionFidelity::Dropped)
            .count();
        let values: usize = converted
            .report
            .details
            .iter()
            .filter(|d| d.fidelity == ConversionFidelity::Approximated)
            .map(|d| d.reasons.len())
            .sum();
        let count = |code| codes.iter().filter(|&&c| c == code).count();
        assert_eq!(count(codes::CONVERT_ELEMENT_UNSUPPORTED), dropped, "{to:?}");
        assert_eq!(count(codes::CONVERT_ITEM_APPROXIMATED), values, "{to:?}");

        let target = lift(&converted.bytes).unwrap();
        let wall = target.find(Id::Global(global(60))).unwrap();
        let layers = target.components.material.get(wall).unwrap();
        assert_eq!(
            (
                layers.kind,
                layers.direction.as_deref(),
                layers.offset.as_ref().map(|o| o.value),
                layers
                    .parts
                    .iter()
                    .map(|p| (p.thickness.as_ref().map(|t| t.value), p.name.as_deref()))
                    .collect::<Vec<_>>()
            ),
            (
                MaterialKind::Layers,
                Some("AXIS2"),
                Some(5.0),
                vec![(Some(100.0), (to != Version::Ifc2x3).then_some("Outer"))]
            ),
            "{to:?}"
        );
        let storey = target.find(Id::Global(global(40))).unwrap();
        let parent = target
            .components
            .parent
            .get(wall)
            .unwrap()
            .of
            .clone()
            .unwrap();
        assert_eq!(
            (parent.kind, parent.target),
            (ParentKind::Contained, storey)
        );
        let sets = &target.components.properties.get(wall).unwrap().sets;
        let common: Vec<_> = sets
            .iter()
            .filter(|set| set.name == "Pset_WallCommon")
            .flat_map(|set| &set.properties)
            .map(|p| (p.name.as_str(), p.value.clone()))
            .collect();
        assert_eq!(
            common,
            [(
                "IsExternal",
                Some(TypedValue {
                    type_name: Some("IfcBoolean".to_owned()),
                    value: Literal::Boolean(true),
                })
            )],
            "{to:?}"
        );
        // 型は PredefinedType と HasPropertySets のプロパティを保つ
        let wall_type = target.find(Id::Global(global(64))).unwrap();
        assert_eq!(
            target.components.r#type.get(wall).map(|t| t.target),
            Some(wall_type)
        );
        assert_eq!(
            target
                .components
                .class
                .get(wall_type)
                .and_then(|c| c.predefined.as_deref()),
            Some("SHEAR"),
            "{to:?}"
        );
        let reference: Vec<_> = target
            .components
            .properties
            .get(wall_type)
            .unwrap()
            .sets
            .iter()
            .flat_map(|set| &set.properties)
            .map(|p| (p.name.as_str(), p.value.clone()))
            .collect();
        assert_eq!(
            reference,
            [(
                "Reference",
                Some(TypedValue {
                    type_name: Some("IfcIdentifier".to_owned()),
                    value: Literal::Text("W-1".to_owned()),
                })
            )],
            "{to:?}"
        );
        // 面積と時間の単位を足し、数量を書く。度の単位は書かない
        let kinds: Vec<_> = target
            .document
            .units
            .iter()
            .map(|u| (u.kind.as_str(), u.name.as_str()))
            .collect();
        for unit in [("AREAUNIT", "SQUARE_METRE"), ("TIMEUNIT", "SECOND")] {
            assert!(kinds.contains(&unit), "{to:?}: {kinds:?}");
        }
        let names: Vec<_> = quantities(&target, 60)
            .into_iter()
            .flat_map(|set| set.2.into_iter().map(|q| q.0))
            .collect();
        let expected: &[&str] = if to == Version::Ifc4x3 {
            &["NetSideArea", "Duration"]
        } else {
            &["NetSideArea", "Duration", "Count"]
        };
        assert_eq!(names, expected, "{to:?}");
        let project = target.document.project.unwrap();
        let names: Vec<_> = target
            .components
            .properties
            .get(project)
            .unwrap()
            .sets
            .iter()
            .flat_map(|set| {
                set.properties
                    .iter()
                    .map(|p| (set.name.as_str(), p.name.as_str()))
            })
            .collect();
        assert_eq!(names, [("Zumen_Project", "Phase")], "{to:?}");
        assert!(target.find(Id::Global(global(50))).is_none());
        assert!(target.find(Id::Global(global(70))).is_none());
    }
}

#[test]
fn needs_a_length_unit_it_can_write() {
    let src = rich().replace(".MILLI.,.METRE.", "$,.FOOT.");
    let source = lift_any(src.as_bytes()).unwrap();
    let (converted, codes) = run(&source, &options(Version::Ifc4));
    assert!(converted.is_none());
    assert_eq!(codes, [codes::CONVERT_ELEMENT_UNSUPPORTED]);
}

#[allow(clippy::disallowed_methods)]
#[test]
fn writes_the_conversions_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_AUTHORED") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let source = lift(&source(Version::Ifc4).unwrap()).unwrap();
    for to in VERSIONS {
        let (converted, _) = run(&source, &options(to));
        std::fs::write(
            folder.join(format!("{}.converted.ifc", to.file_schema())),
            converted.unwrap().bytes,
        )
        .unwrap();
    }
}
