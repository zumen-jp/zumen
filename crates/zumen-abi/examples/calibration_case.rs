//! 校正の適合入力を、公開の図面バイナリ writer で作る。
//!
//! `cargo run -p zumen-abi --example calibration_case [--check]`
//! 受領図面は使わず、同じ fixture を ABI の回帰テストからも読む。

#![cfg_attr(test, allow(dead_code))]

#[cfg(not(test))]
use std::path::Path;

#[cfg(not(test))]
use zumen_abi1::{Session, Status};
#[cfg(not(test))]
use zumen_base::{Budget, CancelToken, Diagnostics};
use zumen_base::{Cx, Key, Ref, Stop};
use zumen_drawing::{
    Calibration, Drawing2D, Element, Hash, LayerId, PaperPoint, Shape, Sheet, SheetId, SheetItem,
    Style, Unit, binary,
};
#[cfg(not(test))]
use zumen_json::Value;

/// 校正の並び・公開 ID・未知単位・原本ハッシュを区別できる合成の図面。
pub fn fixture(cx: &mut Cx<'_>) -> Result<(binary::Source, Drawing2D), Stop> {
    let sheet = |id, name: &str, record, x0, x1| Sheet {
        id: SheetId(id),
        name: Some(name.to_owned()),
        size: None,
        elements: vec![Element {
            source: Ref::at_record(record),
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Line {
                start: PaperPoint {
                    sheet: SheetId(id),
                    x: x0,
                    y: 0.0,
                },
                end: PaperPoint {
                    sheet: SheetId(id),
                    x: x1,
                    y: 4.0,
                },
            },
        }],
        views: Vec::new(),
        order: vec![SheetItem::Element(0)],
        source: Ref::default().with_key(Key::Handle(0xAA)),
    };
    let mut drawing = Drawing2D {
        sheets: vec![
            sheet(7, "large", 31, 1e16, 1e16 + 4.0),
            sheet(9, "local", 32, 0.0, 10.0),
        ],
        ..Drawing2D::default()
    };
    let mut drafts = Vec::new();
    for calibration in [
        Calibration {
            id: "metric".to_owned(),
            name: Some("測距".to_owned()),
            sheet: SheetId(7),
            bounds: [1e16, 0.0, 1e16 + 4.0, 4.0],
            matrix: [2.0, 0.0, 0.0, 2.0],
            unit: Unit::Meter,
            unit_label: "m".to_owned(),
            source: Ref::at_record(41)
                .with_bytes(410, 450)
                .with_key(Key::Handle(0xB1)),
        },
        Calibration {
            id: "local".to_owned(),
            name: None,
            sheet: SheetId(9),
            bounds: [0.0, 0.0, 20.0, 20.0],
            matrix: [1.0, 0.0, 0.0, 1.0],
            unit: Unit::Unknown,
            unit_label: "尺".to_owned(),
            source: Ref::at_record(43)
                .with_bytes(430, 470)
                .with_key(Key::Instance(77)),
        },
        Calibration {
            id: "overlap".to_owned(),
            name: Some("重なり".to_owned()),
            sheet: SheetId(7),
            bounds: [1e16, 0.0, 1e16 + 4.0, 4.0],
            matrix: [1.0, 0.0, 0.0, 1.0],
            unit: Unit::Meter,
            unit_label: "m".to_owned(),
            source: Ref::at_record(42)
                .with_bytes(420, 460)
                .with_key(Key::Handle(0xB2)),
        },
    ] {
        drafts.push(calibration.into_draft(cx)?);
    }
    drawing.settle_extensions(drafts, cx)?;
    Ok((
        binary::Source {
            format: "test-calibration".to_owned(),
            family: "drawing".to_owned(),
            hash: Hash([0x11; 16]),
        },
        drawing,
    ))
}

#[cfg(not(test))]
fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        for digit in [byte >> 4, byte & 15] {
            text.push(char::from(if digit < 10 {
                b'0' + digit
            } else {
                b'a' + digit - 10
            }));
        }
    }
    text
}

#[cfg(not(test))]
fn text<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    match value.pointer(pointer) {
        Some(Value::String(text)) => Some(text),
        _ => None,
    }
}

#[cfg(not(test))]
fn minimum_budget(session: &mut Session, request: &str, field: &str) -> Result<u64, String> {
    let prefix = request.strip_suffix('}').ok_or("要求の終端がない")?;
    let code = format!("budget.{}.exceeded", field.replace('_', "-"));
    let mut limit = 0;
    for _ in 0..4096 {
        let request = format!("{prefix},\"budget\":{{\"{field}\":{limit}}}}}");
        let reply = session.call(request.as_bytes(), &CancelToken::new());
        if reply.status == Status::Ok {
            return Ok(limit);
        }
        let response = zumen_json::parse(&reply.response).map_err(|error| format!("{error:?}"))?;
        if text(&response, "/diagnostics/0/code") != Some(code.as_str()) {
            return Err(format!("予算以外で停止: {response:?}"));
        }
        let next = match response.pointer("/diagnostics/0/params/requested") {
            Some(Value::Number(number)) => number.as_str().parse::<u64>().ok(),
            _ => None,
        }
        .ok_or("停止量がない")?;
        if next <= limit {
            return Err("停止量が進まない".to_owned());
        }
        limit = next;
    }
    Err("予算測量の回数を超えた".to_owned())
}

#[cfg(not(test))]
fn generate() -> Result<String, String> {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let (source, drawing) = fixture(&mut cx).map_err(|error| format!("{error:?}"))?;
    let bytes = binary::write(&source, &drawing, &mut cx).map_err(|error| format!("{error:?}"))?;
    let mut overflow = drawing.clone();
    for extension in &mut overflow.extensions {
        if extension.namespace == "drawing.calibration" && extension.name == "local" {
            for field in &mut extension.fields {
                if field.key == "matrix_x" {
                    field.value = zumen_drawing::FieldValue::Point([f64::MAX, 0.0, 0.0]);
                }
            }
        }
    }
    let overflow_bytes =
        binary::write(&source, &overflow, &mut cx).map_err(|error| format!("{error:?}"))?;
    let mut session = Session::new(b"").map_err(|status| format!("{status:?}"))?;
    session
        .push(b"calibrated", &bytes)
        .map_err(|status| format!("{status:?}"))?;
    let opened = session.call(
        br#"{"v":1,"op":"open","input":"calibrated"}"#,
        &CancelToken::new(),
    );
    if opened.status != Status::Ok {
        return Err(format!("open失敗: {:?}", opened.response));
    }
    let work = minimum_budget(&mut session, LIST, "work")?;
    let decoded = minimum_budget(&mut session, LIST, "decoded_bytes")?;
    let measure_work = minimum_budget(&mut session, MEASURE, "work")?;
    let measure_decoded = minimum_budget(&mut session, MEASURE, "decoded_bytes")?;
    let generated = TEMPLATE
        .replace("$HEX", &hex(&bytes))
        .replace("$OVERFLOW_HEX", &hex(&overflow_bytes))
        .replace("$LIST_WORK_LESS", &work.saturating_sub(1).to_string())
        .replace("$LIST_WORK", &work.to_string())
        .replace("$LIST_DECODED_LESS", &decoded.saturating_sub(1).to_string())
        .replace("$LIST_DECODED", &decoded.to_string())
        .replace(
            "$MEASURE_WORK_LESS",
            &measure_work.saturating_sub(1).to_string(),
        )
        .replace("$MEASURE_WORK", &measure_work.to_string())
        .replace(
            "$MEASURE_DECODED_LESS",
            &measure_decoded.saturating_sub(1).to_string(),
        )
        .replace("$MEASURE_DECODED", &measure_decoded.to_string());
    zumen_json::parse(generated.as_bytes()).map_err(|error| format!("{error:?}"))?;
    Ok(generated)
}

#[cfg(not(test))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let check = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("--check") => true,
        Some(other) => return Err(format!("知らない引数: {other}").into()),
    };
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/conformance/cases/calibration.json");
    let generated = generate()?;
    if check {
        if std::fs::read_to_string(&path)? != generated {
            return Err("校正の適合ケースが生成結果と異なる".into());
        }
    } else {
        std::fs::write(path, generated)?;
    }
    Ok(())
}

const LIST: &str = r#"{"v":1,"op":"calibrations","doc":"calibrated"}"#;
const MEASURE: &str = r#"{"v":1,"op":"measure_distance","doc":"calibrated","from":"sheet/hAA","calibration":"metric","start":{"origin":{"x":1e16,"y":0},"offset":{"x":0.25,"y":0}},"end":{"origin":{"x":1e16,"y":0},"offset":{"x":0.75,"y":0}}}"#;

const TEMPLATE: &str = r#"{
  "description": "校正入りの合成ZMDRを公開push/openで読み、元の文書hash、公開用紙ID、原拡張順・未知単位・明示ID・分割座標・閉領域・予算停止と再利用を確かめる。cargo run -p zumen-abi --example calibration_case [--check] で再現する。形・ID・入力バイト列は操作で変えない",
  "module": "drawing",
  "steps": [
    { "push": { "name": "calibrated", "hex": "$HEX" } },
    { "call": { "v": 1, "op": "open", "input": "calibrated" }, "expect": { "/result/format": "test-calibration" } },
    { "call": { "v": 1, "op": "drawing", "doc": "calibrated" }, "expect": { "/result/hash": "v2:11111111111111111111111111111111", "/result/sheets/0/id": "sheet/hAA", "/result/sheets/1/id": "sheet/hAA/1" } },
    {
      "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "expected_hash": "v2:11111111111111111111111111111111" },
      "expect": {
        "/result/hash": "v2:11111111111111111111111111111111", "/result/total": 3,
        "/result/items/0/id": "metric", "/result/items/0/name": "測距", "/result/items/0/from": "sheet/hAA", "/result/items/0/bounds": [10000000000000000, 0, 10000000000000004, 4], "/result/items/0/matrix": [2, 0, 0, 2], "/result/items/0/unit": "meter", "/result/items/0/unit_label": "m", "/result/items/0/source/record": 41, "/result/items/0/source/bytes/start": 410, "/result/items/0/source/bytes/end": 450, "/result/items/0/source/handle": "B1",
        "/result/items/1/id": "local", "/result/items/1/from": "sheet/hAA/1", "/result/items/1/unit": "unknown", "/result/items/1/unit_label": "尺", "/result/items/1/source/instance": "77",
        "/result/items/2/id": "overlap", "/result/items/2/from": "sheet/hAA"
      },
      "absent": ["/result/next_offset", "/result/items/1/name", "/diagnostics/0"]
    },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "limit": 1 }, "expect": { "/result/total": 3, "/result/next_offset": 1, "/result/items/0/id": "metric" }, "absent": ["/result/items/1"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "offset": 1, "limit": 1 }, "expect": { "/result/next_offset": 2, "/result/items/0/id": "local" } },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "offset": 2, "limit": 1 }, "expect": { "/result/items/0/id": "overlap" }, "absent": ["/result/next_offset"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "from": "sheet/hAA", "offset": 1, "limit": 1 }, "expect": { "/result/total": 2, "/result/items/0/id": "overlap" }, "absent": ["/result/next_offset"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "from": "sheet/hAA/1" }, "expect": { "/result/total": 1, "/result/items/0/id": "local" } },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "offset": 8 }, "expect": { "/result/items": [], "/result/total": 3 }, "absent": ["/result/next_offset"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "from": "sheet/7" }, "status": "failed", "expect": { "/diagnostics/0/code": "call.value.invalid", "/diagnostics/0/params/pointer": "/from" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "expected_hash": "v2:22222222222222222222222222222222" }, "status": "failed", "expect": { "/diagnostics/0/code": "call.value.invalid", "/diagnostics/0/params/pointer": "/expected_hash" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "limit": 0 }, "status": "failed", "expect": { "/diagnostics/0/code": "call.value.invalid", "/diagnostics/0/params/pointer": "/limit" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "limit": 10001 }, "status": "failed", "expect": { "/diagnostics/0/params/pointer": "/limit" } },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "extra": true }, "status": "failed", "expect": { "/diagnostics/0/code": "call.field.unsupported", "/diagnostics/0/params/pointer": "/extra" } },
    { "call": { "v": 1, "op": "calibrations" }, "status": "failed", "expect": { "/diagnostics/0/code": "call.field.missing", "/diagnostics/0/params/pointer": "/doc" } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "metric", "start": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": 0.25, "y": 0 } }, "end": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": 0.75, "y": 0 } } }, "expect": { "/result/hash": "v2:11111111111111111111111111111111", "/result/from": "sheet/hAA", "/result/calibration": "metric", "/result/distance": 1, "/result/unit": "meter", "/result/unit_label": "m" } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "overlap", "start": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": 0.25, "y": 0 } }, "end": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": 0.75, "y": 0 } } }, "expect": { "/result/distance": 0.5, "/result/calibration": "overlap" } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA/1", "calibration": "local", "start": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 0, "y": 0 } }, "end": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 3, "y": 4 } } }, "expect": { "/result/distance": 5, "/result/unit": "unknown", "/result/unit_label": "尺" } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA/1", "calibration": "local", "start": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 1, "y": 0 } }, "end": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": -9999999999999998, "y": 0 } } }, "expect": { "/result/distance": 1 } },
    { "call_text": "{\"v\":1,\"op\":\"measure_distance\",\"doc\":\"calibrated\",\"from\":\"sheet/hAA/1\",\"calibration\":\"local\",\"start\":{\"origin\":{\"x\":1.7976931348623157e308,\"y\":0},\"offset\":{\"x\":-1.7976931348623157e308,\"y\":0}},\"end\":{\"origin\":{\"x\":-1.7976931348623157e308,\"y\":0},\"offset\":{\"x\":1.7976931348623157e308,\"y\":0}}}", "expect": { "/result/distance": 0 } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA/1", "calibration": "metric", "start": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 0, "y": 0 } }, "end": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 1, "y": 0 } } }, "status": "failed", "expect": { "/diagnostics/0/code": "drawing.measurement.failed", "/diagnostics/0/params/id": "metric", "/diagnostics/0/params/reason": "sheet-mismatch" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "unknown", "start": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 0, "y": 0 } }, "end": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 1, "y": 0 } } }, "status": "failed", "expect": { "/diagnostics/0/code": "call.value.invalid", "/diagnostics/0/params/pointer": "/calibration" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "metric", "start": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": -0.25, "y": 0 } }, "end": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": 0.75, "y": 0 } } }, "status": "failed", "expect": { "/diagnostics/0/code": "drawing.measurement.failed", "/diagnostics/0/params/reason": "outside-bounds" }, "absent": ["/result"] },
    { "call_text": "{\"v\":1,\"op\":\"measure_distance\",\"doc\":\"calibrated\",\"from\":\"sheet/hAA\",\"calibration\":\"metric\",\"start\":{\"origin\":{\"x\":1e16,\"y\":0},\"offset\":{\"x\":0,\"y\":0}},\"end\":{\"origin\":{\"x\":10000000000000004,\"y\":0},\"offset\":{\"x\":5e-324,\"y\":0}}}", "status": "failed", "expect": { "/diagnostics/0/code": "drawing.measurement.failed", "/diagnostics/0/params/reason": "outside-bounds" }, "absent": ["/result"] },
    { "call_text": "{\"v\":1,\"op\":\"measure_distance\",\"doc\":\"calibrated\",\"from\":\"sheet/hAA\",\"calibration\":\"metric\",\"start\":{\"origin\":{\"x\":1e309,\"y\":0},\"offset\":{\"x\":0,\"y\":0}},\"end\":{\"origin\":{\"x\":1e16,\"y\":0},\"offset\":{\"x\":0,\"y\":0}}}", "status": "failed", "expect": { "/diagnostics/0/code": "call.value.invalid", "/diagnostics/0/params/pointer": "/start/origin/x" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "metric", "start": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": 0, "y": 0 }, "extra": 0 }, "end": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": 0, "y": 0 } } }, "status": "failed", "expect": { "/diagnostics/0/code": "call.field.unsupported", "/diagnostics/0/params/pointer": "/start/extra" } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "metric", "start": { "origin": { "x": 10000000000000000, "y": 0 } }, "end": { "origin": { "x": 10000000000000000, "y": 0 }, "offset": { "x": 0, "y": 0 } } }, "status": "failed", "expect": { "/diagnostics/0/code": "call.field.missing", "/diagnostics/0/params/pointer": "/start/offset" } },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "budget": { "work": 0 } }, "status": "failed", "expect": { "/diagnostics/0/code": "budget.work.exceeded" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "budget": { "work": $LIST_WORK_LESS } }, "status": "failed", "expect": { "/diagnostics/0/code": "budget.work.exceeded", "/diagnostics/0/params/requested": $LIST_WORK }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "budget": { "work": $LIST_WORK } }, "expect": { "/result/total": 3 } },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "budget": { "decoded_bytes": 0 } }, "status": "failed", "expect": { "/diagnostics/0/code": "budget.decoded-bytes.exceeded" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "budget": { "decoded_bytes": $LIST_DECODED_LESS } }, "status": "failed", "expect": { "/diagnostics/0/code": "budget.decoded-bytes.exceeded", "/diagnostics/0/params/requested": $LIST_DECODED }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated", "budget": { "decoded_bytes": $LIST_DECODED } }, "expect": { "/result/total": 3 } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "metric", "start": { "origin": { "x": 1e16, "y": 0 }, "offset": { "x": 0.25, "y": 0 } }, "end": { "origin": { "x": 1e16, "y": 0 }, "offset": { "x": 0.75, "y": 0 } }, "budget": { "work": $MEASURE_WORK_LESS } }, "status": "failed", "expect": { "/diagnostics/0/code": "budget.work.exceeded", "/diagnostics/0/params/requested": $MEASURE_WORK }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "metric", "start": { "origin": { "x": 1e16, "y": 0 }, "offset": { "x": 0.25, "y": 0 } }, "end": { "origin": { "x": 1e16, "y": 0 }, "offset": { "x": 0.75, "y": 0 } }, "budget": { "work": $MEASURE_WORK } }, "expect": { "/result/distance": 1 } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "metric", "start": { "origin": { "x": 1e16, "y": 0 }, "offset": { "x": 0.25, "y": 0 } }, "end": { "origin": { "x": 1e16, "y": 0 }, "offset": { "x": 0.75, "y": 0 } }, "budget": { "decoded_bytes": $MEASURE_DECODED_LESS } }, "status": "failed", "expect": { "/diagnostics/0/code": "budget.decoded-bytes.exceeded", "/diagnostics/0/params/requested": $MEASURE_DECODED }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA", "calibration": "metric", "start": { "origin": { "x": 1e16, "y": 0 }, "offset": { "x": 0.25, "y": 0 } }, "end": { "origin": { "x": 1e16, "y": 0 }, "offset": { "x": 0.75, "y": 0 } }, "budget": { "decoded_bytes": $MEASURE_DECODED } }, "expect": { "/result/distance": 1 } },
    { "call": { "v": 1, "op": "calibrations", "doc": "calibrated" }, "expect": { "/result/total": 3, "/result/items/0/id": "metric", "/result/items/1/id": "local", "/result/items/2/id": "overlap" } },
    { "push": { "name": "overflow", "hex": "$OVERFLOW_HEX" } },
    { "call": { "v": 1, "op": "open", "input": "overflow" }, "expect": { "/result/format": "test-calibration" } },
    { "call": { "v": 1, "op": "measure_distance", "doc": "overflow", "from": "sheet/hAA/1", "calibration": "local", "start": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 0, "y": 0 } }, "end": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 2, "y": 0 } } }, "status": "failed", "expect": { "/diagnostics/0/code": "drawing.measurement.failed", "/diagnostics/0/params/id": "local", "/diagnostics/0/params/reason": "overflow" }, "absent": ["/result"] },
    { "call": { "v": 1, "op": "measure_distance", "doc": "calibrated", "from": "sheet/hAA/1", "calibration": "local", "start": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 0, "y": 0 } }, "end": { "origin": { "x": 0, "y": 0 }, "offset": { "x": 3, "y": 4 } } }, "expect": { "/result/distance": 5, "/result/unit": "unknown", "/result/unit_label": "尺" } },
    { "call": { "v": 1, "op": "entities", "doc": "calibrated", "fields": ["id", "geometry"] }, "expect": { "/result/total": 2, "/result/items/0/id": "sheet/hAA/r31", "/result/items/1/id": "sheet/hAA/1/r32" } },
    { "call": { "v": 1, "op": "lift", "doc": "calibrated" }, "expect": { "/result/blob": 0, "/blobs/0/kind": "drawing" } },
    { "take": 0 }
  ]
}
"#;
