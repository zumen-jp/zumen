//! 公開 push/open/construct による、作図の期待値・切断点・診断・予算停止と文書の再利用。
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力の成功と応答の値を検査するテスト。

use crate::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::binary;
use zumen_json::Value;

#[path = "../../examples/construct_case.rs"]
mod fixture;

const FAILED: &str = "drawing.element-construction.failed";

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
    session.push(b"construct", &bytes).unwrap();
    assert_eq!(
        call(&mut session, r#"{"v":1,"op":"open","input":"construct"}"#).0,
        Status::Ok
    );
    session
}
fn element(id: &str) -> String {
    format!(r#"{{"kind":"element","id":"space/0/{id}"}}"#)
}
fn request(operation: &str, target: &str, extra: &str) -> String {
    format!(
        r#"{{"v":1,"op":"construct","doc":"construct","from":"space/0","operation":"{operation}","target":{}{extra}}}"#,
        element(target)
    )
}
fn boundaries(list: &[&str]) -> String {
    format!(
        r#","boundaries":[{}]"#,
        list.iter()
            .map(|id| element(id))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn trim(target: &str, list: &[&str], x: f64, y: f64) -> String {
    request(
        "trim",
        target,
        &format!(r#"{},"pick":{{"x":{x},"y":{y}}}"#, boundaries(list)),
    )
}
fn extend(target: &str, list: &[&str], end: &str) -> String {
    request(
        "extend",
        target,
        &format!(r#"{},"end":"{end}""#, boundaries(list)),
    )
}
fn corner(operation: &str, target: &str, second: &str, extra: &str) -> String {
    request(
        operation,
        target,
        &format!(r#","second":{}{extra}"#, element(second)),
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
fn ok(session: &mut Session, request: &str) -> Value {
    let (status, value) = call(session, request);
    assert_eq!(status, Status::Ok, "{request}: {value:?}");
    value
}
/// 失敗の (コード, 理由か指す場所, 記録)。
fn failure(session: &mut Session, request: &str) -> (String, String, Option<f64>) {
    let (status, value) = call(session, request);
    assert_eq!(status, Status::Failed, "{request}: {value:?}");
    assert!(value.pointer("/result").is_none());
    let detail = [
        "/diagnostics/0/params/reason",
        "/diagnostics/0/params/pointer",
    ]
    .into_iter()
    .find(|path| value.pointer(path).is_some())
    .map_or_else(String::new, |path| text(&value, path).to_owned());
    let record = value
        .pointer("/diagnostics/0/at/0/record")
        .map(|_| number(&value, "/diagnostics/0/at/0/record"));
    (
        text(&value, "/diagnostics/0/code").to_owned(),
        detail,
        record,
    )
}
fn construction_failure(reason: &str, record: Option<f64>) -> (String, String, Option<f64>) {
    (FAILED.to_owned(), reason.to_owned(), record)
}
/// 形を (種類, 数値の並び) にする。線分は始点・終点、円は中心・半径、円弧は中心・半径・角度。
fn shape(value: &Value) -> (String, Vec<f64>) {
    let kind = text(value, "/shape").to_owned();
    let paths = match kind.as_str() {
        "line" => ["/start/0", "/start/1", "/end/0", "/end/1"].as_slice(),
        "circle" => ["/center/0", "/center/1", "/radius"].as_slice(),
        _ => [
            "/center/0",
            "/center/1",
            "/radius",
            "/start_angle",
            "/end_angle",
        ]
        .as_slice(),
    };
    (kind, paths.iter().map(|path| number(value, path)).collect())
}
fn numbers(value: &Value, path: &str) -> Vec<f64> {
    shape(value.pointer(path).unwrap()).1
}
fn near(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} {expected:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() <= 1e-9, "{actual:?} {expected:?}");
    }
}

#[test]
fn offsets_lines_circles_and_arcs_of_both_directions() {
    let mut session = session();
    let value = ok(&mut session, &request("offset", "h40", r#","distance":2"#));
    assert_eq!(
        text(&value, "/result/hash"),
        "v2:46464646464646464646464646464646"
    );
    assert_eq!(text(&value, "/result/unit"), "millimeter");
    assert_eq!(number(&value, "/result/tolerance"), 0.001);
    assert_eq!(
        value.pointer("/result/approximate"),
        Some(&Value::Bool(false))
    );
    assert!(list(&value, "/result/replace").is_empty());
    assert!(list(&value, "/result/cuts").is_empty());
    assert!(list(&value, "/diagnostics").is_empty());
    assert_eq!(
        shape(&list(&value, "/result/add")[0]),
        ("line".to_owned(), vec![0.0, 2.0, 10.0, 2.0])
    );
    let value = ok(&mut session, &request("offset", "h40", r#","distance":-2"#));
    assert_eq!(numbers(&value, "/result/add/0"), [0.0, -2.0, 10.0, -2.0]);
    let value = ok(&mut session, &request("offset", "h44", r#","distance":1"#));
    assert_eq!(numbers(&value, "/result/add/0"), [20.0, 0.0, 4.0]);
    let value = ok(&mut session, &request("offset", "h44", r#","distance":-1"#));
    assert_eq!(numbers(&value, "/result/add/0"), [20.0, 0.0, 2.0]);
    // 外側を正にするので、反時計回りと時計回りの同じ円弧は同じ結果になる。
    for target in ["h45", "h4E"] {
        let value = ok(&mut session, &request("offset", target, r#","distance":1"#));
        assert_eq!(text(&value, "/result/add/0/shape"), "arc");
        near(
            &numbers(&value, "/result/add/0"),
            &[0.0, 0.0, 6.0, 0.0, 90.0],
        );
        let value = ok(
            &mut session,
            &request("offset", target, r#","distance":-1"#),
        );
        near(
            &numbers(&value, "/result/add/0"),
            &[0.0, 0.0, 4.0, 0.0, 90.0],
        );
    }
    for (target, distance) in [("h44", "-3"), ("h44", "-4"), ("h45", "-5")] {
        assert_eq!(
            failure(
                &mut session,
                &request("offset", target, &format!(r#","distance":{distance}"#))
            )
            .1,
            "no-solution"
        );
    }
}

#[test]
fn trims_between_the_nearest_cuts() {
    let mut session = session();
    let value = ok(&mut session, &trim("h40", &["h42", "h43"], 6.5, 0.4));
    assert_eq!(text(&value, "/result/replace/0/id"), "space/0/h40");
    let pieces = list(&value, "/result/replace/0/geometries");
    assert_eq!(pieces.len(), 2);
    assert_eq!(shape(&pieces[0]).1, [0.0, 0.0, 5.0, 0.0]);
    assert_eq!(shape(&pieces[1]).1, [8.0, 0.0, 10.0, 0.0]);
    let cuts = list(&value, "/result/cuts");
    assert_eq!(text(&cuts[0], "/boundary"), "space/0/h42");
    assert_eq!(text(&cuts[1], "/boundary"), "space/0/h43");
    assert_eq!(number(&cuts[1], "/position/x"), 8.0);
    assert_eq!(cuts[0].pointer("/rounded"), Some(&Value::Bool(false)));
    assert_eq!(cuts[0].pointer("/adjusted"), Some(&Value::Bool(false)));
    assert!(list(&value, "/result/add").is_empty());
    // 端の区間を削ると 1 本になる。境界の順は結果を変えない。
    let value = ok(&mut session, &trim("h40", &["h43", "h42"], 2.0, 0.0));
    let pieces = list(&value, "/result/replace/0/geometries");
    assert_eq!(pieces.len(), 1);
    assert_eq!(shape(&pieces[0]).1, [5.0, 0.0, 10.0, 0.0]);
    assert_eq!(list(&value, "/result/cuts").len(), 1);
    // 円は 2 つの切断点の間を削り、反時計回りの円弧を残す。
    let value = ok(&mut session, &trim("h44", &["h47"], 23.0, 0.0));
    near(
        &numbers(&value, "/result/replace/0/geometries/0"),
        &[20.0, 0.0, 3.0, 90.0, 270.0],
    );
    assert_eq!(list(&value, "/result/cuts").len(), 2);
    // 円弧は元の端の角度を保ち、切った側だけを交点の角度にする。
    let value = ok(&mut session, &trim("h45", &["h46"], 5.0, 1.0));
    let angle = libm::atan2(3.0, 4.0).to_degrees();
    near(
        &numbers(&value, "/result/replace/0/geometries/0"),
        &[0.0, 0.0, 5.0, angle, 90.0],
    );
    assert_eq!(number(&value, "/result/cuts/0/position/x"), 4.0);
    assert_eq!(number(&value, "/result/cuts/0/position/y"), 3.0);
}

#[test]
fn extends_to_the_first_boundary() {
    let mut session = session();
    let value = ok(&mut session, &extend("h40", &["h44"], "end"));
    assert_eq!(
        numbers(&value, "/result/replace/0/geometries/0"),
        [0.0, 0.0, 17.0, 0.0]
    );
    assert_eq!(text(&value, "/result/cuts/0/boundary"), "space/0/h44");
    let value = ok(&mut session, &extend("h45", &["h46"], "end"));
    let angle = libm::atan2(3.0, 4.0).to_degrees();
    near(
        &numbers(&value, "/result/replace/0/geometries/0"),
        &[0.0, 0.0, 5.0, 0.0, 180.0 - angle],
    );
    // 時計回りの円弧の終点は 0 度で、時計回りに x = 4 の線まで延びる。
    let value = ok(&mut session, &extend("h4E", &["h4F"], "end"));
    near(
        &numbers(&value, "/result/replace/0/geometries/0"),
        &[0.0, 0.0, 5.0, 360.0 - libm::acos(0.8).to_degrees(), 90.0],
    );
    let value = ok(&mut session, &extend("h40", &["h49"], "end"));
    assert_eq!(
        value.pointer("/result/approximate"),
        Some(&Value::Bool(true))
    );
    let end = number(&value, "/result/replace/0/geometries/0/end/0");
    // 弦の誤差（分割の目標）と格子の 1 間隔を足した範囲。
    assert!((end - 26.0).abs() <= 0.001 + 0.00025, "{end}");
    assert_eq!(text(&value, "/result/cuts/0/boundary"), "space/0/h49");
    let diagnostics = list(&value, "/diagnostics");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        text(&diagnostics[0], "/code"),
        "drawing.element-construction.approximated"
    );
    assert_eq!(text(&diagnostics[0], "/params/id"), "space/0/h49");
    assert_eq!(number(&diagnostics[0], "/at/0/record"), 9.0);
}

#[test]
fn chamfers_and_fillets_keep_the_original_directions() {
    let mut session = session();
    let value = ok(
        &mut session,
        &corner("chamfer", "h40", "h41", r#","distances":[2,3]"#),
    );
    assert_eq!(text(&value, "/result/replace/0/id"), "space/0/h40");
    assert_eq!(text(&value, "/result/replace/1/id"), "space/0/h41");
    assert_eq!(
        numbers(&value, "/result/replace/0/geometries/0"),
        [2.0, 0.0, 10.0, 0.0]
    );
    assert_eq!(
        numbers(&value, "/result/replace/1/geometries/0"),
        [0.0, 3.0, 0.0, 10.0]
    );
    assert_eq!(numbers(&value, "/result/add/0"), [2.0, 0.0, 0.0, 3.0]);
    // 2 本目から見た角は逆の順になる。
    let value = ok(
        &mut session,
        &corner("chamfer", "h41", "h40", r#","distances":[3,2]"#),
    );
    assert_eq!(numbers(&value, "/result/add/0"), [0.0, 3.0, 2.0, 0.0]);
    // 全長の面取りは要素を消す。
    let value = ok(
        &mut session,
        &corner("chamfer", "h40", "h41", r#","distances":[10,1]"#),
    );
    assert!(list(&value, "/result/replace/0/geometries").is_empty());
    assert_eq!(list(&value, "/result/replace/1/geometries").len(), 1);
    let value = ok(
        &mut session,
        &corner("fillet", "h40", "h41", r#","radius":2"#),
    );
    near(
        &numbers(&value, "/result/add/0"),
        &[2.0, 2.0, 2.0, 180.0, 270.0],
    );
    assert_eq!(
        numbers(&value, "/result/replace/1/geometries/0"),
        [0.0, 2.0, 0.0, 10.0]
    );
    for (operation, second, extra, reason) in [
        ("chamfer", "h41", r#","distances":[20,1]"#, "no-solution"),
        ("chamfer", "h4C", r#","distances":[1,1]"#, "no-corner"),
        ("fillet", "h4D", r#","radius":1"#, "no-solution"),
        ("fillet", "h41", r#","radius":20"#, "no-solution"),
    ] {
        assert_eq!(
            failure(&mut session, &corner(operation, "h40", second, extra)),
            construction_failure(reason, Some(0.0))
        );
    }
    assert_eq!(
        failure(
            &mut session,
            &corner("fillet", "h40", "h45", r#","radius":1"#)
        ),
        construction_failure("unsupported-shape", Some(5.0))
    );
}

#[test]
fn reports_failures_with_the_element() {
    let mut session = session();
    for (request, reason, record) in [
        (
            request("offset", "h48", r#","distance":1"#),
            "unsupported-shape",
            8.0,
        ),
        (extend("h44", &["h47"], "start"), "unsupported-shape", 4.0),
        (extend("h40", &["h4A"], "end"), "numeric-range", 10.0),
        (extend("h40", &["h44"], "start"), "no-intersection", 0.0),
        (trim("h40", &["h47"], 2.0, 0.0), "no-intersection", 0.0),
        (trim("h40", &["h42"], 12.0, 0.0), "pick-outside", 0.0),
        (trim("h40", &["h42"], 5.0, 0.0), "ambiguous-pick", 0.0),
    ] {
        assert_eq!(
            failure(&mut session, &request),
            construction_failure(reason, Some(record)),
            "{request}"
        );
    }
    assert_eq!(
        failure(&mut session, &request("offset", "h99", r#","distance":1"#)),
        construction_failure("id-not-found", None)
    );
}

#[test]
fn checks_the_fields_of_each_operation() {
    let mut session = session();
    let pick = r#","pick":{"x":0,"y":0}"#;
    for (request, pointer) in [
        (request("offset", "h40", ""), "/distance"),
        (request("offset", "h40", r#","distance":0"#), "/distance"),
        (
            request("offset", "h40", r#","distance":1,"radius":1"#),
            "/radius",
        ),
        (
            request("offset", "h40", &format!(r#","distance":1{pick}"#)),
            "/pick",
        ),
        (request("trim", "h40", pick), "/boundaries"),
        (request("trim", "h40", &boundaries(&["h42"])), "/pick"),
        (
            request("trim", "h40", &format!(r#","boundaries":[]{pick}"#)),
            "/boundaries",
        ),
        (trim("h40", &["h42", "h40"], 0.0, 0.0), "/boundaries/1"),
        (trim("h40", &["h42", "h42"], 0.0, 0.0), "/boundaries/1"),
        (request("extend", "h40", &boundaries(&["h42"])), "/end"),
        (
            request("chamfer", "h40", r#","distances":[1,1]"#),
            "/second",
        ),
        (
            corner("chamfer", "h40", "h40", r#","distances":[1,1]"#),
            "/second",
        ),
        (
            corner("chamfer", "h40", "h41", r#","distances":[1]"#),
            "/distances",
        ),
        (
            corner("chamfer", "h40", "h41", r#","distances":[1,0]"#),
            "/distances/1",
        ),
        (corner("fillet", "h40", "h41", r#","radius":0"#), "/radius"),
        (
            corner("fillet", "h40", "h41", r#","radius":1,"distance":1"#),
            "/distance",
        ),
        (
            request("offset", "h40", r#","distance":1,"tolerance":0"#),
            "/tolerance",
        ),
        (
            request("offset", "h40", r#","distance":1,"expected_hash":"wrong""#),
            "/expected_hash",
        ),
    ] {
        assert_eq!(
            failure(&mut session, &request),
            ("call.value.invalid".to_owned(), pointer.to_owned(), None),
            "{request}"
        );
    }
}

#[test]
fn budget_boundaries_and_cancellation_leave_a_reusable_document() {
    let mut session = session();
    for field in ["work", "decoded_bytes", "segments"] {
        let base = extend("h40", &["h42", "h44", "h49"], "end");
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
    let offset = request("offset", "h40", r#","distance":1"#);
    assert_eq!(
        session.call(offset.as_bytes(), &cancel).status,
        Status::Failed
    );
    let value = ok(&mut session, &offset);
    assert_eq!(numbers(&value, "/result/add/0"), [0.0, 1.0, 10.0, 1.0]);
    let value = ok(&mut session, r#"{"v":1,"op":"drawing","doc":"construct"}"#);
    assert_eq!(
        text(&value, "/result/hash"),
        "v2:46464646464646464646464646464646"
    );
}
