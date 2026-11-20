//! apply の copy で要素を複写する（#889）。写しの ID とハッシュ、元の要素の不変、拒む場合の原子性。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 公開サンプルと自作入力と、期待する応答だけを扱う。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

#[path = "../examples/apply_batch_tables_case.rs"]
mod cases;

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

fn ok(session: &mut Session, request: &str) -> Value {
    let (status, value) = call(session, request);
    assert_eq!(status, Status::Ok, "{value:?}");
    value
}

fn open(bytes: &[u8]) -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"a", bytes).unwrap();
    ok(&mut session, r#"{"v":1,"op":"open","input":"a"}"#);
    session
}

fn json(value: &Value) -> String {
    let mut writer = Writer::new();
    writer.value(value);
    String::from_utf8(writer.into_bytes()).unwrap()
}

fn items(value: &Value) -> &[Value] {
    let Some(Value::Array(items)) = value.pointer("/result/items") else {
        panic!("{value:?}")
    };
    items
}

const QUERY: &str = r#"{"v":1,"op":"entities","doc":"a","fields":["hash","layer","style","geometry","extensions"]}"#;

/// 実寸の空間の最初の要素
fn first_model(entities: &Value) -> &Value {
    items(entities)
        .iter()
        .find(|item| matches!(item.pointer("/id"), Some(Value::String(id)) if id.starts_with("space/0/")))
        .unwrap()
}

#[test]
fn copies_keep_the_source_and_return_new_ids_in_dxf_and_dwg() {
    for bytes in [
        include_bytes!("../../../samples/dwg/zumen-layout-r2000_ascii.dxf").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2004.dwg").as_slice(),
        include_bytes!("../../../samples/dwg/zumen-layout-r2018.dwg").as_slice(),
    ] {
        let mut session = open(bytes);
        let original = ok(&mut session, QUERY);
        let line = first_model(&original);
        let target = format!(
            r#""id":{},"hash":{}"#,
            json(line.pointer("/id").unwrap()),
            json(line.pointer("/hash").unwrap())
        );
        // 同じ要素を、元の位置と、ずらした位置に 2 回写す
        let applied = ok(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":[
                {{"kind":"copy",{target},"matrix":[1,0,0,0,1,0]}},
                {{"kind":"copy",{target},"matrix":[1,0,10,0,1,20]}}
            ]}}"#
            ),
        );
        let written = session.take(0).unwrap();
        let after = ok(&mut session, QUERY);
        assert_eq!(items(&after).len(), items(&original).len() + 2);
        for item in items(&original) {
            assert!(items(&after).contains(item), "{item:?}");
        }
        let found = |at: usize| {
            let id = applied
                .pointer(&format!("/result/applied/{at}/id"))
                .unwrap();
            let item = items(&after)
                .iter()
                .find(|item| item.pointer("/id") == Some(id))
                .unwrap();
            assert_eq!(
                item.pointer("/hash"),
                applied.pointer(&format!("/result/applied/{at}/hash"))
            );
            item
        };
        // 元の位置の写しは、ID のほかは元と同じ
        let same = found(0);
        for field in ["/hash", "/layer", "/style", "/geometry"] {
            assert_eq!(same.pointer(field), line.pointer(field), "{field}");
        }
        let moved = found(1);
        for field in ["/layer", "/style"] {
            assert_eq!(moved.pointer(field), line.pointer(field), "{field}");
        }
        assert_ne!(moved.pointer("/geometry"), line.pointer("/geometry"));
        assert_eq!(ok(&mut open(&written), QUERY), after);
    }
}

#[test]
fn copies_after_a_change_of_the_source_are_refused_whole() {
    let mut session = open(include_bytes!(
        "../../../samples/dwg/zumen-layout-r2000_ascii.dxf"
    ));
    let original = ok(&mut session, QUERY);
    let line = first_model(&original);
    let target = format!(
        r#""id":{},"hash":{}"#,
        json(line.pointer("/id").unwrap()),
        json(line.pointer("/hash").unwrap())
    );
    // 先の変更が動かした要素は、動かす前の値で写すことになるので拒む
    let (status, response) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[
            {{"kind":"transform",{target},"matrix":[1,0,5,0,1,0]}},
            {{"kind":"copy",{target},"matrix":[1,0,0,0,1,0]}}
        ]}}"#
        ),
    );
    assert_eq!(status, Status::Failed, "{response:?}");
    let text = json(&response);
    assert!(text.contains("change.copy.unsupported"), "{text}");
    assert!(
        text.contains(r#""source""#) && text.contains(r#""changed""#),
        "{text}"
    );
    assert!(session.take(0).is_none());
    assert_eq!(ok(&mut session, QUERY), original);
}

#[test]
fn copies_keep_the_source_and_return_new_ids_in_jww_and_sxf() {
    for (format, bytes, _) in cases::other_inputs() {
        let mut session = open(&bytes);
        let original = ok(&mut session, QUERY);
        let first = &items(&original)[0];
        let target = format!(
            r#""id":{},"hash":{}"#,
            json(first.pointer("/id").unwrap()),
            json(first.pointer("/hash").unwrap())
        );
        let applied = ok(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":[
                {{"kind":"copy",{target},"matrix":[1,0,0,0,1,0]}},
                {{"kind":"copy",{target},"matrix":[1,0,10,0,1,20]}}
            ]}}"#
            ),
        );
        let written = session.take(0).unwrap();
        let after = ok(&mut session, QUERY);
        assert_eq!(items(&after).len(), items(&original).len() + 2, "{format}");
        for item in items(&original) {
            assert!(items(&after).contains(item), "{format}: {item:?}");
        }
        let same_id = applied.pointer("/result/applied/0/id").unwrap();
        let same = items(&after)
            .iter()
            .find(|item| item.pointer("/id") == Some(same_id))
            .unwrap();
        assert_eq!(
            same.pointer("/hash"),
            applied.pointer("/result/applied/0/hash"),
            "{format}"
        );
        for field in ["/hash", "/layer", "/style", "/geometry"] {
            assert_eq!(
                same.pointer(field),
                first.pointer(field),
                "{format}: {field}"
            );
        }
        let moved_id = applied.pointer("/result/applied/1/id").unwrap();
        let moved = items(&after)
            .iter()
            .find(|item| item.pointer("/id") == Some(moved_id))
            .unwrap();
        assert_eq!(
            moved.pointer("/hash"),
            applied.pointer("/result/applied/1/hash"),
            "{format}"
        );
        assert_ne!(
            moved.pointer("/geometry"),
            first.pointer("/geometry"),
            "{format}"
        );
        assert_eq!(ok(&mut open(&written), QUERY), after, "{format}");
    }
}

#[test]
fn dwg_copies_keep_line_weight_and_rgb_and_refuse_r2000_rgb() {
    let mut session = open(include_bytes!(
        "../../../samples/dwg/zumen-layout-r2018.dwg"
    ));
    let original = ok(&mut session, QUERY);
    let first = first_model(&original);
    let restyled = ok(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"restyle","id":{},"hash":{},"line_weight":{{"mm":0.35}},"color":{{"rgb":[10,20,30]}}}}]}}"#,
            json(first.pointer("/id").unwrap()),
            json(first.pointer("/hash").unwrap())
        ),
    );
    let written = session.take(0).unwrap();
    let mut session = open(&written);
    let before = ok(&mut session, QUERY);
    let id = restyled.pointer("/result/applied/0/id").unwrap();
    let item = items(&before)
        .iter()
        .find(|item| item.pointer("/id") == Some(id))
        .unwrap();
    // R2018 DWG では線幅（370）と RGB 色（420）を保ったまま複写できる
    let copied = ok(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"copy","id":{},"hash":{},"matrix":[1,0,5,0,1,0]}}]}}"#,
            json(item.pointer("/id").unwrap()),
            json(item.pointer("/hash").unwrap())
        ),
    );
    let copied_bytes = session.take(0).unwrap();
    let after = ok(&mut session, QUERY);
    let copied_id = copied.pointer("/result/applied/0/id").unwrap();
    let copied_item = items(&after)
        .iter()
        .find(|el| el.pointer("/id") == Some(copied_id))
        .unwrap();
    assert_eq!(copied_item.pointer("/style"), item.pointer("/style"));
    assert_eq!(ok(&mut open(&copied_bytes), QUERY), after);

    // R2000 DWG へ RGB 色を持つ要素を from_doc で複写しようとすると、値を落とさず全体を拒む
    session
        .push(
            b"r2000",
            include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg"),
        )
        .unwrap();
    ok(&mut session, r#"{"v":1,"op":"open","input":"r2000"}"#);
    let r2000_before = ok(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"r2000","fields":["hash","layer","style","geometry"]}"#,
    );
    let (status, response) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"apply","doc":"r2000","edits":[{{"kind":"copy","from_doc":"a","id":{},"hash":{},"matrix":[1,0,5,0,1,0]}}]}}"#,
            json(item.pointer("/id").unwrap()),
            json(item.pointer("/hash").unwrap())
        ),
    );
    assert_eq!(status, Status::Failed, "{response:?}");
    let text = json(&response);
    assert!(
        text.contains("change.copy.unsupported") && text.contains(r#""true_color""#),
        "{text}"
    );
    assert!(session.take(0).is_none());
    assert_eq!(
        ok(
            &mut session,
            r#"{"v":1,"op":"entities","doc":"r2000","fields":["hash","layer","style","geometry"]}"#
        ),
        r2000_before
    );
}

#[test]
fn copies_support_place_and_from_doc_and_refuse_saf() {
    // 1. DXF での別配置先（place）と別文書（from_doc）複写
    let dxf = include_bytes!("../../../samples/dwg/zumen-layout-r2000_ascii.dxf");
    let mut session = open(dxf);
    session.push(b"b", dxf).unwrap();
    ok(&mut session, r#"{"v":1,"op":"open","input":"b"}"#);
    let before_b = ok(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"b","fields":["hash","layer","style","geometry"]}"#,
    );
    let line = first_model(&before_b);
    let applied = ok(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"apply","doc":"b","edits":[{{"kind":"copy","from_doc":"a","place":"space/0","id":{},"hash":{},"matrix":[1,0,10,0,1,20]}}]}}"#,
            json(line.pointer("/id").unwrap()),
            json(line.pointer("/hash").unwrap())
        ),
    );
    let written_b = session.take(0).unwrap();
    let after_b = ok(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"b","fields":["hash","layer","style","geometry"]}"#,
    );
    assert_eq!(items(&after_b).len(), items(&before_b).len() + 1);
    let copied_id = applied.pointer("/result/applied/0/id").unwrap();
    assert!(
        items(&after_b)
            .iter()
            .any(|el| el.pointer("/id") == Some(copied_id))
    );
    assert_eq!(
        ok(
            &mut open(&written_b),
            r#"{"v":1,"op":"entities","doc":"a","fields":["hash","layer","style","geometry"]}"#
        ),
        after_b
    );

    // 2. SXF で SAF 属性結合識別子（$$ATRF$$）を持つ要素の複写は saf で拒む
    let saf_sfc = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n/*SXF3\n#10 = layer_feature(\\'L\\','1')\nSXF3*/\n/*SXF3\n#20 = line_feature('1','1','1','1','0','0','1','0')\nSXF3*/\n/*SXF3\n#30 = sfig_org_feature(\\'$$ATRF$$10$$A.SAF\\','3')\nSXF3*/\n/*SXF3\n#40 = sfig_locate_feature('1',\\'$$ATRF$$10$$A.SAF\\','0','0','0','1','1')\nSXF3*/\n/*SXF3\n#50 = drawing_sheet_feature(\\'S\\','3','1','0','0')\nSXF3*/\nENDSEC;\nEND-ISO-10303-21;\n";
    let mut saf_session = open(saf_sfc);
    let saf_entities = ok(&mut saf_session, QUERY);
    let saf_el = items(&saf_entities)
        .iter()
        .find(|el| matches!(el.pointer("/id"), Some(Value::String(s)) if s.starts_with("sheet/")))
        .unwrap();
    let (status, response) = call(
        &mut saf_session,
        &format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"copy","id":{},"hash":{},"matrix":[1,0,5,0,1,0]}}]}}"#,
            json(saf_el.pointer("/id").unwrap()),
            json(saf_el.pointer("/hash").unwrap())
        ),
    );
    assert_eq!(status, Status::Failed, "{response:?}");
    let text = json(&response);
    assert!(
        text.contains("change.copy.unsupported") && text.contains(r#""saf""#),
        "{text}"
    );
}
