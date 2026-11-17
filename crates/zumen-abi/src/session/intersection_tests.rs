//! 公開 push/open/intersect による、交点・接点・重なりの期待値・診断・予算停止と文書の再利用。
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力の成功と応答の値を検査するテスト。

use crate::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::binary;
use zumen_json::Value;

#[path = "../../examples/intersection_case.rs"]
mod fixture;

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}
fn session() -> Session {
    let (source, drawing) = fixture::fixture();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let bytes = binary::write(
        &source,
        &drawing,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    let mut session = Session::new(b"").unwrap();
    session.push(b"intersect", &bytes).unwrap();
    assert_eq!(
        call(&mut session, r#"{"v":1,"op":"open","input":"intersect"}"#).0,
        Status::Ok
    );
    session
}
fn request(first: &str, second: &str, extra: &str) -> String {
    format!(
        r#"{{"v":1,"op":"intersect","doc":"intersect","from":"space/0","first":{{"kind":"element","id":"space/0/{first}"}},"second":{{"kind":"element","id":"space/0/{second}"}}{extra}}}"#
    )
}
fn number(value: &Value, path: &str) -> f64 {
    zumen_json::number(value.pointer(path).unwrap()).unwrap()
}
fn text<'a>(value: &'a Value, path: &str) -> &'a str {
    let Some(Value::String(value)) = value.pointer(path) else {
        panic!("{path}: {value:?}")
    };
    value
}
fn list<'a>(value: &'a Value, path: &str) -> &'a [Value] {
    let Some(Value::Array(items)) = value.pointer(path) else {
        panic!("{path}: {value:?}")
    };
    items
}
/// 交点を (x, y, tangent, rounded) の列にする。
fn points(value: &Value) -> Vec<(f64, f64, bool, bool)> {
    list(value, "/result/points")
        .iter()
        .map(|item| {
            (
                number(item, "/position/x"),
                number(item, "/position/y"),
                item.pointer("/tangent") == Some(&Value::Bool(true)),
                item.pointer("/rounded") == Some(&Value::Bool(true)),
            )
        })
        .collect()
}

#[test]
fn crossings_tangents_and_overlaps_have_independent_expectations() {
    let mut session = session();
    for (first, second, expected) in [
        ("h10", "h11", vec![(5.0, 0.0, false, false)]),
        (
            "h10",
            "h12",
            vec![(2.0, 0.0, false, false), (8.0, 0.0, false, false)],
        ),
        ("h12", "h13", vec![(8.0, 0.0, true, false)]),
        ("h10", "h13", vec![(8.0, 0.0, false, false)]),
        (
            "h11",
            "h12",
            vec![(5.0, -3.0, false, false), (5.0, 3.0, false, false)],
        ),
        ("h10", "h18", vec![]),
    ] {
        for (a, b) in [(first, second), (second, first)] {
            let (status, result) = call(&mut session, &request(a, b, ""));
            assert_eq!(status, Status::Ok, "{a} {b}: {result:?}");
            assert_eq!(points(&result), expected, "{a} {b}");
            assert!(list(&result, "/result/overlaps").is_empty());
            assert_eq!(text(&result, "/result/unit"), "millimeter");
            assert_eq!(
                result.pointer("/result/approximate"),
                Some(&Value::Bool(false))
            );
        }
    }
}

#[test]
fn overlaps_drop_their_endpoints_from_the_points() {
    let mut session = session();
    let (status, result) = call(&mut session, &request("h10", "h15", ""));
    assert_eq!(status, Status::Ok, "{result:?}");
    assert!(points(&result).is_empty());
    let overlaps = list(&result, "/result/overlaps");
    assert_eq!(overlaps.len(), 1);
    assert_eq!(text(&overlaps[0], "/kind"), "segment");
    assert_eq!(number(&overlaps[0], "/start/x"), 0.0);
    assert_eq!(number(&overlaps[0], "/end/x"), 4.0);

    let (status, result) = call(&mut session, &request("h10", "h10/1", ""));
    assert_eq!(status, Status::Ok, "{result:?}");
    let overlaps = list(&result, "/result/overlaps");
    assert_eq!(number(&overlaps[0], "/start/x"), 2.0);
    assert_eq!(number(&overlaps[0], "/end/x"), 6.0);

    for (a, b) in [("h12", "h14"), ("h14", "h12")] {
        let (status, result) = call(&mut session, &request(a, b, ""));
        assert_eq!(status, Status::Ok, "{result:?}");
        assert!(points(&result).is_empty());
        let overlaps = list(&result, "/result/overlaps");
        assert_eq!(overlaps.len(), 1);
        assert_eq!(text(&overlaps[0], "/kind"), "arc");
        assert_eq!(number(&overlaps[0], "/radius"), 3.0);
        assert_eq!(
            [
                number(&overlaps[0], "/start/x"),
                number(&overlaps[0], "/end/x"),
                number(&overlaps[0], "/center/x"),
            ],
            [8.0, 2.0, 5.0]
        );
    }
    let (status, result) = call(&mut session, &request("h14", "h1A", ""));
    assert_eq!(status, Status::Ok, "{result:?}");
    assert_eq!(
        points(&result),
        vec![(2.0, 0.0, false, false), (8.0, 0.0, false, false)]
    );
}

#[test]
fn approximations_and_failures_identify_the_source_without_changing_ids() {
    let mut session = session();
    let before = call(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"intersect","fields":["hash"]}"#,
    );
    let (status, result) = call(&mut session, &request("h11", "h16", ""));
    assert_eq!(status, Status::Ok, "{result:?}");
    let found = points(&result);
    assert_eq!(found.len(), 2);
    let y = 5.0 * (0.75_f64).sqrt();
    for ((x, value, tangent, _), expected) in found.iter().zip([-y, y]) {
        assert_eq!(*x, 5.0);
        assert!((value - expected).abs() < 0.001, "{found:?}");
        assert!(!tangent);
    }
    assert_eq!(
        result.pointer("/result/approximate"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        text(&result, "/diagnostics/0/code"),
        "drawing.element-intersection.approximated"
    );
    assert_eq!(text(&result, "/diagnostics/0/params/id"), "space/0/h16");
    assert_eq!(number(&result, "/diagnostics/0/at/0/record"), 6.0);

    for (first, second, id, reason, record) in [
        ("h10", "h17", "space/0/h17", "unsupported-shape", Some(7.0)),
        ("h10", "h19", "space/0/h19", "numeric-range", Some(9.0)),
        ("h10", "h99", "space/0/h99", "id-not-found", None),
    ] {
        let (status, result) = call(&mut session, &request(first, second, ""));
        assert_eq!(status, Status::Failed, "{result:?}");
        assert!(result.pointer("/result").is_none());
        assert_eq!(
            text(&result, "/diagnostics/0/code"),
            "drawing.element-intersection.failed"
        );
        assert_eq!(text(&result, "/diagnostics/0/params/id"), id);
        assert_eq!(text(&result, "/diagnostics/0/params/reason"), reason);
        assert_eq!(
            result
                .pointer("/diagnostics/0/at/0/record")
                .map(|value| zumen_json::number(value).unwrap()),
            record
        );
    }
    for (request, pointer) in [
        (request("h10", "h10", ""), "/second"),
        (request("h10", "h11", r#","tolerance":0"#), "/tolerance"),
        (
            request("h10", "h11", r#","expected_hash":"v2:00""#),
            "/expected_hash",
        ),
    ] {
        let (status, result) = call(&mut session, &request);
        assert_eq!(status, Status::Failed, "{result:?}");
        assert_eq!(text(&result, "/diagnostics/0/params/pointer"), pointer);
    }
    let after = call(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"intersect","fields":["hash"]}"#,
    );
    assert_eq!(before, after);
}

#[test]
fn budget_boundaries_and_cancellation_leave_a_reusable_document() {
    let mut session = session();
    for field in ["work", "decoded_bytes", "segments"] {
        let base = request("h11", "h16", "");
        let prefix = base.strip_suffix('}').unwrap();
        let with_budget = |limit| format!(r#"{prefix},"budget":{{"{field}":{limit}}}}}"#);
        let mut high = 1_u64;
        while call(&mut session, &with_budget(high)).0 != Status::Ok {
            high *= 2;
            assert!(high < 1 << 30);
        }
        let mut low = 0;
        while low + 1 < high {
            let middle = low + (high - low) / 2;
            if call(&mut session, &with_budget(middle)).0 == Status::Ok {
                high = middle;
            } else {
                low = middle;
            }
        }
        let (status, result) = call(&mut session, &with_budget(high - 1));
        assert_eq!(status, Status::Failed);
        assert!(result.pointer("/result").is_none());
        let expected = format!("budget.{}.exceeded", field.replace('_', "-"));
        assert!(
            list(&result, "/diagnostics")
                .iter()
                .any(|value| text(value, "/code") == expected)
        );
        assert_eq!(call(&mut session, &with_budget(high)).0, Status::Ok);
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        session
            .call(request("h10", "h11", "").as_bytes(), &cancel)
            .status,
        Status::Failed
    );
    assert_eq!(
        points(&call(&mut session, &request("h10", "h11", "")).1),
        vec![(5.0, 0.0, false, false)]
    );
}
