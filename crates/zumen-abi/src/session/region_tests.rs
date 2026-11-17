//! 公開 push/open/regions による、閉領域の期待値・丸め・使わなかった部分・診断・予算停止と文書の再利用。
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力の成功と応答の値を検査するテスト。

use crate::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::binary;
use zumen_json::Value;

#[path = "../../examples/region_case.rs"]
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
    session.push(b"regions", &bytes).unwrap();
    assert_eq!(
        call(&mut session, r#"{"v":1,"op":"open","input":"regions"}"#).0,
        Status::Ok
    );
    session
}
fn ids(list: &[&str]) -> String {
    list.iter()
        .map(|id| format!(r#"{{"kind":"element","id":"space/0/{id}"}}"#))
        .collect::<Vec<_>>()
        .join(",")
}
fn request(operation: &str, subject: &[&str], extra: &str) -> String {
    format!(
        r#"{{"v":1,"op":"regions","doc":"regions","from":"space/0","operation":"{operation}","subject":[{}]{extra}}}"#,
        ids(subject)
    )
}
fn boolean(operation: &str, subject: &[&str], clip: &[&str], extra: &str) -> String {
    request(
        operation,
        subject,
        &format!(r#","clip":[{}]{extra}"#, ids(clip)),
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
/// 各領域の (面積, 穴の数, 外周の点の数)。
fn areas(value: &Value) -> Vec<(f64, usize, usize)> {
    list(value, "/result/regions")
        .iter()
        .map(|item| {
            (
                number(item, "/area"),
                list(item, "/holes").len(),
                list(item, "/outer").len(),
            )
        })
        .collect()
}
fn ok(session: &mut Session, request: &str) -> Value {
    let (status, result) = call(session, request);
    assert_eq!(status, Status::Ok, "{request}: {result:?}");
    result
}

#[test]
fn detection_splits_faces_and_reports_branches_and_holes() {
    let mut session = session();
    let lines = ["h23", "h24", "h25", "h26", "h27", "h28"];
    let result = ok(&mut session, &request("detect", &lines, ""));
    assert_eq!(areas(&result), vec![(50.0, 0, 3), (50.0, 0, 3)]);
    let first: Vec<_> = list(&result, "/result/regions/0/outer")
        .iter()
        .map(|p| (number(p, "/x"), number(p, "/y")))
        .collect();
    assert_eq!(first, vec![(0.0, -20.0), (10.0, -20.0), (10.0, -10.0)]);
    let discarded = list(&result, "/result/discarded");
    assert_eq!(discarded.len(), 1);
    assert_eq!(text(&discarded[0], "/id"), "space/0/h27");
    assert_eq!(text(&discarded[0], "/kind"), "open");
    assert_eq!(
        [
            number(&discarded[0], "/start/x"),
            number(&discarded[0], "/end/x")
        ],
        [10.0, 14.0]
    );
    assert!(list(&result, "/result/rounded").is_empty());
    assert_eq!(text(&result, "/result/unit"), "millimeter");
    assert_eq!(
        result.pointer("/result/approximate"),
        Some(&Value::Bool(false))
    );
    // 要素の順序を変えても同じ領域。
    let mut reversed = lines;
    reversed.reverse();
    let other = ok(&mut session, &request("detect", &reversed, ""));
    assert_eq!(
        result.pointer("/result/regions"),
        other.pointer("/result/regions")
    );

    let nested = ok(&mut session, &request("detect", &["h20", "h30"], ""));
    assert_eq!(areas(&nested), vec![(96.0, 1, 4), (4.0, 0, 4)]);
}

#[test]
fn boolean_operations_follow_the_fill_rule() {
    let mut session = session();
    for (operation, total) in [
        ("union", 175.0),
        ("intersection", 25.0),
        ("difference", 75.0),
        ("xor", 150.0),
    ] {
        let result = ok(&mut session, &boolean(operation, &["h20"], &["h21"], ""));
        let sum: f64 = areas(&result).iter().map(|a| a.0).sum();
        assert_eq!(sum, total, "{operation}");
    }
    let a = ok(&mut session, &boolean("union", &["h20"], &["h21"], ""));
    let b = ok(&mut session, &boolean("union", &["h21"], &["h20"], ""));
    assert_eq!(a.pointer("/result/regions"), b.pointer("/result/regions"));
    let d = ok(&mut session, &boolean("difference", &["h21"], &["h20"], ""));
    assert_eq!(areas(&d), vec![(75.0, 0, 6)]);

    let even = ok(&mut session, &request("union", &["h20", "h30"], ""));
    assert_eq!(areas(&even), vec![(96.0, 1, 4)]);
    let nonzero = ok(
        &mut session,
        &request("union", &["h20", "h30"], r#","fill":"non_zero""#),
    );
    assert_eq!(areas(&nonzero), vec![(100.0, 0, 4)]);

    let flat = ok(&mut session, &request("union", &["h2A"], ""));
    assert!(areas(&flat).is_empty());
    let discarded = list(&flat, "/result/discarded");
    assert_eq!(discarded.len(), 1);
    assert_eq!(text(&discarded[0], "/id"), "space/0/h2A");
    assert_eq!(text(&discarded[0], "/kind"), "degenerate");
    assert!(discarded[0].pointer("/start").is_none());
}

#[test]
fn approximation_rounding_and_failures_identify_the_source() {
    let mut session = session();
    let before = call(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"regions","fields":["hash"]}"#,
    );
    let circle = ok(&mut session, &request("detect", &["h29"], ""));
    let found = areas(&circle);
    assert_eq!(found.len(), 1);
    let exact = std::f64::consts::PI * 4.0;
    // 内接する多角形で、矢高は許容差の半分以下。格子の丸めの分を足して比べる。
    assert!((exact - found[0].0).abs() < 0.0006 * 4.0 * std::f64::consts::PI);
    assert_eq!(
        circle.pointer("/result/approximate"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        text(&circle, "/diagnostics/0/code"),
        "drawing.element-region.approximated"
    );
    assert_eq!(text(&circle, "/diagnostics/0/params/id"), "space/0/h29");
    assert_eq!(number(&circle, "/diagnostics/0/at/0/record"), 8.0);

    let crossing = ok(&mut session, &request("detect", &["h2C", "h2D", "h2E"], ""));
    let found = areas(&crossing);
    assert_eq!(found.len(), 1);
    let x = 0.999 / 1.999;
    assert!((found[0].0 - x / 2.0).abs() < 0.001, "{found:?}");

    let rounded = list(&crossing, "/result/rounded");
    let ids: Vec<_> = rounded.iter().map(|r| text(r, "/id")).collect();
    assert_eq!(ids, vec!["space/0/h2C", "space/0/h2D"]);
    for item in rounded {
        assert!((number(item, "/position/x") - x).abs() < 0.001);
        assert!((number(item, "/position/y") - (50.0 + x)).abs() < 0.001);
    }
    let open: Vec<_> = list(&crossing, "/result/discarded")
        .iter()
        .map(|r| (text(r, "/id"), text(r, "/kind")))
        .collect();
    assert_eq!(open, vec![("space/0/h2C", "open"), ("space/0/h2D", "open")]);

    for (operation, subject, id, reason, record) in [
        ("union", "h23", "space/0/h23", "not-closed", Some(2.0)),
        (
            "detect",
            "h2B",
            "space/0/h2B",
            "unsupported-shape",
            Some(10.0),
        ),
        ("detect", "h2F", "space/0/h2F", "numeric-range", Some(14.0)),
        ("detect", "h99", "space/0/h99", "id-not-found", None),
    ] {
        let (status, result) = call(&mut session, &request(operation, &["h20", subject], ""));
        assert_eq!(status, Status::Failed, "{result:?}");
        assert!(result.pointer("/result").is_none());
        assert_eq!(
            text(&result, "/diagnostics/0/code"),
            "drawing.element-region.failed"
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
        (request("detect", &["h20", "h20"], ""), "/subject/1"),
        (request("detect", &[], ""), "/subject"),
        (boolean("detect", &["h20"], &["h21"], ""), "/clip"),
        (
            request("detect", &["h20"], r#","fill":"even_odd""#),
            "/fill",
        ),
        (boolean("union", &["h20"], &["h21", "h21"], ""), "/clip/1"),
        (
            request("detect", &["h20"], r#","tolerance":0"#),
            "/tolerance",
        ),
        (
            request("detect", &["h20"], r#","expected_hash":"v2:00""#),
            "/expected_hash",
        ),
    ] {
        let (status, result) = call(&mut session, &request);
        assert_eq!(status, Status::Failed, "{result:?}");
        assert_eq!(text(&result, "/diagnostics/0/params/pointer"), pointer);
    }
    let after = call(
        &mut session,
        r#"{"v":1,"op":"entities","doc":"regions","fields":["hash"]}"#,
    );
    assert_eq!(before, after);
}

#[test]
fn budget_boundaries_and_cancellation_leave_a_reusable_document() {
    let mut session = session();
    for field in ["work", "decoded_bytes", "segments"] {
        let base = request("detect", &["h20", "h30", "h29"], "");
        let prefix = base.strip_suffix('}').unwrap();
        let with_budget = |limit| format!(r#"{prefix},"budget":{{"{field}":{limit}}}}}"#);
        let mut high = 1_u64;
        while call(&mut session, &with_budget(high)).0 != Status::Ok {
            high *= 2;
            assert!(high < 1 << 34);
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
            .call(request("detect", &["h20"], "").as_bytes(), &cancel)
            .status,
        Status::Failed
    );
    assert_eq!(
        areas(&ok(&mut session, &request("detect", &["h20"], ""))),
        vec![(100.0, 0, 4)]
    );
}
