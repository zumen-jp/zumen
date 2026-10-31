//! 建物モデルの ID・ハッシュ・配置・関係の索引（決定 0207）。モデルは、どれも試験の中で組み立てる。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    Building, Class, Document, ElementRef, GlobalId, Group, Hash, Id, Identity, Literal, Local,
    Membership, Opening, Parent, ParentKind, Placement, Properties, Property, PropertySet,
    Transform, TypeOf, TypedValue, Unit,
};

const PROJECT: &str = "0YvctVUKr0kugbFTf53O9L";
const STOREY: &str = "1YvctVUKr0kugbFTf53O9L";
const WALL: &str = "2YvctVUKr0kugbFTf53O9L";

fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn found(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

fn mm() -> Unit {
    Unit {
        kind: "LENGTHUNIT".to_owned(),
        prefix: Some("MILLI".to_owned()),
        name: "METRE".to_owned(),
        scale: None,
    }
}

fn wall_parts(building: &mut Building, wall: ElementRef, storey: ElementRef, height: f64) {
    let c = &mut building.components;
    c.identity.insert(
        wall,
        Identity {
            name: Some("Wall".to_owned()),
            ..Identity::default()
        },
    );
    c.class.insert(
        wall,
        Class {
            name: "IfcWall".to_owned(),
            predefined: Some("STANDARD".to_owned()),
            user_defined: None,
            original: "IfcWallStandardCase".to_owned(),
        },
    );
    c.parent.insert(
        wall,
        Parent {
            of: Some(Membership {
                target: storey,
                kind: ParentKind::Contained,
                order: None,
            }),
            referenced_in: Vec::new(),
        },
    );
    c.properties.insert(
        wall,
        Properties {
            sets: vec![PropertySet {
                name: "Pset_WallCommon".to_owned(),
                properties: vec![Property {
                    name: "Height".to_owned(),
                    value: Some(TypedValue {
                        type_name: Some("IfcLengthMeasure".to_owned()),
                        value: Literal::Real(height),
                    }),
                    unit: Some(mm()),
                }],
            }],
        },
    );
}

/// 階と壁の 2 つの要素の建物モデル。`storey_first` で並びを、`base` でインスタンスの番号を変える
fn model(storey_first: bool, base: u64, height: f64) -> Option<(Building, ElementRef, ElementRef)> {
    let (built, _) = with(Limits::default(), |cx| {
        let mut building = Building::new(Document::default());
        let (storey, wall) = if storey_first {
            let storey = building.push(Some(STOREY), base, cx)?;
            (storey, building.push(Some(WALL), base + 1, cx)?)
        } else {
            let wall = building.push(Some(WALL), base, cx)?;
            (building.push(Some(STOREY), base + 1, cx)?, wall)
        };
        wall_parts(&mut building, wall, storey, height);
        Some((building, storey, wall))
    });
    built
}

#[test]
fn global_ids_and_uuids_convert_both_ways() {
    for (uuid, global) in [
        (
            "00000000-0000-0000-0000-000000000000",
            "0000000000000000000000",
        ),
        (
            "00000000-0000-0000-0000-000000000001",
            "0000000000000000000001",
        ),
        (
            "00000000-0000-0000-0000-000000000040",
            "0000000000000000000010",
        ),
        (
            "40000000-0000-0000-0000-000000000000",
            "1000000000000000000000",
        ),
        (
            "ffffffff-ffff-ffff-ffff-ffffffffffff",
            "3$$$$$$$$$$$$$$$$$$$$$",
        ),
    ] {
        let from_uuid = GlobalId::parse_uuid(uuid).unwrap();
        let from_global = GlobalId::parse(global).unwrap();
        assert_eq!(from_uuid, from_global, "{uuid}");
        assert_eq!(from_global.to_string(), global);
        assert_eq!(from_global.uuid_string(), uuid);
        assert_eq!(
            GlobalId::from_uuid(from_global.to_uuid()),
            from_global,
            "{uuid}"
        );
    }
    assert!(GlobalId::parse_uuid("FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF").is_some());
    let id = GlobalId::parse(PROJECT).unwrap();
    assert_eq!(GlobalId::parse(&id.to_string()), Some(id));
    assert_eq!(GlobalId::parse_uuid(&id.uuid_string()), Some(id));

    // 22 文字でないもの、使えない文字、最初の文字が 3 より大きいもの
    for bad in [
        "",
        "000000000000000000000",
        "00000000000000000000000",
        "000000000000000000000-",
        "4000000000000000000000",
        "0000000000000000000000\u{3042}",
    ] {
        assert_eq!(GlobalId::parse(bad), None, "{bad}");
    }
    for bad in [
        "",
        "00000000-0000-0000-0000-00000000000",
        "0000000g-0000-0000-0000-000000000000",
    ] {
        assert_eq!(GlobalId::parse_uuid(bad), None, "{bad}");
    }
}

#[test]
fn ids_are_global_ids_or_instance_numbers() {
    assert_eq!("i120".parse::<Id>(), Ok(Id::Instance(120)));
    assert_eq!("i0".parse::<Id>(), Ok(Id::Instance(0)));
    assert_eq!(Id::Instance(120).to_string(), "i120");
    let global: Id = PROJECT.parse().unwrap();
    assert_eq!(global, Id::Global(GlobalId::parse(PROJECT).unwrap()));
    assert_eq!(global.to_string(), PROJECT);
    for bad in ["i", "i012", "i-1", "I120", "120", "i1a"] {
        assert!(bad.parse::<Id>().is_err(), "{bad}");
    }

    let (building, diag) = with(Limits::default(), |cx| {
        let mut building = Building::new(Document::default());
        let first = building.push(Some(PROJECT), 1, cx).unwrap();
        // 2 度目の GlobalId と、使えない GlobalId と、GlobalId のない資源は、番号で作る
        let again = building.push(Some(PROJECT), 2, cx).unwrap();
        let broken = building.push(Some("not a global id"), 3, cx).unwrap();
        let material = building.push(None, 4, cx).unwrap();
        // 同じ ID になる要素は足さない
        assert_eq!(building.push(None, 4, cx), None);
        assert_eq!(building.id(first), Some(global));
        assert_eq!(building.id(again), Some(Id::Instance(2)));
        assert_eq!(building.id(broken), Some(Id::Instance(3)));
        assert_eq!(building.id(material), Some(Id::Instance(4)));
        assert_eq!(
            building
                .element(again)
                .unwrap()
                .source()
                .global_id
                .as_deref(),
            Some(PROJECT)
        );
        assert_eq!(building.find(Id::Instance(3)), Some(broken));
        assert_eq!(building.find(global), Some(first));
        building
    });
    assert_eq!(building.elements().len(), 4);
    assert_eq!(
        found(&diag),
        [
            codes::BUILDING_GLOBAL_ID_EXISTS,
            codes::BUILDING_GLOBAL_ID_INVALID,
            codes::BUILDING_ID_EXISTS
        ]
    );
}

#[test]
fn hashes_depend_only_on_components() {
    let (a, _, wall_a) = model(true, 10, 2500.0).unwrap();
    // 並びとインスタンスの番号（出所の位置）を変えても、同じハッシュ
    let (b, _, wall_b) = model(false, 500, 2500.0).unwrap();
    let hash = Hash::element(&a, wall_a).unwrap();
    assert_eq!(Hash::element(&b, wall_b), Some(hash));
    assert_ne!(Hash::element(&a, wall_a), Hash::element(&a, ElementRef(0)));
    // 成分の値を変えると変わる。-0 と 0 は同じ
    let (c, _, wall_c) = model(true, 10, 2600.0).unwrap();
    assert_ne!(Hash::element(&c, wall_c), Some(hash));
    let (zero, _, wall_zero) = model(true, 10, 0.0).unwrap();
    let (minus, _, wall_minus) = model(true, 10, -0.0).unwrap();
    assert_eq!(
        Hash::element(&zero, wall_zero),
        Hash::element(&minus, wall_minus)
    );
    // 参照は指す先の ID で入れるので、指す先の GlobalId が変わると変わる
    let ((other, wall_other), _) = with(Limits::default(), |cx| {
        let mut building = Building::new(Document::default());
        let storey = building.push(Some(PROJECT), 10, cx).unwrap();
        let wall = building.push(Some(WALL), 11, cx).unwrap();
        wall_parts(&mut building, wall, storey, 2500.0);
        (building, wall)
    });
    assert_ne!(Hash::element(&other, wall_other), Some(hash));
    // 成分を除くと変わる
    let mut d = a.clone();
    d.components.properties.remove(wall_a);
    assert_ne!(Hash::element(&d, wall_a), Some(hash));
    assert_eq!(Hash::element(&a, ElementRef(9)), None);

    // 書き方
    let text = hash.to_string();
    assert!(text.starts_with("v1:") && text.len() == 35, "{text}");
    assert_eq!(text.parse::<Hash>(), Ok(hash));
    assert!(text.replace("v1:", "v2:").parse::<Hash>().is_err());
    assert!(text.get(..34).unwrap().parse::<Hash>().is_err());

    // 予算を数えるものも、同じハッシュ
    let (checked, _) = with(Limits::default(), |cx| {
        Hash::element_checked(&a, wall_a, cx)
    });
    assert_eq!(checked, Ok(Some(hash)));
}

#[test]
fn hashing_counts_work() {
    let (mut big, _, wall) = model(true, 10, 2500.0).unwrap();
    big.components.identity.insert(
        wall,
        Identity {
            description: Some("x".repeat(10_000)),
            ..Identity::default()
        },
    );
    let (result, _) = with(Limits::default().with(Kind::Work, Limit::Finite(3)), |cx| {
        Hash::element_checked(&big, wall, cx)
    });
    assert!(result.is_err());
    let (result, _) = with(
        Limits::default().with(Kind::Work, Limit::Finite(20)),
        |cx| Hash::element_checked(&big, wall, cx),
    );
    assert_eq!(result, Ok(Hash::element(&big, wall)));
}

#[test]
fn document_hashes() {
    let bytes = b"ISO-10303-21;".repeat(500);
    let hash = Hash::document(&bytes);
    assert_ne!(Hash::document(b"ISO-10303-21;"), hash);
    let (checked, _) = with(Limits::default(), |cx| Hash::document_checked(&bytes, cx));
    assert_eq!(checked, Ok(hash));
    let (stopped, _) = with(Limits::default().with(Kind::Work, Limit::Finite(2)), |cx| {
        Hash::document_checked(&bytes, cx)
    });
    assert!(stopped.is_err());

    let (a, _, _) = model(true, 10, 2500.0).unwrap();
    let mut b = a.clone();
    assert_eq!(Hash::document_values(&a), Hash::document_values(&b));
    b.document.units.push(mm());
    assert_ne!(Hash::document_values(&a), Hash::document_values(&b));
}

/// `count` の要素を、1 つ前の要素の配置からの移動で並べる
fn chain(count: u64) -> Option<(Building, Vec<ElementRef>)> {
    let (built, _) = with(Limits::default(), |cx| {
        let mut building = Building::new(Document::default());
        let mut elements: Vec<ElementRef> = Vec::new();
        for n in 0..count {
            let element = building.push(None, n + 1, cx)?;
            building.components.placement.insert(
                element,
                Placement {
                    relative_to: elements.last().copied(),
                    local: Local::Transform(Transform::translation(1000.0, 0.0, 0.0)),
                },
            );
            elements.push(element);
        }
        Some((building, elements))
    });
    built
}

#[test]
fn placements_follow_the_chain() {
    let (mut building, elements) = chain(3).unwrap();
    let (first, last) = (elements[0], elements[2]);
    // 最初の配置を Z の軸の周りに 90° 回し、上げる
    building.components.placement.insert(
        first,
        Placement {
            relative_to: None,
            local: Local::Transform(Transform([
                [0.0, -1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 3000.0],
            ])),
        },
    );
    let (world, diag) = with(Limits::default(), |cx| building.world_placement(last, cx));
    let world = world.unwrap().unwrap();
    assert!(diag.is_empty());
    assert_eq!(world.apply([0.0, 0.0, 0.0]), [0.0, 2000.0, 3000.0]);
    assert_eq!(world.apply([1.0, 0.0, 0.0]), [0.0, 2001.0, 3000.0]);

    // 配置を持たない要素は、診断なしに配置なし
    let (none, diag) = with(Limits::default(), |cx| {
        let mut building = building.clone();
        let bare = building.push(None, 99, cx).unwrap();
        building.world_placement(bare, cx)
    });
    assert_eq!(none, Ok(None));
    assert!(diag.is_empty());
}

#[test]
fn placement_cycles_and_gaps_have_no_placement() {
    let (mut building, elements) = chain(3).unwrap();
    let (first, last) = (elements[0], elements[2]);
    building.components.placement.insert(
        first,
        Placement {
            relative_to: Some(last),
            local: Local::Transform(Transform::IDENTITY),
        },
    );
    let (result, diag) = with(Limits::default(), |cx| building.world_placement(last, cx));
    assert_eq!(result, Ok(None));
    assert_eq!(found(&diag), [codes::BUILDING_PLACEMENT_INVALID]);

    let (mut building, elements) = chain(3).unwrap();
    let (first, last) = (elements[0], elements[2]);
    building.components.placement.remove(first);
    let (result, diag) = with(Limits::default(), |cx| building.world_placement(last, cx));
    assert_eq!(result, Ok(None));
    assert_eq!(found(&diag), [codes::BUILDING_PLACEMENT_MISSING]);
    assert_eq!(
        diag.iter().next().unwrap().param("id"),
        Some(&zumen_base::Value::Text("i1".to_owned()))
    );

    let (mut building, elements) = chain(3).unwrap();
    let (first, last) = (elements[0], elements[2]);
    building.components.placement.insert(
        first,
        Placement {
            relative_to: None,
            local: Local::Unsupported("IfcGridPlacement".to_owned()),
        },
    );
    let (result, diag) = with(Limits::default(), |cx| building.world_placement(last, cx));
    assert_eq!(result, Ok(None));
    assert_eq!(found(&diag), [codes::BUILDING_PLACEMENT_UNSUPPORTED]);
}

#[test]
fn placement_depth_is_budgeted() {
    let (building, elements) = chain(5).unwrap();
    let last = *elements.last().unwrap();
    let (result, _) = with(
        Limits::default().with(Kind::Depth, Limit::Finite(4)),
        |cx| building.world_placement(last, cx),
    );
    assert!(result.is_err());
    // たどり終えると深さを戻すので、同じ予算で何度でも引ける
    let (results, _) = with(
        Limits::default().with(Kind::Depth, Limit::Finite(5)),
        |cx| {
            [
                building.world_placement(last, cx),
                building.world_placement(last, cx),
            ]
        },
    );
    for result in results {
        let world = result.unwrap().unwrap();
        assert_eq!(world.apply([0.0; 3]), [5000.0, 0.0, 0.0]);
    }
}

#[test]
fn relations_are_looked_up_backwards() {
    let ((building, e), _) = with(Limits::default(), |cx| {
        let mut building = Building::new(Document::default());
        let mut e = Vec::new();
        // 0 プロジェクト・1 階・2 壁・3 壁・4 型・5 開口・6 建具・7 系統・8 2 階・9 柱
        for n in 0..10 {
            e.push(building.push(None, n + 1, cx).unwrap());
        }
        let c = &mut building.components;
        let member = |target, kind| Parent {
            of: Some(Membership {
                target,
                kind,
                order: None,
            }),
            referenced_in: Vec::new(),
        };
        c.parent.insert(e[1], member(e[0], ParentKind::Aggregates));
        c.parent.insert(e[8], member(e[0], ParentKind::Aggregates));
        // 後から入れても、要素の番号の順に並ぶ
        c.parent.insert(e[3], member(e[1], ParentKind::Contained));
        c.parent.insert(e[2], member(e[1], ParentKind::Contained));
        c.parent.insert(
            e[9],
            Parent {
                of: Some(Membership {
                    target: e[1],
                    kind: ParentKind::Contained,
                    order: None,
                }),
                referenced_in: vec![e[8]],
            },
        );
        c.r#type.insert(e[2], TypeOf { target: e[4] });
        c.r#type.insert(e[3], TypeOf { target: e[4] });
        c.opening.insert(e[5], Opening::Voids(e[2]));
        c.opening.insert(e[6], Opening::Fills(e[5]));
        c.group.insert(e[2], Group { groups: vec![e[7]] });
        c.group.insert(e[9], Group { groups: vec![e[7]] });
        (building, e)
    });
    let (relations, _) = with(Limits::default(), |cx| building.relations(cx));
    let relations = relations.unwrap();
    assert_eq!(relations.children(e[0]), [e[1], e[8]]);
    assert_eq!(relations.children(e[1]), [e[2], e[3], e[9]]);
    assert_eq!(relations.referenced(e[8]), [e[9]]);
    assert_eq!(relations.instances(e[4]), [e[2], e[3]]);
    assert_eq!(relations.openings(e[2]), [e[5]]);
    assert_eq!(relations.fillings(e[5]), [e[6]]);
    assert_eq!(relations.members(e[7]), [e[2], e[9]]);
    assert_eq!(relations.children(e[9]), []);

    let (result, diag) = with(Limits::default().with(Kind::Work, Limit::Finite(5)), |cx| {
        building.relations(cx)
    });
    assert!(result.is_err());
    assert!(diag.is_empty());
}
