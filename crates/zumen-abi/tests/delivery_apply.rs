//! 管理 XML を変更する公開操作と、停止後の再利用。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作の固定入力を検査する。

#[path = "../examples/delivery_apply_case.rs"]
mod cases;

use cases::{APPLY, NAME, UNCHANGED, call};
use zumen_abi1::Status;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_delivery::Record;
use zumen_json::Value;

fn text<'a>(value: &'a Value, path: &str) -> &'a str {
    let Some(Value::String(text)) = value.pointer(path) else {
        panic!("{path}: {value:?}")
    };
    text
}
fn child<'a>(record: &'a Record, name: &str, index: usize) -> &'a Record {
    record
        .children
        .iter()
        .filter(|r| r.name == name)
        .nth(index)
        .unwrap()
}

#[test]
fn changes_values_preserves_declarations_and_passes_delivery_check() {
    for encoding in cases::ENCODINGS {
        let mut session = cases::session(encoding);
        let result = call(&mut session, APPLY);
        assert_eq!(text(&result, "/blobs/0/kind"), "file");
        let written = session.take(0).unwrap();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let package = zumen_delivery::read(
            &[("INDEX_C.XML", &written)],
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        assert!(diagnostics.iter().next().is_none());
        let managed = &package.managed[0];
        assert_eq!(managed.doctype.as_deref(), Some("synthetic.dtd"));
        assert_eq!(
            child(&managed.root, "title", 0).value.as_deref(),
            Some(cases::CONTENT)
        );
        for (index, expected) in ["ONE.P21", "TWO.P21", "THREE.P21"].iter().enumerate() {
            assert_eq!(
                child(child(&managed.root, "entry", index), "file", 0)
                    .value
                    .as_deref(),
                Some(*expected)
            );
        }
        assert_eq!(
            child(child(&managed.root, "entry", 0), "note", 0)
                .value
                .as_deref(),
            Some("A")
        );
        assert_eq!(
            child(child(&managed.root, "entry", 1), "note", 0)
                .value
                .as_deref(),
            Some("B")
        );
        assert_eq!(
            child(&managed.root, "extra", 0).value.as_deref(),
            Some("新")
        );
        assert_eq!(
            child(child(&managed.root, "extra", 0), "child", 0)
                .value
                .as_deref(),
            Some("value")
        );
        call(&mut session, UNCHANGED);
        assert_eq!(session.take(0).unwrap(), cases::source(encoding));
    }
    // 同じ操作を適合入力へも生成し、プロファイルの規則との照合まで検査する。
    assert!(!cases::steps(None).is_empty());
}

#[test]
fn missing_targets_and_stale_hashes_leave_inputs_and_blobs_unchanged() {
    let mut session = cases::session("utf8");
    for (request, code) in cases::invalid_requests() {
        let reply = session.call(request.as_bytes(), &CancelToken::new());
        assert_eq!(reply.status, Status::Failed, "{request}");
        let result = zumen_json::parse(&reply.response).unwrap();
        assert_eq!(text(&result, "/diagnostics/0/code"), code, "{result:?}");
        assert_eq!(result.pointer("/blobs"), Some(&Value::Array(vec![])));
        assert!(result.pointer("/result").is_none());
        assert!(session.take(0).is_none());
        call(&mut session, UNCHANGED);
        assert_eq!(session.take(0).unwrap(), cases::source("utf8"));
    }
    let missing = UNCHANGED.replace(NAME, "missing");
    let reply = session.call(missing.as_bytes(), &CancelToken::new());
    assert_eq!(reply.status, Status::Failed);
    assert_eq!(
        text(
            &zumen_json::parse(&reply.response).unwrap(),
            "/diagnostics/0/code"
        ),
        "call.input.missing"
    );
}

#[test]
fn encoding_rejection_is_atomic_and_retry_uses_the_same_original() {
    let mut session = cases::session("sjis");
    for unsupported in ["😀", "¥", "‾"] {
        let request = APPLY.replace("filled", unsupported);
        let reply = session.call(request.as_bytes(), &CancelToken::new());
        assert_eq!(reply.status, Status::Failed);
        assert_eq!(
            text(
                &zumen_json::parse(&reply.response).unwrap(),
                "/diagnostics/0/code"
            ),
            "xml.edit-encoding.failed"
        );
        assert!(session.take(0).is_none());
    }
    call(&mut session, APPLY);
    let expected = session.take(0).unwrap();
    call(&mut session, APPLY);
    assert_eq!(session.take(0).unwrap(), expected);
}

#[test]
fn every_budget_boundary_and_cancellation_publish_nothing() {
    for kind in ["input_bytes", "decoded_bytes", "records", "depth", "work"] {
        let limit = cases::minimum_budget(kind);
        for amount in [0, limit / 2, limit - 1] {
            let mut session = cases::session("utf8");
            let request = cases::budget_request(kind, amount).replacen(
                "\"v\":1",
                "\"v\":1,\"partial\":true",
                1,
            );
            let reply = session.call(request.as_bytes(), &CancelToken::new());
            assert_eq!(reply.status, Status::Failed);
            let result = zumen_json::parse(&reply.response).unwrap();
            let Some(Value::Array(diagnostics)) = result.pointer("/diagnostics") else {
                panic!("{result:?}")
            };
            assert!(
                diagnostics
                    .iter()
                    .any(|d| text(d, "/code")
                        == format!("budget.{}.exceeded", kind.replace('_', "-"))),
                "{result:?}"
            );
            assert_eq!(result.pointer("/blobs"), Some(&Value::Array(vec![])));
            assert!(session.take(0).is_none());
            call(&mut session, &cases::budget_request(kind, limit));
            assert!(session.take(0).is_some());
            call(&mut session, UNCHANGED);
            assert_eq!(session.take(0).unwrap(), cases::source("utf8"));
        }
    }
    let mut session = cases::session("utf8");
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        session.call(APPLY.as_bytes(), &cancel).status,
        Status::Failed
    );
    assert!(session.take(0).is_none());
    call(&mut session, APPLY);
}
