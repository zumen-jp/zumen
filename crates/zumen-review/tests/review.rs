//! レビューの文書の読み書き・引き継ぎ・取り込み（決定 0214・0218）。図面と建物モデルは、どれも試験の中で組み立てる。

use std::collections::BTreeMap;

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Key, Limits, Ref, codes};
use zumen_building::{Building, Document, ElementRef};
use zumen_drawing::{
    Axes, Drawing2D, Element, LayerId, ModelPoint, ModelSpace, Shape, SpaceId, Style, Unit,
};
use zumen_json::Value;
use zumen_review::{
    Adopted, Change, Location, MarkupKind, Place, Reference, Review, Status, Target, Version,
    adopt, carry,
};

const WALL: &str = "2YvctVUKr0kugbFTf53O9L";
const SLAB: &str = "3YvctVUKr0kugbFTf53O9L";

fn with<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn found(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

/// 要素の ID からハッシュを引く版
struct Hashes {
    document: String,
    elements: BTreeMap<String, String>,
}

impl Version for Hashes {
    fn hash(&self) -> String {
        self.document.clone()
    }

    fn element(&self, id: &str) -> Option<String> {
        self.elements.get(id).cloned()
    }
}

fn line(x: f64, handle: u64) -> Element<ModelPoint> {
    let at = |x| ModelPoint {
        space: SpaceId(0),
        x,
        y: 0.0,
    };
    Element {
        source: Ref::at_record(handle).with_key(Key::Handle(handle)),
        layer: LayerId(0),
        style: Style::BLOCK,
        shape: Shape::Line {
            start: at(x),
            end: at(x + 1.0),
        },
    }
}

/// 線分の図面。`lines` は線分の始点の X とハンドル
fn drawing(lines: &[(f64, u64)]) -> Drawing2D {
    Drawing2D {
        spaces: vec![ModelSpace {
            id: SpaceId(0),
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            elements: lines.iter().map(|&(x, h)| line(x, h)).collect(),
            order: Vec::new(),
        }],
        ..Drawing2D::default()
    }
}

/// 図面の要素の ID とハッシュ（決定 0156）
fn drawing_version(drawing: &Drawing2D, document: &[u8]) -> (Hashes, Vec<String>) {
    let mut elements = BTreeMap::new();
    let mut ids = Vec::new();
    let _ = zumen_drawing::id::each_element(drawing, |_, id, found| {
        elements.insert(id.clone(), found.hash().to_string());
        ids.push(id);
        Ok::<(), ()>(())
    });
    let version = Hashes {
        document: zumen_drawing::Hash::document(document).to_string(),
        elements,
    };
    (version, ids)
}

/// 壁と床の建物モデル。`slab` が偽なら床を持たない。`name` は壁の名前
fn building(slab: bool, name: &str) -> Option<Building> {
    let (built, _) = with(|cx| {
        let mut building = Building::new(Document::default());
        let wall = building.push(Some(WALL), 10, cx)?;
        if slab {
            building.push(Some(SLAB), 11, cx)?;
        }
        building.components.identity.insert(
            wall,
            zumen_building::Identity {
                name: Some(name.to_owned()),
                ..zumen_building::Identity::default()
            },
        );
        Some(building)
    });
    built
}

/// 建物モデルの要素の ID とハッシュ（決定 0207）
fn building_version(building: &Building, document: &str) -> Hashes {
    let mut elements = BTreeMap::new();
    for index in 0..building.elements().len() {
        let element = ElementRef(index as u32);
        if let (Some(id), Some(hash)) = (
            building.id(element),
            zumen_building::Hash::element(building, element),
        ) {
            elements.insert(id.to_string(), hash.to_string());
        }
    }
    Hashes {
        document: document.to_owned(),
        elements,
    }
}

fn reference(version: &Hashes, id: &str) -> Reference {
    Reference {
        id: id.to_owned(),
        hash: version.element(id),
    }
}

fn target(id: &str, format: &str, version: &Hashes) -> Target {
    Target {
        id: id.to_owned(),
        format: format.to_owned(),
        hash: version.hash(),
        name: None,
        extra: Vec::new(),
    }
}

/// 図面の指摘を 1 つ、建物モデルの指摘を 1 つ持つ文書
fn sample(plan: &Hashes, plan_ids: &[String], model: &Hashes) -> Option<Review> {
    let mut review = Review::default();
    review.targets.push(target("plan", "dxf", plan));
    review.targets.push(target("model", "ifc", model));
    let (adopted, diag) = with(|cx| {
        adopt(
            &review,
            &target("plan", "dxf", plan),
            &[Adopted {
                rule: "layer-name".to_owned(),
                diagnostic: zumen_json::parse(br#"{"code":"profile.layer.name","severity":"warning","count":1,"at":{},"params":{}}"#).ok()?,
                elements: plan_ids.iter().map(|id| reference(plan, id)).collect(),
            }],
            cx,
        )
        .ok()
    });
    assert!(found(&diag).is_empty());
    let (mut review, added) = adopted?;
    assert_eq!(added.len(), 1);
    let mut topic = review.topics.first()?.clone();
    topic.id = "00000000-0000-8000-8000-000000000001".to_owned();
    topic.title = "壁の名前".to_owned();
    topic.locations = vec![Location {
        id: "wall".to_owned(),
        target: "model".to_owned(),
        place: Place::Space {
            camera: None,
            selected: vec![reference(model, WALL)],
            visible: Some(true),
            exceptions: Vec::new(),
            colored: vec![zumen_review::Colored {
                color: "ff0000".to_owned(),
                elements: vec![reference(model, SLAB)],
            }],
            clips: Vec::new(),
        },
        carried: None,
        extra: Vec::new(),
    }];
    topic.origin = None;
    review.topics.push(topic);
    review.markups.push(zumen_review::Markup {
        id: "m1".to_owned(),
        kind: MarkupKind::Cloud,
        target: "plan".to_owned(),
        view: None,
        points: vec![[0.0, 0.0], [10.0, 5.0]],
        text: None,
        topic: review.topics.first().map(|t| t.id.clone()),
        carried: None,
        extra: Vec::new(),
    });
    Some(review)
}

#[test]
fn saves_and_reads_back_with_unknown_keys() {
    let plan_drawing = drawing(&[(0.0, 0x2f), (5.0, 0x30)]);
    let (plan, ids) = drawing_version(&plan_drawing, b"plan v1");
    let model = building_version(&building(true, "Wall").unwrap(), "v1:model");
    let mut review = sample(&plan, &ids, &model).unwrap();
    review.extra.push(("app".to_owned(), Value::Bool(true)));
    review.topics[0]
        .extra
        .push(("x_note".to_owned(), Value::String("保つ".to_owned())));
    let bytes = review.to_bytes();
    let (read, diag) = with(|cx| Review::read(&bytes, cx));
    assert!(found(&diag).is_empty());
    let read = read.unwrap();
    assert_eq!(read, review);
    assert_eq!(read.to_bytes(), bytes);
    assert_eq!(read.topics[0].status, Status::Open);
    assert_eq!(
        read.topics[0].locations[0].id,
        format!("{}/l1", read.topics[0].id)
    );
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with(r#"{"format":"zumen-review","version":1,"#));
    assert!(text.ends_with(r#""app":true}"#));
}

#[test]
fn adopted_topic_ids_come_from_the_inputs() {
    let plan_drawing = drawing(&[(0.0, 0x2f)]);
    let (plan, ids) = drawing_version(&plan_drawing, b"plan v1");
    let model = building_version(&building(true, "Wall").unwrap(), "v1:model");
    let first = sample(&plan, &ids, &model).unwrap();
    let second = sample(&plan, &ids, &model).unwrap();
    let id = &first.topics[0].id;
    assert_eq!(id, &second.topics[0].id);
    assert_eq!(id.len(), 36);
    assert_eq!(id.chars().nth(14), Some('8'));
    assert_eq!(first.topics[0].title, "layer-name: profile.layer.name");
    // 取り込み済みの指摘は足さない
    let target = first.targets[0].clone();
    let finding = Adopted {
        rule: "layer-name".to_owned(),
        diagnostic: first.topics[0].origin.as_ref().unwrap().diagnostic.clone(),
        elements: Vec::new(),
    };
    let (again, diag) = with(|cx| adopt(&first, &target, &[finding.clone(), finding], cx));
    assert!(found(&diag).is_empty());
    let (again, added) = again.unwrap();
    assert_eq!(added.len(), 1);
    assert_eq!(again.topics.len(), first.topics.len() + 1);
}

#[test]
fn carries_drawing_and_building_references_to_new_versions() {
    let (plan, ids) = drawing_version(&drawing(&[(0.0, 0x2f), (5.0, 0x30)]), b"plan v1");
    let model = building_version(&building(true, "Wall").unwrap(), "v1:model");
    let review = sample(&plan, &ids, &model).unwrap();

    // 図面: 1 本目は同じ、2 本目は動き、3 本目は増えた
    let (plan2, _) = drawing_version(
        &drawing(&[(0.0, 0x2f), (7.0, 0x30), (9.0, 0x31)]),
        b"plan v2",
    );
    let (carried, diag) = with(|cx| carry(&review, "plan", &plan2, cx));
    assert!(found(&diag).is_empty());
    let (carried, states) = carried.unwrap();
    let states: Vec<_> = states.iter().map(|s| s.state).collect();
    assert_eq!(states, [Change::Unchanged, Change::Changed]);
    assert_eq!(carried.targets[0].hash, plan2.hash());
    let location = &carried.topics[0].locations[0];
    let record = location.carried.as_ref().unwrap();
    assert_eq!(record.from, plan.hash());
    assert_eq!(record.state, Change::Changed);
    let Place::Plane { elements, .. } = &location.place else {
        panic!("2D の場所");
    };
    assert_eq!(elements[1].hash, plan2.element(&ids[1]));
    assert_eq!(
        carried.markups[0].carried.as_ref().unwrap().state,
        Change::Unchanged
    );
    // 建物モデルの場所は、別の対象なので触らない
    assert_eq!(
        carried.topics[1].locations[0],
        review.topics[1].locations[0]
    );

    // 建物モデル: 壁の名前が変わり、床がなくなった
    let model2 = building_version(&building(false, "Wall 2").unwrap(), "v1:model2");
    let (carried, diag) = with(|cx| carry(&review, "model", &model2, cx));
    assert!(found(&diag).is_empty());
    let (carried, states) = carried.unwrap();
    let states: Vec<_> = states
        .iter()
        .map(|s| (s.element.as_str(), s.state))
        .collect();
    assert_eq!(states, [(WALL, Change::Changed), (SLAB, Change::Removed)]);
    let location = &carried.topics[1].locations[0];
    assert_eq!(location.carried.as_ref().unwrap().state, Change::Changed);
    let Place::Space { colored, .. } = &location.place else {
        panic!("3D の場所");
    };
    // なくなった参照は、元のハッシュのまま残す
    assert_eq!(colored[0].elements[0].hash, model.element(SLAB));

    // 同じ版へ移すと、どれも変わっていない
    let (same, _) = with(|cx| carry(&review, "model", &model, cx));
    let (_, states) = same.unwrap();
    assert!(states.iter().all(|s| s.state == Change::Unchanged));
}

#[test]
fn all_removed_references_remove_the_location() {
    let (plan, ids) = drawing_version(&drawing(&[(0.0, 0x2f)]), b"plan v1");
    let model = building_version(&building(true, "Wall").unwrap(), "v1:model");
    let review = sample(&plan, &ids, &model).unwrap();
    let (empty, _) = drawing_version(&drawing(&[]), b"plan v2");
    let (carried, _) = with(|cx| carry(&review, "plan", &empty, cx));
    let (carried, _) = carried.unwrap();
    let record = carried.topics[0].locations[0].carried.as_ref().unwrap();
    assert_eq!(record.state, Change::Removed);
}

#[test]
fn cannot_carry_or_adopt_into_unknown_or_stale_targets() {
    let (plan, ids) = drawing_version(&drawing(&[(0.0, 0x2f)]), b"plan v1");
    let model = building_version(&building(true, "Wall").unwrap(), "v1:model");
    let review = sample(&plan, &ids, &model).unwrap();
    let (result, diag) = with(|cx| carry(&review, "section", &plan, cx));
    assert!(result.is_err());
    assert_eq!(found(&diag), [codes::REVIEW_TARGET_MISSING]);

    let (plan2, _) = drawing_version(&drawing(&[(0.0, 0x2f)]), b"plan v2");
    let (result, diag) = with(|cx| adopt(&review, &target("plan", "dxf", &plan2), &[], cx));
    assert!(result.is_err());
    assert_eq!(found(&diag), [codes::REVIEW_TARGET_STALE]);
}

#[test]
fn rejects_invalid_documents() {
    let cases: [(&str, Code); 9] = [
        ("{", codes::REVIEW_FILE_INVALID),
        (r#"{"format":"zumen-review"}"#, codes::REVIEW_FILE_INVALID),
        (
            r#"{"format":"bcf","version":1}"#,
            codes::REVIEW_FORMAT_UNSUPPORTED,
        ),
        (
            r#"{"format":"zumen-review","version":2}"#,
            codes::REVIEW_VERSION_UNSUPPORTED,
        ),
        (
            r#"{"format":"zumen-review","version":1,"topics":[{"id":"a","title":"","status":"waiting"}]}"#,
            codes::REVIEW_FILE_INVALID,
        ),
        (
            r#"{"format":"zumen-review","version":1,"topics":[{"id":"a","title":"","status":"open"},{"id":"a","title":"","status":"open"}]}"#,
            codes::REVIEW_ID_EXISTS,
        ),
        (
            r#"{"format":"zumen-review","version":1,"topics":[{"id":"a","title":"","status":"open","locations":[{"id":"l","target":"plan","kind":"2d"}]}]}"#,
            codes::REVIEW_REFERENCE_MISSING,
        ),
        (
            r#"{"format":"zumen-review","version":1,"topics":[{"id":"a","title":"","status":"open","related":["b"]}]}"#,
            codes::REVIEW_REFERENCE_MISSING,
        ),
        (
            r#"{"format":"zumen-review","version":1,"targets":[{"id":"plan","format":"dxf","hash":"h"}],"topics":[{"id":"a","title":"","status":"open","locations":[{"id":"l","target":"plan","kind":"2d","elements":[{"id":"e","hash":"h","note":1}]}]}]}"#,
            codes::REVIEW_FILE_INVALID,
        ),
    ];
    for (text, code) in cases {
        let (result, diag) = with(|cx| Review::read(text.as_bytes(), cx));
        assert!(result.is_err(), "{text}");
        assert_eq!(found(&diag), [code], "{text}");
    }
    let (_, diag) = with(|cx| {
        Review::read(br#"{"format":"zumen-review","version":1,"topics":[{"id":"a","title":"","status":"waiting"}]}"#, cx)
    });
    assert_eq!(
        diag.iter().next().and_then(|d| d.param("pointer").cloned()),
        Some(zumen_base::Value::Text("/topics/0/status".to_owned()))
    );
}
