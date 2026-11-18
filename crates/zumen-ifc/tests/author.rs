//! 空間・要素・関係を足し、除き、置き直す変更（#897）。ファイルは、どれもテストの中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_building::{
    Building, ElementRef, GlobalId, GridSet, Id, Local, MaterialKind, Opening, ParentKind,
    SpatialKind, Transform,
};
use zumen_ifc::{
    Edit, FillingKind, Ifc, IfcFile, IfcLive, NewBeam, NewColumn, NewElement, NewFilling, NewGrid,
    NewGridAxis, NewLayer, NewOpening, NewPlacement, NewSection, NewSlab, NewSpace, NewWall,
    OpeningChange, RootAttribute, SpaceKind, Target, WallChange, apply,
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
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    let stopped = live.apply(
        &building_edits(),
        b"seed",
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    assert!(matches!(stopped, Err(Error::Stopped(_))));
    assert!(live.is_clean());
    assert_eq!((live.undo_len(), live.redo_len()), (0, 0));
    assert_eq!(with(|cx| live.save(cx)).0.ok(), Some(src.into_bytes()));
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
        // 建具は開口の配置に置き、壁と同じ階に包含で結ぶ
        for (filling, opening, name, predefined) in [
            (door, door_opening, "IfcDoor", "DOOR"),
            (window, window_opening, "IfcWindow", "WINDOW"),
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
        assert!(elevation.ends_with(",3500.);"), "{schema}: {elevation}");
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
        // 壁の開口は新しい層の範囲を貫き、扉は開口の配置のまま
        assert_eq!(
            solids(&after, 500).unwrap(),
            [([0.0, 150.0, 0.0], 300.0)],
            "{schema}"
        );
        assert_eq!(
            translation(&building, door_opening),
            Some([1000.0, 0.0, 0.0])
        );
        let placement = building.components.placement.get(door).unwrap();
        assert_eq!(placement.relative_to, Some(door_opening));
        // 動かした開口は、壁 301 の層の範囲のまま新しい大きさで押し出し、窓の全体の高さと幅も合わせる
        assert_eq!(
            translation(&building, window_opening),
            Some([800.0, 0.0, 1000.0])
        );
        assert_eq!(solids(&after, 502).unwrap(), [([0.0, 100.0, 0.0], 200.0)]);
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
