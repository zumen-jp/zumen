//! 建物モデルから新しい IFC を書く変換（#395）。ファイルは、どれもテストの中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_building::{
    Building, ElementRef, GlobalId, Id, Literal, Local, ParentKind, Transform, TypedValue,
};
use zumen_ifc::{
    ConvertOptions, Converted, Edit, Header, Ifc, LengthUnit, NewElement, NewPlacement, NewSpace,
    NewType, OwnerHistory, SpaceKind, Target, Template, Version, apply, convert, create,
};
use zumen_sdk::conversion::ConversionFidelity;
use zumen_sdk::{Codec, Lift};

const VERSIONS: [Version; 3] = [Version::Ifc2x3, Version::Ifc4, Version::Ifc4x3];

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

/// 読んで建物モデルにする。人・組織・次元の指数を落とした診断のほかは出ないこと
fn lift(src: &[u8]) -> Option<Building> {
    let (file, diag) = with(|cx| Ifc::read(src, cx));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    let file = file.ok()?;
    let (building, diag) = with(|cx| file.lift(cx));
    let others: Vec<_> = diag
        .iter()
        .filter(|d| d.code() != codes::IFC_ENTITY_DROPPED)
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
    let (applied, diag) = with(|cx| apply(&file, &edits, b"seed", cx));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    applied.ok().map(|a| a.bytes)
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
            let class = c.class.get(e)?;
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

            let approximated: Vec<_> = converted
                .report
                .details
                .iter()
                .filter(|d| d.fidelity != ConversionFidelity::Native)
                .map(|d| (d.kind.as_str(), d.fidelity, d.reasons.clone()))
                .collect();
            if lost_column {
                assert_eq!(
                    approximated,
                    [
                        ("IfcDoorType", ConversionFidelity::Dropped, Vec::new()),
                        (
                            "IfcDoor",
                            ConversionFidelity::Approximated,
                            vec!["type".to_owned()]
                        ),
                        (
                            "IfcColumn",
                            ConversionFidelity::Approximated,
                            vec!["predefined".to_owned()]
                        ),
                    ],
                    "{from:?} → {to:?}"
                );
                let count = |code| codes.iter().filter(|&&c| c == code).count();
                assert_eq!(
                    (
                        count(codes::CONVERT_ELEMENT_UNSUPPORTED),
                        count(codes::CONVERT_ITEM_APPROXIMATED),
                        codes.len()
                    ),
                    (1, 2, 3),
                    "{codes:?}"
                );
            } else {
                assert_eq!(approximated, [], "{from:?} → {to:?}");
                assert!(codes.is_empty(), "{from:?} → {to:?}: {codes:?}");
            }
            // プロジェクト・空間 4 つ・要素 5 つ・型（IFC2X3 からは壁の型だけ）
            let types = if from == Version::Ifc2x3 { 1 } else { 2 };
            assert_eq!(converted.report.total, 10 + types, "{from:?} → {to:?}");
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
        "#3=IFCUNITASSIGNMENT((#4,#5));".to_owned(),
        "#4=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);".to_owned(),
        "#5=IFCSIUNIT(*,.AREAUNIT.,$,.SQUARE_METRE.);".to_owned(),
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
        assert_eq!(
            detail("IfcWallType"),
            Some((ConversionFidelity::Native, Vec::new())),
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
