//! 空間・要素・関係を足し、除き、置き直す変更（#897）。ファイルは、どれもテストの中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_building::{
    Building, ElementRef, GlobalId, Id, Local, Opening, ParentKind, SpatialKind, Transform,
};
use zumen_ifc::{Edit, Ifc, IfcFile, NewElement, NewPlacement, NewSpace, SpaceKind, Target, apply};
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
        elevation: Some(elevation),
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
        assert_eq!(spatial.elevation.as_ref().map(|m| m.value), Some(3000.0));
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

#[test]
fn the_document_engine_rejects_structural_edits_for_now() {
    let src = sample("IFC4", true);
    let (live, _) = with(|cx| zumen_ifc::IfcLive::open(src.as_bytes(), cx));
    let mut live = live.unwrap();
    let (result, diag) = with(|cx| live.apply(&building_edits(), b"seed", cx));
    assert!(matches!(result, Err(Error::Invalid)));
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CHANGE_EDIT_UNSUPPORTED)
    );
    assert!(live.is_clean());
    assert_eq!(with(|cx| live.save(cx)).0.unwrap(), src.as_bytes());
}
