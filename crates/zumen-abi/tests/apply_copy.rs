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
fn copies_after_a_change_of_the_source_and_in_jww_sxf_are_refused_whole() {
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
    // JWW・SXF は、まだ写せない
    for (format, bytes, _) in cases::other_inputs() {
        let mut session = open(&bytes);
        let original = ok(&mut session, QUERY);
        let first = &items(&original)[0];
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"copy","id":{},"hash":{},"matrix":[1,0,0,0,1,0]}}]}}"#,
                json(first.pointer("/id").unwrap()),
                json(first.pointer("/hash").unwrap())
            ),
        );
        assert_eq!(status, Status::Failed, "{format}: {response:?}");
        assert!(
            json(&response).contains("change.edit.unsupported"),
            "{format}: {response:?}"
        );
        assert!(session.take(0).is_none());
        assert_eq!(ok(&mut session, QUERY), original);
    }
}

#[test]
fn dwg_copies_with_values_that_added_entities_cannot_hold_are_refused() {
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
    let (status, response) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"copy","id":{},"hash":{},"matrix":[1,0,5,0,1,0]}}]}}"#,
            json(item.pointer("/id").unwrap()),
            json(item.pointer("/hash").unwrap())
        ),
    );
    // 足した図形は、まだ線の太さと RGB の色を持てない。値を落として写さず、全体を拒む
    assert_eq!(status, Status::Failed, "{response:?}");
    assert!(
        json(&response).contains("dwg.change.unsupported"),
        "{response:?}"
    );
    assert!(session.take(0).is_none());
    assert_eq!(ok(&mut session, QUERY), before);
}
