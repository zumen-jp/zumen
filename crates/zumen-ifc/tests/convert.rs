//! 建物モデルから新しい IFC を書く変換（#395）。合成の 3 版と公開サンプル（`samples/ifc`・`samples/stbridge`）で確かめる。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_building::{
    Building, ElementRef, FaceModelKind, GlobalId, Id, Literal, Local, MaterialKind, ParentKind,
    QuantityKind, Transform, TypedValue,
};
use zumen_ifc::{
    ConvertOptions, Converted, Edit, Header, Ifc, LengthUnit, NewConstituent, NewElement, NewFaces,
    NewItem, NewMaterial, NewMaterialLayer, NewPlacement, NewQuantity, NewSpace, NewType, NewUnit,
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
    // 形。壁は芯の線と、2 つの半空間で切り取った矩形、床は穴のある多角形、柱は円の押し出しの写像
    let [wall, slab, column] = shape::examples();
    edits.extend(
        [
            (4, shape::clipped(wall)),
            (7, slab),
            (8, shape::mapped(column)),
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

/// 敷地・建物・階と、回転と掃引の形を持つ壁・床・柱（30〜32）の文書。`degree` なら平面角の単位を度にする
fn sweep_source(version: Version, degree: bool) -> Option<Vec<u8>> {
    let template = Template {
        version,
        unit: LengthUnit::Millimeter,
        project: global(1),
        name: None,
        header: Header::default(),
        owner: Some(owner()),
    };
    let (bytes, _) = with(|cx| create(&template, cx));
    let mut text = String::from_utf8(bytes.ok()?).ok()?;
    if degree {
        let radian = "IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)";
        assert!(text.contains(radian));
        text = text
            .replace(
                radian,
                "IFCCONVERSIONBASEDUNIT(#9001,.PLANEANGLEUNIT.,'degree',#9002)",
            )
            .replace(
                "ENDSEC;
END-ISO-10303-21;",
                "#9001=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);
                 #9002=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.0174532925199433),#9003);
                 #9003=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);
ENDSEC;
END-ISO-10303-21;",
            );
    }
    let (file, _) = with(|cx| Ifc::read(text.as_bytes(), cx));
    let file = file.ok()?;
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
    ];
    for (n, class) in [(30, "IfcWall"), (31, "IfcSlab"), (32, "IfcColumn")] {
        edits.push(element(
            class,
            n,
            None,
            Some(Target::Added(2)),
            at(0.0, 0.0, 0.0, Target::Added(2)),
        ));
    }
    for (n, shape) in (3..).zip(shape::sweep_examples()) {
        edits.push(Edit::Shape {
            element: Target::Added(n),
            shape,
        });
    }
    let (applied, diag) = with(|cx| apply(&file, &edits, b"seed", cx));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    applied.ok().map(|a| a.bytes)
}

/// 回転の角度を近い値で比べられるよう、ラジアンを 1e-12 で丸めた形
fn rounded(mut shape: zumen_ifc::NewShape) -> zumen_ifc::NewShape {
    for item in &mut shape.body {
        if let NewItem::Revolution(r) = item {
            r.angle = (r.angle * 1e12).round() / 1e12;
        }
    }
    shape
}

#[test]
fn converts_revolutions_and_sweeps() {
    let sources = VERSIONS
        .map(|from| (from, false))
        .into_iter()
        .chain([(Version::Ifc4, true)]);
    for (from, degree) in sources {
        let source = lift(&sweep_source(from, degree).unwrap()).unwrap();
        for to in VERSIONS {
            let (converted, _) = run(&source, &options(to));
            let target = lift(&converted.unwrap().bytes).unwrap();
            for n in [30, 31, 32] {
                let of = |building: &Building| {
                    building
                        .find(Id::Global(global(n)))
                        .and_then(|e| shape::shape_of(building, e))
                };
                // 角度はラジアンにし、IFC2X3 へは掃引の始点と終点を埋める
                let wanted = of(&source).map(|s| {
                    let s = shape::as_written(&s);
                    rounded(if to == Version::Ifc2x3 {
                        shape::with_parameters(s)
                    } else {
                        s
                    })
                });
                assert!(wanted.is_some(), "{from:?} {n}");
                assert_eq!(
                    of(&target).map(rounded),
                    wanted,
                    "{from:?} {degree} → {to:?} {n}"
                );
            }
        }
    }
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
            // 形は、書き手が省かずに書く配置のほかは保つ。IFC2X3 と IFC4 からは囲みの箱の座標系が違うので、
            // 箱を落とす
            let boxes = (from == Version::Ifc2x3) == (to == Version::Ifc2x3);
            for n in [20, 22, 23, 24] {
                let of = |building: &Building| {
                    building
                        .find(Id::Global(global(n)))
                        .and_then(|e| shape::shape_of(building, e))
                };
                let wanted = of(&source)
                    .map(|s| shape::as_written(&s))
                    .map(|s| if boxes { s } else { shape::unboxed(s) });
                assert!(wanted.is_some(), "{from:?} {n}");
                let found = of(&target);
                // IFC2X3 へは三角形の箱を、点を共有する多角形の閉殻にする
                if n == 22 && to == Version::Ifc2x3 && from != Version::Ifc2x3 {
                    let body = found.map(|s| s.body);
                    let Some([NewItem::Faces(NewFaces::Polygons(graph))]) = body.as_deref() else {
                        panic!("{from:?} → {to:?}: {body:?}");
                    };
                    assert_eq!(graph.kind, FaceModelKind::FacetedSolid);
                    assert_eq!((graph.faces.len(), graph.points.len()), (12, 8));
                    continue;
                }
                assert_eq!(found, wanted, "{from:?} → {to:?} {n}");
                // 書き手が書いた表現をそのまま移せる組では、所有する形のハッシュ（fingerprint）も一致する
                if boxes {
                    let src_fp = source
                        .find(Id::Global(global(n)))
                        .and_then(|e| source.geometry(e))
                        .map(|g| g.fingerprint());
                    let dst_fp = target
                        .find(Id::Global(global(n)))
                        .and_then(|e| target.geometry(e))
                        .map(|g| g.fingerprint());
                    assert!(src_fp.is_some(), "{from:?} {n}");
                    assert_eq!(dst_fp, src_fp, "{from:?} → {to:?} {n} fingerprint");
                }
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
                        vec![
                            "material".to_owned(),
                            "quantities".to_owned(),
                            "representation".to_owned(),
                        ],
                    ),
                    (
                        "IfcWallType",
                        ConversionFidelity::Approximated,
                        vec!["material".to_owned()],
                    ),
                ]);
            }
            // IFC4 からは材料の並びが非推奨なので、構成要素にする。壁を切る囲みの箱は落とす
            if from == Version::Ifc2x3 && to != Version::Ifc2x3 {
                wanted.extend([
                    (
                        "IfcDoor",
                        ConversionFidelity::Approximated,
                        vec!["material".to_owned()],
                    ),
                    (
                        "IfcWall",
                        ConversionFidelity::Approximated,
                        vec!["representation".to_owned()],
                    ),
                ]);
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

#[test]
fn checks_predefined_types_against_the_target_version_enum() {
    let template = Template {
        version: Version::Ifc4x3,
        unit: LengthUnit::Millimeter,
        project: global(1),
        name: Some("Predefined".to_owned()),
        header: Header::default(),
        owner: Some(owner()),
    };
    let base = with(|cx| create(&template, cx)).0.unwrap();
    let file = with(|cx| Ifc::read(&base, cx)).0.unwrap();

    // 書き手（apply）も、バージョンの列挙にない値や ElementType のない USERDEFINED を断る
    for (ver, edit) in [
        (
            Version::Ifc2x3,
            element_type("IfcWallType", 90, Some("SOLIDWALL")),
        ),
        (
            Version::Ifc4,
            element_type("IfcWallType", 91, Some("RETAININGWALL")),
        ),
        (
            Version::Ifc4,
            element_type("IfcWallType", 92, Some("USERDEFINED")),
        ),
        (
            Version::Ifc2x3,
            element("IfcRoof", 93, Some("USERDEFINED"), None, None),
        ),
        (
            Version::Ifc4,
            element("IfcWall", 94, Some("USERDEFINED"), None, None),
        ),
    ] {
        let tpl = Template {
            version: ver,
            ..template.clone()
        };
        let tpl_bytes = with(|cx| create(&tpl, cx)).0.unwrap();
        let tpl_file = with(|cx| Ifc::read(&tpl_bytes, cx)).0.unwrap();
        let (res, diag) = with(|cx| apply(&tpl_file, std::slice::from_ref(&edit), b"seed", cx));
        assert!(res.is_err(), "{ver:?}: {edit:?}");
        assert_eq!(
            diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
            [codes::CHANGE_EDIT_UNSUPPORTED],
            "{ver:?}: {edit:?}"
        );
    }

    // IFC4X3 の列挙にある SOLIDWALL（IFC4 以降）・RETAININGWALL（IFC4X3 のみ）・USERDEFINED（型）を作る
    let edits = [
        space(
            SpaceKind::Building,
            9,
            Target::Instance(1),
            Some(NewPlacement {
                relative_to: None,
                local: Transform::IDENTITY,
            }),
            None,
        ),
        space(
            SpaceKind::Storey,
            10,
            Target::Added(0),
            at(0.0, 0.0, 0.0, Target::Added(0)),
            Some(0.0),
        ),
        element_type("IfcWallType", 20, Some("SOLIDWALL")),
        element_type("IfcWallType", 21, Some("RETAININGWALL")),
        Edit::ElementType(NewType {
            class: "IfcRoofType".to_owned(),
            predefined: Some("USERDEFINED".to_owned()),
            element_type: Some("CustomRoof".to_owned()),
            name: Some("RT".to_owned()),
            global_id: Some(global(22)),
        }),
        element(
            "IfcWall",
            30,
            Some("RETAININGWALL"),
            Some(Target::Added(1)),
            at(0.0, 0.0, 0.0, Target::Added(1)),
        ),
    ];
    let src_bytes = with(|cx| apply(&file, &edits, b"seed", cx))
        .0
        .unwrap()
        .bytes;
    let source = lift(&src_bytes).unwrap();

    for to in VERSIONS {
        let (converted, diag) = with(|cx| convert(&source, b"document", &options(to), cx));
        let converted = converted.unwrap();
        let count = |code| {
            diag.iter()
                .filter(|d| d.code() == code)
                .map(|d| d.count())
                .sum::<u64>()
        };
        let target = lift(&converted.bytes).unwrap();
        let predef = |n: u32| {
            target
                .find(Id::Global(global(n)))
                .and_then(|e| target.components.class.get(e))
                .and_then(|c| c.predefined.clone())
        };
        match to {
            Version::Ifc2x3 => {
                assert_eq!(predef(20).as_deref(), Some("NOTDEFINED"));
                assert_eq!(predef(21).as_deref(), Some("NOTDEFINED"));
                assert_eq!(predef(22), None);
                assert_eq!(predef(30), None);
                assert_eq!(count(codes::CONVERT_ITEM_APPROXIMATED), 3);
                assert_eq!(count(codes::CONVERT_ELEMENT_UNSUPPORTED), 1);
            }
            Version::Ifc4 => {
                assert_eq!(predef(20).as_deref(), Some("SOLIDWALL"));
                assert_eq!(predef(21).as_deref(), Some("NOTDEFINED"));
                assert_eq!(predef(22).as_deref(), Some("USERDEFINED"));
                assert_eq!(predef(30), None);
                assert_eq!(count(codes::CONVERT_ITEM_APPROXIMATED), 2);
                assert_eq!(count(codes::CONVERT_ELEMENT_UNSUPPORTED), 0);
            }
            Version::Ifc4x3 => {
                assert_eq!(predef(20).as_deref(), Some("SOLIDWALL"));
                assert_eq!(predef(21).as_deref(), Some("RETAININGWALL"));
                assert_eq!(predef(22).as_deref(), Some("USERDEFINED"));
                assert_eq!(predef(30).as_deref(), Some("RETAININGWALL"));
                assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
            }
        }
    }
}

fn root_globals(bytes: &[u8]) -> Option<std::collections::BTreeSet<(String, String)>> {
    let text = core::str::from_utf8(bytes).ok()?;
    let mut set = std::collections::BTreeSet::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix('#').and_then(|s| s.split_once('=')) else {
            continue;
        };
        let Some((entity, args)) = rest.1.split_once("('") else {
            continue;
        };
        let Some((gid, _)) = args.split_once('\'') else {
            continue;
        };
        if gid.len() == 22 {
            set.insert((entity.to_owned(), gid.to_owned()));
        }
    }
    Some(set)
}

#[test]
fn keeps_unchanged_global_ids_when_reconverting_an_edited_model() {
    // 1. IfcProject の GlobalId を持つ IFC4 を一部編集して再変換しても、未変更の要素・関係・セットの GlobalId は不変
    let base_bytes = source(Version::Ifc4).unwrap();
    let base_building = lift(&base_bytes).unwrap();
    let wall_inst = instance_of(&base_bytes, global(20)).unwrap();
    let col_inst = instance_of(&base_bytes, global(24)).unwrap();
    let storey_inst = instance_of(&base_bytes, global(12)).unwrap();
    let file = with(|cx| Ifc::read(&base_bytes, cx)).0.unwrap();
    let edits = [
        Edit::Attribute {
            element: wall_inst,
            attribute: zumen_ifc::RootAttribute::Name,
            value: Some("W-Edited".to_owned()),
        },
        Edit::Property {
            element: wall_inst,
            set: "Pset_WallCommon".to_owned(),
            name: "Reference".to_owned(),
            value: zumen_ifc::NewValue {
                type_name: "IfcIdentifier".to_owned(),
                literal: zumen_ifc::NewLiteral::Text("W-2".to_owned()),
            },
        },
        element(
            "IfcBeam",
            99,
            Some("BEAM"),
            Some(Target::Instance(storey_inst)),
            at(0.0, 1000.0, 2800.0, Target::Instance(storey_inst)),
        ),
        Edit::Remove {
            element: Target::Instance(col_inst),
        },
    ];
    let edited_bytes = with(|cx| apply(&file, &edits, b"edit-seed", cx))
        .0
        .unwrap()
        .bytes;
    let edited_building = lift(&edited_bytes).unwrap();

    for to in VERSIONS {
        let before = run(&base_building, &options(to)).0.unwrap();
        let after = run(&edited_building, &options(to)).0.unwrap();
        let before_roots = root_globals(&before.bytes).unwrap();
        let after_roots = root_globals(&after.bytes).unwrap();
        for n in [1, 10, 11, 12, 13, 20, 21, 22, 23, 25] {
            let gid = global(n).to_string();
            assert!(
                before_roots.iter().any(|(_, g)| g == &gid),
                "{to:?} before missing {n}"
            );
            assert!(
                after_roots.iter().any(|(_, g)| g == &gid),
                "{to:?} after missing {n}"
            );
        }
        // 未変更の集約・開口・充填・型の定義・数量のセットの GlobalId もすべて保たれる
        for kind in [
            "IFCRELAGGREGATES",
            "IFCRELVOIDSELEMENT",
            "IFCRELFILLSELEMENT",
            "IFCRELDEFINESBYTYPE",
            "IFCELEMENTQUANTITY",
        ] {
            for k in before_roots.iter().filter(|(entity, _)| entity == kind) {
                // 除いた柱（24）は型も開口も数量も持たないため、元のこれらの関係の GlobalId はすべて残る
                assert!(
                    after_roots.contains(k),
                    "{to:?}: relation/set GlobalId changed or vanished: {k:?}"
                );
            }
        }
    }

    // 2. GlobalId を持たない入力（Id::Instance になる文書）を一部編集し、同じ名前空間で再変換しても未変更要素の導出 GlobalId は不変
    let mut inst_text = String::from_utf8(source(Version::Ifc4).unwrap()).unwrap();
    for n in [1, 10, 11, 12, 13, 20, 21, 22, 23, 24, 25, 26] {
        inst_text = inst_text.replace(&format!("'{}'", global(n)), &format!("'id-{n}'"));
    }
    let inst_base = lift_any(inst_text.as_bytes()).unwrap();
    assert!(matches!(
        inst_base.id(inst_base.document.project.unwrap()),
        Some(Id::Instance(_))
    ));
    let inst_edited_text = inst_text
        .replace("'E20'", "'E20-Edited'")
        .lines()
        .filter(|line| !line.contains("'id-24'") && !line.contains("'E24'"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let inst_edited = lift_any(inst_edited_text.as_bytes()).unwrap();
    let mut opts = options(Version::Ifc4);
    opts.namespace = Some(b"instance-ns".to_vec());
    for to in VERSIONS {
        opts.version = to;
        let before = run(&inst_base, &opts).0.unwrap();
        let after = run(&inst_edited, &opts).0.unwrap();
        let before_lifted = lift_any(&before.bytes).unwrap();
        let after_lifted = lift_any(&after.bytes).unwrap();
        let collect_ids =
            |b: &Building| -> std::collections::BTreeMap<String, (String, Option<String>)> {
                (0..b.elements().len())
                    .filter_map(|i| {
                        let e = ElementRef(u32::try_from(i).ok()?);
                        let gid = match b.id(e)? {
                            Id::Global(g) => g.to_string(),
                            Id::Instance(_) => return None,
                        };
                        let cls = b.components.class.get(e)?.name.clone();
                        let name = b.components.identity.get(e).and_then(|id| id.name.clone());
                        Some((gid, (cls, name)))
                    })
                    .collect()
            };
        let mut expected = collect_ids(&before_lifted);
        expected.retain(|_, (_, name)| name.as_deref() != Some("E24"));
        for (_, name) in expected.values_mut() {
            if name.as_deref() == Some("E20") {
                *name = Some("E20-Edited".to_owned());
            }
        }
        let found = collect_ids(&after_lifted);
        assert_eq!(found, expected, "{to:?}: derived GlobalIds changed on edit");
    }
}

fn world_matrix(building: &Building, start: ElementRef) -> Option<[[f64; 4]; 3]> {
    let mut current = Some(start);
    let mut acc = Transform::IDENTITY;
    let mut chain = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    while let Some(e) = current {
        if !seen.insert(e) {
            return None;
        }
        let Some(p) = building.components.placement.get(e) else {
            break;
        };
        let Local::Transform(local) = p.local else {
            return None;
        };
        chain.push(local);
        current = p.relative_to;
    }
    for local in chain.into_iter().rev() {
        acc = acc.then(&local);
    }
    Some(acc.0)
}

fn public_ifc_paths() -> Option<Vec<(String, std::path::PathBuf)>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/ifc");
    let mut stack = vec![root.clone()];
    let mut found = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).ok()? {
            let path = entry.ok()?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("ifc"))
            {
                let rel = path
                    .strip_prefix(&root)
                    .ok()?
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                found.push((rel, path));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Some(found)
}

#[allow(clippy::disallowed_methods)]
#[test]
fn round_trips_public_ifc_samples_across_all_versions() {
    let out_dir = std::env::var_os("ZUMEN_IFC_CONVERTED_SAMPLES").map(std::path::PathBuf::from);
    if let Some(dir) = &out_dir {
        std::fs::create_dir_all(dir).unwrap();
    }
    let samples = public_ifc_paths().unwrap();
    assert_eq!(samples.len(), 12);
    const GEOMETRY_SAMPLES: &[&str] = &[
        "bim-whale/TallBuilding.ifc",
        "gni/model_9.ifc",
        "text2bim/sample_model.ifc",
    ];
    for (name, path) in samples {
        let with_geometry = out_dir.is_some() || GEOMETRY_SAMPLES.contains(&name.as_str());
        let raw = std::fs::read(&path).unwrap();
        let file = with(|cx| Ifc::read(&raw, cx)).0.unwrap();
        let source: Building = if with_geometry {
            with(|cx| file.lift_geometry(cx)).0.unwrap()
        } else {
            with(|cx| file.lift(cx)).0.unwrap()
        };
        let mut opts = options(Version::Ifc4);
        opts.limit = 100_000;
        for to in VERSIONS {
            opts.version = to;
            let (res, diag) = with(|cx| convert(&source, &raw, &opts, cx));
            let converted = res.unwrap_or_else(|e| panic!("{name} → {to:?}: {e:?} {diag:?}"));
            assert!(!converted.report.truncated, "{name} → {to:?}");
            if let Some(dir) = &out_dir {
                let flat = name.replace('/', "__");
                std::fs::write(
                    dir.join(format!("{}.{}.ifc", flat, to.file_schema())),
                    &converted.bytes,
                )
                .unwrap();
            }

            // 忠実度の集計・明細と診断の種類・件数・出所の一致
            let dropped_details: Vec<_> = converted
                .report
                .details
                .iter()
                .filter(|d| d.fidelity == ConversionFidelity::Dropped)
                .collect();
            let approx_reasons: usize = converted
                .report
                .details
                .iter()
                .filter(|d| d.fidelity == ConversionFidelity::Approximated)
                .map(|d| d.reasons.len())
                .sum();
            let unsupported_diags: Vec<_> = diag
                .iter()
                .filter(|d| d.code() == codes::CONVERT_ELEMENT_UNSUPPORTED)
                .collect();
            let approx_diags: Vec<_> = diag
                .iter()
                .filter(|d| d.code() == codes::CONVERT_ITEM_APPROXIMATED)
                .collect();
            let unsupported_count: u64 = unsupported_diags.iter().map(|d| d.count()).sum();
            let approx_count: u64 = approx_diags.iter().map(|d| d.count()).sum();
            assert_eq!(
                unsupported_count,
                dropped_details.len() as u64,
                "{name} → {to:?}"
            );
            assert_eq!(approx_count, approx_reasons as u64, "{name} → {to:?}");
            assert_eq!(
                diag.len(),
                unsupported_diags.len() + approx_diags.len(),
                "{name} → {to:?}"
            );
            for d in diag.iter() {
                assert!(
                    !d.places().is_empty(),
                    "{name} → {to:?}: diagnostic missing source place: {d:?}"
                );
            }

            // 書いた IFC を読み直し、ID・クラス・空間の親子・配置・型・プロパティ・数量・形を照らす
            let out_file = with(|cx| Ifc::read(&converted.bytes, cx)).0.unwrap();
            let target: Building = if with_geometry {
                with(|cx| out_file.lift_geometry(cx)).0.unwrap()
            } else {
                with(|cx| out_file.lift(cx)).0.unwrap()
            };
            let reasons_by_place: std::collections::BTreeMap<_, _> = converted
                .report
                .details
                .iter()
                .map(|d| (format!("{:?}", d.source), &d.reasons))
                .collect();

            for idx in 0..target.elements().len() {
                let dst = ElementRef(u32::try_from(idx).unwrap());
                let Some(dst_class) = target.components.class.get(dst) else {
                    continue;
                };
                if dst_class.name == "IfcMaterial" {
                    continue;
                }
                let id = target.id(dst).unwrap();
                let src = source
                    .find(id)
                    .unwrap_or_else(|| panic!("{name} → {to:?}: unknown id {id}"));
                let src_class = source.components.class.get(src).unwrap();
                assert_eq!(dst_class.name, src_class.name, "{name} → {to:?} {id}");

                let empty_reasons = Vec::new();
                let reasons = reasons_by_place
                    .get(&format!("{:?}", source.place(src)))
                    .copied()
                    .unwrap_or(&empty_reasons);
                let has_reason = |r: &str| reasons.iter().any(|x| x == r);

                if !has_reason("parent")
                    && Some(src) != source.document.project
                    && let Some(src_parent) = source
                        .components
                        .parent
                        .get(src)
                        .and_then(|p| p.of.as_ref())
                {
                    let dst_parent = target
                        .components
                        .parent
                        .get(dst)
                        .and_then(|p| p.of.as_ref())
                        .unwrap_or_else(|| panic!("{name} → {to:?} {id}: missing parent"));
                    assert_eq!(dst_parent.kind, src_parent.kind, "{name} → {to:?} {id}");
                    assert_eq!(
                        target.id(dst_parent.target),
                        source.id(src_parent.target),
                        "{name} → {to:?} {id}"
                    );
                }

                if !has_reason("placement")
                    && Some(src) != source.document.project
                    && source.components.placement.get(src).is_some()
                    && let Some(src_m) = world_matrix(&source, src)
                {
                    let dst_m = world_matrix(&target, dst)
                        .unwrap_or_else(|| panic!("{name} → {to:?} {id}: missing placement"));
                    for (r1, r2) in src_m.iter().zip(&dst_m) {
                        for (a, b) in r1.iter().zip(r2) {
                            assert!(
                                (a - b).abs() <= 1e-6,
                                "{name} → {to:?} {id}: placement {src_m:?} != {dst_m:?}"
                            );
                        }
                    }
                }

                if !has_reason("type")
                    && let Some(src_type) = source.components.r#type.get(src)
                {
                    let dst_type = target
                        .components
                        .r#type
                        .get(dst)
                        .unwrap_or_else(|| panic!("{name} → {to:?} {id}: missing type"));
                    assert_eq!(
                        target.id(dst_type.target),
                        source.id(src_type.target),
                        "{name} → {to:?} {id}"
                    );
                }

                if !has_reason("properties")
                    && let Some(src_props) = source.components.properties.get(src)
                {
                    let norm = |tv: Option<TypedValue>| -> Option<TypedValue> {
                        let mut tv = tv?;
                        if to == Version::Ifc4x3
                            && tv
                                .type_name
                                .as_deref()
                                .is_some_and(|t| t.eq_ignore_ascii_case("IfcCountMeasure"))
                            && let Literal::Real(r) = tv.value
                            && r.is_finite()
                            && r.fract() == 0.0
                        {
                            #[allow(clippy::cast_possible_truncation)]
                            {
                                tv.value = Literal::Integer(r as i64);
                            }
                        }
                        Some(tv)
                    };
                    let flatten = |props: &zumen_building::Properties| {
                        let mut v: Vec<_> = props
                            .sets
                            .iter()
                            .flat_map(|s| {
                                s.properties.iter().map(|p| {
                                    (s.name.clone(), p.name.clone(), norm(p.value.clone()))
                                })
                            })
                            .collect();
                        v.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
                        v
                    };
                    let dst_props = target
                        .components
                        .properties
                        .get(dst)
                        .unwrap_or_else(|| panic!("{name} → {to:?} {id}: missing properties"));
                    assert_eq!(
                        flatten(dst_props),
                        flatten(src_props),
                        "{name} → {to:?} {id}"
                    );
                }

                if !has_reason("quantities")
                    && let Some(src_qtys) = source.components.quantities.get(src)
                {
                    let flatten = |qtys: &zumen_building::Quantities| {
                        let mut v: Vec<_> = qtys
                            .sets
                            .iter()
                            .flat_map(|s| {
                                s.quantities.iter().map(move |q| {
                                    (
                                        s.name.clone(),
                                        q.name.clone(),
                                        q.kind,
                                        q.value.value,
                                        q.formula.clone(),
                                    )
                                })
                            })
                            .collect();
                        v.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
                        v
                    };
                    let dst_qtys = target
                        .components
                        .quantities
                        .get(dst)
                        .unwrap_or_else(|| panic!("{name} → {to:?} {id}: missing quantities"));
                    assert_eq!(flatten(dst_qtys), flatten(src_qtys), "{name} → {to:?} {id}");
                }

                if !has_reason("representation")
                    && source
                        .geometry(src)
                        .is_some_and(|g| g.data().root.is_some())
                {
                    assert_eq!(
                        shape::shape_of(&target, dst),
                        shape::shape_of(&source, src).map(|s| shape::as_written(&s)),
                        "{name} → {to:?} {id}"
                    );
                }
            }

            // 変換後の IFC をもう一度同じバージョンへ変換すると、書いたすべての形の所有ハッシュ（fingerprint）が完全に一致する
            if with_geometry && to == Version::Ifc4 {
                let again = with(|cx| convert(&target, &converted.bytes, &opts, cx))
                    .0
                    .unwrap();
                let again_file = with(|cx| Ifc::read(&again.bytes, cx)).0.unwrap();
                let again_target = with(|cx| again_file.lift_geometry(cx)).0.unwrap();
                for idx in 0..target.elements().len() {
                    let dst = ElementRef(u32::try_from(idx).unwrap());
                    let Some(geom1) = target.geometry(dst).filter(|g| g.data().root.is_some())
                    else {
                        continue;
                    };
                    let id = target.id(dst).unwrap();
                    let dst2 = again_target.find(id).unwrap();
                    let geom2 = again_target.geometry(dst2).unwrap_or_else(|| {
                        panic!("{name} → {to:?} {id}: missing re-converted geometry")
                    });
                    assert_eq!(
                        geom2.fingerprint(),
                        geom1.fingerprint(),
                        "{name} → {to:?} {id} fingerprint"
                    );
                }
            }
        }
    }
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
    // 公開サンプル（IFC2X3・IFC4・IFC4X3）を各版へ変換した IFC も外部照合用に書く
    let public_dir = folder.join("public");
    std::fs::create_dir_all(&public_dir).unwrap();
    let samples_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/ifc");
    for (slug, rel) in [
        ("text2bim", "text2bim/sample_model.ifc"),
        ("gni_model_9", "gni/model_9.ifc"),
        ("rinya", "rinya/wood-office-architecture.ifc"),
    ] {
        let raw = std::fs::read(samples_root.join(rel)).unwrap();
        let file = with(|cx| Ifc::read(&raw, cx)).0.unwrap();
        let building = with(|cx| file.lift_geometry(cx)).0.unwrap();
        for to in VERSIONS {
            let mut opts = options(to);
            opts.limit = 100_000;
            let converted = with(|cx| convert(&building, &raw, &opts, cx)).0.unwrap();
            std::fs::write(
                public_dir.join(format!("{slug}.{}.ifc", to.file_schema())),
                converted.bytes,
            )
            .unwrap();
        }
    }
}
