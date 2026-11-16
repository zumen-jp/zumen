//! 公開 push/open/measure による、数値の期待値・診断・予算停止と文書の再利用。
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力の成功と応答の値を検査するテスト。

use crate::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::binary;
use zumen_json::Value;

#[path = "../../examples/measurement_case.rs"]
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
    session.push(b"measure", &bytes).unwrap();
    assert_eq!(
        call(&mut session, r#"{"v":1,"op":"open","input":"measure"}"#).0,
        Status::Ok
    );
    session
}
fn request(id: &str, quantity: &str, extra: &str) -> String {
    format!(
        r#"{{"v":1,"op":"measure","doc":"measure","from":"space/0","quantity":"{quantity}","first":{{"kind":"element","id":"space/0/{id}"}}{extra}}}"#
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

#[test]
fn analytic_and_sampled_curves_have_independent_numeric_expectations() {
    let mut session = session();
    for (id, quantity, value, error, approximate) in [
        ("h10", "length", 5.0, 0.0, false),
        ("h10/1", "length", 7.0, 0.0, false),
        ("h11", "area", 4.0 * std::f64::consts::PI, 0.0, false),
        ("h12", "length", 2.5 * std::f64::consts::PI, 0.0, false),
        ("h13", "perimeter", 12.0, 0.0, false),
        ("h13", "area", 6.0, 0.0, false),
        // 完全楕円積分による周長と、2 次 Bézier の速度の積分。
        ("h14", "perimeter", 48.442_241_102_738_38, 0.0001, true),
        ("h15", "length", 14.789_428_575_445_976, 0.0001, true),
        ("h16", "length", 20.0, 0.0, false),
        ("h17", "length", 10.0, 1e-12, true),
        ("h20", "area", 100.0, 0.0, true),
    ] {
        let (status, result) = call(
            &mut session,
            &request(id, quantity, r#", "tolerance":0.00001"#),
        );
        assert_eq!(status, Status::Ok, "{id}: {result:?}");
        assert!(
            (number(&result, "/result/value") - value).abs() <= error,
            "{id}: {result:?}"
        );
        assert_eq!(
            result.pointer("/result/approximate"),
            Some(&Value::Bool(approximate))
        );
    }
}

#[test]
fn failures_and_approximations_identify_the_source_without_changing_ids() {
    let mut session = session();
    let before = call(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"measure","fields":["hash"]}"#,
    );
    for (id, quantity, reason, record) in [
        ("h18", "length", "unsupported-shape", 8.0),
        ("h12", "area", "not-closed", 2.0),
    ] {
        let (status, result) = call(&mut session, &request(id, quantity, ""));
        assert_eq!(status, Status::Failed);
        assert!(result.pointer("/result").is_none());
        assert_eq!(
            text(&result, "/diagnostics/0/code"),
            "drawing.element-measurement.failed"
        );
        assert_eq!(text(&result, "/diagnostics/0/params/reason"), reason);
        assert_eq!(number(&result, "/diagnostics/0/at/0/record"), record);
    }
    let (status, result) = call(&mut session, &request("h14", "perimeter", ""));
    assert_eq!(status, Status::Ok);
    assert_eq!(
        text(&result, "/diagnostics/0/code"),
        "drawing.element-measurement.approximated"
    );
    assert_eq!(number(&result, "/diagnostics/0/at/0/record"), 4.0);
    let after = call(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"measure","fields":["hash"]}"#,
    );
    assert_eq!(before, after);
}

#[test]
fn budget_boundaries_and_cancellation_leave_a_reusable_document() {
    let mut session = session();
    for field in ["work", "decoded_bytes", "segments"] {
        let base = request("h15", "length", "");
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
        let Some(Value::Array(diagnostics)) = result.pointer("/diagnostics") else {
            panic!("{result:?}")
        };
        let expected = format!("budget.{}.exceeded", field.replace('_', "-"));
        assert!(
            diagnostics
                .iter()
                .any(|value| text(value, "/code") == expected)
        );
        assert_eq!(call(&mut session, &with_budget(high)).0, Status::Ok);
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        session
            .call(request("h10", "length", "").as_bytes(), &cancel)
            .status,
        Status::Failed
    );
    assert_eq!(
        number(
            &call(&mut session, &request("h10", "length", "")).1,
            "/result/value"
        ),
        5.0
    );
}
