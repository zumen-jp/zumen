//! 空間・要素・関係を足し、除き、置き直す変更（#897）。ファイルは、どれもテストの中で作る。
#![cfg(feature = "building")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_building::{
    Building, ElementRef, GlobalId, GridSet, Id, Local, MaterialKind, Opening, ParentKind,
    QuantityKind, SpatialKind, Transform,
};
use zumen_ifc::{
    Edit, FillingKind, Ifc, IfcFile, IfcLive, NewBeam, NewColumn, NewConstituent, NewElement,
    NewFilling, NewGrid, NewGridAxis, NewLayer, NewLayerUsage, NewMaterial, NewMaterialLayer,
    NewOpening, NewPlacement, NewQuantity, NewSection, NewSlab, NewSpace, NewUnit, NewWall,
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
        // 形まで読める。形を持たないのは空間と建具だけで、ほかの診断は出ない
        let file = read(&built).unwrap();
        let (shaped, diag) = with(|cx| file.lift_geometry(cx));
        assert!(
            diag.iter().all(|d| d.code() == codes::IFC_SHAPE_MISSING),
            "{schema}: {:?}",
            diag.iter().collect::<Vec<_>>()
        );
        let shaped = shaped.unwrap();
        for n in [2000, 2010, 2011, 2012, 2020, 3000, 3020] {
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
