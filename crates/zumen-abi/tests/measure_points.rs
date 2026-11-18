//! measure の点だけの距離。原点と相対座標を分けた点と、校正しない用紙の座標の距離（#888）。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作入力と、期待する応答だけを扱う。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

/// 実寸の空間に線分 1 本、用紙に線分 1 本を持つ、校正のない DXF
const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n3\n21\n4\n0\nLINE\n67\n1\n8\n0\n10\n0\n20\n0\n11\n10\n21\n0\n0\nENDSEC\n0\nEOF\n";

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

fn json(value: &Value) -> String {
    let mut writer = Writer::new();
    writer.value(value);
    String::from_utf8(writer.into_bytes()).unwrap()
}

fn open() -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"a", DXF.as_bytes()).unwrap();
    let (status, _) = call(&mut session, r#"{"v":1,"op":"open","input":"a"}"#);
    assert_eq!(status, Status::Ok);
    session
}

/// 点 `first` と `second` の距離を `from` で測る
fn measure(session: &mut Session, from: &str, first: &str, second: &str) -> (Status, Value) {
    call(
        session,
        &format!(
            r#"{{"v":1,"op":"measure","doc":"a","from":"{from}","quantity":"distance","first":{first},"second":{second}}}"#
        ),
    )
}

fn split(origin: [f64; 2], offset: [f64; 2]) -> String {
    format!(
        r#"{{"kind":"split_point","position":{{"origin":{{"x":{:?},"y":{:?}}},"offset":{{"x":{:?},"y":{:?}}}}}}}"#,
        origin[0], origin[1], offset[0], offset[1]
    )
}

fn point(x: f64, y: f64) -> String {
    format!(r#"{{"kind":"point","position":{{"x":{x:?},"y":{y:?}}}}}"#)
}

fn number(value: &Value, pointer: &str) -> f64 {
    match value.pointer(pointer) {
        Some(Value::Number(number)) => number.to_f64().unwrap(),
        other => panic!("{pointer}: {other:?} in {}", json(value)),
    }
}

fn text(value: &Value, pointer: &str) -> String {
    match value.pointer(pointer) {
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

/// 失敗の応答の最初の診断の code と reason
fn refused(status: Status, value: &Value) -> (String, String) {
    assert_eq!(status, Status::Failed, "{}", json(value));
    assert!(value.pointer("/result").is_none(), "{}", json(value));
    (
        text(value, "/diagnostics/0/code"),
        text(value, "/diagnostics/0/params/reason"),
    )
}

fn failed(reason: &str) -> (String, String) {
    ("drawing.element-measurement.failed".into(), reason.into())
}

#[test]
fn split_points_keep_small_offsets_from_large_and_different_origins() {
    let mut session = open();
    let (status, value) = measure(
        &mut session,
        "space/0",
        &split([1e16, 1e16], [0.0, 0.0]),
        &split([1e16, 1e16], [3.0, 4.0]),
    );
    assert_eq!(status, Status::Ok, "{}", json(&value));
    assert_eq!(number(&value, "/result/value"), 5.0);
    assert_eq!(text(&value, "/result/unit"), "millimeter");
    assert!(value.pointer("/result/sheet").is_none());
    assert_eq!(
        value.pointer("/result/approximate"),
        Some(&Value::Bool(false))
    );
    assert_eq!(number(&value, "/result/nearest_second/x"), 1e16 + 4.0);
    // 原点の違う 2 点。先に足すと 1e16 + 0.5 は 1e16 に丸まる
    let (status, value) = measure(
        &mut session,
        "space/0",
        &split([1e16, 1e16], [0.5, 0.0]),
        &split([0.0, 1e16], [1e16 + 4.0, 4.0]),
    );
    assert_eq!(status, Status::Ok, "{}", json(&value));
    assert_eq!(number(&value, "/result/value"), libm::hypot(3.5, 4.0));
    // 普通の点と分けた点を組み合わせる
    let (status, value) = measure(
        &mut session,
        "space/0",
        &point(1.0, 1.0),
        &split([1.0, 1.0], [3.0, 4.0]),
    );
    assert_eq!(status, Status::Ok, "{}", json(&value));
    assert_eq!(number(&value, "/result/value"), 5.0);
    // 普通の点どうしの距離は、これまでどおり
    let (status, value) = measure(&mut session, "space/0", &point(0.0, 0.0), &point(3.0, 4.0));
    assert_eq!(status, Status::Ok, "{}", json(&value));
    assert_eq!(number(&value, "/result/value"), 5.0);
}

#[test]
fn split_points_refuse_elements_overflow_stale_hashes_and_budgets() {
    let mut session = open();
    let (_, entities) = call(&mut session, r#"{"v":1,"op":"entities","doc":"a"}"#);
    let id = text(&entities, "/result/items/0/id");
    assert!(id.starts_with("space/0/"), "{}", json(&entities));
    let element = format!(r#"{{"kind":"element","id":"{id}"}}"#);
    let (status, value) = measure(
        &mut session,
        "space/0",
        &element,
        &split([0.0, 0.0], [1.0, 1.0]),
    );
    assert_eq!(refused(status, &value), failed("split-point-with-element"));
    let (status, value) = measure(
        &mut session,
        "space/0",
        &split([f64::MAX, 0.0], [0.0, 0.0]),
        &split([-f64::MAX, 0.0], [0.0, 0.0]),
    );
    assert_eq!(refused(status, &value), failed("overflow"));
    // 距離でない計測では使えない
    let (status, value) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"measure","doc":"a","from":"space/0","quantity":"length","first":{}}}"#,
            split([0.0, 0.0], [1.0, 1.0])
        ),
    );
    assert_eq!(status, Status::Failed, "{}", json(&value));
    // 古いハッシュと予算の停止の後も、文書を使える
    let (status, value) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"measure","doc":"a","from":"space/0","quantity":"distance","first":{},"second":{},"expected_hash":"v2:00"}}"#,
            split([0.0, 0.0], [0.0, 0.0]),
            split([0.0, 0.0], [3.0, 4.0])
        ),
    );
    assert_eq!(refused(status, &value).0, "call.value.invalid");
    for from in ["space/0", "sheet/0"] {
        let (status, value) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"measure","doc":"a","from":"{from}","quantity":"distance","first":{},"second":{},"budget":{{"work":200}}}}"#,
                split([0.0, 0.0], [0.0, 0.0]),
                split([0.0, 0.0], [3.0, 4.0])
            ),
        );
        assert_eq!(refused(status, &value).0, "budget.work.exceeded");
    }
    let (status, value) = measure(
        &mut session,
        "sheet/0",
        &split([0.0, 0.0], [0.0, 0.0]),
        &point(3.0, 4.0),
    );
    assert_eq!(status, Status::Ok, "{}", json(&value));
}

#[test]
fn sheets_without_calibrations_return_paper_distances() {
    let mut session = open();
    let (_, calibrations) = call(&mut session, r#"{"v":1,"op":"calibrations","doc":"a"}"#);
    assert_eq!(number(&calibrations, "/result/total"), 0.0);
    let (status, value) = measure(&mut session, "sheet/0", &point(0.0, 0.0), &point(3.0, 4.0));
    assert_eq!(status, Status::Ok, "{}", json(&value));
    assert_eq!(number(&value, "/result/value"), 5.0);
    assert_eq!(value.pointer("/result/sheet"), Some(&Value::Bool(true)));
    assert_eq!(text(&value, "/result/from"), "sheet/0");
    assert_eq!(text(&value, "/result/unit"), "millimeter");
    let (status, value) = measure(
        &mut session,
        "sheet/0",
        &split([3e15, 3e15], [0.25, 0.0]),
        &split([3e15, 3e15], [3.25, 4.0]),
    );
    assert_eq!(status, Status::Ok, "{}", json(&value));
    assert_eq!(number(&value, "/result/value"), 5.0);
    // 用紙の要素、距離でない計測、ない用紙は測らない
    let (status, value) = measure(
        &mut session,
        "sheet/0",
        r#"{"kind":"element","id":"sheet/0/r4"}"#,
        &point(0.0, 0.0),
    );
    assert_eq!(refused(status, &value), failed("sheet-element"));
    let (status, value) = call(
        &mut session,
        r#"{"v":1,"op":"measure","doc":"a","from":"sheet/0","quantity":"length","first":{"kind":"point","position":{"x":0,"y":0}}}"#,
    );
    assert_eq!(refused(status, &value).0, "call.value.invalid");
    let (status, value) = measure(&mut session, "sheet/9", &point(0.0, 0.0), &point(3.0, 4.0));
    assert_eq!(refused(status, &value).0, "call.value.invalid");
}
