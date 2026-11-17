//! 公開 entities で、形の値で表せない要素の geometry を黙って省かず、種類と元のレコードを診断する（#884）。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力と期待する応答を読むテスト。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::Value;

/// 線分 1 本と、幅のある LWPOLYLINE 1 本（幅のある折れ線は geometry の値で表せない）の DXF
const INPUT: &str = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n5\n10\n8\n0\n10\n0\n20\n0\n11\n10\n21\n5\n\
0\nLWPOLYLINE\n5\n11\n8\n0\n90\n2\n70\n0\n43\n0.5\n10\n0\n20\n0\n10\n4\n20\n3\n\
0\nENDSEC\n0\nEOF\n";

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

fn entities(session: &mut Session, fields: &str) -> Value {
    let (status, result) = call(
        session,
        &format!(r#"{{"v":1,"op":"entities","doc":"a","fields":{fields}}}"#),
    );
    assert_eq!(status, Status::Ok, "{result:?}");
    result
}

/// この試験の診断（図面の単位などの、読みの診断を除く）
fn diagnostics(result: &Value) -> Vec<&Value> {
    let Some(Value::Array(items)) = result.pointer("/diagnostics") else {
        panic!("diagnostics: {result:?}");
    };
    let code = Value::String("drawing.entity-geometry.unsupported".to_owned());
    items
        .iter()
        .filter(|item| item.pointer("/code") == Some(&code))
        .collect()
}

#[test]
fn omitted_geometry_is_reported_with_kind_and_record() {
    let mut session = Session::new(b"").unwrap();
    session.push(b"a", INPUT.as_bytes()).unwrap();
    let opened = call(&mut session, r#"{"v":1,"op":"open","input":"a"}"#);
    assert_eq!(opened.0, Status::Ok, "{opened:?}");

    let result = entities(&mut session, r#"["id","kind","geometry"]"#);
    let Some(Value::Array(items)) = result.pointer("/result/items") else {
        panic!("items: {result:?}");
    };
    assert_eq!(items.len(), 2, "{result:?}");
    assert!(items[0].pointer("/geometry").is_some(), "{result:?}");
    assert!(items[1].pointer("/geometry").is_none(), "{result:?}");
    let found = diagnostics(&result);
    assert_eq!(found.len(), 1, "{result:?}");
    assert_eq!(
        found[0].pointer("/severity"),
        Some(&Value::String("warning".to_owned()))
    );
    assert_eq!(
        found[0].pointer("/params/kind"),
        Some(&Value::String("polyline".to_owned()))
    );
    assert!(found[0].pointer("/at/0/record").is_some(), "{result:?}");

    // geometry を求めなければ、診断も出さない
    let plain = entities(&mut session, r#"["id","kind"]"#);
    assert!(diagnostics(&plain).is_empty(), "{plain:?}");
}
