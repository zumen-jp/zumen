//! 入力なしの作図と、失敗時に文書を残さない契約。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 合成入力と固定の期待値。

#[path = "../examples/create_case.rs"]
mod cases;

use cases::{APPLY, CREATE, DRAWING, ENTITIES, TABLES, call};
use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::Value;

fn array<'a>(value: &'a Value, path: &str) -> &'a [Value] {
    let Some(Value::Array(items)) = value.pointer(path) else {
        panic!("{path}: {value:?}")
    };
    items
}

fn text<'a>(value: &'a Value, path: &str) -> &'a str {
    let Some(Value::String(text)) = value.pointer(path) else {
        panic!("{path}: {value:?}")
    };
    text
}

#[test]
fn creates_default_tables_and_empty_spaces_then_edits_and_reopens() {
    let mut session = Session::new(b"").unwrap();
    let created = call(&mut session, CREATE);
    let empty = session.take(0).unwrap();
    assert_eq!(text(&created, "/result/doc"), "a");
    assert_eq!(text(&created, "/result/format"), "dxf");
    assert_eq!(text(&created, "/blobs/0/kind"), "file");
    assert!(array(&created, "/diagnostics").is_empty());
    let drawing = call(&mut session, DRAWING);
    assert_eq!(
        drawing.pointer("/result/hash"),
        created.pointer("/result/hash")
    );
    assert_eq!(array(&drawing, "/result/spaces").len(), 1);
    assert_eq!(text(&drawing, "/result/spaces/0/id"), "space/0");
    assert_eq!(text(&drawing, "/result/spaces/0/unit"), "millimeter");
    assert!(array(&call(&mut session, ENTITIES), "/result/items").is_empty());
    let tables = call(&mut session, TABLES);
    assert_eq!(
        array(&tables, "/result/layers")
            .iter()
            .map(|v| text(v, "/name"))
            .collect::<Vec<_>>(),
        ["0"]
    );
    let names = |kind: &str| {
        array(&tables, &format!("/result/{kind}"))
            .iter()
            .map(|v| text(v, "/name").to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(names("line_types"), ["ByBlock", "ByLayer", "Continuous"]);
    assert_eq!(names("text_styles"), ["Standard"]);
    assert_eq!(names("dimension_styles"), ["Standard"]);
    let mut reopened = Session::new(b"").unwrap();
    reopened.push(b"a", &empty).unwrap();
    call(&mut reopened, r#"{"v":1,"op":"open","input":"a"}"#);
    assert_eq!(call(&mut reopened, DRAWING), drawing);
    assert_eq!(call(&mut reopened, TABLES), tables);
    let applied = call(&mut session, APPLY);
    assert_eq!(array(&applied, "/result/applied").len(), 4);
    let written = session.take(0).unwrap();
    let entities = call(&mut session, ENTITIES);
    let items = array(&entities, "/result/items");
    assert_eq!(items.len(), 3);
    assert_eq!(text(&items[0], "/geometry/shape"), "line");
    assert_eq!(text(&items[0], "/layer"), "Draft");
    assert_eq!(text(&items[1], "/geometry/shape"), "circle");
    assert_eq!(text(&items[2], "/geometry/content"), "Test");
    assert_eq!(text(&items[2], "/geometry/style"), "Standard");
    let tables = call(&mut session, TABLES);
    let mut reopened = Session::new(b"").unwrap();
    reopened.push(b"a", &written).unwrap();
    call(&mut reopened, r#"{"v":1,"op":"open","input":"a"}"#);
    assert_eq!(call(&mut reopened, ENTITIES), entities);
    assert_eq!(call(&mut reopened, TABLES), tables);
}

#[test]
fn unit_and_default_are_preserved_and_names_do_not_change_file_bytes() {
    let mut default = None;
    for (unit, insunits) in [
        ("millimeter", 4),
        ("centimeter", 5),
        ("meter", 6),
        ("inch", 1),
        ("foot", 2),
        ("unknown", 0),
    ] {
        let mut session = Session::new(b"").unwrap();
        call(
            &mut session,
            &format!(r#"{{"v":1,"op":"create","format":"dxf","to":"a","unit":"{unit}"}}"#),
        );
        let bytes = session.take(0).unwrap();
        assert!(String::from_utf8_lossy(&bytes).contains(&format!("$INSUNITS\n70\n{insunits}\n")));
        assert_eq!(
            text(&call(&mut session, DRAWING), "/result/spaces/0/unit"),
            unit
        );
        if unit == "millimeter" {
            default = Some(bytes);
        }
    }
    for name in ["a", "new.dxf", "日本語.dxf"] {
        let mut session = Session::new(b"").unwrap();
        call(
            &mut session,
            &format!(r#"{{"v":1,"op":"create","format":"dxf","to":"{name}"}}"#),
        );
        assert_eq!(session.take(0), default);
    }
}

#[test]
fn capabilities_advertise_only_creatable_formats() {
    let mut session = Session::new(b"").unwrap();
    let capability = call(&mut session, r#"{"v":1,"op":"capabilities"}"#);
    assert!(array(&capability, "/result/ops").contains(&Value::String("create".into())));
    for item in array(&capability, "/result/formats") {
        assert_eq!(
            item.pointer("/create"),
            Some(&Value::Bool(text(item, "/id") == "dxf"))
        );
    }
}

#[test]
fn invalid_values_and_existing_names_leave_the_document_unchanged() {
    let mut session = Session::new(b"").unwrap();
    call(&mut session, CREATE);
    let before = call(&mut session, DRAWING);
    for (request, code) in [
        (CREATE, "call.document.exists"),
        (
            r#"{"v":1,"op":"create","format":"sxf","to":"new"}"#,
            "create.format.unsupported",
        ),
        (
            r#"{"v":1,"op":"create","format":"jww","to":"new"}"#,
            "create.format.unsupported",
        ),
        (
            r#"{"v":1,"op":"create","format":"DXF","to":"new"}"#,
            "create.format.unsupported",
        ),
        (
            r#"{"v":1,"op":"create","format":"dxf","to":"../new"}"#,
            "call.value.invalid",
        ),
        (
            r#"{"v":1,"op":"create","format":"dxf","to":"new","unit":"yard"}"#,
            "call.value.invalid",
        ),
    ] {
        let reply = session.call(request.as_bytes(), &CancelToken::new());
        assert_eq!(reply.status, Status::Failed);
        let value = zumen_json::parse(&reply.response).unwrap();
        assert_eq!(text(&value, "/diagnostics/0/code"), code, "{value:?}");
        assert!(array(&value, "/blobs").is_empty());
        assert!(session.take(0).is_none());
        assert_eq!(call(&mut session, DRAWING), before);
    }
    call(
        &mut session,
        r#"{"v":1,"op":"create","format":"dxf","to":"new"}"#,
    );
    call(&mut session, r#"{"v":1,"op":"close","docs":["a"]}"#);
    call(&mut session, CREATE);
    assert_eq!(call(&mut session, DRAWING), before);
}

#[test]
fn budget_boundaries_and_cancellation_publish_nothing_and_allow_retry() {
    for kind in ["work", "decoded_bytes", "records"] {
        let limit = cases::minimum_budget(kind);
        assert!(limit > 0);
        for amount in [0, limit / 2, limit - 1] {
            let mut session = Session::new(b"").unwrap();
            let reply = session.call(
                cases::budget_request(kind, amount).as_bytes(),
                &CancelToken::new(),
            );
            assert_eq!(reply.status, Status::Failed);
            let value = zumen_json::parse(&reply.response).unwrap();
            assert!(
                array(&value, "/diagnostics")
                    .iter()
                    .any(|v| text(v, "/code")
                        == format!("budget.{}.exceeded", kind.replace('_', "-"))),
                "{value:?}"
            );
            assert!(array(&value, "/blobs").is_empty());
            assert!(session.take(0).is_none());
            call(&mut session, &cases::budget_request(kind, limit));
            assert!(session.take(0).is_some());
        }
    }
    let mut session = Session::new(b"").unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        session.call(CREATE.as_bytes(), &cancel).status,
        Status::Failed
    );
    assert!(session.take(0).is_none());
    call(&mut session, CREATE);
}
