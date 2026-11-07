use super::*;

// 読み取る図面の本体を用意し、旧版と現行のハッシュを同じバイト列から別々に求める。
const DRAWING: &[u8] =
    b"0\nSECTION\n2\nENTITIES\n0\nPOINT\n5\nA\n10\n4\n20\n2\n0\nENDSEC\n0\nEOF\n";
const TOPIC_ID: &str = "00000000-0000-4000-8000-000000000001";
const OTHER_TOPIC: &str = "00000000-0000-4000-8000-000000000002";

fn moved(view: &str, scale: f64, offset: [f64; 2]) -> View {
    View::Moved(Moved {
        view: view.into(),
        scale,
        offset,
    })
}

/// JWW の用紙は、旧版と現行で座標系が同じ
fn map() -> Mapping {
    Mapping {
        target: "plan".into(),
        default_view: Some(View::Same("sheet/0".into())),
        views: BTreeMap::from([("二階".into(), View::Same("sheet/1".into()))]),
        topic_ids: BTreeMap::from([("old-pin".into(), TOPIC_ID.into())]),
    }
}

/// m 単位の DXF のモデル空間と、余白を持つレイアウト。旧版はモデル空間を mm にし、
/// 原点を (35000, 120000) m に寄せた。レイアウトは余白と印刷の原点の分だけずれる
fn dxf_map() -> Mapping {
    Mapping {
        target: "plan".into(),
        default_view: Some(moved("space/0", 0.001, [35000.0, 120000.0])),
        views: BTreeMap::from([("Layout1".into(), moved("sheet/0", 1.0, [-15.0, -25.0]))]),
        topic_ids: BTreeMap::new(),
    }
}

fn legacy_source_as(format: &str, items: &str) -> String {
    let sha = crate::sha256::hex(DRAWING);
    format!(
        r#"{{"format":"zumen-markup","version":1,"drawing":{{"sha256":"{sha}","format":"{format}"}},"items":[{items}],"targets":["original"],"private/field~":true}}"#
    )
}

fn legacy_source(items: &str) -> String {
    legacy_source_as("jww", items)
}

fn legacy_as(format: &str, items: &str) -> Value {
    json::parse(legacy_source_as(format, items).as_bytes()).unwrap()
}

fn legacy(items: &str) -> Value {
    legacy_as("jww", items)
}

fn failure(source: &Value, map: &Mapping) -> String {
    migrate(source, DRAWING, map).err().unwrap().pointer()
}

fn read_review(bytes: &[u8]) -> Review {
    let mut budget = Budget::new(Limits::default());
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let review = Review::read(bytes, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)).unwrap();
    assert_eq!(diagnostics.iter().count(), 0);
    review
}

#[test]
fn all_kinds_round_trip_and_report_values_without_a_current_field() {
    let items = r##"
        {"id":"line","kind":"redline","points":[[1,2],[-3,4]],"arrows":"end","target":"old"},
        {"id":"cloud","kind":"cloud","view":"二階","points":[[3,0],[0,5],[-2,0]],"text":"境界"},
        {"id":"note","kind":"text","position":[20,25],"text":"注記","color":"#FF000080"},
        {"id":"picture","kind":"photo","position":[2,4],"photo":"https://example.invalid/photo","text":"説明"},
        {"id":"old-pin","kind":"finding","view":"二階","position":[-2,3],"points":[[2,1],[-3,5],[0,-3]],"text":"要確認","status":"resolved","author":"A","createdAt":"2026-11-01T00:00:00Z","entities":["A"],"carried":{"from":"0000000000000000000000000000000000000000000000000000000000000000","state":"changed"}},
        {"id":"00000000-0000-4000-8000-000000000002","kind":"finding","position":[3,3],"status":"closed"}
    "##;
    let result = migrate(&legacy(items), DRAWING, &map()).unwrap();
    let bytes = result.review.to_bytes();
    let review = read_review(&bytes);
    assert_eq!(review.to_bytes(), bytes);
    assert_eq!(review.targets[0].hash, Hash::document(DRAWING).to_string());
    assert_eq!(review.targets[0].format, "jww");
    assert_eq!(
        review
            .markups
            .iter()
            .map(|m| (m.kind, m.id.as_str(), m.view.as_deref()))
            .collect::<Vec<_>>(),
        [
            (MarkupKind::Redline, "line", Some("sheet/0")),
            (MarkupKind::Cloud, "cloud", Some("sheet/1")),
            (MarkupKind::Text, "note", Some("sheet/0")),
            (MarkupKind::Photo, "picture", Some("sheet/0")),
        ]
    );
    assert!(review.markups.iter().all(|m| m.target == "plan"));
    assert_eq!(review.markups[0].points, [[1.0, 2.0], [-3.0, 4.0]]);
    assert_eq!(review.markups[1].text.as_deref(), Some("境界"));
    assert_eq!(review.markups[2].text.as_deref(), Some("注記"));
    assert_eq!(
        review.markups[3].text.as_deref(),
        Some("https://example.invalid/photo")
    );
    assert_eq!(review.topics[0].id, TOPIC_ID);
    assert_eq!(review.topics[0].title, "要確認");
    assert_eq!(review.topics[0].status, Status::Resolved);
    assert_eq!(review.topics[0].author.as_deref(), Some("A"));
    assert_eq!(
        review.topics[0].created.as_deref(),
        Some("2026-11-01T00:00:00Z")
    );
    assert_eq!(review.topics[1].id, OTHER_TOPIC);
    let location = &review.topics[0].locations[0];
    assert_eq!((&*location.id, &*location.target), ("old-pin", "plan"));
    assert_eq!(
        location.place,
        Place::Plane {
            view: Some("sheet/1".into()),
            pin: Some([-2.0, 3.0]),
            extent: Some([-3.0, -3.0, 2.0, 5.0]),
            elements: vec![]
        }
    );
    let source = legacy(items);
    let output = json::parse(&bytes).unwrap();
    for (pointer, field) in [
        ("/items/0/arrows", "/markups/0/legacy.arrows"),
        ("/items/0/target", "/markups/0/legacy.target"),
        ("/items/2/color", "/markups/2/legacy.color"),
        ("/items/3/text", "/markups/3/legacy.text"),
        ("/items/4/points", "/topics/0/legacy.points"),
        ("/items/4/entities", "/topics/0/legacy.entities"),
        ("/items/4/carried", "/topics/0/legacy.carried"),
        ("/targets", "/legacy.targets"),
        ("/private~1field~0", "/legacy.private~1field~0"),
    ] {
        assert!(result.retained.contains(&pointer.into()));
        assert_eq!(
            output.pointer(field).unwrap(),
            source.pointer(pointer).unwrap()
        );
    }
    assert!(result.retained.contains(&"/items".into()));
    assert_eq!(result.retained.len(), 10);
}

// 旧版の JSON Schema（zumen-legacy の 7c1c400e）のキーと、知らないキーをすべて持つ項目
#[test]
fn every_legacy_key_moves_to_a_field_or_is_kept_and_reported() {
    let common = r##""category":"check","color":"#ff0000","lineWidth":0.35,"entities":["A"],"carried":{"from":"00","state":"changed"},"data":{"k":1},"x-application":true"##;
    let dated =
        r#""author":"A","createdAt":"2026-11-01T00:00:00Z","updatedAt":"2026-11-02T00:00:00Z""#;
    let items = format!(
        r#"{{"id":"line","kind":"redline","view":"二階","points":[[0,0],[1,1]],"closed":true,"arrows":"both","text":"線",{dated},{common}}},
        {{"id":"cloud","kind":"cloud","points":[[0,0],[1,0],[0,1]],"arcLength":5,{dated},{common}}},
        {{"id":"note","kind":"text","position":[0,0],"text":"注記","height":2.5,"angle":90,{dated},{common}}},
        {{"id":"pin","kind":"finding","position":[0,0],"points":[[0,0],[1,0],[0,1]],"status":"open","label":"1","priority":"high","text":"題",{dated},{common}}},
        {{"id":"picture","kind":"photo","position":[0,0],"photo":"p","direction":45,"takenAt":"2026-11-01T00:00:00Z","text":"説明",{dated},{common}}}"#
    );
    let sha = crate::sha256::hex(DRAWING);
    let source = json::parse(
        format!(
            r#"{{"format":"zumen-markup","version":1,"drawing":{{"sha256":"{sha}","format":"jww","x-extension":1}},"items":[{items}],"data":{{"k":1}},"x-extension":{{"a":1}}}}"#
        )
        .as_bytes(),
    )
    .unwrap();
    let result = migrate(&source, DRAWING, &map()).unwrap();
    let output = json::parse(&result.review.to_bytes()).unwrap();
    // 現行のフィールドに移すキー。ほかは legacy. のキーに残し、元の位置を報告する。
    fn current_keys(kind: &str) -> &'static [&'static str] {
        match kind {
            "redline" | "cloud" => &["id", "kind", "view", "points", "text"],
            "text" => &["id", "kind", "view", "position", "text"],
            "photo" => &["id", "kind", "view", "position", "photo"],
            _ => &[
                "id",
                "kind",
                "view",
                "position",
                "status",
                "text",
                "author",
                "createdAt",
                "updatedAt",
            ],
        }
    }
    let places = [
        "/markups/0",
        "/markups/1",
        "/markups/2",
        "/topics/0",
        "/markups/3",
    ];
    let mut checked = 0;
    for (index, place) in places.into_iter().enumerate() {
        let Some(Value::Object(item)) = source.pointer(&format!("/items/{index}")) else {
            panic!("{index}");
        };
        let Some(Value::String(kind)) = item.get("kind") else {
            panic!("{index}");
        };
        for (key, value) in item.iter() {
            if current_keys(kind).contains(&key) {
                continue;
            }
            let pointer = format!("/items/{index}/{key}");
            assert!(result.retained.contains(&pointer), "{pointer}");
            assert_eq!(
                output.pointer(&format!("{place}/legacy.{key}")),
                Some(value),
                "{pointer}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 58);
    for (pointer, field) in [
        ("/data", "/legacy.data"),
        ("/x-extension", "/legacy.x-extension"),
        ("/drawing/x-extension", "/targets/0/legacy.x-extension"),
    ] {
        assert!(result.retained.contains(&pointer.into()), "{pointer}");
        assert_eq!(output.pointer(field), source.pointer(pointer), "{pointer}");
    }
    // 項目のキーと、文書・図面の知らないキー、描画順のほかには報告しない。
    assert!(result.retained.contains(&"/items".into()));
    assert_eq!(result.retained.len(), checked + 4);
    assert_eq!(
        output.pointer("/markups/0/legacy.view"),
        Some(&Value::String("二階".into()))
    );
}

#[test]
fn preserves_the_legacy_drawing_order_and_the_original_view_names() {
    let source = legacy(
        r#"{"id":"first","kind":"photo","position":[0,0],"photo":"p"},
        {"id":"old-pin","kind":"finding","view":"二階","position":[0,0],"status":"open"},
        {"id":"last","kind":"text","view":"二階","position":[0,0],"text":"A"}"#,
    );
    let result = migrate(&source, DRAWING, &map()).unwrap();
    let output = json::parse(&result.review.to_bytes()).unwrap();
    assert_eq!(
        output.pointer("/legacy_item_order"),
        Some(&json::parse(br#"["first","old-pin","last"]"#).unwrap())
    );
    assert_eq!(
        output.pointer("/topics/0/legacy.view"),
        source.pointer("/items/1/view")
    );
    assert_eq!(
        output.pointer("/markups/1/legacy.view"),
        source.pointer("/items/2/view")
    );
    assert!(output.pointer("/markups/0/legacy.view").is_none());
}

#[test]
fn points_move_by_the_declared_transform_and_keep_the_legacy_values() {
    let items = r#"
        {"id":"line","kind":"redline","points":[[1500,-250],[0,0]]},
        {"id":"note","kind":"text","view":"Layout1","position":[100,50],"text":"注記"},
        {"id":"pin","kind":"finding","position":[1000,2000],"points":[[0,0],[2000,0],[0,1000]],"status":"open"},
        {"id":"picture","kind":"photo","view":"Layout1","position":[15,25],"photo":"p"}
    "#;
    let source = legacy_as("dxf", items);
    let result = migrate(&source, DRAWING, &dxf_map()).unwrap();
    let bytes = result.review.to_bytes();
    let review = read_review(&bytes);
    assert_eq!(review.targets[0].format, "dxf");
    assert_eq!(review.markups[0].view.as_deref(), Some("space/0"));
    assert_eq!(
        review.markups[0].points,
        [[35001.5, 119999.75], [35000.0, 120000.0]]
    );
    assert_eq!(review.markups[1].view.as_deref(), Some("sheet/0"));
    assert_eq!(review.markups[1].points, [[85.0, 25.0]]);
    assert_eq!(review.markups[2].points, [[0.0, 0.0]]);
    assert_eq!(
        review.topics[0].locations[0].place,
        Place::Plane {
            view: Some("space/0".into()),
            pin: Some([35001.0, 120002.0]),
            extent: Some([35000.0, 120000.0, 35002.0, 120001.0]),
            elements: vec![]
        }
    );
    let output = json::parse(&bytes).unwrap();
    for (pointer, field) in [
        ("/items/0/points", "/markups/0/legacy.points"),
        ("/items/1/position", "/markups/1/legacy.position"),
        ("/items/2/position", "/topics/0/legacy.position"),
        ("/items/2/points", "/topics/0/legacy.points"),
        ("/items/3/position", "/markups/2/legacy.position"),
    ] {
        assert!(result.retained.contains(&pointer.into()), "{pointer}");
        assert_eq!(
            output.pointer(field).unwrap(),
            source.pointer(pointer).unwrap()
        );
    }
}

#[test]
fn unmoved_points_are_accepted_only_for_jww_sheets() {
    let text = r#"{"id":"a","kind":"text","position":[0,1],"text":"a"}"#;
    assert!(map().check_views("jww").is_ok());
    for format in ["dxf", "dwg", "sfc", "pdf"] {
        let error = map().check_views(format).unwrap_err();
        assert!(error.contains("default_view"), "{error}");
        assert_eq!(failure(&legacy_as(format, text), &map()), "/drawing/format");
    }
    let model = Mapping {
        views: BTreeMap::from([("Model".into(), View::Same("space/0".into()))]),
        ..map()
    };
    let error = model.check_views("jww").unwrap_err();
    assert!(error.contains(r#"views の "Model""#), "{error}");
    // 変換を書けば、ほかの形式も移せる。SXF の入力の種類は形式名にまとめる。
    let result = migrate(&legacy_as("sfz", text), DRAWING, &dxf_map()).unwrap();
    assert_eq!(result.review.targets[0].format, "sxf");
    let output = json::parse(&result.review.to_bytes()).unwrap();
    assert_eq!(
        output.pointer("/targets/0/legacy.format"),
        Some(&Value::String("sfz".into()))
    );
}

#[test]
fn points_that_overflow_the_transform_are_reported() {
    let map = Mapping {
        default_view: Some(moved("space/0", 1e300, [0.0, 0.0])),
        ..dxf_map()
    };
    for (items, pointer) in [
        (
            r#"{"id":"a","kind":"redline","points":[[0,0],[1e10,0]]}"#,
            "/items/0/points/1",
        ),
        (
            r#"{"id":"a","kind":"text","position":[0,-1e10],"text":"a"}"#,
            "/items/0/position",
        ),
        (
            r#"{"id":"a","kind":"finding","position":[0,0],"points":[[0,0],[1,0],[1e10,1]],"status":"open"}"#,
            "/items/0/points/2",
        ),
    ] {
        assert_eq!(failure(&legacy_as("dxf", items), &map), pointer);
    }
}

#[test]
fn findings_without_a_uuid_get_a_stable_id_from_the_drawing_and_item() {
    let sha = crate::sha256::hex(DRAWING);
    // Python の hashlib で別に求めた値
    assert_eq!(
        topic_uuid(&sha, "指摘-1"),
        "ebb55117-5741-8468-9897-1b8a9cd50944"
    );
    let items = r#"{"id":"指摘-1","kind":"finding","position":[0,0],"status":"open"},
        {"id":"指摘-2","kind":"finding","position":[0,0],"status":"open"}"#;
    let first = migrate(&legacy(items), DRAWING, &map()).unwrap();
    let again = migrate(&legacy(items), DRAWING, &map()).unwrap();
    assert_eq!(first.review.to_bytes(), again.review.to_bytes());
    assert_eq!(first.review.topics[0].id, topic_uuid(&sha, "指摘-1"));
    assert_ne!(first.review.topics[0].id, first.review.topics[1].id);
    for id in first.review.topics.iter().map(|topic| topic.id.as_bytes()) {
        assert_eq!(id[14], b'8');
        assert!(matches!(id[19], b'8' | b'9' | b'a' | b'b'));
    }
    assert!(first.mapped.is_empty());
    // 対応表は旧版の UUID より優先する。使った対応を記録する。
    let override_uuid = Mapping {
        topic_ids: BTreeMap::from([(OTHER_TOPIC.into(), TOPIC_ID.into())]),
        ..map()
    };
    let item =
        format!(r#"{{"id":"{OTHER_TOPIC}","kind":"finding","position":[0,0],"status":"open"}}"#);
    let result = migrate(&legacy(&item), DRAWING, &override_uuid).unwrap();
    assert_eq!(result.review.topics[0].id, TOPIC_ID);
    assert_eq!(result.mapped, BTreeSet::from([OTHER_TOPIC.to_owned()]));
}

#[test]
fn mapping_needs_bounded_ids_uuids_and_finite_transforms() {
    let check = |text: &str| {
        toml::from_str::<Mapping>(text)
            .map_err(|_| "TOML".to_owned())
            .and_then(|map| map.check())
    };
    let model = "target = \"plan\"\n[views]\n\"二階\" = \"sheet/1\"\nModel = { view = \"space/0\", scale = 0.001, offset = [35000, 120000] }\n";
    assert_eq!(check(model), Ok(()));
    let long = "a".repeat(1025);
    for (text, part) in [
        ("target = \"\"\n".to_owned(), "1〜1024"),
        (
            format!("target = \"plan\"\ndefault_view = \"{long}\"\n"),
            "1〜1024",
        ),
        (
            "target = \"plan\"\n[topic_ids]\na = \"not-a-uuid\"\n".to_owned(),
            "UUID",
        ),
        (
            "target = \"plan\"\n[views]\na = { view = \"\", scale = 1, offset = [0, 0] }\n"
                .to_owned(),
            "1〜1024",
        ),
    ] {
        let error = check(&text).unwrap_err();
        assert!(error.contains(part), "{text}: {error}");
    }
    for transform in [
        "scale = 0, offset = [0, 0]",
        "scale = -1, offset = [0, 0]",
        "scale = nan, offset = [0, 0]",
        "scale = inf, offset = [0, 0]",
        "scale = 1, offset = [inf, 0]",
        "scale = 1, offset = [0, nan]",
    ] {
        let text =
            format!("target = \"plan\"\ndefault_view = {{ view = \"space/0\", {transform} }}\n");
        let error = check(&text).unwrap_err();
        assert!(error.contains("default_view の scale"), "{text}: {error}");
    }
    for text in [
        "target = \"plan\"\ndefault_view = { view = \"space/0\", scale = 1, offset = [0, 0], rotation = 0 }\n",
        "target = \"plan\"\ndefault_view = { view = \"space/0\", scale = 1, offset = [0, 0, 0] }\n",
        "target = \"plan\"\ndefault_view = { view = \"space/0\", scale = 1 }\n",
    ] {
        assert_eq!(check(text), Err("TOML".to_owned()), "{text}");
    }
}

#[test]
fn wrong_file_scope_id_or_geometry_does_not_produce_a_document() {
    let finding = r#"{"id":"old-pin","kind":"finding","position":[0,1],"status":"open"}"#;
    let map = map();
    assert_eq!(
        migrate(&legacy(finding), b"another drawing", &map)
            .err()
            .unwrap()
            .pointer(),
        "/drawing/sha256"
    );
    for (items, pointer) in [
        (
            r#"{"id":"old-pin","kind":"finding","position":[0,1],"status":"new"}"#,
            "/items/0/status",
        ),
        (
            r#"{"id":"","kind":"finding","position":[0,1],"status":"open"}"#,
            "/items/0/id",
        ),
        (
            r#"{"id":"a","kind":"text","view":"wrong","position":[0,1],"text":"a"}"#,
            "/items/0/view",
        ),
        (
            r#"{"id":"a","kind":"text","position":[0,1,2],"text":"a"}"#,
            "/items/0/position",
        ),
        (
            r#"{"id":"a","kind":"cloud","points":[[0,0],[1,1]]}"#,
            "/items/0/points",
        ),
        (
            r#"{"id":"a","kind":"photo","position":[0,1]}"#,
            "/items/0/photo",
        ),
        (
            r#"{"id":"a","kind":"stamp","position":[0,1]}"#,
            "/items/0/kind",
        ),
    ] {
        assert_eq!(failure(&legacy(items), &map), pointer);
    }
    let no_default = Mapping {
        default_view: None,
        ..map
    };
    assert_eq!(failure(&legacy(finding), &no_default), "/items/0/view");
}

#[test]
fn item_count_is_limited() {
    let items = (0..=ITEM_LIMIT)
        .map(|index| format!(r#"{{"id":"{index}","kind":"text","position":[0,0],"text":"a"}}"#))
        .collect::<Vec<_>>();
    let result = migrate(&legacy(&items[..ITEM_LIMIT].join(",")), DRAWING, &map()).unwrap();
    assert_eq!(result.review.markups.len(), ITEM_LIMIT);
    assert_eq!(failure(&legacy(&items.join(",")), &map()), "/items");
}

#[test]
fn rejects_source_and_destination_id_collisions() {
    let map = map();
    for id in ["old-pin", TOPIC_ID] {
        let items = format!(
            r#"{{"id":"old-pin","kind":"finding","position":[0,0],"status":"open"}},{{"id":"{id}","kind":"finding","position":[1,1],"status":"closed"}}"#
        );
        assert_eq!(failure(&legacy(&items), &map), "/items/1/id");
    }
}

// コマンドは、読み直せた文書だけを新しいファイルに書く。既存のファイルを上書きせず、失敗したら出力を残さない。
#[test]
fn command_writes_only_a_new_document_that_reads_back() {
    let dir = crate::formats::tests::TempDir::new();
    let path = |name: &str| dir.0.join(name).to_str().unwrap().to_owned();
    let items = r#"{"id":"note","kind":"text","position":[1,2],"text":"注記"},{"id":"old-pin","kind":"finding","view":"二階","position":[0,0],"status":"open"}"#;
    fs::write(path("markup.json"), legacy_source(items)).unwrap();
    fs::write(path("dxf.json"), legacy_source_as("dxf", items)).unwrap();
    fs::write(path("drawing.jww"), DRAWING).unwrap();
    let mapping = format!(
        "target = \"plan\"\ndefault_view = \"sheet/0\"\n\n[views]\n\"二階\" = \"sheet/1\"\n\n[topic_ids]\n\"old-pin\" = \"{TOPIC_ID}\"\n"
    );
    fs::write(path("mapping.toml"), &mapping).unwrap();
    let run_to = |markup: &str, drawing: &str, mapping: &str, output: &str| {
        run(&path(markup), &path(drawing), &path(mapping), &path(output))
    };

    run_to("markup.json", "drawing.jww", "mapping.toml", "review.json").unwrap();
    let written = fs::read(path("review.json")).unwrap();
    let review = read_review(&written);
    assert_eq!((review.markups.len(), review.topics.len()), (1, 1));
    assert_eq!(review.topics[0].id, TOPIC_ID);
    assert_eq!(review.targets[0].hash, Hash::document(DRAWING).to_string());

    // 同じ出力先には書かない。前の文書はそのまま残る。
    assert!(run_to("markup.json", "drawing.jww", "mapping.toml", "review.json").is_err());
    assert_eq!(fs::read(path("review.json")).unwrap(), written);

    // 旧版の記録と違う図面、未知のキーを持つ対応表、使わない指摘の ID を持つ対応表、
    // 変換のない DXF のビューでは、出力を作らない。
    let mut other = DRAWING.to_vec();
    let last = other.len() - 2;
    other[last] = b'X';
    fs::write(path("other.jww"), other).unwrap();
    let error = run_to("markup.json", "other.jww", "mapping.toml", "mismatch.json").unwrap_err();
    assert!(error.contains("/drawing/sha256"), "{error}");
    fs::write(
        path("unknown.toml"),
        format!("{mapping}\n[extra]\nkey = 1\n"),
    )
    .unwrap();
    assert!(run_to("markup.json", "drawing.jww", "unknown.toml", "unknown.json").is_err());
    fs::write(
        path("unused.toml"),
        format!("{mapping}\"old-pni\" = \"{OTHER_TOPIC}\"\n"),
    )
    .unwrap();
    let error = run_to("markup.json", "drawing.jww", "unused.toml", "unused.json").unwrap_err();
    assert!(error.contains(r#""old-pni""#), "{error}");
    let error = run_to("dxf.json", "drawing.jww", "mapping.toml", "dxf-review.json").unwrap_err();
    assert!(error.contains("scale"), "{error}");
    for output in [
        "mismatch.json",
        "unknown.json",
        "unused.json",
        "dxf-review.json",
    ] {
        assert!(!dir.0.join(output).exists(), "{output}");
    }
}
