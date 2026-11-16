//! 図面の件数・ビュー・記号定義の公開契約を独立した期待値で確かめる。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作入力と期待する応答だけを扱う。
use zumen_abi1::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Drawing2D, SheetId, SpaceId, binary};
use zumen_json::Value;
#[path = "../examples/drawing_metadata_case.rs"]
mod fixture;
#[path = "../examples/drawing_metadata_formats.rs"]
mod formats;

fn opened(drawing: Drawing2D) -> Session {
    let bytes = binary::write(
        &fixture::fixture().0,
        &drawing,
        &mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ),
    )
    .unwrap();
    let mut session = Session::new(b"").unwrap();
    session.push(b"a", &bytes).unwrap();
    formats::call(&mut session, r#"{"v":1,"op":"open","input":"a"}"#).unwrap();
    session
}
fn call(session: &mut Session, op: &str) -> Value {
    let result = formats::call(session, &format!(r#"{{"v":1,"op":"{op}","doc":"a"}}"#)).unwrap();
    zumen_json::parse(&result).unwrap()
}
fn expect(value: &Value, path: &str, json: &str) {
    assert_eq!(
        value.pointer(path),
        Some(&zumen_json::parse(json.as_bytes()).unwrap()),
        "{path}: {value:?}"
    );
}

#[test]
fn counts_every_kind_without_expanding_symbols_and_preserves_view_transforms() {
    let mut session = opened(fixture::fixture().1);
    let result = call(&mut session, "drawing");
    for name in [
        "lines",
        "texts",
        "circles",
        "curves",
        "polylines",
        "formatted_texts",
        "dimensions",
        "leaders",
        "callouts",
        "tables",
        "underlays",
        "symbols",
        "hatches",
        "points",
    ] {
        expect(&result, &format!("/result/spaces/0/elements/{name}"), "1");
        expect(&result, &format!("/result/spaces/1/elements/{name}"), "0");
        expect(
            &result,
            &format!("/result/sheets/0/elements/{name}"),
            if name == "lines" { "1" } else { "0" },
        );
    }
    expect(&result, "/result/sheets/0/views", "3");
    expect(
        &result,
        "/result/sheets/0/view_details/0/space",
        r#""space/0""#,
    );
    expect(
        &result,
        "/result/sheets/0/view_details/0/matrix",
        "[0,-2,10,2,0,20]",
    );
    expect(
        &result,
        "/result/sheets/0/view_details/0/ratio",
        "[0.002,0.002]",
    );
    expect(&result, "/result/sheets/0/view_details/0/rotation", "90");
    expect(&result, "/result/sheets/0/view_details/0/mirrored", "false");
    expect(
        &result,
        "/result/sheets/0/view_details/0/model_clip",
        r#"[{"x":0,"y":0},{"x":2,"y":0},{"x":2,"y":2},{"x":0,"y":2}]"#,
    );
    expect(
        &result,
        "/result/sheets/0/view_details/0/extent",
        r#"{"min_x":0,"min_y":0,"max_x":2,"max_y":2}"#,
    );
    expect(&result, "/result/sheets/0/view_details/1/mirrored", "true");
    expect(
        &result,
        "/result/sheets/0/view_details/1/matrix",
        "[1,1,0,0,-1,0]",
    );
    assert!(
        result
            .pointer("/result/sheets/0/view_details/1/extent")
            .is_none()
    );
    assert!(
        result
            .pointer("/result/sheets/0/view_details/2/ratio")
            .is_none()
    );
    // 旧フィールドの extent は従来どおり線と文字から求める。
    expect(
        &result,
        "/result/spaces/0/extent",
        r#"{"min_x":0,"min_y":0,"max_x":2,"max_y":3}"#,
    );
}

#[test]
fn returns_unused_definitions_duplicate_ids_and_attribute_defaults_in_source_order() {
    let mut session = opened(fixture::fixture().1);
    let result = call(&mut session, "tables");
    for (index, name, id) in [(0, "part", "space/h30"), (1, "unused", "space/h30/1")] {
        let at = format!("/result/symbols/{index}");
        expect(&result, &format!("{at}/name"), &format!("\"{name}\""));
        expect(&result, &format!("{at}/id"), &format!("\"{id}\""));
        expect(&result, &format!("{at}/base"), r#"{"x":3,"y":4}"#);
        expect(&result, &format!("{at}/attributes/0/tag"), r#""TAG""#);
        expect(
            &result,
            &format!("{at}/attributes/0/formatted_value"),
            r#""書式\\Pつき""#,
        );
        expect(&result, &format!("{at}/attributes/0/hidden"), "true");
        expect(
            &result,
            &format!("{at}/attributes/0/constant"),
            if index == 0 { "true" } else { "false" },
        );
        let response = formats::call(
            &mut session,
            &format!(r#"{{"v":1,"op":"entities","doc":"a","place":"{id}"}}"#),
        )
        .unwrap();
        expect(&zumen_json::parse(&response).unwrap(), "/result/total", "0");
    }
    expect(&result, "/result/symbols/1/external", r#""unopened.dxf""#);
}

#[test]
fn invalid_references_singular_transforms_and_nonfinite_values_are_diagnosed() {
    for (mutation, reason) in [
        (0, "view-space"),
        (1, "view-transform"),
        (2, "view-singular"),
        (3, "view-clip"),
        (4, "view-scale"),
        (5, "view-range"),
    ] {
        let mut drawing = fixture::fixture().1;
        let view = &mut drawing.sheets[0].views[0];
        match mutation {
            0 => view.space = SpaceId(99),
            1 => view.sheet = SheetId(99),
            2 => view.transform.m = [1.0, 1.0, 0.0, 1.0, 1.0, 0.0],
            3 => view.clip[0].sheet = SheetId(99),
            4 => view.transform.m[0] = f64::MAX,
            _ => {
                view.transform.m = [1e-300, 0.0, 0.0, 0.0, 1e-300, 0.0];
                view.clip[0].x = 1e300;
            }
        }
        // 有限な大きい行列の列長がオーバーフローするケース。
        if mutation == 4 {
            view.transform.m[3] = f64::MAX;
        }
        let mut session = opened(drawing);
        let reply = session.call(br#"{"v":1,"op":"drawing","doc":"a"}"#, &CancelToken::new());
        assert_eq!(reply.status, Status::Failed);
        let result = zumen_json::parse(&reply.response).unwrap();
        expect(
            &result,
            "/diagnostics/0/code",
            r#""drawing.metadata.failed""#,
        );
        expect(
            &result,
            "/diagnostics/0/params/reason",
            &format!("\"{reason}\""),
        );
        expect(&result, "/diagnostics/0/params/id", r#""sheet/h51""#);
        assert!(result.pointer("/result").is_none());
        call(&mut session, "tables");
    }
}

#[test]
fn five_encodings_can_add_a_symbol_named_by_the_definition_list_and_reopen_it() {
    formats::steps().unwrap();
}

#[test]
fn unsupported_symbol_options_reject_the_batch_without_changing_the_document() {
    for (format, input, place, layer) in formats::inputs().into_iter().skip(2) {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a", &input).unwrap();
        formats::call(&mut session, r#"{"v":1,"op":"open","input":"a"}"#).unwrap();
        let before = call(&mut session, "drawing");
        let tables = call(&mut session, "tables");
        let Some(Value::String(name)) = tables.pointer("/result/symbols/0/name") else {
            panic!("{tables:?}")
        };
        let valid = format!(
            r#"{{"kind":"add","place":"{place}","layer":"{layer}","geometry":{{"shape":"symbol","name":"{name}","position":[10,20],"scale":[2,2],"rotation":30}}}}"#
        );
        for extra in [
            r#", "color":{"rgb":[1,2,3]}"#,
            r#", "line_weight":{"mm":0.5}"#,
        ] {
            let invalid = valid.replacen(r#""kind":"add""#, &format!(r#""kind":"add"{extra}"#), 1);
            let request =
                format!(r#"{{"v":1,"op":"apply","doc":"a","edits":[{valid},{invalid}]}}"#);
            let response = session.call(request.as_bytes(), &CancelToken::new());
            assert_eq!(
                response.status,
                Status::Failed,
                "{format}: {:?}",
                String::from_utf8_lossy(&response.response)
            );
            assert!(session.take(0).is_none());
            assert_eq!(call(&mut session, "drawing"), before, "{format}");
            assert_eq!(call(&mut session, "tables"), tables, "{format}");
        }
    }
}
