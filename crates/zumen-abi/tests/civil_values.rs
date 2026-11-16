//! J-LandXML の保持値を、出所・単位・高さの基準とともに共通 API から取得する。
#![cfg(feature = "civil")]
#![allow(clippy::unwrap_used, reason = "固定入力の期待値")]
use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::Value;

const INPUT: &str = include_str!("../../zumen-landxml/tests/fixtures/domain_values.xml");
const DETAILS: &str = r#"{"v":1,"op":"civil","doc":"road.xml","details":true,"distances":[25]}"#;

fn call(session: &mut Session, request: &str, status: Status) -> Value {
    let response = session.call(request.as_bytes(), &CancelToken::new());
    assert_eq!(
        response.status,
        status,
        "{}",
        String::from_utf8_lossy(&response.response)
    );
    zumen_json::parse(&response.response).unwrap()
}
fn expected(value: &Value, pointer: &str, json: &str) {
    assert_eq!(
        value.pointer(pointer),
        Some(&zumen_json::parse(json.as_bytes()).unwrap()),
        "{pointer}"
    );
}
fn open() -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"road.xml", INPUT.as_bytes()).unwrap();
    call(
        &mut session,
        r#"{"v":1,"op":"open","input":"road.xml"}"#,
        Status::Ok,
    );
    session
}

#[test]
fn detail_values_keep_raw_heights_and_original_points() {
    let mut session = open();
    let value = call(&mut session, DETAILS, Status::Ok);
    for (pointer, json) in [
        ("/result/alignment", r#""road""#),
        ("/result/positions/0/height", "45"),
        ("/result/details/height_reference/name", r#""local""#),
        ("/result/details/height_reference/tokyo_bay_offset", "-1.25"),
        ("/result/details/point_sets/0/points/0/northing", "123"),
        ("/result/details/point_sets/0/points/0/easting", "456"),
        ("/result/details/point_sets/0/points/0/height", "42.5"),
        (
            "/result/details/point_sets/0/points/0/class",
            r#""3級基準点""#,
        ),
        (
            "/result/details/alignment/intersection_points/1/easting",
            "470",
        ),
        (
            "/result/details/alignment/ground_profiles/0/points/1",
            r#"{"distance":25,"height":43}"#,
        ),
        ("/result/details/alignment/cant/0/start", "[-2,2]"),
        ("/result/details/alignment/cant/0/end", "[4,4]"),
        (
            "/result/details/alignment/cross_sections/0/lines/0/height_reference",
            r#""formation_offset""#,
        ),
        (
            "/result/details/alignment/cross_sections/0/lines/0/points/0",
            r#"{"offset":-2,"height":-0.5}"#,
        ),
        (
            "/result/details/alignment/cross_sections/0/lines/0/description",
            r#""道路面""#,
        ),
        (
            "/result/details/alignment/cross_sections/0/lines/0/side",
            r#""left""#,
        ),
    ] {
        expected(&value, pointer, json);
    }
    assert!(
        value
            .pointer("/result/details/point_sets/0/points/1/height")
            .is_none()
    );
    assert!(
        value
            .pointer("/result/details/alignment/cross_sections/0/lines/0/source/bytes")
            .is_some()
    );
    let normal = call(
        &mut session,
        r#"{"v":1,"op":"civil","doc":"road.xml"}"#,
        Status::Ok,
    );
    assert!(normal.pointer("/result/details").is_none());
    assert_eq!(value, call(&mut session, DETAILS, Status::Ok));
}

#[test]
fn detail_budget_stops_leave_document_usable() {
    let mut session = open();
    let expected_response = call(&mut session, DETAILS, Status::Ok);
    for budget in [r#"{"work":1}"#, r#"{"decoded_bytes":4096}"#] {
        let value = call(
            &mut session,
            &format!(r#"{{"v":1,"op":"civil","doc":"road.xml","details":true,"budget":{budget}}}"#),
            Status::Failed,
        );
        expected(&value, "/blobs", "[]");
        assert!(value.pointer("/result").is_none());
        assert_eq!(call(&mut session, DETAILS, Status::Ok), expected_response);
    }
    call(
        &mut session,
        r#"{"v":1,"op":"civil","alignment":{"horizontal":[]},"details":true}"#,
        Status::Failed,
    );
}

#[test]
fn detail_selection_requires_a_unique_alignment() {
    let mut session = Session::new(b"").unwrap();
    let src = INPUT.replace(
        "</Alignments>",
        r#"<Alignment name="another" staStart="0"/></Alignments>"#,
    );
    session.push(b"road.xml", src.as_bytes()).unwrap();
    call(
        &mut session,
        r#"{"v":1,"op":"open","input":"road.xml"}"#,
        Status::Ok,
    );
    call(&mut session, DETAILS, Status::Failed);
    let value = call(
        &mut session,
        r#"{"v":1,"op":"civil","doc":"road.xml","details":true,"id":"another"}"#,
        Status::Ok,
    );
    expected(&value, "/result/details/alignment/id", r#""another""#);
    expected(&value, "/result/details/alignment/cant", "[]");
}
