use super::*;

// 読み取る図面の本体を用意し、旧版と現行のハッシュを同じバイト列から別々に求める。
const DRAWING: &[u8] =
    b"0\nSECTION\n2\nENTITIES\n0\nPOINT\n5\nA\n10\n4\n20\n2\n0\nENDSEC\n0\nEOF\n";
const TOPIC_ID: &str = "00000000-0000-4000-8000-000000000001";
const OTHER_TOPIC: &str = "00000000-0000-4000-8000-000000000002";

fn map() -> Mapping {
    Mapping {
        target: "plan".into(),
        default_view: Some("sheet/0".into()),
        views: BTreeMap::from([("二階".into(), "sheet/1".into())]),
        topic_ids: BTreeMap::from([("old-pin".into(), TOPIC_ID.into())]),
    }
}

fn legacy_source(items: &str) -> String {
    let sha = crate::sha256::hex(DRAWING);
    format!(
        r#"{{"format":"zumen-markup","version":1,"drawing":{{"sha256":"{sha}","format":"dxf"}},"items":[{items}],"targets":["original"],"private/field~":true}}"#
    )
}

fn legacy(items: &str) -> Value {
    json::parse(legacy_source(items).as_bytes()).unwrap()
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
    assert_eq!(review.targets[0].format, "dxf");
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
            r#"{"id":"unmapped","kind":"finding","position":[0,1],"status":"open"}"#,
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
    ] {
        assert_eq!(
            migrate(&legacy(items), DRAWING, &map)
                .err()
                .unwrap()
                .pointer(),
            pointer
        );
    }
    let no_default = Mapping {
        default_view: None,
        ..map
    };
    assert_eq!(
        migrate(&legacy(finding), DRAWING, &no_default)
            .err()
            .unwrap()
            .pointer(),
        "/items/0/view"
    );
}

#[test]
fn rejects_source_and_destination_id_collisions() {
    let map = map();
    for id in ["old-pin", TOPIC_ID] {
        let items = format!(
            r#"{{"id":"old-pin","kind":"finding","position":[0,0],"status":"open"}},{{"id":"{id}","kind":"finding","position":[1,1],"status":"closed"}}"#
        );
        assert_eq!(
            migrate(&legacy(&items), DRAWING, &map)
                .err()
                .unwrap()
                .pointer(),
            "/items/1/id"
        );
    }
}

// コマンドは、読み直せた文書だけを新しいファイルに書く。既存のファイルを上書きせず、失敗したら出力を残さない。
#[test]
fn command_writes_only_a_new_document_that_reads_back() {
    let dir = crate::formats::tests::TempDir::new();
    let path = |name: &str| dir.0.join(name).to_str().unwrap().to_owned();
    let items = r#"{"id":"note","kind":"text","position":[1,2],"text":"注記"},{"id":"old-pin","kind":"finding","view":"二階","position":[0,0],"status":"open"}"#;
    fs::write(path("markup.json"), legacy_source(items)).unwrap();
    fs::write(path("drawing.dxf"), DRAWING).unwrap();
    let mapping = format!(
        "target = \"plan\"\ndefault_view = \"sheet/0\"\n\n[views]\n\"二階\" = \"sheet/1\"\n\n[topic_ids]\n\"old-pin\" = \"{TOPIC_ID}\"\n"
    );
    fs::write(path("mapping.toml"), &mapping).unwrap();
    let run_to = |drawing: &str, mapping: &str, output: &str| {
        run(
            &path("markup.json"),
            &path(drawing),
            &path(mapping),
            &path(output),
        )
    };

    run_to("drawing.dxf", "mapping.toml", "review.json").unwrap();
    let written = fs::read(path("review.json")).unwrap();
    let review = read_review(&written);
    assert_eq!((review.markups.len(), review.topics.len()), (1, 1));
    assert_eq!(review.topics[0].id, TOPIC_ID);
    assert_eq!(review.targets[0].hash, Hash::document(DRAWING).to_string());

    // 同じ出力先には書かない。前の文書はそのまま残る。
    assert!(run_to("drawing.dxf", "mapping.toml", "review.json").is_err());
    assert_eq!(fs::read(path("review.json")).unwrap(), written);

    // 旧版の記録と違う図面と、未知のキーを持つ対応表では、出力を作らない。
    let mut other = DRAWING.to_vec();
    let last = other.len() - 2;
    other[last] = b'X';
    fs::write(path("other.dxf"), other).unwrap();
    let error = run_to("other.dxf", "mapping.toml", "mismatch.json").unwrap_err();
    assert!(error.contains("/drawing/sha256"), "{error}");
    fs::write(
        path("unknown.toml"),
        format!("{mapping}\n[extra]\nkey = 1\n"),
    )
    .unwrap();
    assert!(run_to("drawing.dxf", "unknown.toml", "unknown.json").is_err());
    for output in ["mismatch.json", "unknown.json"] {
        assert!(!dir.0.join(output).exists(), "{output}");
    }
}
