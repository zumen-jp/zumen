//! プラグインを、Rust で束ねる形と、「コアとプラグイン」の CLI を `process` の形で使う形で呼ぶ（#399）。wasm の
//! 別のモジュールの形は、`tests/wasm.mjs` で確かめる。

use zumen_abi1::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::Shape;
use zumen_plugin_template::{Lines, PLUGINS, codes};
use zumen_sdk::{Codec, Lift, testkit};

const SAMPLE: &[u8] = b"ZUMEN-LINES 1\n0 0 1000 0\n\n1000 0 1000 x\n1000 0 1000 500\n";

#[cfg(test)]
fn call(session: &mut Session, request: &str) -> (Status, String) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, String::from_utf8(reply.response).unwrap())
}

/// 開いて、図形の ID と、capabilities の形式を確かめる
#[cfg(test)]
fn check(session: &mut Session) {
    session.push(b"a.txt", SAMPLE).unwrap();
    let (status, open) = call(session, r#"{"v": 1, "op": "open", "input": "a.txt"}"#);
    assert_eq!(status, Status::Ok, "{open}");
    assert!(open.contains(r#""format":"zumen_lines""#), "{open}");
    assert!(open.contains("zumen_lines.line.invalid"), "{open}");
    let (status, entities) = call(
        session,
        r#"{"v": 1, "op": "entities", "doc": "a.txt", "fields": ["hash"]}"#,
    );
    assert_eq!(status, Status::Ok, "{entities}");
    assert!(entities.contains(r#""id":"space/0/r0""#), "{entities}");
    assert!(entities.contains(r#""id":"space/0/r1""#), "{entities}");
    let (_, capabilities) = call(session, r#"{"v": 1, "op": "capabilities"}"#);
    assert!(
        capabilities.contains(r#""id":"zumen_lines","family":"zumen_lines""#),
        "{capabilities}"
    );
    // プラグインの文書は書き戻せない
    let hash = entities
        .split(r#""hash":""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap();
    let request = format!(
        r#"{{"v": 1, "op": "apply", "doc": "a.txt", "edits": [{{"kind": "remove", "id": "space/0/r0", "hash": "{hash}"}}]}}"#
    );
    let (status, apply) = call(session, &request);
    assert_eq!(status, Status::Failed, "{apply}");
    assert!(apply.contains("call.document.unsupported"), "{apply}");
}

#[test]
fn the_codec_reads_and_lifts() {
    let (file, diagnostics) = testkit::read::<Lines>(SAMPLE);
    let file = file.unwrap();
    assert_eq!(file.lines.len(), 2);
    assert_eq!(
        diagnostics.iter().map(|d| d.code()).collect::<Vec<_>>(),
        [codes::LINE_INVALID]
    );
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let drawing = file
        .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    let Shape::Line { end, .. } = &drawing.spaces[0].elements[1].shape else {
        panic!("線分でない");
    };
    assert_eq!((end.x, end.y), (1000.0, 500.0));
    // 1 行目が違えば読まない
    let (file, diagnostics) = testkit::read::<Lines>(b"ZUMEN-LINES 2\n");
    assert!(file.is_err());
    assert_eq!(diagnostics[0].code(), codes::FILE_INVALID);
    assert!(!Lines::SIGNATURES[0].matches(b"ZUMEN-LINES 2\n"));
}

#[test]
fn the_codec_keeps_the_sdk_contracts() {
    testkit::stops_at_same_place::<Lines>(SAMPLE).unwrap();
    for end in 0..SAMPLE.len() {
        testkit::survives::<Lines>(&SAMPLE[..end]).unwrap();
    }
}

#[test]
fn rust_installs_the_plugin_before_opening_a_session() {
    zumen_abi1::install(PLUGINS);
    let mut session = Session::new(b"").unwrap();
    check(&mut session);
}

#[test]
fn the_core_and_plugin_cli_serves_the_process_form() {
    let path = env!("CARGO_BIN_EXE_zumen-lines").replace('\\', "\\\\");
    let options = format!(r#"{{"v": 1, "engine": "process", "process_path": "{path}"}}"#);
    let mut session = Session::new(options.as_bytes()).unwrap();
    check(&mut session);
}
