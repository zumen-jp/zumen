//! 空間・要素・関係を足し、除き、置き直す変更（#897）。ファイルは、どれもテストの中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    BooleanOperation, Building, CartesianMapping3, ElementRef, FaceBoundRef, FaceSetRef, GlobalId,
    GridSet, Id, IndexedFaces, Local, MappingScale3, MaterialKind, Opening, ParentKind,
    QuantityKind, SpatialKind, Transform,
};
use zumen_ifc::{
    Edit, FillingKind, Header, Ifc, IfcFile, IfcLive, LengthUnit, NewAxis3, NewBeam, NewColumn,
    NewConstituent, NewElement, NewFaces, NewFilling, NewGrid, NewGridAxis, NewHalfSpaceLimit,
    NewItem, NewLayer, NewLayerUsage, NewLiteral, NewMapped, NewMaterial, NewMaterialLayer,
    NewOpening, NewPlacement, NewProfileKind, NewQuantity, NewRevolution, NewSection, NewShape,
    NewSlab, NewSolid, NewSpace, NewSweptDisk, NewType, NewUnit, NewValue, NewWall, OpeningChange,
    OwnerHistory, RootAttribute, SpaceKind, Target, Template, Version, WallChange, apply, create,
};
use zumen_sdk::{Codec, Error, Lift};

const SCHEMAS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn global(n: u32) -> GlobalId {
    GlobalId::from_uuid(u128::from(n).to_be_bytes())
}

fn gid(n: u32) -> String {
    global(n).to_string()
}

/// プロジェクト・敷地・壁の型だけの文書。`history` が偽なら所有履歴を書かない
fn sample(schema: &str, history: bool) -> String {
    let owner = if history { "#2" } else { "$" };
    let mut lines = vec![
        format!(
            "#1=IFCPROJECT('{}',{owner},'Project',$,$,$,$,$,#3);",
            gid(1)
        ),
        "#3=IFCUNITASSIGNMENT((#4));".to_owned(),
        "#4=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);".to_owned(),
        format!(
            "#10=IFCSITE('{}',{owner},'Site',$,$,#11,$,$,.ELEMENT.,$,$,$,$,$);",
            gid(10)
        ),
        "#11=IFCLOCALPLACEMENT($,#12);".to_owned(),
        "#12=IFCAXIS2PLACEMENT3D(#13,$,$);".to_owned(),
        "#13=IFCCARTESIANPOINT((0.,0.,0.));".to_owned(),
        format!("#14=IFCRELAGGREGATES('{}',{owner},$,$,#1,(#10));", gid(14)),
        format!(
            "#20=IFCWALLTYPE('{}',{owner},'Wall type',$,$,$,$,$,$,.SOLIDWALL.);",
            gid(20)
        ),
    ];
    if history {
        lines.insert(1, "#2=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);".to_owned());
    }
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\nFILE_NAME('a.ifc','',(''),(''),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n{}\nENDSEC;\nEND-ISO-10303-21;\n",
        lines.join("\n")
    )
}

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn read(src: &[u8]) -> Option<IfcFile> {
    let (file, diag) = with(|cx| Ifc::read(src, cx));
    assert!(diag.is_empty(), "{:?}", diag.iter().collect::<Vec<_>>());
    file.ok()
}

fn lift(src: &[u8]) -> Option<Building> {
    let file = read(src)?;
    with(|cx| file.lift(cx)).0.ok()
}

fn run(src: &[u8], edits: &[Edit]) -> (Result<Vec<u8>, Error>, Vec<Code>) {
    let Some(file) = read(src) else {
        return (Err(Error::Invalid), Vec::new());
    };
    let (result, diag) = with(|cx| apply(&file, edits, b"seed", cx));
    (
        result.map(|applied| applied.bytes),
        diag.iter().map(|d| d.code()).collect(),
    )
}

fn element(building: &Building, n: u32) -> Option<ElementRef> {
    building.find(Id::Global(global(n)))
}

fn at(x: f64, y: f64, z: f64, relative_to: Target) -> NewPlacement {
    NewPlacement {
        relative_to: Some(relative_to),
        local: Transform::translation(x, y, z),
    }
}

fn space(kind: SpaceKind, n: u32, parent: Target, elevation: f64) -> Edit {
    Edit::Space(NewSpace {
        kind,
        name: Some(format!("S{n}")),
        parent,
        placement: None,
        // IFC4.3 は建物の ElevationOfRefHeight を非推奨にしたので、どのバージョンでも建物には書かない
        elevation: (kind != SpaceKind::Building).then_some(elevation),
        global_id: Some(global(n)),
    })
}

fn new_element(
    class: &str,
    n: u32,
    container: Option<Target>,
    placement: Option<NewPlacement>,
) -> Edit {
    Edit::Element(NewElement {
        class: class.to_owned(),
        predefined: None,
        name: Some(format!("E{n}")),
        container,
        placement,
        global_id: Some(global(n)),
    })
}

/// 建物・2 つの階・壁・開口・扉を足す変更。足した順の番号は 0 建物・1 1F・2 2F・3 壁・4 開口・5 扉
fn building_edits() -> Vec<Edit> {
    vec![
        space(SpaceKind::Building, 100, Target::Instance(10), 0.0),
        space(SpaceKind::Storey, 101, Target::Added(0), 0.0),
        space(SpaceKind::Storey, 102, Target::Added(0), 3000.0),
        new_element(
            "IfcWall",
            200,
            Some(Target::Added(1)),
            Some(at(1000.0, 0.0, 0.0, Target::Added(1))),
        ),
        new_element(
            "IfcOpeningElement",
            201,
            None,
            Some(at(500.0, 0.0, 0.0, Target::Added(3))),
        ),
        new_element(
            "IfcDoor",
            202,
            Some(Target::Added(1)),
            Some(at(0.0, 0.0, 0.0, Target::Added(4))),
        ),
        Edit::Void {
            host: Target::Added(3),
            opening: Target::Added(4),
        },
        Edit::Fill {
            opening: Target::Added(4),
            filler: Target::Added(5),
        },
        Edit::Type {
            element: Target::Added(3),
            type_of: Some(Target::Instance(20)),
        },
    ]
}

fn translation(building: &Building, element: ElementRef) -> Option<[f64; 3]> {
    let placement = building.components.placement.get(element)?;
    let Local::Transform(t) = &placement.local else {
        return None;
    };
    let [x, y, z] = t.0;
    Some([*x.get(3)?, *y.get(3)?, *z.get(3)?])
}

fn instance_of(src: &str, n: u32) -> Option<u64> {
    let needle = format!("('{}'", gid(n));
    let line = src.lines().find(|line| line.contains(&needle))?;
    let (id, _) = line.strip_prefix('#')?.split_once('=')?;
    id.parse().ok()
}

#[test]
fn adds_spaces_elements_and_relations_in_every_version() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let (bytes, codes) = run(src.as_bytes(), &building_edits());
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = bytes.unwrap();
        let after = String::from_utf8(bytes.clone()).unwrap();
        // 元の行は、バイト単位で同じまま残る
        let data_end = src.find("\nENDSEC;\nEND-ISO").unwrap();
        assert!(after.starts_with(src.get(..data_end).unwrap()), "{schema}");
        assert!(after.ends_with(src.get(data_end..).unwrap()), "{schema}");

        let building = lift(&bytes).unwrap();
        let site = element(&building, 10).unwrap();
        let block = element(&building, 100).unwrap();
        let first = element(&building, 101).unwrap();
        let second = element(&building, 102).unwrap();
        let wall = element(&building, 200).unwrap();
        let opening = element(&building, 201).unwrap();
        let door = element(&building, 202).unwrap();
        let parent = |e| {
            building
                .components
                .parent
                .get(e)
                .unwrap()
                .of
                .clone()
                .unwrap()
        };
        assert_eq!(parent(block).target, site, "{schema}");
        assert_eq!(parent(block).kind, ParentKind::Aggregates);
        assert_eq!(parent(first).target, block);
        assert_eq!(parent(second).target, block);
        assert_eq!(parent(wall).target, first);
        assert_eq!(parent(wall).kind, ParentKind::Contained);
        assert_eq!(parent(door).target, first);
        let spatial = building.components.spatial.get(second).unwrap();
        assert_eq!(spatial.kind, SpatialKind::Storey);
        // IFC4.3 では Elevation を書かず、階の配置の z が高さになる
        let written = (schema != "IFC4X3_ADD2").then_some(3000.0);
        assert_eq!(spatial.elevation.as_ref().map(|m| m.value), written);
        let level = building.components.level.get(second).unwrap();
        assert_eq!(level.height.value, 3000.0, "{schema}");
        // 配置なしで足した階は、IFC2X3 と IFC4 では親の配置に恒等で置き、IFC4.3 では高さだけ上げる
        let z = if written.is_some() { 0.0 } else { 3000.0 };
        assert_eq!(
            translation(&building, second),
            Some([0.0, 0.0, z]),
            "{schema}"
        );
        assert_eq!(
            building.components.placement.get(wall).unwrap().relative_to,
            Some(first)
        );
        assert_eq!(translation(&building, wall).unwrap(), [1000.0, 0.0, 0.0]);
        assert_eq!(
            building
                .components
                .placement
                .get(opening)
                .unwrap()
                .relative_to,
            Some(wall)
        );
        assert_eq!(
            building.components.placement.get(door).unwrap().relative_to,
            Some(opening)
        );
        assert_eq!(
            building.components.opening.get(opening),
            Some(&Opening::Voids(wall)),
            "{schema}"
        );
        assert_eq!(
            building.components.opening.get(door),
            Some(&Opening::Fills(opening))
        );
        assert_eq!(
            building.components.r#type.get(wall).map(|t| t.target),
            Some(element(&building, 20).unwrap())
        );
        let class = building.components.class.get(door).unwrap();
        assert_eq!(class.name, "IfcDoor");

        // 同じ入力と変更なら同じバイト列
        let (again, _) = run(src.as_bytes(), &building_edits());
        assert_eq!(again.unwrap(), bytes);
    }
}

#[test]
fn moves_retypes_and_removes_elements() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let built = String::from_utf8(run(src.as_bytes(), &building_edits()).0.unwrap()).unwrap();
        let wall = Target::Instance(instance_of(&built, 200).unwrap());
        let door = Target::Instance(instance_of(&built, 202).unwrap());
        let second = Target::Instance(instance_of(&built, 102).unwrap());
        let edits = [
            Edit::Place {
                element: wall,
                placement: at(2000.0, 500.0, 0.0, second),
            },
            Edit::Contain {
                element: wall,
                space: second,
            },
            Edit::Type {
                element: wall,
                type_of: None,
            },
        ];
        let (bytes, codes) = run(built.as_bytes(), &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let moved = bytes.unwrap();
        let building = lift(&moved).unwrap();
        let wall_ref = element(&building, 200).unwrap();
        let second_ref = element(&building, 102).unwrap();
        assert_eq!(
            translation(&building, wall_ref).unwrap(),
            [2000.0, 500.0, 0.0]
        );
        let parent = building.components.parent.get(wall_ref).unwrap();
        assert_eq!(parent.of.as_ref().unwrap().target, second_ref);
        assert!(building.components.r#type.get(wall_ref).is_none());
        // 開口と扉は、壁の配置からの相対のまま付いてくる
        let opening = element(&building, 201).unwrap();
        assert_eq!(
            building
                .components
                .placement
                .get(opening)
                .unwrap()
                .relative_to,
            Some(wall_ref)
        );
        // 型の関係は空になったので除く
        let text = String::from_utf8(moved.clone()).unwrap();
        assert!(!text.contains("IFCRELDEFINESBYTYPE"), "{schema}");

        // 扉と壁を除くと、開口と、使われなくなった配置・関係も除く
        let (bytes, codes) = run(
            &moved,
            &[
                Edit::Remove { element: door },
                Edit::Remove { element: wall },
            ],
        );
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let removed = String::from_utf8(bytes.unwrap()).unwrap();
        for n in [200, 201, 202] {
            assert!(!removed.contains(&gid(n)), "{schema}: {n}");
        }
        assert!(!removed.contains("IFCRELVOIDSELEMENT"));
        assert!(!removed.contains("IFCRELFILLSELEMENT"));
        assert!(!removed.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE"));
        // 残るのは敷地・建物・2 つの階の配置だけ
        assert_eq!(
            removed.matches("IFCLOCALPLACEMENT(").count(),
            4,
            "{removed}"
        );
        assert!(!removed.contains("\n\n"));
        let building = lift(removed.as_bytes()).unwrap();
        assert_eq!(building.elements().len(), 6);
        // 元の文書の行はそのまま残る
        let data_end = src.find("\nENDSEC;\nEND-ISO").unwrap();
        assert!(removed.starts_with(src.get(..data_end).unwrap()));
    }
}

#[test]
fn keeps_shared_placements_when_removing() {
    let src = sample("IFC4", true);
    let built = String::from_utf8(run(src.as_bytes(), &building_edits()).0.unwrap()).unwrap();
    // 壁だけを除く。扉の配置は開口の配置を親にしているので、開口と壁の配置は残る
    let wall = Target::Instance(instance_of(&built, 200).unwrap());
    let (bytes, codes) = run(built.as_bytes(), &[Edit::Remove { element: wall }]);
    assert!(codes.is_empty(), "{codes:?}");
    let after = String::from_utf8(bytes.unwrap()).unwrap();
    assert!(!after.contains(&gid(200)));
    assert!(!after.contains(&gid(201)));
    assert!(after.contains(&gid(202)));
    let building = lift(after.as_bytes()).unwrap();
    let door = element(&building, 202).unwrap();
    assert!(building.components.placement.get(door).is_some());
    assert_eq!(after.matches("IFCLOCALPLACEMENT(").count(), 7);
}

#[test]
fn rejects_without_changing_anything() {
    let src = sample("IFC4", true);
    let built = String::from_utf8(run(src.as_bytes(), &building_edits()).0.unwrap()).unwrap();
    let first = Target::Instance(instance_of(&built, 101).unwrap());
    let wall = Target::Instance(instance_of(&built, 200).unwrap());
    let cases: Vec<(&str, Vec<Edit>)> = vec![
        // 渡された GlobalId が文書にある
        (
            "duplicate",
            vec![new_element("IfcWall", 200, Some(first), None)],
        ),
        // 書ける表にない実体
        (
            "class",
            vec![new_element("IfcFurniture", 300, Some(first), None)],
        ),
        // 所属する空間でない
        (
            "container",
            vec![new_element("IfcWall", 300, Some(wall), None)],
        ),
        // 要素を持つ階は除かない
        ("children", vec![Edit::Remove { element: first }]),
        // 配置の親が自分の子
        (
            "cycle",
            vec![Edit::Place {
                element: wall,
                placement: at(
                    0.0,
                    0.0,
                    0.0,
                    Target::Instance(instance_of(&built, 201).unwrap()),
                ),
            }],
        ),
        // 剛体でない変換
        (
            "scale",
            vec![Edit::Place {
                element: wall,
                placement: NewPlacement {
                    relative_to: None,
                    local: Transform([
                        [2.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                    ]),
                },
            }],
        ),
        // 同じ要求で、まだ足していない要素
        (
            "handle",
            vec![Edit::Remove {
                element: Target::Added(5),
            }],
        ),
        // 除いた要素への変更
        (
            "removed",
            vec![
                Edit::Remove { element: wall },
                Edit::Contain {
                    element: wall,
                    space: first,
                },
            ],
        ),
    ];
    for (name, edits) in cases {
        // 先に当てられる変更を置いても、何も書かない
        let mut all = vec![space(SpaceKind::Storey, 400, first, 0.0)];
        all.extend(edits);
        // 階の下に階は置けないので、親を建物に替える
        if let Edit::Space(s) = &mut all[0] {
            s.parent = Target::Instance(instance_of(&built, 100).unwrap());
        }
        let (bytes, codes) = run(built.as_bytes(), &all);
        assert!(matches!(bytes, Err(Error::Invalid)), "{name}");
        assert!(
            codes.contains(&codes::CHANGE_EDIT_UNSUPPORTED),
            "{name}: {codes:?}"
        );
    }
}

/// IFC4.3 は建物の ElevationOfRefHeight と階の Elevation を非推奨にした。階の高さは親の配置からの z で書き、
/// 配置で表せない高さは受けない
#[test]
fn takes_storey_heights_from_placements_in_ifc4x3() {
    let src = sample("IFC4X3_ADD2", true);
    let building = |elevation| {
        Edit::Space(NewSpace {
            kind: SpaceKind::Building,
            name: None,
            parent: Target::Instance(10),
            placement: None,
            elevation,
            global_id: Some(global(100)),
        })
    };
    let storey = |placement, elevation| {
        Edit::Space(NewSpace {
            kind: SpaceKind::Storey,
            name: None,
            parent: Target::Added(0),
            placement,
            elevation: Some(elevation),
            global_id: Some(global(101)),
        })
    };
    // 配置と高さが合えば、Elevation を書かずに配置で高さを持つ
    for placement in [None, Some(at(0.0, 0.0, 3000.0, Target::Added(0)))] {
        let edits = [building(None), storey(placement, 3000.0)];
        let bytes = run(src.as_bytes(), &edits).0.unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let line = text.lines().find(|l| l.contains(&gid(101))).unwrap();
        assert!(line.ends_with(",.ELEMENT.,$);"), "{line}");
        let lifted = lift(&bytes).unwrap();
        let level = element(&lifted, 101).unwrap();
        let level = lifted.components.level.get(level).unwrap();
        assert_eq!(level.height.value, 3000.0);
    }
    let cases = [
        // 建物の標高は配置で表せない
        vec![building(Some(0.0))],
        // 配置の z が高さと違う
        vec![
            building(None),
            storey(Some(at(0.0, 0.0, 0.0, Target::Added(0))), 3000.0),
        ],
        // 親でない配置からの z
        vec![
            building(None),
            storey(Some(at(0.0, 0.0, 3000.0, Target::Instance(10))), 3000.0),
        ],
    ];
    for edits in cases {
        let (bytes, codes) = run(src.as_bytes(), &edits);
        assert!(matches!(bytes, Err(Error::Invalid)), "{edits:?}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{edits:?}");
    }
}

#[test]
fn needs_an_owner_history_only_in_ifc2x3() {
    for (schema, ok) in [("IFC2X3", false), ("IFC4", true), ("IFC4X3_ADD2", true)] {
        let src = sample(schema, false);
        let (bytes, codes) = run(
            src.as_bytes(),
            &[space(SpaceKind::Building, 100, Target::Instance(10), 0.0)],
        );
        assert_eq!(bytes.is_ok(), ok, "{schema}: {codes:?}");
        if ok {
            let text = String::from_utf8(bytes.unwrap()).unwrap();
            assert!(text.contains(&format!("IFCBUILDING('{}',$,", gid(100))));
        }
    }
}

#[test]
fn mints_global_ids_from_the_seed() {
    let src = sample("IFC4", true);
    let edit = Edit::Space(NewSpace {
        kind: SpaceKind::Building,
        name: None,
        parent: Target::Instance(10),
        placement: None,
        elevation: None,
        global_id: None,
    });
    let (a, _) = run(src.as_bytes(), std::slice::from_ref(&edit));
    let (b, _) = run(src.as_bytes(), &[edit]);
    let a = a.unwrap();
    assert_eq!(a, b.unwrap());
    let building = lift(&a).unwrap();
    assert_eq!(building.elements().len(), 4);
    assert!(
        building
            .elements()
            .iter()
            .all(|e| matches!(e.id(), Id::Global(_)))
    );
}

/// 文書の `('` で始まる 22 文字の GlobalId
fn global_ids(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let (_, rest) = line.split_once("('")?;
            let (global_id, _) = rest.split_once('\'')?;
            (global_id.len() == 22).then(|| global_id.to_owned())
        })
        .collect()
}

#[test]
fn keys_relation_global_ids_by_what_they_join() {
    // 関係とセットの GlobalId は、足した順でなく、種類と結ぶ要素とセットの名前から決まる（#395）
    let note = |set: &str| Edit::Property {
        element: 10,
        set: set.to_owned(),
        name: "Note".to_owned(),
        value: NewValue {
            type_name: "IfcLabel".to_owned(),
            literal: NewLiteral::Text("a".to_owned()),
        },
    };
    let wall = || new_element("IfcWall", 200, Some(Target::Instance(10)), None);
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let (alone, codes) = run(src.as_bytes(), &[wall(), note("Pset_A")]);
        let alone = alone.unwrap_or_else(|_| panic!("{schema}: {codes:?}"));
        let (after, codes) = run(
            src.as_bytes(),
            &[
                space(SpaceKind::Building, 100, Target::Instance(10), 0.0),
                new_element("IfcSlab", 201, Some(Target::Added(0)), None),
                note("Pset_B"),
                wall(),
                note("Pset_A"),
            ],
        );
        let after = after.unwrap_or_else(|_| panic!("{schema}: {codes:?}"));
        // 壁の包含・セット・セットの関係は、先に足した建物・床・別のセットによらない
        let (alone, after) = (global_ids(&alone), global_ids(&after));
        assert_eq!(alone.len(), 8, "{schema}");
        for global_id in &alone {
            assert!(after.contains(global_id), "{schema}: {global_id}");
        }
        let mut unique = after.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), after.len(), "{schema}");
    }
}

#[test]
fn writes_predefined_types_by_version() {
    let edit = |predefined: Option<&str>| {
        Edit::Element(NewElement {
            class: "IfcWall".to_owned(),
            predefined: predefined.map(str::to_owned),
            name: None,
            container: None,
            placement: None,
            global_id: Some(global(300)),
        })
    };
    // IFC2X3 の壁は定義された種類を持たない
    let (bytes, codes) = run(
        sample("IFC2X3", true).as_bytes(),
        &[edit(Some("SOLIDWALL"))],
    );
    assert!(bytes.is_err());
    assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED]);
    let (bytes, _) = run(sample("IFC2X3", true).as_bytes(), &[edit(None)]);
    let text = String::from_utf8(bytes.unwrap()).unwrap();
    assert!(text.contains(&format!("=IFCWALL('{}',#2,$,$,$,$,$,$);", gid(300))));
    let (bytes, _) = run(sample("IFC4", true).as_bytes(), &[edit(Some("solidwall"))]);
    let text = String::from_utf8(bytes.unwrap()).unwrap();
    assert!(text.contains(&format!(
        "=IFCWALL('{}',#2,$,$,$,$,$,$,.SOLIDWALL.);",
        gid(300)
    )));
    // IFC2X3 の屋根の 9 ShapeType は必須なので、渡されなければ NOTDEFINED にする
    let roof = Edit::Element(NewElement {
        class: "IfcRoof".to_owned(),
        predefined: None,
        name: None,
        container: None,
        placement: None,
        global_id: Some(global(301)),
    });
    let (bytes, _) = run(sample("IFC2X3", true).as_bytes(), &[roof]);
    let text = String::from_utf8(bytes.unwrap()).unwrap();
    assert!(text.contains(".NOTDEFINED.);"));
}

fn rename(element: u64, name: &str) -> Edit {
    Edit::Attribute {
        element,
        attribute: RootAttribute::Name,
        value: Some(name.to_owned()),
    }
}

/// 要素のインスタンス番号と、その要素を変えたときに影響する要素のインスタンス番号
type Edge = (Option<u64>, Vec<Option<u64>>);

fn dependency_edges(live: &IfcLive) -> Option<Vec<Edge>> {
    let instance = |element: ElementRef| {
        live.building()
            .element(element)
            .and_then(|found| found.source().instance)
    };
    let mut edges = Vec::new();
    for index in 0..live.building().elements().len() {
        let element = ElementRef(u32::try_from(index).ok()?);
        let affected = with(|cx| live.affected_elements(&[element], cx)).0.ok()?;
        edges.push((
            instance(element),
            affected.into_iter().map(instance).collect(),
        ));
    }
    Some(edges)
}

/// 保存したバイト列・建物モデル・依存の辺・変換の診断が、保存したファイルを開き直したものと同じか
fn same_as_reopened(live: &IfcLive) -> Option<Vec<u8>> {
    let saved = with(|cx| live.save(cx)).0.ok()?;
    let reopened = with(|cx| IfcLive::open(&saved, cx)).0.ok()?;
    assert_eq!(live.building(), reopened.building());
    assert_eq!(dependency_edges(live)?, dependency_edges(&reopened)?);
    let codes = |engine: &IfcLive| {
        let (result, diag) = with(|cx| engine.report_lift_diagnostics(cx));
        result.ok().map(|()| diag.into_vec())
    };
    assert_eq!(codes(live)?, codes(&reopened)?);
    Some(saved)
}

/// 文書エンジンで当て、保存したファイルに今までの経路で当てたものと、結果・診断・保存が同じか
fn apply_like_batch(live: &mut IfcLive, edits: &[Edit]) -> Option<Vec<u8>> {
    let saved = with(|cx| live.save(cx)).0.ok()?;
    let file = read(&saved)?;
    let expected = with(|cx| apply(&file, edits, b"seed", cx));
    let actual = with(|cx| live.apply(edits, b"seed", cx));
    assert_eq!(actual.1.into_vec(), expected.1.into_vec());
    let expected = expected.0.ok()?;
    assert_eq!(actual.0.ok()?, expected.changed);
    let now = same_as_reopened(live)?;
    assert_eq!(now, expected.bytes);
    assert_eq!(live.building(), &lift(&now)?);
    Some(now)
}

#[test]
fn the_document_engine_applies_structural_edits_like_reopening() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let (live, _) = with(|cx| IfcLive::open(src.as_bytes(), cx));
        let Ok(mut live) = live else {
            panic!("{schema}");
        };
        // 名前を変えて取り消し、やり直しを残したまま構造の編集を当てる
        let renamed = apply_like_batch(&mut live, &[rename(10, "Renamed")]);
        assert!(with(|cx| live.undo(cx)).0.is_ok_and(|done| done));
        assert_eq!(live.redo_len(), 1);
        let opened = live.state_id();
        let built = apply_like_batch(&mut live, &building_edits());
        assert!(built.is_some(), "{schema}");
        assert!(!live.is_clean());
        assert_ne!(live.state_id(), opened);
        assert_eq!((live.undo_len(), live.redo_len()), (1, 0));
        // 作り直した状態の上で、差分で当てる編集と構造の編集を続ける
        let after_rename = apply_like_batch(&mut live, &[rename(10, "Site 2")]);
        let removed = apply_like_batch(
            &mut live,
            &[Edit::Remove {
                element: Target::Instance(10),
            }],
        );
        assert!(removed.is_none(), "{schema}: 子のある敷地は除けない");
        assert_eq!(with(|cx| live.save(cx)).0.ok(), after_rename);
        let states = [
            Some(src.as_bytes().to_vec()),
            built.clone(),
            after_rename.clone(),
        ];
        assert_eq!((live.undo_len(), live.redo_len()), (2, 0));
        // 取り消しとやり直しで、どの状態にも同じバイト列と建物モデルで戻る
        for state in states.iter().rev().skip(1) {
            assert!(with(|cx| live.undo(cx)).0.is_ok_and(|done| done));
            assert_eq!(same_as_reopened(&live), *state, "{schema}");
        }
        assert!(live.is_clean());
        assert_eq!(live.state_id(), opened);
        assert_eq!((live.undo_len(), live.redo_len()), (0, 2));
        assert!(with(|cx| live.undo(cx)).0.is_ok_and(|done| !done));
        for state in states.iter().skip(1) {
            assert!(with(|cx| live.redo(cx)).0.is_ok_and(|done| done));
            assert_eq!(same_as_reopened(&live), *state, "{schema}");
        }
        assert!(with(|cx| live.redo(cx)).0.is_ok_and(|done| !done));
        // 戻ってから当てると、先の状態はやり直せない
        assert!(with(|cx| live.undo(cx)).0.is_ok_and(|done| done));
        assert!(with(|cx| live.undo(cx)).0.is_ok_and(|done| done));
        assert!(apply_like_batch(&mut live, &[rename(10, "Renamed")]).is_some());
        assert_eq!(with(|cx| live.save(cx)).0.ok(), renamed);
        assert_eq!((live.undo_len(), live.redo_len()), (1, 0));
        assert!(with(|cx| live.redo(cx)).0.is_ok_and(|done| !done));
    }
}

#[test]
fn adds_site_types_and_rejects_invalid_containment_or_cycles() {
    for (schema, version) in [
        ("IFC2X3", Version::Ifc2x3),
        ("IFC4", Version::Ifc4),
        ("IFC4X3_ADD2", Version::Ifc4x3),
    ] {
        let template = Template {
            version,
            unit: LengthUnit::Millimeter,
            project: global(1),
            name: Some("Project".to_owned()),
            header: Header::default(),
            owner: Some(OwnerHistory {
                person: "Author".to_owned(),
                organization: "Organization".to_owned(),
                developer: "zumen".to_owned(),
                application: "zumen".to_owned(),
                version: "0.3.0".to_owned(),
                identifier: "zumen".to_owned(),
                created: 1_700_000_000,
            }),
        };
        let (base, diag) = with(|cx| create(&template, cx));
        assert!(diag.is_empty(), "{schema}");
        let base = String::from_utf8(base.unwrap()).unwrap();
        let data_end = base.find("\nENDSEC;\nEND-ISO").unwrap();

        // IFC2X3 は扉の型（IfcDoorType）を持たないので断る
        let door_type = Edit::ElementType(NewType {
            class: "IfcDoorType".to_owned(),
            predefined: Some("DOOR".to_owned()),
            element_type: None,
            name: Some("DT-1".to_owned()),
            global_id: Some(global(112)),
        });
        let (door_res, door_codes) = run(base.as_bytes(), std::slice::from_ref(&door_type));
        assert_eq!(
            door_res.is_ok(),
            schema != "IFC2X3",
            "{schema}: {door_codes:?}"
        );

        let mut edits = vec![
            Edit::Space(NewSpace {
                kind: SpaceKind::Site,
                name: Some("Site".to_owned()),
                parent: Target::Instance(1),
                placement: Some(NewPlacement {
                    relative_to: None,
                    local: Transform::translation(100.0, 200.0, 0.0),
                }),
                elevation: Some(12500.0),
                global_id: Some(global(10)),
            }),
            space(SpaceKind::Building, 100, Target::Added(0), 0.0),
            space(SpaceKind::Storey, 101, Target::Added(1), 0.0),
            space(SpaceKind::Storey, 102, Target::Added(1), 3000.0),
            Edit::ElementType(NewType {
                class: "IfcWallType".to_owned(),
                predefined: Some("SHEAR".to_owned()),
                element_type: None,
                name: Some("WT-1".to_owned()),
                global_id: Some(global(110)),
            }),
            Edit::ElementType(NewType {
                class: "IfcColumnType".to_owned(),
                predefined: Some("USERDEFINED".to_owned()),
                element_type: Some("C-600".to_owned()),
                name: Some("CT-1".to_owned()),
                global_id: Some(global(111)),
            }),
            new_element(
                "IfcWall",
                200,
                Some(Target::Added(2)),
                Some(at(1000.0, 0.0, 0.0, Target::Added(2))),
            ),
            new_element(
                "IfcOpeningElement",
                201,
                None,
                Some(at(500.0, 0.0, 0.0, Target::Added(6))),
            ),
            Edit::Void {
                host: Target::Added(6),
                opening: Target::Added(7),
            },
            Edit::Type {
                element: Target::Added(6),
                type_of: Some(Target::Added(4)),
            },
        ];
        if schema != "IFC2X3" {
            edits.push(door_type);
        }
        let (built, codes) = run(base.as_bytes(), &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let built = String::from_utf8(built.unwrap()).unwrap();
        assert!(built.starts_with(base.get(..data_end).unwrap()), "{schema}");
        assert!(built.ends_with(base.get(data_end..).unwrap()), "{schema}");

        let lifted = lift(built.as_bytes()).unwrap();
        let site = element(&lifted, 10).unwrap();
        let site_spatial = lifted.components.spatial.get(site).unwrap();
        assert_eq!(site_spatial.kind, SpatialKind::Site);
        assert_eq!(
            site_spatial.ref_elevation.as_ref().map(|m| m.value),
            Some(12500.0)
        );
        let wt = element(&lifted, 110).unwrap();
        let ct = element(&lifted, 111).unwrap();
        let w = element(&lifted, 200).unwrap();
        assert_eq!(lifted.components.r#type.get(w).map(|t| t.target), Some(wt));
        assert_eq!(
            lifted
                .components
                .class
                .get(ct)
                .and_then(|c| c.predefined.as_deref()),
            Some("USERDEFINED")
        );

        let site_inst = Target::Instance(instance_of(&built, 10).unwrap());
        let bldg_inst = Target::Instance(instance_of(&built, 100).unwrap());
        let first_inst = Target::Instance(instance_of(&built, 101).unwrap());
        let opening_inst = Target::Instance(instance_of(&built, 201).unwrap());
        let wt_inst = Target::Instance(instance_of(&built, 110).unwrap());

        // 開口の空間包含と、集約の循環（敷地を子孫の建物・階の下へ移す）は断る
        let invalid_cases: Vec<Vec<Edit>> = vec![
            vec![new_element(
                "IfcOpeningElement",
                390,
                Some(first_inst),
                None,
            )],
            vec![Edit::Contain {
                element: opening_inst,
                space: first_inst,
            }],
            vec![Edit::Contain {
                element: site_inst,
                space: bldg_inst,
            }],
            vec![Edit::Contain {
                element: site_inst,
                space: first_inst,
            }],
        ];
        for invalid in invalid_cases {
            let (res, codes) = run(built.as_bytes(), &invalid);
            assert!(matches!(res, Err(Error::Invalid)), "{schema}: {invalid:?}");
            assert_eq!(
                codes,
                [codes::CHANGE_EDIT_UNSUPPORTED],
                "{schema}: {invalid:?}"
            );
        }

        // 開口を IFCREL でないインスタンス（プロパティ参照）が指す壁は、関係を書き換えずに断る
        let opening_raw = instance_of(&built, 201).unwrap();
        let wall_raw = instance_of(&built, 200).unwrap();
        let with_ref = built.replace(
            "ENDSEC;\nEND-ISO-10303-21;",
            &format!(
                "#9900=IFCPROPERTYSINGLEVALUE('Ref',$,#{opening_raw},$);\n\
                 #9901=IFCPROPERTYSET('{}',#2,'Pset_Ref',$,(#9900));\n\
                 #9902=IFCRELDEFINESBYPROPERTIES('{}',#2,$,$,(#1),#9901);\n\
                 ENDSEC;\nEND-ISO-10303-21;",
                gid(9901),
                gid(9902),
            ),
        );
        let (res, codes) = run(
            with_ref.as_bytes(),
            &[
                Edit::Remove {
                    element: Target::Instance(wall_raw),
                },
                rename(10, "Untouched"),
            ],
        );
        assert!(matches!(res, Err(Error::Invalid)), "{schema}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{schema}");

        // 型を除くと、型の関係も除かれ、要素自体は残る
        let (after_type_rm, codes) = run(built.as_bytes(), &[Edit::Remove { element: wt_inst }]);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let after_type_rm = after_type_rm.unwrap();
        let lifted_after = lift(&after_type_rm).unwrap();
        assert!(element(&lifted_after, 110).is_none());
        let w_after = element(&lifted_after, 200).unwrap();
        assert!(lifted_after.components.r#type.get(w_after).is_none());
    }
}

#[test]
fn the_document_engine_tracks_every_structural_dependency_and_zip_history() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let zip_src = zumen_zip::write(&[("model/a.ifc", src.as_bytes())]).unwrap();
        for zipped in [false, true] {
            let input = if zipped {
                zip_src.as_slice()
            } else {
                src.as_bytes()
            };
            let (live, diag) = with(|cx| IfcLive::open(input, cx));
            assert!(diag.is_empty(), "{schema} zip={zipped}");
            let mut live = live.unwrap();
            assert_eq!(live.is_zip(), zipped);

            // 1 段目: 建物・2 階・通り芯・新しい型・壁・開口・扉・柱と関係をまとめて足す
            let step1 = vec![
                space(SpaceKind::Building, 100, Target::Instance(10), 0.0),
                space(SpaceKind::Storey, 101, Target::Added(0), 0.0),
                space(SpaceKind::Storey, 102, Target::Added(0), 3000.0),
                Edit::Grid(grid(103, Target::Added(1))),
                Edit::ElementType(NewType {
                    class: "IfcWallType".to_owned(),
                    predefined: Some("SHEAR".to_owned()),
                    element_type: None,
                    name: Some("WT-110".to_owned()),
                    global_id: Some(global(110)),
                }),
                new_element(
                    "IfcWall",
                    200,
                    Some(Target::Added(1)),
                    Some(at(1000.0, 0.0, 0.0, Target::Added(1))),
                ),
                new_element(
                    "IfcOpeningElement",
                    201,
                    None,
                    Some(at(500.0, 0.0, 0.0, Target::Added(5))),
                ),
                new_element(
                    "IfcDoor",
                    202,
                    Some(Target::Added(1)),
                    Some(at(0.0, 0.0, 0.0, Target::Added(6))),
                ),
                new_element(
                    "IfcColumn",
                    203,
                    Some(Target::Added(1)),
                    Some(at(0.0, 2000.0, 0.0, Target::Added(1))),
                ),
                Edit::Void {
                    host: Target::Added(5),
                    opening: Target::Added(6),
                },
                Edit::Fill {
                    opening: Target::Added(6),
                    filler: Target::Added(7),
                },
                Edit::Type {
                    element: Target::Added(5),
                    type_of: Some(Target::Added(4)),
                },
                Edit::Type {
                    element: Target::Added(8),
                    type_of: Some(Target::Instance(20)),
                },
            ];
            let expected1 = run(src.as_bytes(), &step1).0.unwrap();
            let (changed1, diag1) = with(|cx| live.apply(&step1, b"seed", cx));
            assert!(diag1.is_empty(), "{schema} zip={zipped}");
            assert!(changed1.unwrap() > 0);
            assert_eq!(with(|cx| live.save_spf(cx)).0.unwrap(), expected1);

            // 依存辺（包含・集約・相対配置・型・開口と充填・通り芯）がすべて追えることを確かめる
            let find = |engine: &IfcLive, n: u32| element(engine.building(), n).unwrap();
            let affected_by = |engine: &IfcLive, n: u32| -> Vec<ElementRef> {
                let root = find(engine, n);
                with(|cx| engine.affected_elements(&[root], cx)).0.unwrap()
            };
            let [
                bldg,
                first,
                second,
                grid_ref,
                wt110,
                wall_ref,
                open_ref,
                door_ref,
                col_ref,
                wt20,
            ] = [100, 101, 102, 103, 110, 200, 201, 202, 203, 20].map(|n| find(&live, n));
            let site_affected = affected_by(&live, 10);
            for target in [
                bldg, first, second, grid_ref, wall_ref, open_ref, door_ref, col_ref,
            ] {
                assert!(site_affected.contains(&target), "{schema}: {target:?}");
            }
            let first_affected = affected_by(&live, 101);
            for target in [grid_ref, wall_ref, open_ref, door_ref, col_ref] {
                assert!(first_affected.contains(&target), "{schema}: {target:?}");
            }
            assert!(!first_affected.contains(&second));
            // 型 -> 要素（壁から開口・扉へも推移する）
            let wt110_affected = affected_by(&live, 110);
            for target in [wall_ref, open_ref, door_ref] {
                assert!(wt110_affected.contains(&target));
            }
            assert!(!wt110_affected.contains(&wt110));
            assert!(!wt110_affected.contains(&col_ref));
            let wt20_affected = affected_by(&live, 20);
            assert!(!wt20_affected.contains(&wt20));
            assert!(wt20_affected.contains(&col_ref));
            // 相対配置・開口と充填（壁 <-> 開口 -> 扉）
            let wall_affected = affected_by(&live, 200);
            assert!(wall_affected.contains(&open_ref));
            assert!(wall_affected.contains(&door_ref));
            let open_affected = affected_by(&live, 201);
            assert!(open_affected.contains(&wall_ref));
            assert!(open_affected.contains(&door_ref));
            // 通り芯の軸 -> 通り芯
            if let Some(zumen_building::Grid::Grid { axes }) =
                live.building().components.grid.get(grid_ref)
            {
                assert_eq!(axes.len(), 4);
                for &(_, axis_elem) in axes {
                    let axis_affected = with(|cx| live.affected_elements(&[axis_elem], cx))
                        .0
                        .unwrap();
                    assert!(axis_affected.contains(&grid_ref), "{schema}");
                }
            } else {
                panic!("{schema}: grid missing");
            }
            let state1_saved = with(|cx| live.save(cx)).0.unwrap();

            // 2 段目: 差分で当てる編集（属性とプロパティ）
            let spf1_text = String::from_utf8(expected1.clone()).unwrap();
            let wall_inst = instance_of(&spf1_text, 200).unwrap();
            let step2 = vec![
                rename(wall_inst, "Wall-Renamed"),
                Edit::Property {
                    element: wall_inst,
                    set: "Pset_WallCommon".to_owned(),
                    name: "Reference".to_owned(),
                    value: NewValue {
                        type_name: "IfcIdentifier".to_owned(),
                        literal: NewLiteral::Text("W1".to_owned()),
                    },
                },
            ];
            let expected2 = run(&expected1, &step2).0.unwrap();
            with(|cx| live.apply(&step2, b"seed", cx)).0.unwrap();
            assert_eq!(with(|cx| live.save_spf(cx)).0.unwrap(), expected2);
            let state2_saved = with(|cx| live.save(cx)).0.unwrap();

            // 3 段目: 柱と通り芯を 2F へ置き直し・移し、型を付け替え・外す
            let second_inst = Target::Instance(instance_of(&spf1_text, 102).unwrap());
            let col_inst = Target::Instance(instance_of(&spf1_text, 203).unwrap());
            let grid_inst = Target::Instance(instance_of(&spf1_text, 103).unwrap());
            let step3 = vec![
                Edit::Place {
                    element: col_inst,
                    placement: at(2500.0, 1000.0, 0.0, second_inst),
                },
                Edit::Contain {
                    element: col_inst,
                    space: second_inst,
                },
                Edit::Contain {
                    element: grid_inst,
                    space: second_inst,
                },
                Edit::Type {
                    element: Target::Instance(wall_inst),
                    type_of: Some(Target::Instance(20)),
                },
                Edit::Type {
                    element: col_inst,
                    type_of: None,
                },
            ];
            let expected3 = run(&expected2, &step3).0.unwrap();
            with(|cx| live.apply(&step3, b"seed", cx)).0.unwrap();
            assert_eq!(with(|cx| live.save_spf(cx)).0.unwrap(), expected3);
            // 置き直した後の依存辺も開き直した状態と一致する
            let reopened3 = with(|cx| IfcLive::open(&expected3, cx)).0.unwrap();
            assert_eq!(dependency_edges(&live), dependency_edges(&reopened3));
            let col3 = find(&live, 203);
            let grid3 = find(&live, 103);
            let wall3 = find(&live, 200);
            assert!(affected_by(&live, 102).contains(&col3));
            assert!(affected_by(&live, 102).contains(&grid3));
            assert!(!affected_by(&live, 101).contains(&col3));
            assert!(affected_by(&live, 110).is_empty());
            assert!(affected_by(&live, 20).contains(&wall3));
            assert!(!affected_by(&live, 20).contains(&col3));
            let _ = wt110;
            let _ = wt20;
            let state3_saved = with(|cx| live.save(cx)).0.unwrap();

            // 4 段目: 扉・壁（開口も連鎖）・通り芯を除く
            let spf3_text = String::from_utf8(expected3.clone()).unwrap();
            let door_inst = Target::Instance(instance_of(&spf3_text, 202).unwrap());
            let step4 = vec![
                Edit::Remove { element: door_inst },
                Edit::Remove {
                    element: Target::Instance(instance_of(&spf3_text, 200).unwrap()),
                },
                Edit::Remove { element: grid_inst },
            ];
            let expected4 = run(&expected3, &step4).0.unwrap();
            with(|cx| live.apply(&step4, b"seed", cx)).0.unwrap();
            assert_eq!(with(|cx| live.save_spf(cx)).0.unwrap(), expected4);
            let state4_saved = with(|cx| live.save(cx)).0.unwrap();

            // 4 段すべて取り消すと元のバイト列（ifcZIP なら元のアーカイブそのもの）に戻り、やり直すと各段に戻る
            let saved_states = [
                input.to_vec(),
                state1_saved,
                state2_saved,
                state3_saved,
                state4_saved,
            ];
            let spf_states = [
                src.as_bytes().to_vec(),
                expected1,
                expected2,
                expected3,
                expected4,
            ];
            assert_eq!((live.undo_len(), live.redo_len()), (4, 0));
            for idx in (0..4).rev() {
                assert_eq!(with(|cx| live.undo(cx)).0.ok(), Some(true));
                assert_eq!(with(|cx| live.save(cx)).0.unwrap(), saved_states[idx]);
                assert_eq!(with(|cx| live.save_spf(cx)).0.unwrap(), spf_states[idx]);
                let reopened = with(|cx| IfcLive::open(&spf_states[idx], cx)).0.unwrap();
                assert_eq!(live.building(), reopened.building());
                assert_eq!(dependency_edges(&live), dependency_edges(&reopened));
            }
            assert!(live.is_clean());
            assert_eq!(live.state_id(), 0);
            for idx in 1..=4 {
                assert_eq!(with(|cx| live.redo(cx)).0.ok(), Some(true));
                assert_eq!(with(|cx| live.save(cx)).0.unwrap(), saved_states[idx]);
                assert_eq!(with(|cx| live.save_spf(cx)).0.unwrap(), spf_states[idx]);
            }
        }
    }
}

#[test]
fn the_document_engine_keeps_its_state_on_refused_or_stopped_edits() {
    let src = sample("IFC4", true);
    let (live, _) = with(|cx| IfcLive::open(src.as_bytes(), cx));
    let Ok(mut live) = live else {
        panic!("open");
    };
    let refused = [Edit::Remove {
        element: Target::Instance(99),
    }];
    let (result, diag) = with(|cx| live.apply(&refused, b"seed", cx));
    assert!(matches!(result, Err(Error::Invalid)));
    assert!(!diag.is_empty());
    assert!(live.is_clean());

    // 構造の編集・差分の編集・それぞれの undo / redo のすべての段で、どの予算や中断で止まっても状態を変えない
    let structural = building_edits();
    let diff = [rename(10, "After")];
    let perform = |engine: &mut IfcLive, phase: usize, cx: &mut Cx| -> Result<bool, Error> {
        match phase {
            0 => engine.apply(&structural, b"seed", cx).map(|n| n > 0),
            1 => engine.apply(&diff, b"seed", cx).map(|n| n > 0),
            2 | 3 => engine.undo(cx),
            _ => engine.redo(cx),
        }
    };
    for phase in 0..6 {
        let before_bytes = with(|cx| live.save(cx)).0.unwrap();
        let before_building = live.building().clone();
        let before_edges = dependency_edges(&live).unwrap();
        let before_meta = (
            live.is_clean(),
            live.state_id(),
            live.undo_len(),
            live.redo_len(),
        );
        let mut measured = Budget::default();
        let ok_cancel = CancelToken::new();
        perform(
            &mut live.clone(),
            phase,
            &mut Cx::new(&mut measured, &mut Diagnostics::new(), &ok_cancel),
        )
        .unwrap();
        let mut expected_next = live.clone();
        with(|cx| perform(&mut expected_next, phase, cx)).0.unwrap();
        let expected_bytes = with(|cx| expected_next.save(cx)).0.unwrap();

        for kind in Kind::ALL {
            let used = measured.used(kind);
            if used == 0 {
                continue;
            }
            for limit in [0, used / 2, used - 1, used] {
                let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                let mut diag = Diagnostics::new();
                let mut candidate = live.clone();
                let outcome = perform(
                    &mut candidate,
                    phase,
                    &mut Cx::new(&mut budget, &mut diag, &ok_cancel),
                );
                if limit < used {
                    assert!(
                        matches!(outcome, Err(Error::Stopped(_))),
                        "phase={phase} {kind:?} limit={limit} used={used}"
                    );
                    assert_eq!(with(|cx| candidate.save(cx)).0.unwrap(), before_bytes);
                    assert_eq!(candidate.building(), &before_building);
                    assert_eq!(dependency_edges(&candidate).unwrap(), before_edges);
                    assert_eq!(
                        (
                            candidate.is_clean(),
                            candidate.state_id(),
                            candidate.undo_len(),
                            candidate.redo_len(),
                        ),
                        before_meta
                    );
                } else {
                    assert!(
                        outcome.is_ok(),
                        "phase={phase} {kind:?} limit={limit} used={used}"
                    );
                    assert_eq!(with(|cx| candidate.save(cx)).0.unwrap(), expected_bytes);
                }
            }
        }

        let cancel = CancelToken::new();
        cancel.cancel();
        let stopped = perform(
            &mut live,
            phase,
            &mut Cx::new(
                &mut Budget::new(Limits::default()),
                &mut Diagnostics::new(),
                &cancel,
            ),
        );
        assert!(matches!(stopped, Err(Error::Stopped(_))));
        assert_eq!(with(|cx| live.save(cx)).0.unwrap(), before_bytes);
        assert_eq!(live.building(), &before_building);
        assert_eq!(dependency_edges(&live).unwrap(), before_edges);
        assert_eq!(
            (
                live.is_clean(),
                live.state_id(),
                live.undo_len(),
                live.redo_len(),
            ),
            before_meta
        );

        assert_eq!(with(|cx| perform(&mut live, phase, cx)).0.ok(), Some(true));
        assert_eq!(with(|cx| live.save(cx)).0.unwrap(), expected_bytes);
    }
}

fn layer(material: &str, thickness: f64) -> NewLayer {
    NewLayer {
        material: material.to_owned(),
        thickness,
    }
}

/// 空間 `storey` の芯 `from`-`to` の、高さ 2800 で 2 層（コンクリート 150・断熱 50）の壁
fn wall(n: u32, storey: Target, from: [f64; 2], to: [f64; 2]) -> Edit {
    Edit::Wall(NewWall {
        storey,
        start: from,
        end: to,
        height: 2800.0,
        layers: vec![layer("Concrete", 150.0), layer("Insulation", 50.0)],
        offset: None,
        type_of: None,
        name: Some(format!("W{n}")),
        global_id: Some(global(n)),
    })
}

/// 建物と、高さ 3000 の階を足し、その階に壁を 2 つ足す変更
fn wall_edits() -> Vec<Edit> {
    vec![
        space(SpaceKind::Building, 100, Target::Instance(10), 0.0),
        Edit::Space(NewSpace {
            kind: SpaceKind::Storey,
            name: Some("2F".to_owned()),
            parent: Target::Added(0),
            placement: Some(at(0.0, 0.0, 3000.0, Target::Added(0))),
            elevation: Some(3000.0),
            global_id: Some(global(101)),
        }),
        wall(300, Target::Added(1), [1000.0, 0.0], [1000.0, 4000.0]),
        wall(301, Target::Added(1), [1000.0, 4000.0], [-2000.0, 4000.0]),
    ]
}

fn count(src: &str, entity: &str) -> usize {
    let needle = format!("={entity}(");
    src.lines().filter(|line| line.contains(&needle)).count()
}

#[test]
fn adds_walls_from_the_centreline_height_and_layers_in_every_version() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let (result, codes) = run(src.as_bytes(), &wall_edits());
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = result.unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let building = lift(&bytes).unwrap();
        let storey = element(&building, 101).unwrap();
        let wall = element(&building, 300).unwrap();
        let class = building.components.class.get(wall).unwrap();
        assert_eq!(class.name, "IfcWall", "{schema}");
        let original = if schema == "IFC2X3" {
            "IfcWallStandardCase"
        } else {
            "IfcWall"
        };
        assert!(class.original.eq_ignore_ascii_case(original), "{schema}");
        let parent = building.components.parent.get(wall).unwrap();
        let of = parent.of.as_ref().unwrap();
        assert_eq!((of.target, of.kind), (storey, ParentKind::Contained));
        // 芯の始点に置き、芯の向きを局所の x 軸にする
        let placement = building.components.placement.get(wall).unwrap();
        assert_eq!(placement.relative_to, Some(storey));
        let Local::Transform(local) = &placement.local else {
            panic!("{schema}");
        };
        assert_eq!(
            local.0,
            [
                [0.0, -1.0, 0.0, 1000.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0]
            ]
        );
        let second = element(&building, 301).unwrap();
        assert_eq!(translation(&building, second), Some([1000.0, 4000.0, 0.0]));
        // 材料の層は芯の左へ並べ、厚さの合計の半分だけ右からはじめる
        let material = building.components.material.get(wall).unwrap();
        assert_eq!(material.kind, MaterialKind::Layers);
        assert_eq!(material.direction.as_deref(), Some("AXIS2"));
        assert_eq!(material.offset.as_ref().map(|m| m.value), Some(-100.0));
        let thickness: Vec<_> = material
            .parts
            .iter()
            .map(|part| part.thickness.as_ref().map(|t| t.value))
            .collect();
        assert_eq!(thickness, [Some(150.0), Some(50.0)]);
        // 表現は芯の線（Axis）と押し出し（Body）
        assert_eq!(count(&text, "IFCSHAPEREPRESENTATION"), 4);
        assert_eq!(count(&text, "IFCPOLYLINE"), 4);
        let file = read(&bytes).unwrap();
        let id = instance_of(&text, 300).unwrap();
        let (shape, _) = with(|cx| file.product_shape(id, cx));
        let shape = shape.unwrap();
        let depths: Vec<f64> = shape
            .nodes
            .iter()
            .filter_map(|node| with(|cx| file.extruded_area(node, cx)).0.ok())
            .map(|area| area.definition.depth)
            .collect();
        assert_eq!(depths, [2800.0], "{schema}");
        // 文脈と材料は、同じ要求の壁で使い回す
        assert_eq!(count(&text, "IFCGEOMETRICREPRESENTATIONCONTEXT"), 1);
        assert_eq!(count(&text, "IFCGEOMETRICREPRESENTATIONSUBCONTEXT"), 2);
        assert_eq!(count(&text, "IFCMATERIALLAYERSETUSAGE"), 2);
        // 同じ入力と変更なら同じバイト列。文書エンジンでも同じ
        assert_eq!(run(src.as_bytes(), &wall_edits()).0.unwrap(), bytes);
        let (live, _) = with(|cx| IfcLive::open(src.as_bytes(), cx));
        let mut live = live.unwrap();
        assert_eq!(
            apply_like_batch(&mut live, &wall_edits()),
            Some(bytes),
            "{schema}"
        );
    }
}

#[test]
fn refuses_walls_it_cannot_write_without_changing_anything() {
    let src = sample("IFC4", true);
    let base = |edit: &dyn Fn(&mut NewWall)| {
        let Edit::Wall(mut new) = wall(300, Target::Instance(10), [0.0, 0.0], [1000.0, 0.0]) else {
            panic!("wall");
        };
        edit(&mut new);
        Edit::Wall(new)
    };
    let cases = [
        base(&|w| w.end = w.start),
        base(&|w| w.start = [f64::NAN, 0.0]),
        base(&|w| w.height = 0.0),
        base(&|w| w.layers.clear()),
        base(&|w| w.layers[0].thickness = -1.0),
        base(&|w| w.offset = Some(f64::INFINITY)),
        base(&|w| w.storey = Target::Instance(20)),
        base(&|w| w.storey = Target::Added(0)),
        base(&|w| w.global_id = Some(global(10))),
    ];
    for case in cases {
        let (result, codes) = run(src.as_bytes(), std::slice::from_ref(&case));
        assert!(matches!(result, Err(Error::Invalid)), "{case:?}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{case:?}");
    }
    // IFC2X3 で所有履歴がなければ書かない
    let src = sample("IFC2X3", false);
    let (result, codes) = run(
        src.as_bytes(),
        &[wall(300, Target::Instance(10), [0.0, 0.0], [1.0, 0.0])],
    );
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED]);
}

#[test]
fn removes_a_wall_with_its_shape_and_layers_but_keeps_the_materials() {
    let src = sample("IFC4", true);
    let (result, _) = run(
        src.as_bytes(),
        &[wall(300, Target::Instance(10), [0.0, 0.0], [1000.0, 0.0])],
    );
    let added = String::from_utf8(result.unwrap()).unwrap();
    let id = instance_of(&added, 300).unwrap();
    let (result, codes) = run(
        added.as_bytes(),
        &[Edit::Remove {
            element: Target::Instance(id),
        }],
    );
    assert!(codes.is_empty(), "{codes:?}");
    let removed = String::from_utf8(result.unwrap()).unwrap();
    for entity in [
        "IFCWALL",
        "IFCEXTRUDEDAREASOLID",
        "IFCSHAPEREPRESENTATION",
        "IFCMATERIALLAYERSETUSAGE",
        "IFCMATERIALLAYER",
        "IFCRELASSOCIATESMATERIAL",
        "IFCRELCONTAINEDINSPATIALSTRUCTURE",
    ] {
        assert_eq!(count(&removed, entity), 0, "{entity}");
    }
    assert_eq!(count(&removed, "IFCMATERIAL"), 2);
    assert_eq!(count(&removed, "IFCGEOMETRICREPRESENTATIONSUBCONTEXT"), 2);
}

/// 空間 `storey` の 6000×4000 で 2 層（コンクリート 200・仕上げ 50）の床
fn slab(n: u32, storey: Target) -> NewSlab {
    NewSlab {
        storey,
        outline: vec![[0.0, 0.0], [6000.0, 0.0], [6000.0, 4000.0], [0.0, 4000.0]],
        layers: vec![layer("Concrete", 200.0), layer("Finish", 50.0)],
        elevation: None,
        type_of: None,
        name: Some(format!("S{n}")),
        global_id: Some(global(n)),
    }
}

/// 空間 `storey` の原点の、高さ 2800 で 400×600 の、y 軸を向く柱
fn column(n: u32, storey: Target) -> NewColumn {
    NewColumn {
        storey,
        position: [0.0, 0.0],
        base: None,
        height: 2800.0,
        section: NewSection {
            width: 400.0,
            depth: 600.0,
        },
        direction: Some([0.0, 1.0]),
        material: Some("Concrete".to_owned()),
        type_of: None,
        name: Some(format!("C{n}")),
        global_id: Some(global(n)),
    }
}

/// 空間 `storey` の高さ 2800 の、x 軸に沿う長さ 6000 で 300×600 の梁
fn beam(n: u32, storey: Target) -> NewBeam {
    NewBeam {
        storey,
        start: [0.0, 0.0, 2800.0],
        end: [6000.0, 0.0, 2800.0],
        section: NewSection {
            width: 300.0,
            depth: 600.0,
        },
        material: Some("Steel".to_owned()),
        type_of: None,
        name: Some(format!("B{n}")),
        global_id: Some(global(n)),
    }
}

/// 建物と高さ 3000 の階を足し、その階に床・柱・梁を足す変更
fn frame_edits() -> Vec<Edit> {
    let mut edits = wall_edits();
    edits.truncate(2);
    edits.extend([
        Edit::Slab(slab(400, Target::Added(1))),
        Edit::Column(column(401, Target::Added(1))),
        Edit::Beam(beam(402, Target::Added(1))),
    ]);
    edits
}

/// 要素 `n` の `Body` の押し出しの向きと深さ
fn extrusions(bytes: &[u8], n: u32) -> Option<Vec<([f64; 3], f64)>> {
    let file = read(bytes)?;
    let text = String::from_utf8(bytes.to_vec()).ok()?;
    let id = instance_of(&text, n)?;
    let (shape, _) = with(|cx| file.product_shape(id, cx));
    let found = shape
        .ok()?
        .nodes
        .iter()
        .filter_map(|node| with(|cx| file.extruded_area(node, cx)).0.ok())
        .map(|area| (area.definition.direction, area.definition.depth))
        .collect();
    Some(found)
}

fn local(building: &Building, element: ElementRef) -> Option<[[f64; 4]; 3]> {
    let placement = building.components.placement.get(element)?;
    match &placement.local {
        Local::Transform(t) => Some(t.0),
        _ => None,
    }
}

#[test]
fn adds_slabs_columns_and_beams_from_intent_values_in_every_version() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let (result, codes) = run(src.as_bytes(), &frame_edits());
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = result.unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let building = lift(&bytes).unwrap();
        let storey = element(&building, 101).unwrap();
        let [slab, column, beam] = [400, 401, 402].map(|n| element(&building, n).unwrap());
        for (element, name) in [(slab, "IfcSlab"), (column, "IfcColumn"), (beam, "IfcBeam")] {
            let class = building.components.class.get(element).unwrap();
            assert_eq!(class.name, name, "{schema}");
            let parent = building.components.parent.get(element).unwrap();
            let of = parent.of.as_ref().unwrap();
            assert_eq!((of.target, of.kind), (storey, ParentKind::Contained));
        }
        let class = building.components.class.get(slab).unwrap();
        assert_eq!(class.predefined.as_deref(), Some("FLOOR"), "{schema}");
        // 床は上の面から層の厚さの合計だけ下へ押し出し、層は上から下へ並べる
        assert_eq!(
            local(&building, slab),
            Some([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0]
            ])
        );
        assert_eq!(
            extrusions(&bytes, 400).unwrap(),
            [([0.0, 0.0, -1.0], 250.0)]
        );
        let material = building.components.material.get(slab).unwrap();
        assert_eq!(material.kind, MaterialKind::Layers);
        assert_eq!(material.direction.as_deref(), Some("AXIS3"));
        assert_eq!(material.offset.as_ref().map(|m| m.value), Some(0.0));
        let thickness: Vec<_> = material
            .parts
            .iter()
            .map(|part| part.thickness.as_ref().map(|t| t.value))
            .collect();
        assert_eq!(thickness, [Some(200.0), Some(50.0)]);
        // 柱は断面の中心に置き、向きを局所の x 軸にする
        assert_eq!(
            local(&building, column),
            Some([
                [0.0, -1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0]
            ])
        );
        assert_eq!(
            extrusions(&bytes, 401).unwrap(),
            [([0.0, 0.0, 1.0], 2800.0)]
        );
        // 梁は天端の芯の始点に置き、芯に沿って押し出す
        assert_eq!(
            local(&building, beam),
            Some([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 2800.0]
            ])
        );
        assert_eq!(
            extrusions(&bytes, 402).unwrap(),
            [([0.0, 0.0, 1.0], 6000.0)]
        );
        for element in [column, beam] {
            let material = building.components.material.get(element).unwrap();
            assert_eq!(material.kind, MaterialKind::Single, "{schema}");
        }
        // 表現は Body だけで、文脈は使い回す
        assert_eq!(count(&text, "IFCSHAPEREPRESENTATION"), 3);
        assert_eq!(count(&text, "IFCGEOMETRICREPRESENTATIONSUBCONTEXT"), 1);
        assert_eq!(count(&text, "IFCRELASSOCIATESMATERIAL"), 3);
        // 同じ入力と変更なら同じバイト列。文書エンジンでも同じ
        assert_eq!(run(src.as_bytes(), &frame_edits()).0.unwrap(), bytes);
        let (live, _) = with(|cx| IfcLive::open(src.as_bytes(), cx));
        let mut live = live.unwrap();
        assert_eq!(
            apply_like_batch(&mut live, &frame_edits()),
            Some(bytes),
            "{schema}"
        );
    }
}

#[test]
fn slopes_beams_and_closes_slab_outlines() {
    let src = sample("IFC4", true);
    let mut sloped = beam(402, Target::Instance(10));
    sloped.start = [0.0, 0.0, 0.0];
    sloped.end = [3000.0, 0.0, 4000.0];
    let mut closed = slab(400, Target::Instance(10));
    closed.outline.push([0.0, 0.0]);
    closed.elevation = Some(-50.0);
    let (result, codes) = run(src.as_bytes(), &[Edit::Beam(sloped), Edit::Slab(closed)]);
    assert!(codes.is_empty(), "{codes:?}");
    let bytes = result.unwrap();
    let building = lift(&bytes).unwrap();
    // x は芯の向き、y は水平で芯の左、z は上を向く
    let m = local(&building, element(&building, 402).unwrap()).unwrap();
    let expected = [
        [0.6, 0.0, -0.8, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.8, 0.0, 0.6, 0.0],
    ];
    for (row, want) in m.iter().zip(expected) {
        for (got, want) in row.iter().zip(want) {
            assert!((got - want).abs() < 1e-12, "{m:?}");
        }
    }
    assert_eq!(
        extrusions(&bytes, 402).unwrap(),
        [([0.0, 0.0, 1.0], 5000.0)]
    );
    // 閉じる点は除き、4 つの角と始点に戻る点で書く
    let text = String::from_utf8(bytes.clone()).unwrap();
    let slab = element(&building, 400).unwrap();
    assert_eq!(translation(&building, slab), Some([0.0, 0.0, -50.0]));
    let outline = text
        .lines()
        .filter(|line| line.contains("=IFCPOLYLINE("))
        .map(|line| line.matches('#').count() - 1)
        .collect::<Vec<_>>();
    assert_eq!(outline, [5, 5]);
}

#[test]
fn refuses_slabs_columns_and_beams_it_cannot_write_without_changing_anything() {
    let src = sample("IFC4", true);
    let storey = Target::Instance(10);
    let slab_with = |edit: &dyn Fn(&mut NewSlab)| {
        let mut new = slab(400, storey);
        edit(&mut new);
        Edit::Slab(new)
    };
    let column_with = |edit: &dyn Fn(&mut NewColumn)| {
        let mut new = column(401, storey);
        edit(&mut new);
        Edit::Column(new)
    };
    let beam_with = |edit: &dyn Fn(&mut NewBeam)| {
        let mut new = beam(402, storey);
        edit(&mut new);
        Edit::Beam(new)
    };
    let cases = [
        slab_with(&|s| s.outline.truncate(2)),
        slab_with(&|s| s.outline = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]),
        slab_with(&|s| s.outline[1] = s.outline[0]),
        slab_with(&|s| s.outline[2] = [f64::NAN, 0.0]),
        slab_with(&|s| s.layers.clear()),
        slab_with(&|s| s.elevation = Some(f64::INFINITY)),
        slab_with(&|s| s.storey = Target::Instance(20)),
        column_with(&|c| c.height = 0.0),
        column_with(&|c| c.section.width = -1.0),
        column_with(&|c| c.direction = Some([0.0, 0.0])),
        column_with(&|c| c.position = [f64::NAN, 0.0]),
        column_with(&|c| c.global_id = Some(global(10))),
        beam_with(&|b| b.end = [0.0, 0.0, 6000.0]),
        beam_with(&|b| b.start = [0.0, f64::NAN, 0.0]),
        beam_with(&|b| b.section.depth = 0.0),
        beam_with(&|b| b.storey = Target::Added(0)),
    ];
    for case in cases {
        let (result, codes) = run(src.as_bytes(), std::slice::from_ref(&case));
        assert!(matches!(result, Err(Error::Invalid)), "{case:?}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{case:?}");
    }
}

#[test]
fn removes_a_beam_with_its_shape_but_keeps_the_material() {
    let src = sample("IFC4", true);
    let (result, _) = run(
        src.as_bytes(),
        &[Edit::Beam(beam(402, Target::Instance(10)))],
    );
    let added = String::from_utf8(result.unwrap()).unwrap();
    let id = instance_of(&added, 402).unwrap();
    let (result, codes) = run(
        added.as_bytes(),
        &[Edit::Remove {
            element: Target::Instance(id),
        }],
    );
    assert!(codes.is_empty(), "{codes:?}");
    let removed = String::from_utf8(result.unwrap()).unwrap();
    for entity in [
        "IFCBEAM",
        "IFCEXTRUDEDAREASOLID",
        "IFCSHAPEREPRESENTATION",
        "IFCRELASSOCIATESMATERIAL",
        "IFCRELCONTAINEDINSPATIALSTRUCTURE",
    ] {
        assert_eq!(count(&removed, entity), 0, "{entity}");
    }
    assert_eq!(count(&removed, "IFCMATERIAL"), 1);
}

/// 壁 `host` の、芯に沿う `distance` と下端 `sill` の `width`×`height` の開口
fn opening(n: u32, host: Target, distance: f64, sill: f64, size: [f64; 2]) -> Edit {
    Edit::Opening(NewOpening {
        host,
        distance,
        sill,
        width: size[0],
        height: size[1],
        name: Some(format!("O{n}")),
        global_id: Some(global(n)),
    })
}

fn filling(n: u32, opening: Target, kind: FillingKind, size: [f64; 2]) -> Edit {
    Edit::Filling(NewFilling {
        opening,
        kind,
        width: Some(size[0]),
        height: Some(size[1]),
        type_of: None,
        name: Some(format!("F{n}")),
        global_id: Some(global(n)),
    })
}

/// 壁を 2 つ足した階に、扉と窓の開口を足して埋める変更
fn opening_edits() -> Vec<Edit> {
    let mut edits = wall_edits();
    edits.extend([
        opening(500, Target::Added(2), 1000.0, 0.0, [900.0, 2100.0]),
        filling(501, Target::Added(4), FillingKind::Door, [900.0, 2100.0]),
        opening(502, Target::Added(3), 500.0, 900.0, [1200.0, 1200.0]),
        filling(503, Target::Added(6), FillingKind::Window, [1200.0, 1200.0]),
    ]);
    edits
}

#[test]
fn adds_openings_doors_and_windows_to_walls_in_every_version() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let (result, codes) = run(src.as_bytes(), &opening_edits());
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = result.unwrap();
        let building = lift(&bytes).unwrap();
        let storey = element(&building, 101).unwrap();
        let [wall, door_opening, door, window_opening, window] =
            [300, 500, 501, 502, 503].map(|n| element(&building, n).unwrap());
        let opening_of = |element| building.components.opening.get(element);
        assert_eq!(opening_of(door_opening), Some(&Opening::Voids(wall)));
        assert_eq!(opening_of(door), Some(&Opening::Fills(door_opening)));
        assert_eq!(opening_of(window), Some(&Opening::Fills(window_opening)));
        // 開口は壁の配置からの相対で、芯に沿う距離と下端に置く
        let placement = building.components.placement.get(door_opening).unwrap();
        assert_eq!(placement.relative_to, Some(wall));
        assert_eq!(
            translation(&building, door_opening),
            Some([1000.0, 0.0, 0.0])
        );
        assert_eq!(
            translation(&building, window_opening),
            Some([500.0, 0.0, 900.0])
        );
        // 奥行きは壁の層の厚さで、断面を壁の XZ に置き、層の左の面から右の面へ押し出す
        assert_eq!(extrusions(&bytes, 500).unwrap(), [([0.0, 0.0, 1.0], 200.0)]);
        let file = read(&bytes).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let id = instance_of(&text, 500).unwrap();
        let shape = with(|cx| file.product_shape(id, cx)).0.unwrap();
        let positions: Vec<_> = shape
            .nodes
            .iter()
            .filter_map(|node| with(|cx| file.extruded_area(node, cx)).0.ok())
            .filter_map(|area| area.definition.position)
            .map(|p| (p.location, p.axis, p.reference_direction))
            .collect();
        assert_eq!(
            positions,
            [(
                [0.0, 100.0, 0.0],
                Some([0.0, -1.0, 0.0]),
                Some([1.0, 0.0, 0.0])
            )]
        );
        // 建具は開口の配置に置き、壁の層の中央に厚さの 4 分の 1 の Body を持ち、壁と同じ階に包含で結ぶ
        for (filling, opening, n, name, predefined) in [
            (door, door_opening, 501, "IfcDoor", "DOOR"),
            (window, window_opening, 503, "IfcWindow", "WINDOW"),
        ] {
            let class = building.components.class.get(filling).unwrap();
            assert_eq!(class.name, name, "{schema}");
            let expected = (schema != "IFC2X3").then_some(predefined);
            assert_eq!(class.predefined.as_deref(), expected, "{schema}");
            let placement = building.components.placement.get(filling).unwrap();
            assert_eq!(placement.relative_to, Some(opening));
            let parent = building.components.parent.get(filling).unwrap();
            let of = parent.of.as_ref().unwrap();
            assert_eq!((of.target, of.kind), (storey, ParentKind::Contained));
            assert_eq!(solids(&bytes, n).unwrap(), [([0.0, 25.0, 0.0], 50.0)]);
        }
        // 開口は空間に所属しない
        assert!(
            building
                .components
                .parent
                .get(door_opening)
                .is_none_or(|parent| parent.of.is_none()),
            "{schema}"
        );
        // 同じ入力と変更なら同じバイト列。文書エンジンでも同じ
        assert_eq!(run(src.as_bytes(), &opening_edits()).0.unwrap(), bytes);
        let (live, _) = with(|cx| IfcLive::open(src.as_bytes(), cx));
        let mut live = live.unwrap();
        assert_eq!(
            apply_like_batch(&mut live, &opening_edits()),
            Some(bytes),
            "{schema}"
        );
    }
}

#[test]
fn refuses_openings_and_fillings_it_cannot_write_without_changing_anything() {
    let src = sample("IFC4", true);
    let site = Target::Instance(10);
    let with_wall = |rest: Vec<Edit>| {
        let mut edits = vec![wall(300, site, [0.0, 0.0], [4000.0, 0.0])];
        edits.extend(rest);
        edits
    };
    let door = |opening| filling(501, opening, FillingKind::Door, [900.0, 2100.0]);
    let cases = [
        with_wall(vec![opening(
            500,
            Target::Added(0),
            0.0,
            0.0,
            [0.0, 2100.0],
        )]),
        with_wall(vec![opening(
            500,
            Target::Added(0),
            f64::NAN,
            0.0,
            [900.0, 2100.0],
        )]),
        // 空間と、材料の層のない要素には穴をあけない
        with_wall(vec![opening(500, site, 0.0, 0.0, [900.0, 2100.0])]),
        vec![
            new_element("IfcWall", 300, Some(site), None),
            opening(500, Target::Added(0), 0.0, 0.0, [900.0, 2100.0]),
        ],
        // 開口でないものと、埋めた開口は埋めない
        with_wall(vec![door(Target::Added(0))]),
        with_wall(vec![
            opening(500, Target::Added(0), 0.0, 0.0, [900.0, 2100.0]),
            door(Target::Added(1)),
            filling(502, Target::Added(1), FillingKind::Window, [900.0, 900.0]),
        ]),
        with_wall(vec![
            opening(500, Target::Added(0), 0.0, 0.0, [900.0, 2100.0]),
            filling(501, Target::Added(1), FillingKind::Door, [-1.0, 2100.0]),
        ]),
    ];
    for case in cases {
        let (result, codes) = run(src.as_bytes(), &case);
        assert!(matches!(result, Err(Error::Invalid)), "{case:?}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{case:?}");
    }
}

#[test]
fn removing_a_wall_removes_its_openings() {
    let src = sample("IFC4", true);
    let edits = [
        wall(300, Target::Instance(10), [0.0, 0.0], [4000.0, 0.0]),
        opening(500, Target::Added(0), 1000.0, 0.0, [900.0, 2100.0]),
    ];
    let (result, codes) = run(src.as_bytes(), &edits);
    assert!(codes.is_empty(), "{codes:?}");
    let added = String::from_utf8(result.unwrap()).unwrap();
    let id = instance_of(&added, 300).unwrap();
    let (result, codes) = run(
        added.as_bytes(),
        &[Edit::Remove {
            element: Target::Instance(id),
        }],
    );
    assert!(codes.is_empty(), "{codes:?}");
    let removed = String::from_utf8(result.unwrap()).unwrap();
    for entity in [
        "IFCWALL",
        "IFCOPENINGELEMENT",
        "IFCRELVOIDSELEMENT",
        "IFCEXTRUDEDAREASOLID",
    ] {
        assert_eq!(count(&removed, entity), 0, "{entity}");
    }
}

fn axis(tag: &str, start: [f64; 2], end: [f64; 2]) -> NewGridAxis {
    NewGridAxis {
        tag: tag.to_owned(),
        start,
        end,
    }
}

/// 空間 `storey` の、X 方向に A・B、Y 方向に 1・2 の通り芯
fn grid(n: u32, storey: Target) -> NewGrid {
    NewGrid {
        storey,
        u: vec![
            axis("A", [0.0, -1000.0], [0.0, 5000.0]),
            axis("B", [6000.0, -1000.0], [6000.0, 5000.0]),
        ],
        v: vec![
            axis("1", [-1000.0, 0.0], [7000.0, 0.0]),
            axis("2", [-1000.0, 4000.0], [7000.0, 4000.0]),
        ],
        elevation: None,
        name: Some(format!("G{n}")),
        global_id: Some(global(n)),
    }
}

fn grid_edits() -> Vec<Edit> {
    let mut edits = wall_edits();
    edits.truncate(2);
    edits.push(Edit::Grid(grid(600, Target::Added(1))));
    edits
}

#[test]
fn adds_grids_from_tagged_axes_in_every_version() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let (result, codes) = run(src.as_bytes(), &grid_edits());
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = result.unwrap();
        let building = lift(&bytes).unwrap();
        let storey = element(&building, 101).unwrap();
        let grid = element(&building, 600).unwrap();
        let class = building.components.class.get(grid).unwrap();
        assert_eq!(class.name, "IfcGrid", "{schema}");
        let parent = building.components.parent.get(grid).unwrap();
        let of = parent.of.as_ref().unwrap();
        assert_eq!((of.target, of.kind), (storey, ParentKind::Contained));
        assert_eq!(translation(&building, grid), Some([0.0, 0.0, 0.0]));
        // 軸は U・V の順に、名前と同じ向きで読める
        let file = read(&bytes).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let id = instance_of(&text, 600).unwrap();
        let (axes, diag) = with(|cx| file.grid_axes(id, cx));
        assert!(diag.is_empty(), "{schema}");
        let axes: Vec<_> = axes
            .unwrap()
            .axes
            .iter()
            .map(|axis| {
                (
                    axis.definition.set,
                    axis.definition.tag.clone(),
                    axis.definition.same_sense,
                )
            })
            .collect();
        assert_eq!(
            axes,
            [
                (GridSet::U, Some("A".to_owned()), true),
                (GridSet::U, Some("B".to_owned()), true),
                (GridSet::V, Some("1".to_owned()), true),
                (GridSet::V, Some("2".to_owned()), true),
            ],
            "{schema}"
        );
        // 同じ入力と変更なら同じバイト列。文書エンジンでも同じ
        assert_eq!(run(src.as_bytes(), &grid_edits()).0.unwrap(), bytes);
        let (live, _) = with(|cx| IfcLive::open(src.as_bytes(), cx));
        let mut live = live.unwrap();
        assert_eq!(
            apply_like_batch(&mut live, &grid_edits()),
            Some(bytes),
            "{schema}"
        );
    }
}

#[test]
fn refuses_grids_it_cannot_write_and_removes_grids_with_their_axes() {
    let src = sample("IFC4", true);
    let site = Target::Instance(10);
    let grid_with = |edit: &dyn Fn(&mut NewGrid)| {
        let mut new = grid(600, site);
        edit(&mut new);
        Edit::Grid(new)
    };
    let cases = [
        grid_with(&|g| g.v.clear()),
        grid_with(&|g| g.u[1].tag = "1".to_owned()),
        grid_with(&|g| g.u[0].end = g.u[0].start),
        grid_with(&|g| g.v[0].start = [f64::NAN, 0.0]),
        grid_with(&|g| g.elevation = Some(f64::INFINITY)),
        grid_with(&|g| g.storey = Target::Instance(20)),
    ];
    for case in cases {
        let (result, codes) = run(src.as_bytes(), std::slice::from_ref(&case));
        assert!(matches!(result, Err(Error::Invalid)), "{case:?}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{case:?}");
    }
    let (result, codes) = run(src.as_bytes(), &[Edit::Grid(grid(600, site))]);
    assert!(codes.is_empty(), "{codes:?}");
    let added = String::from_utf8(result.unwrap()).unwrap();
    let id = instance_of(&added, 600).unwrap();
    let (result, codes) = run(
        added.as_bytes(),
        &[Edit::Remove {
            element: Target::Instance(id),
        }],
    );
    assert!(codes.is_empty(), "{codes:?}");
    let removed = String::from_utf8(result.unwrap()).unwrap();
    for entity in ["IFCGRID", "IFCGRIDAXIS", "IFCPOLYLINE"] {
        assert_eq!(count(&removed, entity), 0, "{entity}");
    }
}

/// 要素の押し出しの位置と奥行き
fn solids(bytes: &[u8], n: u32) -> Option<Vec<([f64; 3], f64)>> {
    let file = read(bytes)?;
    let text = String::from_utf8(bytes.to_vec()).ok()?;
    let id = instance_of(&text, n)?;
    let (shape, _) = with(|cx| file.product_shape(id, cx));
    let found = shape
        .ok()?
        .nodes
        .iter()
        .filter_map(|node| with(|cx| file.extruded_area(node, cx)).0.ok())
        .filter_map(|area| Some((area.definition.position?.location, area.definition.depth)))
        .collect();
    Some(found)
}

/// 階 101 を高さ 3500 に、壁 300 を長さ 5000・高さ 3000・3 層（厚さ 300）に、壁 301 の開口 502 を動かして
/// 大きさを変える変更。要素はインスタンスの番号で指す
fn change_edits(text: &str) -> Option<Vec<Edit>> {
    let at = |n| instance_of(text, n).map(Target::Instance);
    Some(vec![
        Edit::Elevation {
            storey: at(101)?,
            elevation: 3500.0,
        },
        Edit::WallShape(WallChange {
            wall: at(300)?,
            start: [1000.0, 0.0],
            end: [1000.0, 5000.0],
            height: 3000.0,
            layers: vec![
                layer("Concrete", 200.0),
                layer("Insulation", 50.0),
                layer("Board", 50.0),
            ],
            offset: None,
            type_of: None,
        }),
        Edit::OpeningShape(OpeningChange {
            opening: at(502)?,
            distance: 800.0,
            sill: 1000.0,
            width: 1500.0,
            height: 1000.0,
        }),
    ])
}

#[test]
fn changes_storeys_walls_and_openings_and_dependents_follow_in_every_version() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let before = run(src.as_bytes(), &opening_edits()).0.unwrap();
        let text = String::from_utf8(before.clone()).unwrap();
        let edits = change_edits(&text).unwrap();
        let (result, codes) = run(&before, &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let after = result.unwrap();
        let changed = String::from_utf8(after.clone()).unwrap();
        let building = lift(&after).unwrap();
        let [storey, wall, door_opening, door, window_opening] =
            [101, 300, 500, 501, 502].map(|n| element(&building, n).unwrap());
        // 階の配置は高さの差だけ上がり、属する要素は配置の相対のまま付いてくる
        assert_eq!(
            translation(&building, storey),
            Some([0.0, 0.0, 3500.0]),
            "{schema}"
        );
        let elevation = changed
            .lines()
            .find(|line| line.contains(&gid(101)))
            .unwrap();
        // IFC4.3 は Elevation を非推奨にしたので、書いていない値を足さない
        let written = if schema == "IFC4X3_ADD2" {
            ",$);"
        } else {
            ",3500.);"
        };
        assert!(elevation.ends_with(written), "{schema}: {elevation}");
        let level = building.components.level.get(storey).unwrap();
        assert_eq!(level.height.value, 3500.0, "{schema}");
        // 壁は新しい芯の向きに置き、高さと層の厚さで押し出す
        let t = local(&building, wall).unwrap();
        assert_eq!(
            [t[0][0], t[1][0], t[0][3], t[1][3]],
            [0.0, 1.0, 1000.0, 0.0]
        );
        assert_eq!(solids(&after, 300).unwrap(), [([0.0, 0.0, 0.0], 3000.0)]);
        // 同じ名前の材料は元の IfcMaterial を使い、新しい名前だけ足す
        assert_eq!(count(&changed, "IFCMATERIAL"), 5, "{schema}");
        assert_eq!(count(&changed, "IFCMATERIALLAYER"), 5, "{schema}");
        assert_eq!(count(&changed, "IFCMATERIALLAYERSETUSAGE"), 2, "{schema}");
        // 壁の開口は新しい層の範囲を貫き、扉は開口の配置のまま厚み 75（300 の 4 分の 1）に追従する
        assert_eq!(
            solids(&after, 500).unwrap(),
            [([0.0, 150.0, 0.0], 300.0)],
            "{schema}"
        );
        assert_eq!(
            solids(&after, 501).unwrap(),
            [([0.0, 37.5, 0.0], 75.0)],
            "{schema}"
        );
        assert_eq!(
            translation(&building, door_opening),
            Some([1000.0, 0.0, 0.0])
        );
        let placement = building.components.placement.get(door).unwrap();
        assert_eq!(placement.relative_to, Some(door_opening));
        // 動かした開口は、壁 301 の層の範囲のまま新しい大きさで押し出し、窓の Body と全体の高さ・幅も合わせる
        assert_eq!(
            translation(&building, window_opening),
            Some([800.0, 0.0, 1000.0])
        );
        assert_eq!(solids(&after, 502).unwrap(), [([0.0, 100.0, 0.0], 200.0)]);
        assert_eq!(solids(&after, 503).unwrap(), [([0.0, 25.0, 0.0], 50.0)]);
        let window = changed
            .lines()
            .find(|line| line.contains(&gid(503)))
            .unwrap();
        assert!(window.contains(",1000.,1500."), "{schema}: {window}");
        // 古い配置と表現は残らない
        for entity in [
            "IFCLOCALPLACEMENT",
            "IFCAXIS2PLACEMENT3D",
            "IFCSHAPEREPRESENTATION",
            "IFCEXTRUDEDAREASOLID",
            "IFCPRODUCTDEFINITIONSHAPE",
        ] {
            assert_eq!(
                count(&changed, entity),
                count(&text, entity),
                "{schema}: {entity}"
            );
        }
        // 同じ入力と変更なら同じバイト列。文書エンジンでも同じで、取り消すと元に、やり直すと直した値に戻る
        assert_eq!(run(&before, &edits).0.unwrap(), after);
        let (live, _) = with(|cx| IfcLive::open(&before, cx));
        let mut live = live.unwrap();
        assert_eq!(
            apply_like_batch(&mut live, &edits),
            Some(after.clone()),
            "{schema}"
        );
        assert_eq!(with(|cx| live.undo(cx)).0.ok(), Some(true));
        assert_eq!(same_as_reopened(&live), Some(before.clone()), "{schema}");
        assert_eq!(with(|cx| live.redo(cx)).0.ok(), Some(true));
        assert_eq!(same_as_reopened(&live), Some(after), "{schema}");
    }
}

/// 形まで読んだ文書を、床の高さ `floor` から 1200 の水平面で切った壁 `n` の切断線と、開口を引いた体積（m³）
#[cfg(all(feature = "drawing", feature = "measure"))]
fn cut_and_volume(bytes: &[u8], n: u32, floor: f64) -> Option<(Vec<[[f64; 2]; 2]>, f64)> {
    use zumen_building::drawing::{self, Paper, Role, View};
    use zumen_building::measure::{self, LengthUnit, Rounding, RoundingMode, RoundingStage, Unit};
    use zumen_crs::ProjectPoint;
    use zumen_drawing::{PaperLength, Shape};
    use zumen_geom3d::Tolerance;
    let file = read(bytes)?;
    let building = with(|cx| file.lift_geometry(cx)).0.ok()?;
    let wall = element(&building, n)?;
    let options = drawing::Options {
        view: View::Plan {
            floor: ProjectPoint {
                x: 0.0,
                y: 0.0,
                z: floor,
            },
            height: 1200.0,
            depth: 2000.0,
        },
        tolerance: 0.01,
        chord: 1.0,
        paper: Paper {
            denominator: 100.0,
            origin: [PaperLength(0.0); 2],
            size: None,
        },
        symbols: &[],
    };
    let out = with(|cx| drawing::cut(&building, options, cx)).0.ok()?;
    let mut lines = Vec::new();
    for line in &out.lines {
        if out.sources.get(line.source)?.element != wall || line.role != Role::Cut {
            continue;
        }
        let Shape::Line { start, end } =
            &out.drawing.spaces.first()?.elements.get(line.index)?.shape
        else {
            return None;
        };
        lines.push([[start.x, start.y], [end.x, end.y]]);
    }
    let options = measure::Options {
        unit: Unit::Volume(LengthUnit::Meter),
        rounding: Rounding {
            mode: RoundingMode::HalfAway,
            digits: 3,
            stage: RoundingStage::Total,
        },
        tolerance: Tolerance {
            length: 1e-6,
            chord: 0.001,
        },
        profile: None,
    };
    let report = with(|cx| measure::volumes(&building, &[wall], true, options, cx))
        .0
        .ok()?;
    Some((lines, report.raw_total?))
}

/// 切断線の端点の x と y の範囲
#[cfg(all(feature = "drawing", feature = "measure"))]
fn extent(lines: &[[[f64; 2]; 2]]) -> [f64; 4] {
    let points = lines.iter().flatten();
    let fold = |pick: fn(&[f64; 2]) -> f64, max: bool| {
        points.clone().map(pick).fold(
            if max {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            },
            |a, b| if max { a.max(b) } else { a.min(b) },
        )
    };
    [
        fold(|p| p[0], false),
        fold(|p| p[0], true),
        fold(|p| p[1], false),
        fold(|p| p[1], true),
    ]
}

#[cfg(all(feature = "drawing", feature = "measure"))]
fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * b.abs().max(1.0)
}

#[test]
#[cfg(all(feature = "drawing", feature = "measure"))]
fn the_cut_and_quantities_follow_changed_walls_and_openings() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let before = run(src.as_bytes(), &opening_edits()).0.unwrap();
        let text = String::from_utf8(before.clone()).unwrap();
        let after = run(&before, &change_edits(&text).unwrap()).0.unwrap();
        // 直す前: 壁 300 は x 900〜1100、y 0〜4000。扉の開口（y 1000〜1900）で線が切れる
        let (lines, volume) = cut_and_volume(&before, 300, 3000.0).unwrap();
        assert_eq!(extent(&lines), [900.0, 1100.0, 0.0, 4000.0], "{schema}");
        assert!(
            near(volume, (4.0 * 2.8 - 0.9 * 2.1) * 0.2),
            "{schema}: {volume}"
        );
        // 直した後: 階の床は 3500 に上がり、壁は厚さ 300 で y 5000 まで伸びる。扉の開口は付いてきて、
        // 新しい厚さを貫く
        let (lines, volume) = cut_and_volume(&after, 300, 3500.0).unwrap();
        assert_eq!(extent(&lines), [850.0, 1150.0, 0.0, 5000.0], "{schema}");
        for [a, b] in &lines {
            assert!(
                !(a[1].min(b[1]) < 1450.0 && 1450.0 < a[1].max(b[1])),
                "{schema}: {a:?} {b:?}"
            );
        }
        assert!(
            near(volume, (5.0 * 3.0 - 0.9 * 2.1) * 0.3),
            "{schema}: {volume}"
        );
        // 窓の開口は幅 1500・高さ 1000 になり、壁 301 の体積から引く量が変わる。開口は芯に沿って
        // 800〜2300（x 200〜-1300）に移り、x -1000 の線は切れる
        let crosses = |lines: &[[[f64; 2]; 2]], x: f64| {
            lines
                .iter()
                .any(|[a, b]| a[0].min(b[0]) < x && x < a[0].max(b[0]))
        };
        let (lines, volume) = cut_and_volume(&before, 301, 3000.0).unwrap();
        assert!(
            near(volume, (3.0 * 2.8 - 1.2 * 1.2) * 0.2),
            "{schema}: {volume}"
        );
        assert!(crosses(&lines, -1000.0), "{schema}");
        let (lines, volume) = cut_and_volume(&after, 301, 3500.0).unwrap();
        assert!(
            near(volume, (3.0 * 2.8 - 1.5 * 1.0) * 0.2),
            "{schema}: {volume}"
        );
        assert!(!crosses(&lines, -1000.0), "{schema}");
        // 扉 501 と窓 503 の切断線と体積も、壁の厚さと開口の大きさに追従する
        let (door_before_lines, door_before_vol) = cut_and_volume(&before, 501, 3000.0).unwrap();
        assert_eq!(
            extent(&door_before_lines),
            [975.0, 1025.0, 1000.0, 1900.0],
            "{schema}"
        );
        assert!(near(door_before_vol, 0.9 * 2.1 * 0.05), "{schema}");
        let (door_after_lines, door_after_vol) = cut_and_volume(&after, 501, 3500.0).unwrap();
        assert_eq!(
            extent(&door_after_lines),
            [962.5, 1037.5, 1000.0, 1900.0],
            "{schema}"
        );
        assert!(near(door_after_vol, 0.9 * 2.1 * 0.075), "{schema}");
        let (win_before_lines, win_before_vol) = cut_and_volume(&before, 503, 3000.0).unwrap();
        assert_eq!(
            extent(&win_before_lines),
            [-700.0, 500.0, 3975.0, 4025.0],
            "{schema}"
        );
        assert!(near(win_before_vol, 1.2 * 1.2 * 0.05), "{schema}");
        let (win_after_lines, win_after_vol) = cut_and_volume(&after, 503, 3500.0).unwrap();
        assert_eq!(
            extent(&win_after_lines),
            [-1300.0, 200.0, 3975.0, 4025.0],
            "{schema}"
        );
        assert!(near(win_after_vol, 1.5 * 1.0 * 0.05), "{schema}");
    }
}

#[test]
fn refuses_changes_it_cannot_write_without_changing_anything() {
    let src = sample("IFC4", true);
    let before = run(src.as_bytes(), &opening_edits()).0.unwrap();
    let text = String::from_utf8(before.clone()).unwrap();
    let at = |n| Target::Instance(instance_of(&text, n).unwrap());
    let wall_change = |wall, end: [f64; 2], thickness: f64| {
        Edit::WallShape(WallChange {
            wall,
            start: [1000.0, 0.0],
            end,
            height: 3000.0,
            layers: vec![layer("Concrete", thickness)],
            offset: None,
            type_of: None,
        })
    };
    let opening_change = |opening, width: f64| {
        Edit::OpeningShape(OpeningChange {
            opening,
            distance: 0.0,
            sill: 0.0,
            width,
            height: 1000.0,
        })
    };
    let cases = [
        // 階でないもの、有限でない高さ
        Edit::Elevation {
            storey: at(300),
            elevation: 0.0,
        },
        Edit::Elevation {
            storey: at(101),
            elevation: f64::NAN,
        },
        // 壁でないもの、長さのない芯、厚さのない層
        wall_change(at(500), [1000.0, 5000.0], 200.0),
        wall_change(at(300), [1000.0, 0.0], 200.0),
        wall_change(at(300), [1000.0, 5000.0], 0.0),
        // 開口でないもの、幅のない開口
        opening_change(at(300), 900.0),
        opening_change(at(500), 0.0),
    ];
    for case in cases {
        let (result, codes) = run(&before, std::slice::from_ref(&case));
        assert!(matches!(result, Err(Error::Invalid)), "{case:?}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{case:?}");
    }
}

/// 建物・1F（高さ 0）・2F（高さ 3000）・1F の通り芯を足し、各階に 6000×4000 を囲む 4 つの壁・床・柱・梁と、
/// 1F に扉、2F に窓を足す変更。番号は階ごとに 1000 ずつずらす
fn two_storey_edits() -> Vec<Edit> {
    let storey = |n: u32, elevation: f64| {
        Edit::Space(NewSpace {
            kind: SpaceKind::Storey,
            name: Some(format!("{}F", n - 1000)),
            parent: Target::Added(0),
            placement: Some(at(0.0, 0.0, elevation, Target::Added(0))),
            elevation: Some(elevation),
            global_id: Some(global(n)),
        })
    };
    let mut edits = vec![
        space(SpaceKind::Building, 1000, Target::Instance(10), 0.0),
        storey(1001, 0.0),
        storey(1002, 3000.0),
        Edit::Grid(grid(1003, Target::Added(1))),
    ];
    let corners = [[0.0, 0.0], [6000.0, 0.0], [6000.0, 4000.0], [0.0, 4000.0]];
    for (level, at) in [(1u32, 1usize), (2, 2)] {
        let base = 1000 * (level + 1);
        let on = Target::Added(at);
        let first = edits.len();
        for (i, from) in corners.iter().enumerate() {
            let to = corners
                .get((i + 1) % corners.len())
                .copied()
                .unwrap_or_default();
            edits.push(wall(base + i as u32, on, *from, to));
        }
        edits.extend([
            Edit::Slab(slab(base + 10, on)),
            Edit::Column(column(base + 11, on)),
            Edit::Beam(beam(base + 12, on)),
        ]);
        let (kind, sill, size) = if level == 1 {
            (FillingKind::Door, 0.0, [900.0, 2100.0])
        } else {
            (FillingKind::Window, 900.0, [1200.0, 1200.0])
        };
        edits.push(opening(base + 20, Target::Added(first), 1000.0, sill, size));
        edits.push(filling(
            base + 21,
            Target::Added(edits.len() - 1),
            kind,
            size,
        ));
    }
    edits
}

#[test]
fn builds_a_two_storey_building_from_edits_only_in_every_version() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let edits = two_storey_edits();
        let (result, codes) = run(src.as_bytes(), &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let built = result.unwrap();
        // 種類ごとの要素の数
        let building = lift(&built).unwrap();
        let mut classes = std::collections::BTreeMap::<String, usize>::new();
        for (_, class) in building.components.class.iter() {
            let name = class.name.replace("StandardCase", "");
            *classes.entry(name).or_default() += 1;
        }
        let expected = [
            ("IfcBeam", 2),
            ("IfcBuilding", 1),
            ("IfcBuildingStorey", 2),
            ("IfcColumn", 2),
            ("IfcDoor", 1),
            ("IfcGrid", 1),
            ("IfcOpeningElement", 2),
            ("IfcSlab", 2),
            ("IfcWall", 8),
            ("IfcWindow", 1),
        ];
        for (class, n) in expected {
            assert_eq!(
                classes.get(class),
                Some(&n),
                "{schema}: {class} {classes:?}"
            );
        }
        // 2F は高さ 3000 に置き、2F の要素は 2F の配置からの相対
        let second = element(&building, 1002).unwrap();
        assert_eq!(translation(&building, second), Some([0.0, 0.0, 3000.0]));
        for n in [3000, 3010, 3011, 3012] {
            let placement = building
                .components
                .placement
                .get(element(&building, n).unwrap())
                .unwrap();
            assert_eq!(placement.relative_to, Some(second), "{schema}: {n}");
        }
        // 形まで読める。形を持たないのは空間と通り芯だけで、建具を含む全建築要素が Body を持ち、ほかの診断は出ない
        let file = read(&built).unwrap();
        let (shaped, diag) = with(|cx| file.lift_geometry(cx));
        assert!(
            diag.iter().all(|d| d.code() == codes::IFC_SHAPE_MISSING),
            "{schema}: {:?}",
            diag.iter().collect::<Vec<_>>()
        );
        let shaped = shaped.unwrap();
        for n in [2000, 2010, 2011, 2012, 2020, 2021, 3000, 3020, 3021] {
            let at = element(&shaped, n).unwrap();
            assert!(shaped.geometry(at).is_some(), "{schema}: {n}");
        }
        // 同じ変更なら同じバイト列。文書エンジンでも同じ
        assert_eq!(run(src.as_bytes(), &edits).0.unwrap(), built);
        let (live, _) = with(|cx| IfcLive::open(src.as_bytes(), cx));
        let mut live = live.unwrap();
        assert_eq!(apply_like_batch(&mut live, &edits), Some(built), "{schema}");
    }
}

/// 2 階建てを足し、階の高さ・壁・開口を直す変更。2F は 3200 に、1F の最初の壁は 3 層で長さ 6500 に、2F の窓の開口は
/// 芯に沿う 2000 と下端 1000 の 1600×1000 にする
fn two_storey_change_edits() -> Option<Vec<Edit>> {
    let mut edits = two_storey_edits();
    let window = edits
        .iter()
        .rposition(|edit| matches!(edit, Edit::Opening(_)))?;
    let wall = edits
        .iter()
        .position(|edit| matches!(edit, Edit::Wall(_)))?;
    edits.extend([
        Edit::Elevation {
            storey: Target::Added(2),
            elevation: 3200.0,
        },
        Edit::WallShape(WallChange {
            wall: Target::Added(wall),
            start: [0.0, 0.0],
            end: [6500.0, 0.0],
            height: 3000.0,
            layers: vec![
                layer("Concrete", 200.0),
                layer("Insulation", 50.0),
                layer("Board", 20.0),
            ],
            offset: None,
            type_of: None,
        }),
        Edit::OpeningShape(OpeningChange {
            opening: Target::Added(window),
            distance: 2000.0,
            sill: 1000.0,
            width: 1600.0,
            height: 1000.0,
        }),
    ]);
    Some(edits)
}

#[test]
#[cfg(all(feature = "drawing", feature = "measure"))]
fn two_storey_live_edits_follow_section_plan_quantities_and_undo_redo() {
    use zumen_building::drawing::{self, Paper, Role, Side, View};
    use zumen_crs::ProjectPoint;
    use zumen_drawing::{PaperLength, Shape};
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let (live, _) = with(|cx| IfcLive::open(src.as_bytes(), cx));
        let mut live = live.unwrap();
        let build_edits = two_storey_edits();
        with(|cx| live.apply(&build_edits, b"seed", cx)).0.unwrap();
        let built = with(|cx| live.save_spf(cx)).0.unwrap();
        let built_text = String::from_utf8(built.clone()).unwrap();
        let at = |n| Target::Instance(instance_of(&built_text, n).unwrap());
        // 2F の依存辺が 2F の壁・開口・窓を含むことを確かめる
        let find = |engine: &IfcLive, n: u32| element(engine.building(), n).unwrap();
        let affected_2f = with(|cx| live.affected_elements(&[find(&live, 1002)], cx))
            .0
            .unwrap();
        for n in [3000, 3010, 3011, 3012, 3020, 3021] {
            assert!(affected_2f.contains(&find(&live, n)), "{schema}: {n}");
        }
        // 階高・壁厚・開口を文書エンジンで直す
        let follow_edits = [
            Edit::Elevation {
                storey: at(1002),
                elevation: 3200.0,
            },
            Edit::WallShape(WallChange {
                wall: at(2000),
                start: [0.0, 0.0],
                end: [6500.0, 0.0],
                height: 3000.0,
                layers: vec![
                    layer("Concrete", 200.0),
                    layer("Insulation", 50.0),
                    layer("Board", 20.0),
                ],
                offset: None,
                type_of: None,
            }),
            Edit::OpeningShape(OpeningChange {
                opening: at(3020),
                distance: 2000.0,
                sill: 1000.0,
                width: 1600.0,
                height: 1000.0,
            }),
        ];
        let expected_changed = run(&built, &follow_edits).0.unwrap();
        with(|cx| live.apply(&follow_edits, b"seed", cx)).0.unwrap();
        let changed = with(|cx| live.save_spf(cx)).0.unwrap();
        assert_eq!(changed, expected_changed, "{schema}");

        // 断面図（x = 2500 で Y 方向に切る鉛直断面）で 2F の窓 3021 の高さが 3000+900 から 3200+1000 へ追従する
        let section_y_extent = |bytes: &[u8], target_gid: u32| -> [f64; 4] {
            let b = lift_geometry(bytes).unwrap();
            let target = element(&b, target_gid).unwrap();
            let opts = drawing::Options {
                view: View::Vertical {
                    start: ProjectPoint {
                        x: 2500.0,
                        y: -1000.0,
                        z: 0.0,
                    },
                    end: ProjectPoint {
                        x: 2500.0,
                        y: 5000.0,
                        z: 0.0,
                    },
                    side: Side::Right,
                    depth: 1000.0,
                    section: true,
                },
                tolerance: 0.01,
                chord: 1.0,
                paper: Paper {
                    denominator: 100.0,
                    origin: [PaperLength(0.0); 2],
                    size: None,
                },
                symbols: &[],
            };
            let cut = with(|cx| drawing::cut(&b, opts, cx)).0.unwrap();
            let segs: Vec<_> = cut
                .lines
                .iter()
                .filter(|l| {
                    cut.sources
                        .get(l.source)
                        .is_some_and(|s| s.element == target)
                        && l.role == Role::Cut
                })
                .filter_map(
                    |l| match cut.drawing.spaces.first()?.elements.get(l.index)?.shape {
                        Shape::Line { start, end } => Some([[start.x, start.y], [end.x, end.y]]),
                        _ => None,
                    },
                )
                .collect();
            extent(&segs)
        };
        // x = 2500 は直す前は窓（x 1000〜2200）の外で壁を切るが、直した後は窓（x 2000〜3600、z 4200〜5200）を切る
        let win_sec = section_y_extent(&changed, 3021);
        assert_eq!(win_sec, [975.0, 1025.0, 4200.0, 5200.0], "{schema}");
        let (_, win_vol) = cut_and_volume(&changed, 3021, 3200.0).unwrap();
        assert!(near(win_vol, 1.6 * 1.0 * 0.05), "{schema}: {win_vol}");

        // 2 段の undo で元のバイト列へ一字一句戻り、2 段の redo で変更後のバイト列へ戻る
        assert_eq!(with(|cx| live.undo(cx)).0.ok(), Some(true));
        assert_eq!(same_as_reopened(&live), Some(built.clone()), "{schema}");
        assert_eq!(with(|cx| live.undo(cx)).0.ok(), Some(true));
        assert_eq!(
            same_as_reopened(&live),
            Some(src.as_bytes().to_vec()),
            "{schema}"
        );
        assert_eq!(with(|cx| live.redo(cx)).0.ok(), Some(true));
        assert_eq!(same_as_reopened(&live), Some(built), "{schema}");
        assert_eq!(with(|cx| live.redo(cx)).0.ok(), Some(true));
        assert_eq!(same_as_reopened(&live), Some(changed), "{schema}");
    }
}

#[test]
fn adds_and_reshapes_elements_from_types_in_every_version() {
    for schema in SCHEMAS {
        let later = schema != "IFC2X3";
        let src = sample(schema, true);
        let mut edits = vec![
            space(SpaceKind::Building, 100, Target::Instance(10), 0.0),
            space(SpaceKind::Storey, 101, Target::Added(0), 0.0),
            Edit::ElementType(NewType {
                class: "IfcWallType".to_owned(),
                predefined: Some("SHEAR".to_owned()),
                element_type: None,
                name: Some("WT-200".to_owned()),
                global_id: Some(global(700)),
            }),
            Edit::Material {
                element: Target::Added(2),
                material: NewMaterial::Layers {
                    layers: vec![
                        material_layer(Some("Concrete"), 160.0, None),
                        material_layer(Some("Insulation"), 40.0, None),
                    ],
                    usage: None,
                },
            },
            Edit::ElementType(NewType {
                class: "IfcWallType".to_owned(),
                predefined: Some("SHEAR".to_owned()),
                element_type: None,
                name: Some("WT-300".to_owned()),
                global_id: Some(global(701)),
            }),
            Edit::Material {
                element: Target::Added(3),
                material: NewMaterial::Layers {
                    layers: vec![material_layer(Some("Concrete"), 300.0, None)],
                    usage: None,
                },
            },
            Edit::ElementType(NewType {
                class: "IfcSlabType".to_owned(),
                predefined: Some("FLOOR".to_owned()),
                element_type: None,
                name: Some("ST-220".to_owned()),
                global_id: Some(global(702)),
            }),
            Edit::Material {
                element: Target::Added(4),
                material: NewMaterial::Layers {
                    layers: vec![material_layer(Some("Concrete"), 220.0, None)],
                    usage: None,
                },
            },
            Edit::ElementType(NewType {
                class: "IfcColumnType".to_owned(),
                predefined: Some("COLUMN".to_owned()),
                element_type: None,
                name: Some("CT-1".to_owned()),
                global_id: Some(global(703)),
            }),
            Edit::ElementType(NewType {
                class: "IfcBeamType".to_owned(),
                predefined: Some("BEAM".to_owned()),
                element_type: None,
                name: Some("BT-1".to_owned()),
                global_id: Some(global(704)),
            }),
            // 層を省き、型 WT-200 の IfcMaterialLayerSet を共有する壁
            Edit::Wall(NewWall {
                storey: Target::Added(1),
                start: [0.0, 0.0],
                end: [4000.0, 0.0],
                height: 2800.0,
                layers: Vec::new(),
                offset: None,
                type_of: Some(Target::Added(2)),
                name: Some("W-Typed".to_owned()),
                global_id: Some(global(710)),
            }),
            Edit::Slab(NewSlab {
                storey: Target::Added(1),
                outline: vec![[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0], [0.0, 3000.0]],
                layers: Vec::new(),
                elevation: None,
                type_of: Some(Target::Added(4)),
                name: Some("S-Typed".to_owned()),
                global_id: Some(global(711)),
            }),
            Edit::Column({
                let mut c = column(712, Target::Added(1));
                c.type_of = Some(Target::Added(5));
                c
            }),
            Edit::Beam({
                let mut b = beam(713, Target::Added(1));
                b.type_of = Some(Target::Added(6));
                b
            }),
            opening(714, Target::Added(7), 1000.0, 0.0, [900.0, 2100.0]),
        ];
        if later {
            edits.push(Edit::ElementType(NewType {
                class: "IfcDoorType".to_owned(),
                predefined: Some("DOOR".to_owned()),
                element_type: None,
                name: Some("DT-1".to_owned()),
                global_id: Some(global(705)),
            }));
        }
        edits.push(Edit::Filling(NewFilling {
            opening: Target::Added(11),
            kind: FillingKind::Door,
            width: None,
            height: None,
            type_of: later.then_some(Target::Added(12)),
            name: Some("D-Typed".to_owned()),
            global_id: Some(global(715)),
        }));
        let (built, codes) = run(src.as_bytes(), &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let built = built.unwrap();
        let text = String::from_utf8(built.clone()).unwrap();
        // 3 つの型の層セットだけで、壁と床は同じ層セットを Usage で参照する
        assert_eq!(count(&text, "IFCMATERIALLAYERSET"), 3, "{schema}");
        assert_eq!(solids(&built, 714).unwrap(), [([0.0, 100.0, 0.0], 200.0)]);
        assert_eq!(solids(&built, 715).unwrap(), [([0.0, 25.0, 0.0], 50.0)]);
        // 扉に DT-1（PredefinedType = DOOR）を結んだ IFC4/4X3 では、OJT001 に従い実例の PredefinedType を空にする
        if later {
            let door_line = text.lines().find(|l| l.contains(&gid(715))).unwrap();
            assert!(door_line.ends_with(",$,$,$,$,$);"), "{schema}: {door_line}");
        }
        // 壁を WT-300（厚さ 300）に付け替えると、開口と扉の奥行きも 300・75 に追従し、WT-200 の層セットは残る
        let reshape = [Edit::WallShape(WallChange {
            wall: Target::Instance(instance_of(&text, 710).unwrap()),
            start: [0.0, 0.0],
            end: [4000.0, 0.0],
            height: 2800.0,
            layers: Vec::new(),
            offset: None,
            type_of: Some(Target::Instance(instance_of(&text, 701).unwrap())),
        })];
        let (after, codes) = run(&built, &reshape);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let after = after.unwrap();
        let after_text = String::from_utf8(after.clone()).unwrap();
        assert_eq!(count(&after_text, "IFCMATERIALLAYERSET"), 3, "{schema}");
        assert_eq!(solids(&after, 714).unwrap(), [([0.0, 150.0, 0.0], 300.0)]);
        assert_eq!(solids(&after, 715).unwrap(), [([0.0, 37.5, 0.0], 75.0)]);
        let lifted = lift(&after).unwrap();
        assert_eq!(
            lifted
                .components
                .r#type
                .get(element(&lifted, 710).unwrap())
                .map(|t| t.target),
            element(&lifted, 701),
            "{schema}"
        );
    }
}

/// 所有履歴の作り手と使い手を書いた、プロジェクトと敷地だけの文書。ほかの読み手の検査で、入力の誤りを出さない
fn complete_sample(schema: &str) -> String {
    let lines = [
        format!("#1=IFCPROJECT('{}',#2,'Project',$,$,$,$,$,#3);", gid(1)),
        "#2=IFCOWNERHISTORY(#5,#8,$,.NOCHANGE.,$,$,$,0);".to_owned(),
        "#3=IFCUNITASSIGNMENT((#4));".to_owned(),
        "#4=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);".to_owned(),
        "#5=IFCPERSONANDORGANIZATION(#6,#7,$);".to_owned(),
        "#6=IFCPERSON($,'Author',$,$,$,$,$,$);".to_owned(),
        "#7=IFCORGANIZATION($,'Organization',$,$,$);".to_owned(),
        "#8=IFCAPPLICATION(#7,'0','Application','Application');".to_owned(),
        format!(
            "#10=IFCSITE('{}',#2,'Site',$,$,#11,$,$,.ELEMENT.,$,$,$,$,$);",
            gid(10)
        ),
        "#11=IFCLOCALPLACEMENT($,#12);".to_owned(),
        "#12=IFCAXIS2PLACEMENT3D(#13,$,$);".to_owned(),
        "#13=IFCCARTESIANPOINT((0.,0.,0.));".to_owned(),
        format!("#14=IFCRELAGGREGATES('{}',#2,$,$,#1,(#10));", gid(14)),
    ];
    format!(
        "ISO-10303-21;
HEADER;
FILE_DESCRIPTION((''),'2;1');
FILE_NAME('a.ifc','',(''),(''),'','','');
FILE_SCHEMA(('{schema}'));
ENDSEC;
DATA;
{}
ENDSEC;
END-ISO-10303-21;
",
        lines.join(
            "
"
        )
    )
}

/// ひな形から空間（敷地・建物・階）・通り芯・型・要素・関係を足し、置き直し・所属変更・型の付け外し・除去まで当てた IFC
fn structural_authored_bytes(schema: &str) -> Option<Vec<u8>> {
    let version = match schema {
        "IFC2X3" => Version::Ifc2x3,
        "IFC4" => Version::Ifc4,
        "IFC4X3_ADD2" => Version::Ifc4x3,
        _ => return None,
    };
    let template = Template {
        version,
        unit: LengthUnit::Millimeter,
        project: global(1),
        name: Some("構造編集".to_owned()),
        header: Header::default(),
        owner: Some(OwnerHistory {
            person: "Author".to_owned(),
            organization: "Organization".to_owned(),
            developer: "zumen".to_owned(),
            application: "zumen".to_owned(),
            version: "0.3.0".to_owned(),
            identifier: "zumen".to_owned(),
            created: 1_700_000_000,
        }),
    };
    let (base, diag) = with(|cx| create(&template, cx));
    if !diag.is_empty() {
        return None;
    }
    let base = base.ok()?;
    let later = schema != "IFC2X3";
    let elem = |class: &str, predefined: Option<&str>, n: u32, container, placement| {
        Edit::Element(NewElement {
            class: class.to_owned(),
            predefined: predefined.map(str::to_owned),
            name: Some(format!("E{n}")),
            container,
            placement,
            global_id: Some(global(n)),
        })
    };
    let step1 = vec![
        Edit::Space(NewSpace {
            kind: SpaceKind::Site,
            name: Some("Site".to_owned()),
            parent: Target::Instance(1),
            placement: Some(NewPlacement {
                relative_to: None,
                local: Transform::translation(100.0, 200.0, 0.0),
            }),
            elevation: Some(12000.0),
            global_id: Some(global(10)),
        }),
        space(SpaceKind::Building, 100, Target::Added(0), 0.0),
        space(SpaceKind::Storey, 101, Target::Added(1), 0.0),
        Edit::Space(NewSpace {
            kind: SpaceKind::Storey,
            name: Some("S102".to_owned()),
            parent: Target::Added(1),
            placement: Some(at(0.0, 0.0, 3000.0, Target::Added(1))),
            elevation: Some(3000.0),
            global_id: Some(global(102)),
        }),
        Edit::Grid(grid(103, Target::Added(2))),
        Edit::ElementType(NewType {
            class: "IfcWallType".to_owned(),
            predefined: Some("SHEAR".to_owned()),
            element_type: None,
            name: Some("WT-1".to_owned()),
            global_id: Some(global(110)),
        }),
        Edit::ElementType(NewType {
            class: "IfcColumnType".to_owned(),
            predefined: Some("USERDEFINED".to_owned()),
            element_type: Some("C-600".to_owned()),
            name: Some("CT-1".to_owned()),
            global_id: Some(global(111)),
        }),
        // 型（WT-1）の PredefinedType が NOTDEFINED でない実例（W-1）は、規則 OJT001 に従い自身の PredefinedType を空にする
        elem(
            "IfcWall",
            None,
            200,
            Some(Target::Added(2)),
            Some(at(1000.0, 0.0, 0.0, Target::Added(2))),
        ),
        elem(
            "IfcOpeningElement",
            later.then_some("OPENING"),
            201,
            None,
            Some(at(500.0, 0.0, 0.0, Target::Added(7))),
        ),
        elem(
            "IfcDoor",
            later.then_some("DOOR"),
            202,
            Some(Target::Added(2)),
            Some(at(0.0, 0.0, 0.0, Target::Added(8))),
        ),
        elem(
            "IfcColumn",
            later.then_some("COLUMN"),
            203,
            Some(Target::Added(2)),
            Some(at(0.0, 4000.0, 0.0, Target::Added(2))),
        ),
        elem(
            "IfcSlab",
            Some("FLOOR"),
            299,
            Some(Target::Added(2)),
            Some(at(0.0, 0.0, 0.0, Target::Added(2))),
        ),
        Edit::Void {
            host: Target::Added(7),
            opening: Target::Added(8),
        },
        Edit::Fill {
            opening: Target::Added(8),
            filler: Target::Added(9),
        },
        Edit::Type {
            element: Target::Added(7),
            type_of: Some(Target::Added(5)),
        },
        Edit::Type {
            element: Target::Added(10),
            type_of: Some(Target::Added(6)),
        },
    ];
    let (mid, codes) = run(&base, &step1);
    if !codes.is_empty() {
        return None;
    }
    let mid = mid.ok()?;
    let mid_text = String::from_utf8(mid.clone()).ok()?;
    let second = Target::Instance(instance_of(&mid_text, 102)?);
    let col = Target::Instance(instance_of(&mid_text, 203)?);
    let temp = Target::Instance(instance_of(&mid_text, 299)?);
    let step2 = [
        Edit::Place {
            element: col,
            placement: at(2000.0, 500.0, 0.0, second),
        },
        Edit::Contain {
            element: col,
            space: second,
        },
        Edit::Type {
            element: col,
            type_of: None,
        },
        Edit::Remove { element: temp },
    ];
    let (out, codes) = run(&mid, &step2);
    if !codes.is_empty() {
        return None;
    }
    out.ok()
}

// 環境変数は照合用の出力先だけに使う。
#[allow(clippy::disallowed_methods)]
#[test]
fn writes_the_authored_files_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_AUTHORED") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    for schema in SCHEMAS {
        let src = complete_sample(schema);
        for (name, edits) in [
            ("built", two_storey_edits()),
            ("changed", two_storey_change_edits().unwrap()),
        ] {
            let (result, codes) = run(src.as_bytes(), &edits);
            assert!(codes.is_empty(), "{schema} {name}: {codes:?}");
            std::fs::write(folder.join(format!("{schema}.{name}.ifc")), result.unwrap()).unwrap();
        }
        std::fs::write(
            folder.join(format!("{schema}.structural.ifc")),
            structural_authored_bytes(schema).unwrap(),
        )
        .unwrap();
    }
}

fn quantity(
    element: u64,
    name: &str,
    kind: QuantityKind,
    value: f64,
    formula: Option<&str>,
) -> Edit {
    Edit::Quantity {
        element,
        set: "Qto_WallBaseQuantities".to_owned(),
        method: Some("BaseQuantities".to_owned()),
        quantity: NewQuantity {
            name: name.to_owned(),
            kind,
            value,
            formula: formula.map(str::to_owned),
        },
    }
}

fn unit(kind: &str, prefix: Option<&str>, name: &str) -> Edit {
    Edit::Unit(NewUnit {
        kind: kind.to_owned(),
        prefix: prefix.map(str::to_owned),
        name: name.to_owned(),
    })
}

#[test]
fn writes_quantities_and_units() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        // 型の数量は 6 HasPropertySets に書き、同じ名前の数量は書き直す
        let edits = [
            unit("AREAUNIT", None, "SQUARE_METRE"),
            unit("massunit", Some("kilo"), "gram"),
            quantity(20, "Length", QuantityKind::Length, 3000.0, None),
            quantity(20, "Area", QuantityKind::Area, 7.5, None),
            quantity(20, "Length", QuantityKind::Length, 3200.0, None),
            quantity(20, "Count", QuantityKind::Count, 2.0, None),
        ];
        let (result, codes) = run(src.as_bytes(), &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = result.unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(
            text.contains("IFCSIUNIT(*,.MASSUNIT.,.KILO.,.GRAM.)"),
            "{text}"
        );
        assert!(text.contains("IFCQUANTITYCOUNT('Count',$,$,2"), "{text}");
        let building = lift(&bytes).unwrap();
        let wall_type = element(&building, 20).unwrap();
        let sets = &building.components.quantities.get(wall_type).unwrap().sets;
        let found: Vec<_> = sets
            .iter()
            .flat_map(|set| {
                set.quantities
                    .iter()
                    .map(|q| (set.method.as_deref(), q.name.as_str(), q.value.value))
            })
            .collect();
        let method = Some("BaseQuantities");
        assert_eq!(
            found,
            [
                (method, "Length", 3200.0),
                (method, "Area", 7.5),
                (method, "Count", 2.0)
            ],
            "{schema}"
        );
        let area = building
            .document
            .units
            .iter()
            .find(|u| u.kind == "AREAUNIT")
            .map(|u| u.name.as_str());
        assert_eq!(area, Some("SQUARE_METRE"), "{schema}");
    }
}

#[test]
fn rejects_quantities_and_units_it_cannot_write() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let mut refused = vec![
            // ひな形が持つ種類・同じ種類・種類に合わない名前・知らない接頭辞
            unit("LENGTHUNIT", None, "METRE"),
            unit("AREAUNIT", None, "METRE"),
            unit("AREAUNIT", Some("MEGAMEGA"), "SQUARE_METRE"),
            quantity(20, "Length", QuantityKind::Length, -1.0, None),
            quantity(20, "Length", QuantityKind::Length, f64::NAN, None),
        ];
        if schema == "IFC2X3" {
            refused.push(quantity(20, "Length", QuantityKind::Length, 1.0, Some("L")));
        }
        if schema == "IFC4X3_ADD2" {
            refused.push(quantity(20, "Count", QuantityKind::Count, 2.5, None));
        }
        for edit in &refused {
            let (result, codes) = run(src.as_bytes(), std::slice::from_ref(edit));
            assert!(result.is_err(), "{schema}: {edit:?}");
            assert_eq!(codes.len(), 1, "{schema}: {edit:?}: {codes:?}");
        }
        let twice = [
            unit("AREAUNIT", None, "SQUARE_METRE"),
            unit("AREAUNIT", Some("MILLI"), "SQUARE_METRE"),
        ];
        let (result, codes) = run(src.as_bytes(), &twice);
        assert!(result.is_err(), "{schema}");
        assert_eq!(codes.len(), 1, "{schema}: {codes:?}");
    }
}

fn material_layer(material: Option<&str>, thickness: f64, name: Option<&str>) -> NewMaterialLayer {
    NewMaterialLayer {
        material: material.map(str::to_owned),
        thickness,
        name: name.map(str::to_owned),
    }
}

fn usage(direction: &str, offset: f64) -> Option<NewLayerUsage> {
    Some(NewLayerUsage {
        direction: direction.to_owned(),
        negative: true,
        offset,
    })
}

/// 材料の構成の種類・構成ごとの材料の名前と厚さと名前・層の向きと基準からのずれ
type MaterialRow = (
    MaterialKind,
    Vec<(Option<String>, Option<f64>, Option<String>)>,
    Option<String>,
    Option<f64>,
);

fn material_of(building: &Building, n: u32) -> Option<MaterialRow> {
    let material = building.components.material.get(element(building, n)?)?;
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
        material.offset.as_ref().map(|o| o.value),
    ))
}

#[test]
fn writes_and_replaces_materials() {
    for schema in SCHEMAS {
        let later = schema != "IFC2X3";
        let src = sample(schema, true);
        let mut edits = building_edits();
        let core = later.then_some("Core");
        edits.extend([
            Edit::Material {
                element: Target::Added(3),
                material: NewMaterial::Single("Concrete".to_owned()),
            },
            // 結び直すと前の構成を外す
            Edit::Material {
                element: Target::Added(3),
                material: NewMaterial::Layers {
                    layers: vec![
                        material_layer(Some("Concrete"), 200.0, core),
                        material_layer(None, 50.0, None),
                        material_layer(Some("Gypsum"), 12.5, None),
                    ],
                    usage: usage("axis2", -100.0),
                },
            },
            // IFC4 からは並びが非推奨なので、名前のない構成要素で書く
            Edit::Material {
                element: Target::Added(5),
                material: if later {
                    NewMaterial::Constituents(
                        ["Wood", "Glass", "Wood"]
                            .map(|material| NewConstituent {
                                material: material.to_owned(),
                                name: None,
                            })
                            .to_vec(),
                    )
                } else {
                    NewMaterial::List(vec![
                        "Wood".to_owned(),
                        "Glass".to_owned(),
                        "Wood".to_owned(),
                    ])
                },
            },
            Edit::Material {
                element: Target::Instance(20),
                material: if later {
                    NewMaterial::Constituents(vec![
                        NewConstituent {
                            material: "Concrete".to_owned(),
                            name: Some("Body".to_owned()),
                        },
                        NewConstituent {
                            material: "Steel".to_owned(),
                            name: None,
                        },
                    ])
                } else {
                    NewMaterial::Single("Concrete".to_owned())
                },
            },
        ]);
        let (result, codes) = run(src.as_bytes(), &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = result.unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        // 同じ名前の材料は 1 つ。外した単一の材料の関係は除く
        assert_eq!(
            count(&text, "IFCMATERIAL"),
            if later { 5 } else { 4 },
            "{text}"
        );
        assert_eq!(count(&text, "IFCRELASSOCIATESMATERIAL"), 3, "{text}");
        let building = lift(&bytes).unwrap();
        let some = |s: &str| Some(s.to_owned());
        assert_eq!(
            material_of(&building, 200),
            Some((
                MaterialKind::Layers,
                vec![
                    (some("Concrete"), Some(200.0), core.map(str::to_owned)),
                    (None, Some(50.0), None),
                    (some("Gypsum"), Some(12.5), None),
                ],
                some("AXIS2"),
                Some(-100.0),
            )),
            "{schema}"
        );
        assert_eq!(
            material_of(&building, 202).map(|m| (m.0, m.1)),
            Some((
                if later {
                    MaterialKind::Constituents
                } else {
                    MaterialKind::List
                },
                vec![
                    (some("Wood"), None, None),
                    (some("Glass"), None, None),
                    (some("Wood"), None, None),
                ],
            )),
            "{schema}"
        );
        let expected = if later {
            (
                MaterialKind::Constituents,
                vec![
                    (some("Concrete"), None, some("Body")),
                    (some("Steel"), None, None),
                ],
            )
        } else {
            (MaterialKind::Single, vec![(some("Concrete"), None, None)])
        };
        assert_eq!(
            material_of(&building, 20).map(|m| (m.0, m.1)),
            Some(expected),
            "{schema}"
        );
    }
}

#[test]
fn rejects_materials_it_cannot_write() {
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let on_type = |material| Edit::Material {
            element: Target::Instance(20),
            material,
        };
        let layers = |layers, usage| NewMaterial::Layers { layers, usage };
        let one = || vec![material_layer(Some("C"), 1.0, None)];
        let mut refused = vec![
            // 敷地には結ばない
            Edit::Material {
                element: Target::Instance(10),
                material: NewMaterial::Single("C".to_owned()),
            },
            on_type(NewMaterial::List(Vec::new())),
            on_type(layers(Vec::new(), None)),
            on_type(layers(vec![material_layer(Some("C"), -1.0, None)], None)),
            on_type(layers(
                vec![material_layer(Some("C"), f64::NAN, None)],
                None,
            )),
            on_type(layers(one(), usage("AXIS4", 0.0))),
            on_type(layers(one(), usage("AXIS2", f64::INFINITY))),
        ];
        if schema == "IFC2X3" {
            // IFC2X3 の層は正の厚さで、名前を持たない。構成要素もない
            refused.extend([
                on_type(layers(vec![material_layer(Some("C"), 0.0, None)], None)),
                on_type(layers(
                    vec![material_layer(Some("C"), 1.0, Some("L"))],
                    None,
                )),
                on_type(NewMaterial::Constituents(vec![NewConstituent {
                    material: "C".to_owned(),
                    name: None,
                }])),
            ]);
        } else {
            // 並びは IFC4 から非推奨
            refused.extend([
                on_type(NewMaterial::Constituents(Vec::new())),
                on_type(NewMaterial::List(vec!["C".to_owned()])),
            ]);
        }
        for edit in &refused {
            let (result, codes) = run(src.as_bytes(), std::slice::from_ref(edit));
            assert!(result.is_err(), "{schema}: {edit:?}");
            assert_eq!(codes.len(), 1, "{schema}: {edit:?}: {codes:?}");
        }
    }
}

#[path = "support/shape.rs"]
mod shape;

fn lift_geometry(src: &[u8]) -> Option<Building> {
    let file = read(src)?;
    with(|cx| file.lift_geometry(cx)).0.ok()
}

#[test]
fn writes_and_replaces_shapes() {
    let [wall, slab, column] = shape::examples();
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let mut edits = building_edits();
        edits.extend([
            Edit::Shape {
                element: Target::Added(3),
                shape: column.clone(),
            },
            // 置き換えると、前の形を除く
            Edit::Shape {
                element: Target::Added(3),
                shape: wall.clone(),
            },
            Edit::Shape {
                element: Target::Added(4),
                shape: slab.clone(),
            },
            Edit::Shape {
                element: Target::Added(5),
                shape: column.clone(),
            },
        ]);
        let (result, codes) = run(src.as_bytes(), &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let bytes = result.unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert_eq!(count(&text, "IFCCIRCLEPROFILEDEF"), 1, "{text}");
        assert_eq!(count(&text, "IFCPRODUCTDEFINITIONSHAPE"), 3, "{text}");
        // 文脈は Axis と Body の 1 つずつ
        assert_eq!(
            count(&text, "IFCGEOMETRICREPRESENTATIONSUBCONTEXT"),
            2,
            "{text}"
        );
        let building = lift_geometry(&bytes).unwrap();
        for (n, shape) in [(200, &wall), (201, &slab), (202, &column)] {
            assert_eq!(
                element(&building, n).and_then(|e| shape::shape_of(&building, e)),
                Some(shape::as_written(shape)),
                "{schema} {n}"
            );
        }
    }
}

/// 建物と階に壁・床・柱（200〜202）を置き、形を書く。読み直して同じ形かを確かめ、`ZUMEN_IFC_AUTHORED` が
/// あれば `<スキーマ>.<name>.ifc` に書き出す
// 環境変数は照合用の出力先だけに使う。
#[allow(clippy::disallowed_methods)]
fn write_shapes(schema: &str, shapes: &[NewShape], name: &str) -> String {
    write_shapes_in(&complete_sample(schema), schema, shapes, name)
}

/// [`write_shapes`] を、渡した文書に
#[allow(clippy::disallowed_methods)]
fn write_shapes_in(src: &str, schema: &str, shapes: &[NewShape], name: &str) -> String {
    let mut edits = building_edits();
    edits.truncate(3);
    for (n, class) in [(200, "IfcWall"), (201, "IfcSlab"), (202, "IfcColumn")] {
        let placement = at(0.0, 0.0, 0.0, Target::Added(1));
        edits.push(new_element(
            class,
            n,
            Some(Target::Added(1)),
            Some(placement),
        ));
    }
    for (n, shape) in shapes.iter().enumerate() {
        edits.push(Edit::Shape {
            element: Target::Added(3 + n),
            shape: shape.clone(),
        });
    }
    let (result, codes) = run(src.as_bytes(), &edits);
    assert!(codes.is_empty(), "{schema}: {codes:?}");
    let bytes = result.unwrap_or_default();
    let building = lift_geometry(&bytes);
    assert!(building.is_some(), "{schema}: 読めない");
    let Some(building) = building else {
        return String::new();
    };
    for (n, shape) in (200..).zip(shapes) {
        // IFC2X3 は掃引の始点と終点を省かない
        let shape = if schema == "IFC2X3" {
            shape::with_parameters(shape.clone())
        } else {
            shape.clone()
        };
        assert_eq!(
            element(&building, n).and_then(|e| shape::shape_of(&building, e)),
            Some(shape::as_written(&shape)),
            "{schema} {n}"
        );
    }
    if let Some(folder) = std::env::var_os("ZUMEN_IFC_AUTHORED") {
        let folder = std::path::PathBuf::from(folder);
        let written = std::fs::create_dir_all(&folder)
            .and_then(|()| std::fs::write(folder.join(format!("{schema}.{name}.ifc")), &bytes));
        assert!(written.is_ok(), "{written:?}");
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn writes_booleans_and_mappings() {
    for schema in SCHEMAS {
        let shapes = shape::csg_examples(schema != "IFC2X3");
        let text = write_shapes(schema, &shapes, "csg");
        for kind in ["Clipping", "CSG", "MappedRepresentation", "SweptSolid"] {
            assert!(
                text.contains(&format!("'Body','{kind}'")),
                "{schema} {kind}"
            );
        }
        for (name, n) in [
            ("IFCBOOLEANCLIPPINGRESULT", 2),
            ("IFCBOOLEANRESULT", 1),
            ("IFCHALFSPACESOLID", 1),
            ("IFCBOXEDHALFSPACE", 1),
            ("IFCPOLYGONALBOUNDEDHALFSPACE", 1),
            // 同じ原点と項目の写像元は 1 つ
            ("IFCREPRESENTATIONMAP", 1),
            ("IFCMAPPEDITEM", 2),
            ("IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM", 1),
        ] {
            assert_eq!(count(&text, name), n, "{schema} {name}");
        }
    }
}

/// 単位の割り当てに平面角の単位を足した文書
fn with_angle_unit(src: &str, unit: &str) -> String {
    let replaced = src.replace(
        "#3=IFCUNITASSIGNMENT((#4));",
        &format!("#3=IFCUNITASSIGNMENT((#4,#9000));\n{unit}"),
    );
    assert_ne!(replaced, src);
    replaced
}

#[test]
fn writes_revolutions_and_sweeps() {
    let radian = "#9000=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);";
    for schema in SCHEMAS {
        let src = with_angle_unit(&complete_sample(schema), radian);
        let shapes = shape::sweep_examples();
        let text = write_shapes_in(&src, schema, &shapes, "sweep");
        for kind in ["SweptSolid", "AdvancedSweptSolid", "SolidModel"] {
            assert!(
                text.contains(&format!("'Body','{kind}'")),
                "{schema} {kind}"
            );
        }
        assert_eq!(count(&text, "IFCREVOLVEDAREASOLID"), 1, "{schema}");
        assert_eq!(count(&text, "IFCSWEPTDISKSOLID"), 2, "{schema}");
    }
}

#[test]
fn writes_revolution_angles_in_the_document_unit() {
    let [revolution, ..] = shape::sweep_examples();
    let degree = "#9000=IFCCONVERSIONBASEDUNIT(#9001,.PLANEANGLEUNIT.,'degree',#9002);\n\
        #9001=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);\n\
        #9002=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.0174532925199433),#9003);\n\
        #9003=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);";
    for schema in SCHEMAS {
        let src = with_angle_unit(&sample(schema, true), degree);
        let mut edits = building_edits();
        edits.push(Edit::Shape {
            element: Target::Added(3),
            shape: revolution.clone(),
        });
        let (result, codes) = run(src.as_bytes(), &edits);
        assert!(codes.is_empty(), "{schema}: {codes:?}");
        let text = String::from_utf8(result.unwrap()).unwrap();
        let line = text
            .lines()
            .find(|line| line.contains("IFCREVOLVEDAREASOLID("))
            .unwrap();
        let angle: f64 = line
            .rsplit(',')
            .next()
            .and_then(|v| v.strip_suffix(");"))
            .and_then(|v| v.parse().ok())
            .unwrap();
        assert!((angle - 180.0).abs() < 1e-9, "{schema} {line}");
        // 平面角の単位がなければ、角度を書けない
        let (_, codes) = run(sample(schema, true).as_bytes(), &edits);
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{schema}");
    }
}

#[test]
fn writes_face_sets() {
    for schema in SCHEMAS {
        let later = schema != "IFC2X3";
        let shapes = shape::face_examples(later, schema == "IFC4X3_ADD2");
        let text = write_shapes(schema, &shapes, "faces");
        // 箱の点は 8 つを共有し、面ごとに写さない
        assert_eq!(
            count(&text, "IFCCLOSEDSHELL"),
            if later { 1 } else { 2 },
            "{text}"
        );
        for kind in [
            "SolidModel",
            "SurfaceModel",
            if later { "Tessellation" } else { "Brep" },
        ] {
            assert!(
                text.contains(&format!("'Body','{kind}'")),
                "{schema} {kind}"
            );
        }
    }
}

#[test]
fn rejects_shapes_it_cannot_write() {
    let [wall, ..] = shape::examples();
    let changed = |change: &dyn Fn(&mut NewShape)| {
        let mut shape = wall.clone();
        change(&mut shape);
        shape
    };
    let solid = |change: &dyn Fn(&mut NewSolid)| {
        changed(&|shape: &mut NewShape| {
            if let Some(NewItem::Solid(solid)) = shape.body.first_mut() {
                change(solid);
            }
        })
    };
    let body = |body: Vec<NewItem>| NewShape { axis: None, body };
    let origin = shape::placed([0.0; 3], None);
    let half = |limit: NewHalfSpaceLimit| shape::half_space(origin.clone(), false, limit);
    let unbounded = || half(NewHalfSpaceLimit::Unbounded);
    let mapped = |items: Vec<NewItem>, scale: MappingScale3| {
        NewItem::Mapped(Box::new(NewMapped {
            origin: origin.clone(),
            target: CartesianMapping3 {
                local_origin: [0.0; 3],
                axis1: None,
                axis2: None,
                axis3: None,
                scale,
            },
            items,
        }))
    };
    let block = || shape::block(1.0, 1.0, 1.0);
    let cut = |first: NewItem, second: NewItem| {
        shape::boolean(BooleanOperation::Difference, first, second)
    };
    let [rotated, piped, _] = shape::sweep_examples();
    let revolved = |change: &dyn Fn(&mut NewRevolution)| {
        let mut shape = rotated.clone();
        if let Some(NewItem::Revolution(r)) = shape.body.first_mut() {
            change(r);
        }
        shape
    };
    let swept = |change: &dyn Fn(&mut NewSweptDisk)| {
        let mut shape = piped.clone();
        if let Some(NewItem::SweptDisk(d)) = shape.body.first_mut() {
            change(d);
        }
        shape
    };
    let mut nested = block();
    for _ in 0..16 {
        nested = shape::boolean(BooleanOperation::Union, nested, block());
    }
    let refused = [
        NewShape {
            axis: None,
            body: Vec::new(),
        },
        // 半空間は表現に直接置かない。写像と押し出しは 1 つの表現にしない
        body(vec![unbounded()]),
        body(vec![
            block(),
            mapped(vec![block()], MappingScale3::Uniform(None)),
        ]),
        // 写像と面の模型は演算の対象にしない
        body(vec![cut(
            mapped(vec![block()], MappingScale3::Uniform(None)),
            unbounded(),
        )]),
        body(vec![cut(
            NewItem::Faces(NewFaces::Polygons(shape::surfaces(true)[0].clone())),
            unbounded(),
        )]),
        // 囲みの箱・多角形・写像の尺度・空の写像元・入れ子の深さ
        body(vec![cut(
            block(),
            half(NewHalfSpaceLimit::Box {
                corner: [0.0; 3],
                dimensions: [1.0, 0.0, 1.0],
            }),
        )]),
        body(vec![cut(
            block(),
            half(NewHalfSpaceLimit::Polygonal {
                position: origin.clone(),
                boundary: vec![[0.0, 0.0], [1.0, 0.0]],
            }),
        )]),
        body(vec![mapped(
            vec![block()],
            MappingScale3::Uniform(Some(0.0)),
        )]),
        body(vec![mapped(
            vec![block()],
            MappingScale3::NonUniform {
                x: None,
                y: Some(f64::NAN),
                z: None,
            },
        )]),
        body(vec![mapped(Vec::new(), MappingScale3::Uniform(None))]),
        body(vec![nested]),
        changed(&|shape| shape.axis = Some(vec![[0.0, 0.0]])),
        solid(&|s| s.depth = 0.0),
        solid(&|s| s.depth = f64::NAN),
        solid(&|s| s.direction = [1.0, 0.0, 0.0]),
        solid(&|s| {
            s.position = Some(NewAxis3 {
                location: [0.0; 3],
                axis: Some([0.0; 3]),
                reference: None,
            });
        }),
        solid(&|s| {
            s.profile.kind = NewProfileKind::Circle {
                radius: 0.0,
                position: None,
            };
        }),
        solid(&|s| {
            s.profile.kind = NewProfileKind::Polygon {
                outer: vec![[0.0, 0.0], [1.0, 0.0]],
                holes: Vec::new(),
            };
        }),
        // 回転の角度と軸、掃引の経路・半径・区間
        revolved(&|r| r.angle = 0.0),
        revolved(&|r| r.angle = 7.0),
        revolved(&|r| r.direction = [0.0, 0.0]),
        swept(&|d| d.directrix.truncate(1)),
        swept(&|d| d.directrix.insert(1, [0.0; 3])),
        swept(&|d| d.radius = 0.0),
        swept(&|d| d.inner_radius = Some(50.0)),
        swept(&|d| d.end = Some(2.5)),
        swept(&|d| {
            d.start = Some(1.0);
            d.end = Some(1.0);
        }),
    ];
    for schema in SCHEMAS {
        let src = sample(schema, true);
        let mut cases: Vec<Vec<Edit>> = refused
            .iter()
            .map(|shape| {
                let mut edits = building_edits();
                edits.push(Edit::Shape {
                    element: Target::Added(3),
                    shape: shape.clone(),
                });
                edits
            })
            .collect();
        let faces = |faces: NewFaces| {
            let mut edits = building_edits();
            edits.push(Edit::Shape {
                element: Target::Added(3),
                shape: NewShape {
                    axis: None,
                    body: vec![NewItem::Faces(faces)],
                },
            });
            edits
        };
        let [shell, sheet] = shape::surfaces(false);
        let brep = shape::brep_box([1.0, 1.0, 1.0]);
        // 開殻の Brep・2 点のループ・外輪が 2 つの面・範囲の外の参照
        let mut open = brep.clone();
        open.sets[0].closed = Some(false);
        let mut short = brep.clone();
        short.loops[0].points.truncate(2);
        let mut outers = brep.clone();
        outers.faces[0].bounds.push(FaceBoundRef(1));
        let mut outside = brep.clone();
        outside.roots[0] = FaceSetRef(5);
        for graph in [open, short, outers, outside] {
            cases.push(faces(NewFaces::Polygons(graph)));
        }
        // 押し出しと面の模型は 1 つの表現にしない
        let mut mixed = building_edits();
        let mut shape = wall.clone();
        shape.body.push(NewItem::Faces(NewFaces::Polygons(shell)));
        mixed.push(Edit::Shape {
            element: Target::Added(3),
            shape,
        });
        cases.push(mixed);
        // 添字の面集合は閉殻の申告があるときだけ演算の対象にする
        if schema != "IFC2X3" {
            let mut open = shape::triangulated_box([1.0, 1.0, 1.0], false);
            open.closed = None;
            let mut edits = building_edits();
            edits.push(Edit::Shape {
                element: Target::Added(3),
                shape: body(vec![cut(
                    NewItem::Faces(NewFaces::Indexed(open)),
                    unbounded(),
                )]),
            });
            cases.push(edits);
        }
        let mut indexed = shape::triangulated_box([1.0, 1.0, 1.0], false);
        if let IndexedFaces::Triangles { indices, .. } = &mut indexed.faces {
            indices[0][0] = 9;
        }
        cases.push(faces(NewFaces::Indexed(indexed)));
        // 面の模型は IFC4 から非推奨
        if schema != "IFC2X3" {
            cases.push(faces(NewFaces::Polygons(sheet)));
        }
        // 添字の面集合は IFC4 から、点のタグは IFC4X3 から
        if schema != "IFC4X3_ADD2" {
            cases.push(faces(NewFaces::Indexed(shape::triangulated_box(
                [1.0, 1.0, 1.0],
                schema == "IFC4",
            ))));
        }
        // 敷地には置かない。配置のない要素にも置かない
        cases.push(vec![Edit::Shape {
            element: Target::Instance(10),
            shape: wall.clone(),
        }]);
        cases.push(vec![
            new_element("IfcWall", 300, None, None),
            Edit::Shape {
                element: Target::Added(0),
                shape: wall.clone(),
            },
        ]);
        for edits in cases {
            let (result, codes) = run(src.as_bytes(), &edits);
            assert!(result.is_err(), "{schema}: {edits:?}");
            assert_eq!(
                codes,
                [codes::CHANGE_EDIT_UNSUPPORTED],
                "{schema}: {edits:?}"
            );
        }
    }
}
