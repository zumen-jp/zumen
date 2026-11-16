//! 公開 entities で、スタイルと拡張の値・参照・編集後の再読・停止の原子性を確かめる。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力と期待する応答を読むテスト。

use zumen_abi1::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Drawing2D, FieldValue, Inherit, LineTypeId, binary};
use zumen_json::Value;

#[path = "../examples/entity_details_case.rs"]
mod fixture;

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}
fn session(drawing: Drawing2D) -> Session {
    let (source, _) = fixture::fixture();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let input = binary::write(
        &source,
        &drawing,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    opened(&input)
}
fn opened(input: &[u8]) -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"a", input).unwrap();
    let reply = call(&mut session, r#"{"v":1,"op":"open","input":"a"}"#);
    assert_eq!(reply.0, Status::Ok, "{reply:?}");
    session
}
fn request(fields: &str, extra: &str) -> String {
    format!(r#"{{"v":1,"op":"entities","doc":"a","fields":{fields}{extra}}}"#)
}
fn text<'a>(value: &'a Value, path: &str) -> &'a str {
    let Some(Value::String(value)) = value.pointer(path) else {
        panic!("{path}: {value:?}")
    };
    value
}
fn expect(value: &Value, path: &str, json: &str) {
    assert_eq!(
        value.pointer(path),
        Some(&zumen_json::parse(json.as_bytes()).unwrap()),
        "{path}: {value:?}"
    );
}

#[test]
fn own_values_extension_order_and_public_ids_are_preserved() {
    let mut session = session(fixture::fixture().1);
    let before = call(
        &mut session,
        &request(r#"["hash","geometry","source"]"#, ""),
    );
    let (status, result) = call(
        &mut session,
        &request(r#"["style","extensions","hash"]"#, ""),
    );
    assert_eq!(status, Status::Ok, "{result:?}");
    expect(
        &result,
        "/result/items/0/style/color",
        r#"{"aci":1,"rgb":[12,34,56]}"#,
    );
    expect(
        &result,
        "/result/items/0/style/line_type",
        r#"{"id":"line-type/h20/1"}"#,
    );
    expect(
        &result,
        "/result/items/0/style/line_weight",
        r#"{"mm":0.35}"#,
    );
    expect(
        &result,
        "/result/items/0/style/transparency",
        r#"{"alpha":128}"#,
    );
    expect(
        &result,
        "/result/items/1/style/color",
        r#"{"inherit":"layer"}"#,
    );
    assert!(
        result
            .pointer("/result/items/1/style/line_weight")
            .is_none()
    );
    expect(&result, "/result/items/2/extensions", "[]");
    for (index, kind, value) in [
        (1, "integer", "9223372036854775807"),
        (2, "integer", "-9223372036854775808"),
        (5, "handle", "FFFFFFFFFFFFFFFF"),
        (6, "bytes", "000f80ff"),
        (7, "bytes", ""),
    ] {
        assert_eq!(
            text(
                &result,
                &format!("/result/items/0/extensions/0/fields/{index}/value/kind")
            ),
            kind
        );
        assert_eq!(
            text(
                &result,
                &format!("/result/items/0/extensions/0/fields/{index}/value/value")
            ),
            value
        );
    }
    assert_eq!(
        text(&result, "/result/items/0/extensions/0/name"),
        "z-first"
    );
    assert_eq!(
        text(&result, "/result/items/0/extensions/1/name"),
        "a-stale"
    );
    expect(&result, "/result/items/0/extensions/0/current", "true");
    expect(&result, "/result/items/0/extensions/1/current", "false");
    assert_eq!(
        result.pointer("/result/items/0/hash"),
        result.pointer("/result/items/0/extensions/0/basis")
    );
    assert_eq!(text(&result, "/result/items/1/id"), "space/0/h1/1");
    assert_eq!(text(&result, "/result/items/1/extensions/0/name"), "second");
    assert_eq!(
        call(
            &mut session,
            &request(r#"["hash","geometry","source"]"#, "")
        ),
        before
    );
}

#[test]
fn invalid_selected_values_fail_with_the_element_source_without_poisoning_the_session() {
    for kind in ["reference", "number", "extension"] {
        let (_, mut drawing) = fixture::fixture();
        match kind {
            "reference" => {
                drawing.spaces[0].elements[0].style.line_type = Inherit::Own(LineTypeId(99))
            }
            "number" => drawing.spaces[0].elements[0].style.line_type_scale = f64::INFINITY,
            _ => drawing.extensions[0].fields[0].value = FieldValue::Real(f64::NAN),
        }
        let mut session = session(drawing);
        let plain = request(r#"["id"]"#, "");
        let before = call(&mut session, &plain);
        let fields = if kind == "extension" {
            r#"["extensions"]"#
        } else {
            r#"["style"]"#
        };
        let (status, result) = call(&mut session, &request(fields, ""));
        assert_eq!(status, Status::Failed, "{kind}: {result:?}");
        assert_eq!(
            text(&result, "/diagnostics/0/code"),
            "drawing.entity-details.failed"
        );
        assert_eq!(text(&result, "/diagnostics/0/params/id"), "space/0/h1");
        expect(&result, "/diagnostics/0/at/0/record", "0");
        assert!(result.pointer("/result").is_none());
        assert_eq!(call(&mut session, &plain), before);
    }
}

fn minimum_budget(session: &mut Session, fields: &str, kind: &str) -> u64 {
    let run = |session: &mut Session, limit| {
        call(
            session,
            &request(fields, &format!(r#", "budget":{{"{kind}":{limit}}}"#)),
        )
    };
    let (mut low, mut high) = (0, 1_u64 << 24);
    assert_eq!(run(session, high).0, Status::Ok);
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if run(session, middle).0 == Status::Ok {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}

#[test]
fn copying_extensions_has_bounded_work_and_allocation_and_retries_exactly() {
    let (_, mut drawing) = fixture::fixture();
    drawing.extensions[0].fields[0].value = FieldValue::Bytes(vec![0xAB; 32 * 1024]);
    let mut session = session(drawing);
    let fields = r#"["style","extensions"]"#;
    let req = request(fields, "");
    let expected = call(&mut session, &req);
    assert_eq!(expected.0, Status::Ok);
    for (kind, code) in [
        ("work", "budget.work.exceeded"),
        ("decoded_bytes", "budget.decoded-bytes.exceeded"),
    ] {
        let minimum = minimum_budget(&mut session, fields, kind);
        let small = minimum_budget(&mut session, r#"["style"]"#, kind);
        assert!(minimum > small, "{kind}: {minimum}, {small}");
        for limit in [small, minimum - 1] {
            let result = call(
                &mut session,
                &request(fields, &format!(r#", "budget":{{"{kind}":{limit}}}"#)),
            );
            assert_eq!(result.0, Status::Failed);
            assert_eq!(text(&result.1, "/diagnostics/0/code"), code);
            assert!(result.1.pointer("/result").is_none());
        }
        assert_eq!(
            call(
                &mut session,
                &request(fields, &format!(r#", "budget":{{"{kind}":{minimum}}}"#))
            ),
            expected
        );
    }
    let token = CancelToken::new();
    token.cancel();
    assert_eq!(session.call(req.as_bytes(), &token).status, Status::Failed);
    assert_eq!(call(&mut session, &req), expected);
}

fn first_input(case: &str) -> Vec<u8> {
    let value = zumen_json::parse(case.as_bytes()).unwrap();
    if let Some(Value::String(value)) = value.pointer("/steps/0/push/text") {
        return value.as_bytes().to_vec();
    }
    let hex = text(&value, "/steps/0/push/hex");
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn all_four_formats_return_the_style_written_by_restyle_and_reopen() {
    for (case, color, system, number) in [
        (
            include_str!("../../../bindings/conformance/cases/apply.json"),
            r#"{"aci":3}"#,
            "aci",
            3,
        ),
        (
            include_str!("../../../bindings/conformance/cases/open_dwg.json"),
            r#"{"aci":3}"#,
            "aci",
            3,
        ),
        (
            include_str!("../../../bindings/conformance/cases/open_jww.json"),
            r#"{"jww":3}"#,
            "jww",
            3,
        ),
        (
            include_str!("../../../bindings/conformance/cases/open_sxf.json"),
            r#"{"sxf":2}"#,
            "sxf",
            2,
        ),
    ] {
        let mut session = opened(&first_input(case));
        let query = request(r#"["style","extensions","hash"]"#, "");
        let before = call(&mut session, &query);
        assert_eq!(before.0, Status::Ok, "{before:?}");
        let id = text(&before.1, "/result/items/0/id");
        let hash = text(&before.1, "/result/items/0/hash");
        let apply = format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"restyle","id":"{id}","hash":"{hash}","color":{color}}}]}}"#
        );
        let written = call(&mut session, &apply);
        assert_eq!(written.0, Status::Ok, "{system}: {written:?}");
        let bytes = session.take(0).unwrap();
        let after = call(&mut session, &query);
        assert_eq!(after.0, Status::Ok, "{after:?}");
        assert_eq!(text(&after.1, "/result/items/0/id"), id);
        expect(
            &after.1,
            &format!("/result/items/0/style/color/{system}"),
            &number.to_string(),
        );
        let mut reread = opened(&bytes);
        assert_eq!(call(&mut reread, &query), after);
    }
}
