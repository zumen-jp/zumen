//! 図面モデルのバイナリ（決定 0247）を lift で書き、図面のモジュールの open で読む。読んだ文書は、元の文書と
//! 同じ答えを返す。

#![cfg(feature = "drawing")]

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_drawing::binary;
use zumen_json::Value;

/// 線・円・円弧・文字・点と、用紙の線を持つ DXF（適合試験の scene.json と同じ）
const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLINE\n5\nA1\n8\n0\n62\n1\n10\n1.5\n20\n2.0\n11\n100.0\n21\n50.0\n\
0\nCIRCLE\n5\nA2\n8\n0\n10\n100000.0\n20\n50000.0\n40\n3.0\n\
0\nARC\n5\nA3\n8\n0\n10\n0.0\n20\n0.0\n40\n10.0\n50\n30.0\n51\n120.0\n\
0\nTEXT\n5\nA4\n8\nTEXT\n10\n-3.0\n20\n4.0\n40\n2.5\n1\nABC\n\
0\nPOINT\n5\nA5\n8\n0\n10\n5.0\n20\n6.0\n\
0\nLINE\n5\nA6\n8\n0\n67\n1\n10\n10.0\n20\n10.0\n11\n410.0\n21\n287.0\n0\nENDSEC\n0\nEOF\n";

/// 図面を読む op の要求。文書の名前は `a`
const READS: &[&str] = &[
    r#"{"v":1,"op":"drawing","doc":"a"}"#,
    r#"{"v":1,"op":"entities","doc":"a","fields":["id","kind","layer","source","bbox","hash","geometry"]}"#,
    r#"{"v":1,"op":"tables","doc":"a"}"#,
    r#"{"v":1,"op":"scene","doc":"a","from":"space/0"}"#,
    r#"{"v":1,"op":"scene","doc":"a","from":"sheet/0"}"#,
    r#"{"v":1,"op":"convert","doc":"a","format":"dxf","to":"b"}"#,
];

fn call(session: &mut Session, request: &str) -> (Status, String) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (
        reply.status,
        String::from_utf8(reply.response).unwrap_or_default(),
    )
}

/// `bytes` を `a` で開いたセッション
fn opened(bytes: &[u8]) -> Option<(Session, Status, String)> {
    let mut session = Session::new(b"").ok()?;
    session.push(b"a", bytes).ok()?;
    let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a"}"#);
    Some((session, status, response))
}

/// 開いた文書を lift で書いたバイナリ
fn lift(session: &mut Session) -> Option<Vec<u8>> {
    let (status, response) = call(session, r#"{"v":1,"op":"lift","doc":"a"}"#);
    assert_eq!(status, Status::Ok, "{response}");
    assert!(response.contains(r#""kind":"drawing""#), "{response}");
    session.take(0)
}

/// 図面を読む op の応答と、blob
fn answers(session: &mut Session) -> Vec<(Status, String, Option<Vec<u8>>)> {
    READS
        .iter()
        .map(|request| {
            let (status, response) = call(session, request);
            (status, response, session.take(0))
        })
        .collect()
}

#[test]
fn the_lifted_drawing_answers_like_the_original() {
    let (mut original, status, opened_original) = opened(DXF.as_bytes()).unwrap();
    assert_eq!(status, Status::Ok, "{opened_original}");
    let bytes = lift(&mut original).unwrap();
    assert!(binary::matches(&bytes));

    let (mut lifted, status, opened_lifted) = opened(&bytes).unwrap();
    assert_eq!(status, Status::Ok, "{opened_lifted}");
    // 元の文書の形式とハッシュのまま
    assert_eq!(opened_lifted, opened_original);
    let expected = answers(&mut original);
    assert!(expected.iter().all(|(status, ..)| *status == Status::Ok));
    assert_eq!(answers(&mut lifted), expected);
    // 読んだものを書き直しても、同じバイト列になる
    assert_eq!(lift(&mut lifted), Some(bytes));
}

#[test]
fn the_lifted_drawing_cannot_be_written_back() {
    let (mut original, ..) = opened(DXF.as_bytes()).unwrap();
    let (mut lifted, ..) = opened(&lift(&mut original).unwrap()).unwrap();
    let (status, response) = call(&mut lifted, r#"{"v":1,"op":"apply","doc":"a","edits":[]}"#);
    assert_eq!(status, Status::Failed);
    assert!(
        response.contains(r#""code":"call.document.unsupported""#),
        "{response}"
    );
}

#[test]
fn broken_binaries_are_refused_with_the_reason() {
    let (mut original, ..) = opened(DXF.as_bytes()).unwrap();
    let bytes = lift(&mut original).unwrap();

    let (_, status, response) = opened(&bytes[..bytes.len() - 1]).unwrap();
    assert_eq!(status, Status::Failed);
    assert!(
        response.contains(r#""code":"drawing.binary.invalid""#),
        "{response}"
    );

    let mut later = bytes.clone();
    later[4..6].copy_from_slice(&(binary::MAJOR + 1).to_le_bytes());
    let (_, status, response) = opened(&later).unwrap();
    assert_eq!(status, Status::Failed);
    assert!(
        response.contains(r#""code":"drawing.binary-version.unsupported""#),
        "{response}"
    );

    let mut session = Session::new(b"").unwrap();
    session.push(b"a", &bytes).unwrap();
    let (status, response) = call(
        &mut session,
        r#"{"v":1,"op":"open","input":"a","budget":{"elements":1}}"#,
    );
    assert_eq!(status, Status::Failed);
    assert!(
        response.contains(r#""code":"budget.elements.exceeded""#),
        "{response}"
    );
}

/// 応答の値の文字列
fn text<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    match value.pointer(pointer) {
        Some(Value::String(text)) => Some(text),
        _ => None,
    }
}

/// バイナリは誰でも渡せるので、コーデックが作らない形の図面モデルでも、op は止まらずに答える
#[test]
fn any_drawing_model_in_a_binary_gets_an_answer() {
    const BUDGET: &str = r#""budget":{"elements":20000,"segments":20000,"work":20000}"#;
    for seed in 0..500 {
        let (source, drawing) = binary::random(seed, false);
        let (mut session, status, response) = opened(&binary::write(&source, &drawing)).unwrap();
        assert_eq!(status, Status::Ok, "seed {seed}: {response}");
        let (_, summary) = call(
            &mut session,
            &format!(r#"{{"v":1,"op":"drawing","doc":"a",{BUDGET}}}"#),
        );
        let summary = zumen_json::parse(summary.as_bytes()).unwrap();
        let mut starts = Vec::new();
        for kind in ["spaces", "sheets"] {
            for index in 0..4 {
                if let Some(id) = text(&summary, &format!("/result/{kind}/{index}/id")) {
                    starts.push(id.to_string());
                }
            }
        }
        for request in READS {
            let request = request.replace(r#""doc":"a""#, &format!(r#""doc":"a",{BUDGET}"#));
            call(&mut session, &request);
        }
        for start in starts {
            call(
                &mut session,
                &format!(r#"{{"v":1,"op":"scene","doc":"a","from":"{start}",{BUDGET}}}"#),
            );
        }
        assert_eq!(
            lift(&mut session),
            Some(binary::write(&source, &drawing)),
            "seed {seed}"
        );
    }
}
