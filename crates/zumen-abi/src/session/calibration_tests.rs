//! 合成の ZMDR を公開の push/open から読み、校正の値・測距・停止と再利用を確かめる。

#![cfg(test)]

use crate::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref};
use zumen_drawing::{Drawing2D, FieldValue, binary};
use zumen_json::Value;

#[path = "../../examples/calibration_case.rs"]
mod fixture;

const HASH: &str = "v2:11111111111111111111111111111111";
const FIRST: &str = "sheet/hAA";
const SECOND: &str = "sheet/hAA/1";
const LIST: &str = r#"{"v":1,"op":"calibrations","doc":"a"}"#;

fn model() -> (binary::Source, Drawing2D) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    fixture::fixture(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)).unwrap()
}

fn written(source: &binary::Source, drawing: &Drawing2D) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    binary::write(
        source,
        drawing,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap()
}

fn invoke(session: &mut Session, request: &str) -> (Status, Value, Vec<u8>) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    let value = zumen_json::parse(&reply.response).unwrap();
    (reply.status, value, reply.response)
}

fn opened(bytes: &[u8]) -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"input.zmdr", bytes).unwrap();
    let (status, response, _) = invoke(
        &mut session,
        r#"{"v":1,"op":"open","input":"input.zmdr","doc":"a"}"#,
    );
    assert_eq!(status, Status::Ok, "{response:?}");
    assert_eq!(text(&response, "/result/format"), "test-calibration");
    session
}

fn session() -> (Session, Vec<u8>) {
    let (source, drawing) = model();
    let bytes = written(&source, &drawing);
    (opened(&bytes), bytes)
}

fn text<'a>(value: &'a Value, pointer: &str) -> &'a str {
    let Some(Value::String(value)) = value.pointer(pointer) else {
        panic!("{pointer}: {value:?}");
    };
    value
}

fn number(value: &Value, pointer: &str) -> f64 {
    zumen_json::number(value.pointer(pointer).unwrap()).unwrap()
}

fn integer(value: &Value, pointer: &str) -> u64 {
    zumen_json::uint(value.pointer(pointer).unwrap(), 0, u64::MAX).unwrap()
}

fn items(value: &Value) -> &[Value] {
    let Some(Value::Array(items)) = value.pointer("/result/items") else {
        panic!("items: {value:?}");
    };
    items
}

fn diagnostic<'a>(response: &'a Value, code: &str) -> &'a Value {
    let Some(Value::Array(diagnostics)) = response.pointer("/diagnostics") else {
        panic!("diagnostics: {response:?}");
    };
    diagnostics
        .iter()
        .find(|diagnostic| text(diagnostic, "/code") == code)
        .unwrap()
}

fn no_result(response: &Value) {
    assert!(response.pointer("/result").is_none(), "{response:?}");
    assert_eq!(response.pointer("/blobs"), Some(&Value::Array(vec![])));
}

fn extra(request: &str, member: &str) -> String {
    format!("{},{member}}}", request.strip_suffix('}').unwrap())
}

fn point(origin: [f64; 2], offset: [f64; 2]) -> String {
    format!(
        r#"{{"origin":{{"x":{},"y":{}}},"offset":{{"x":{},"y":{}}}}}"#,
        origin[0], origin[1], offset[0], offset[1]
    )
}

fn measure(calibration: &str, from: &str, start: &str, end: &str) -> String {
    format!(
        r#"{{"v":1,"op":"measure_distance","doc":"a","from":"{from}","calibration":"{calibration}","start":{start},"end":{end}}}"#
    )
}

fn metric() -> String {
    measure(
        "metric",
        FIRST,
        &point([1e16, 0.0], [0.25, 1.0]),
        &point([1e16, 0.0], [0.75, 1.0]),
    )
}

fn value_failure(session: &mut Session, request: &str, pointer: &str) {
    let (status, response, _) = invoke(session, request);
    assert_eq!(status, Status::Failed, "{response:?}");
    no_result(&response);
    assert_eq!(
        text(
            diagnostic(&response, "call.value.invalid"),
            "/params/pointer"
        ),
        pointer,
        "{response:?}"
    );
}

fn measurement_failure(session: &mut Session, request: &str, id: &str, reason: &str) {
    let (status, response, _) = invoke(session, request);
    assert_eq!(status, Status::Failed, "{response:?}");
    no_result(&response);
    let failed = diagnostic(&response, "drawing.measurement.failed");
    assert_eq!(text(failed, "/params/id"), id);
    assert_eq!(text(failed, "/params/reason"), reason);
    assert_eq!(text(failed, "/severity"), "error");
    assert_eq!(integer(failed, "/count"), 1);
}

#[test]
fn lists_original_extension_order_public_sheets_labels_and_source() {
    let (mut session, bytes) = session();
    assert_ne!(zumen_drawing::Hash::document(&bytes).to_string(), HASH);
    let (status, response, _) = invoke(
        &mut session,
        &extra(LIST, &format!(r#""expected_hash":"{HASH}""#)),
    );
    assert_eq!(status, Status::Ok, "{response:?}");
    assert_eq!(text(&response, "/result/hash"), HASH);
    assert_eq!(integer(&response, "/result/total"), 3);
    assert!(response.pointer("/result/next_offset").is_none());
    assert_eq!(
        items(&response)
            .iter()
            .map(|item| text(item, "/id"))
            .collect::<Vec<_>>(),
        ["metric", "local", "overlap"]
    );
    assert_eq!(text(&response, "/result/items/0/from"), FIRST);
    assert_eq!(text(&response, "/result/items/1/from"), SECOND);
    assert_eq!(text(&response, "/result/items/2/from"), FIRST);
    assert_eq!(text(&response, "/result/items/0/name"), "測距");
    assert!(response.pointer("/result/items/1/name").is_none());
    assert_eq!(text(&response, "/result/items/0/unit"), "meter");
    assert_eq!(text(&response, "/result/items/0/unit_label"), "m");
    assert_eq!(text(&response, "/result/items/1/unit"), "unknown");
    assert_eq!(text(&response, "/result/items/1/unit_label"), "尺");
    for (index, expected) in [1e16, 0.0, 1e16 + 4.0, 4.0].into_iter().enumerate() {
        assert_eq!(
            number(&response, &format!("/result/items/0/bounds/{index}")),
            expected
        );
    }
    for (index, expected) in [2.0, 0.0, 0.0, 2.0].into_iter().enumerate() {
        assert_eq!(
            number(&response, &format!("/result/items/0/matrix/{index}")),
            expected
        );
    }
    assert_eq!(integer(&response, "/result/items/0/source/record"), 41);
    assert_eq!(
        integer(&response, "/result/items/0/source/bytes/start"),
        410
    );
    assert_eq!(integer(&response, "/result/items/0/source/bytes/end"), 450);
    assert_eq!(text(&response, "/result/items/0/source/handle"), "B1");
    assert_eq!(text(&response, "/result/items/1/source/instance"), "77");
    assert_eq!(
        response.pointer("/diagnostics"),
        Some(&Value::Array(vec![]))
    );
    let (_, drawing, _) = invoke(&mut session, r#"{"v":1,"op":"drawing","doc":"a"}"#);
    assert_eq!(text(&drawing, "/result/hash"), HASH);
    assert_eq!(text(&drawing, "/result/sheets/0/id"), FIRST);
    assert_eq!(text(&drawing, "/result/sheets/1/id"), SECOND);
}

#[test]
fn pages_after_filtering_and_checks_limits_and_public_sheet_ids() {
    let (mut session, _) = session();
    for (offset, id, next) in [
        (0, "metric", Some(1)),
        (1, "local", Some(2)),
        (2, "overlap", None),
    ] {
        let request = extra(LIST, &format!(r#""offset":{offset},"limit":1"#));
        let (status, response, _) = invoke(&mut session, &request);
        assert_eq!(status, Status::Ok);
        assert_eq!(items(&response).len(), 1);
        assert_eq!(text(&response, "/result/items/0/id"), id);
        assert_eq!(integer(&response, "/result/total"), 3);
        assert_eq!(
            response
                .pointer("/result/next_offset")
                .map(|_| integer(&response, "/result/next_offset")),
            next
        );
    }
    let (_, filtered, _) = invoke(
        &mut session,
        &extra(LIST, &format!(r#""from":"{FIRST}","offset":1,"limit":1"#)),
    );
    assert_eq!(integer(&filtered, "/result/total"), 2);
    assert_eq!(text(&filtered, "/result/items/0/id"), "overlap");
    assert!(filtered.pointer("/result/next_offset").is_none());
    let (_, filtered, _) = invoke(&mut session, &extra(LIST, &format!(r#""from":"{SECOND}""#)));
    assert_eq!(integer(&filtered, "/result/total"), 1);
    assert_eq!(text(&filtered, "/result/items/0/id"), "local");
    for offset in [3, 9_007_199_254_740_991_u64] {
        let (status, response, _) = invoke(
            &mut session,
            &extra(LIST, &format!(r#""offset":{offset},"limit":10000"#)),
        );
        assert_eq!(status, Status::Ok);
        assert!(items(&response).is_empty());
        assert!(response.pointer("/result/next_offset").is_none());
    }
    for limit in [0, 10001] {
        value_failure(
            &mut session,
            &extra(LIST, &format!(r#""limit":{limit}"#)),
            "/limit",
        );
    }
    value_failure(&mut session, &extra(LIST, r#""from":"sheet/7""#), "/from");
    value_failure(&mut session, &extra(LIST, r#""from":"space/0""#), "/from");
    value_failure(
        &mut session,
        &extra(
            LIST,
            r#""expected_hash":"v2:00000000000000000000000000000000""#,
        ),
        "/expected_hash",
    );
}

#[test]
fn explicit_selection_preserves_small_offsets_unknown_units_and_cancellation() {
    let (mut session, _) = session();
    for (request, expected, id, unit, label) in [
        (
            extra(&metric(), &format!(r#""expected_hash":"{HASH}""#)),
            1.0_f64,
            "metric",
            "meter",
            "m",
        ),
        (
            metric().replace("metric", "overlap"),
            0.5,
            "overlap",
            "meter",
            "m",
        ),
        (
            measure(
                "local",
                SECOND,
                &point([0.0, 0.0], [0.0, 0.0]),
                &point([0.0, 0.0], [3.0, 4.0]),
            ),
            5.0,
            "local",
            "unknown",
            "尺",
        ),
        (
            measure(
                "local",
                SECOND,
                &point([0.0, 0.0], [1.0, 1.0]),
                &point([1e16, 0.0], [-1e16 + 2.0, 1.0]),
            ),
            1.0,
            "local",
            "unknown",
            "尺",
        ),
        (
            measure(
                "local",
                SECOND,
                &point([f64::MAX, 0.0], [-f64::MAX, 1.0]),
                &point([-f64::MAX, 0.0], [f64::MAX, 1.0]),
            ),
            0.0,
            "local",
            "unknown",
            "尺",
        ),
    ] {
        let (status, response, _) = invoke(&mut session, &request);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(
            number(&response, "/result/distance").to_bits(),
            expected.to_bits()
        );
        assert_eq!(text(&response, "/result/calibration"), id);
        assert_eq!(text(&response, "/result/unit"), unit);
        assert_eq!(text(&response, "/result/unit_label"), label);
        assert_eq!(text(&response, "/result/hash"), HASH);
    }
}

#[test]
fn closes_bounds_and_rejects_exact_sums_just_outside_them() {
    let (mut session, _) = session();
    let start = point([1e16, 0.0], [0.0, 0.0]);
    let end = point([1e16, 0.0], [4.0, 0.0]);
    let (status, response, _) = invoke(&mut session, &measure("metric", FIRST, &start, &end));
    assert_eq!(status, Status::Ok);
    assert_eq!(number(&response, "/result/distance"), 8.0);
    for end in [
        point([1e16 + 4.0, 0.0], [f64::from_bits(1), 0.0]),
        point([1e16, 0.0], [-f64::from_bits(1), 0.0]),
        point([1e16, 0.0], [0.0, 5.0]),
    ] {
        measurement_failure(
            &mut session,
            &measure("metric", FIRST, &start, &end),
            "metric",
            "outside-bounds",
        );
    }
}

#[test]
fn reports_wrong_sheet_unknown_calibration_and_hash_without_partial_results() {
    let (mut session, _) = session();
    measurement_failure(
        &mut session,
        &metric().replace(FIRST, SECOND),
        "metric",
        "sheet-mismatch",
    );
    let (_, response, _) = invoke(&mut session, &metric().replace(FIRST, SECOND));
    let failed = diagnostic(&response, "drawing.measurement.failed");
    assert_eq!(integer(failed, "/at/0/record"), 41);
    assert_eq!(integer(failed, "/at/0/bytes/start"), 410);
    value_failure(
        &mut session,
        &metric().replace("metric", "missing"),
        "/calibration",
    );
    value_failure(&mut session, &metric().replace(FIRST, "sheet/7"), "/from");
    value_failure(
        &mut session,
        &extra(&metric(), r#""expected_hash":"wrong""#),
        "/expected_hash",
    );
    let (status, response, _) = invoke(&mut session, &metric());
    assert_eq!(status, Status::Ok, "{response:?}");
}

#[test]
fn reports_matrix_and_distance_overflow_without_changing_the_document() {
    for matrix in [false, true] {
        let (source, mut drawing) = model();
        let local = drawing
            .extensions
            .iter_mut()
            .find(|extension| extension.name == "local")
            .unwrap();
        for field in &mut local.fields {
            match (field.key.as_ref(), matrix) {
                ("bounds_min", false) => field.value = FieldValue::Point([-f64::MAX, 0.0, 0.0]),
                ("bounds_max", false) => field.value = FieldValue::Point([f64::MAX, 20.0, 0.0]),
                ("matrix_x", true) => field.value = FieldValue::Point([f64::MAX, 0.0, 0.0]),
                _ => {}
            }
        }
        let bytes = written(&source, &drawing);
        let mut session = opened(&bytes);
        let (start, end) = if matrix {
            (point([0.0, 0.0], [0.0, 0.0]), point([0.0, 0.0], [2.0, 0.0]))
        } else {
            (
                point([-f64::MAX, 0.0], [0.0, 0.0]),
                point([f64::MAX, 0.0], [0.0, 0.0]),
            )
        };
        measurement_failure(
            &mut session,
            &measure("local", SECOND, &start, &end),
            "local",
            "overflow",
        );
        let (status, response, _) = invoke(&mut session, r#"{"v":1,"op":"lift","doc":"a"}"#);
        assert_eq!(status, Status::Ok, "{response:?}");
        assert_eq!(session.take(0), Some(bytes));
    }
}

#[test]
fn validates_required_unknown_types_and_non_finite_json_before_measurement() {
    let (mut session, _) = session();
    for (request, code, pointer) in [
        (
            r#"{"v":1,"op":"calibrations"}"#.to_owned(),
            "call.field.missing",
            "/doc",
        ),
        (
            extra(LIST, r#""unknown":0"#),
            "call.field.unsupported",
            "/unknown",
        ),
        (
            metric().replace(r#""calibration":"metric","#, ""),
            "call.field.missing",
            "/calibration",
        ),
        (
            metric().replace(r#""offset":{"x":0.25,"y":1}"#, r#""offset":{"x":0.25}"#),
            "call.field.missing",
            "/start/offset/y",
        ),
        (
            metric().replace(
                r#""offset":{"x":0.25,"y":1}"#,
                r#""offset":{"x":0.25,"y":1,"z":0}"#,
            ),
            "call.field.unsupported",
            "/start/offset/z",
        ),
        (
            metric().replace(
                r#""origin":{"x":10000000000000000,"y":0}"#,
                r#""origin":[10000000000000000,0]"#,
            ),
            "call.value.invalid",
            "/start/origin",
        ),
        (
            metric().replacen(r#""x":0.25"#, r#""x":1e309"#, 1),
            "call.value.invalid",
            "/start/offset/x",
        ),
        (
            metric().replacen(r#""x":0.25"#, r#""x":null"#, 1),
            "call.value.invalid",
            "/start/offset/x",
        ),
    ] {
        let (status, response, _) = invoke(&mut session, &request);
        assert_eq!(status, Status::Failed, "{request}: {response:?}");
        no_result(&response);
        assert_eq!(
            text(diagnostic(&response, code), "/params/pointer"),
            pointer,
            "{response:?}"
        );
    }
    for non_finite in ["NaN", "Infinity", "-Infinity"] {
        let (status, response, _) = invoke(&mut session, &metric().replacen("0.25", non_finite, 1));
        assert_eq!(status, Status::Failed);
        diagnostic(&response, "call.json.invalid");
        no_result(&response);
    }
    assert_eq!(invoke(&mut session, &metric()).0, Status::Ok);
}

/// 公開応答の requested を次の予算にし、最後まで進んだ呼出しの最小量を得る。
fn threshold(session: &mut Session, request: &str, unit: &str, expected: Status) -> u64 {
    let code = format!("budget.{}.exceeded", unit.replace('_', "-"));
    let mut limit = 0;
    for _ in 0..256 {
        let limited = extra(request, &format!(r#""budget":{{"{unit}":{limit}}}"#));
        let (status, response, _) = invoke(session, &limited);
        let Some(Value::Array(diagnostics)) = response.pointer("/diagnostics") else {
            panic!()
        };
        if let Some(stopped) = diagnostics
            .iter()
            .find(|diagnostic| text(diagnostic, "/code") == code)
        {
            let requested = integer(stopped, "/params/requested");
            assert!(requested > limit);
            limit = requested;
        } else {
            assert_eq!(status, expected, "{response:?}");
            return limit;
        }
    }
    panic!("予算の測定が進みません: {unit}");
}

#[test]
fn honors_exact_work_and_decoded_limits_cancellation_and_retry_on_both_ops() {
    let (mut session, _) = session();
    let outside = measure(
        "metric",
        FIRST,
        &point([1e16, 0.0], [0.0, 0.0]),
        &point([1e16, 0.0], [0.0, 5.0]),
    );
    for (request, expected) in [
        (LIST.to_owned(), Status::Ok),
        (metric(), Status::Ok),
        (outside, Status::Failed),
    ] {
        let baseline = invoke(&mut session, &request).2;
        for unit in ["work", "decoded_bytes"] {
            let needed = threshold(&mut session, &request, unit, expected);
            assert!(needed > 0);
            let (status, _, exact) = invoke(
                &mut session,
                &extra(&request, &format!(r#""budget":{{"{unit}":{needed}}}"#)),
            );
            assert_eq!(status, expected);
            assert_eq!(exact, baseline);
            for limit in [0, needed - 1] {
                let (status, response, _) = invoke(
                    &mut session,
                    &extra(&request, &format!(r#""budget":{{"{unit}":{limit}}}"#)),
                );
                assert_eq!(status, Status::Failed);
                no_result(&response);
                let stopped = diagnostic(
                    &response,
                    &format!("budget.{}.exceeded", unit.replace('_', "-")),
                );
                if limit != 0 {
                    assert_eq!(integer(stopped, "/params/requested"), needed);
                }
                assert!(session.take(0).is_none());
                assert_eq!(invoke(&mut session, &request).2, baseline);
            }
        }
        let cancel = CancelToken::new();
        cancel.cancel();
        let reply = session.call(request.as_bytes(), &cancel);
        assert_eq!(reply.status, Status::Failed);
        let response = zumen_json::parse(&reply.response).unwrap();
        diagnostic(&response, "call.request.cancelled");
        no_result(&response);
        assert_eq!(invoke(&mut session, &request).2, baseline);
    }
}

#[test]
fn excludes_invalid_and_all_duplicate_calibrations_with_their_diagnostics() {
    let (source, mut drawing) = model();
    let mut duplicate = drawing.extensions[0].clone();
    duplicate.source = Ref::at_record(51);
    drawing.extensions.push(duplicate);
    let mut invalid = drawing.extensions[2].clone();
    invalid.name = "bad-matrix".to_owned();
    invalid
        .fields
        .iter_mut()
        .find(|field| field.key == "matrix_x")
        .unwrap()
        .value = FieldValue::Point([0.0; 3]);
    drawing.extensions.push(invalid);
    let mut invalid = drawing.extensions[2].clone();
    invalid.name = "bad-unit".to_owned();
    invalid
        .fields
        .iter_mut()
        .find(|field| field.key == "unit")
        .unwrap()
        .value = FieldValue::Text("yard".to_owned());
    drawing.extensions.push(invalid);
    let bytes = written(&source, &drawing);
    let mut session = opened(&bytes);
    let (status, response, _) = invoke(&mut session, LIST);
    assert_eq!(status, Status::Ok, "{response:?}");
    assert_eq!(
        items(&response)
            .iter()
            .map(|item| text(item, "/id"))
            .collect::<Vec<_>>(),
        ["local", "overlap"]
    );
    let Some(Value::Array(diagnostics)) = response.pointer("/diagnostics") else {
        panic!()
    };
    for (id, reason, count) in [
        ("metric", "duplicate-id", 2),
        ("bad-matrix", "matrix", 1),
        ("bad-unit", "fields", 1),
    ] {
        let invalid = diagnostics
            .iter()
            .find(|diagnostic| text(diagnostic, "/params/id") == id)
            .unwrap();
        assert_eq!(text(invalid, "/code"), "drawing.calibration.invalid");
        assert_eq!(text(invalid, "/params/reason"), reason);
        assert_eq!(integer(invalid, "/count"), count);
    }
    value_failure(&mut session, &metric(), "/calibration");
    assert_eq!(
        invoke(&mut session, &metric().replace("metric", "overlap")).0,
        Status::Ok
    );
    assert_eq!(
        invoke(&mut session, r#"{"v":1,"op":"lift","doc":"a"}"#).0,
        Status::Ok
    );
    assert_eq!(session.take(0), Some(bytes));
}

#[test]
fn read_only_ops_and_failures_preserve_lift_bytes_and_entity_ids() {
    let (mut session, bytes) = session();
    let entities =
        r#"{"v":1,"op":"entities","doc":"a","fields":["id","hash","source","geometry"]}"#;
    let (status, before, encoded) = invoke(&mut session, entities);
    assert_eq!(status, Status::Ok);
    assert_eq!(text(&before, "/result/items/0/id"), "sheet/hAA/r31");
    assert_eq!(text(&before, "/result/items/1/id"), "sheet/hAA/1/r32");
    assert_eq!(invoke(&mut session, LIST).0, Status::Ok);
    assert_eq!(invoke(&mut session, &metric()).0, Status::Ok);
    assert_eq!(
        invoke(&mut session, r#"{"v":1,"op":"lift","doc":"a"}"#).0,
        Status::Ok
    );
    assert_eq!(session.take(0), Some(bytes.clone()));
    assert!(session.take(0).is_none());
    assert_eq!(
        invoke(&mut session, r#"{"v":1,"op":"lift","doc":"a"}"#).0,
        Status::Ok
    );
    value_failure(
        &mut session,
        &metric().replace("metric", "missing"),
        "/calibration",
    );
    assert!(
        session.take(0).is_none(),
        "失敗した操作へ前のblobを残さない"
    );
    assert_eq!(invoke(&mut session, entities).2, encoded);
    assert_eq!(
        invoke(&mut session, r#"{"v":1,"op":"lift","doc":"a"}"#).0,
        Status::Ok
    );
    assert_eq!(session.take(0), Some(bytes));
}
