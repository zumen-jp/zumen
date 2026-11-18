//! R2000 の ANSI_1252 の DXF の文字と名前を Windows-1252 の表で読み、書き戻して開き直す（#327）。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作入力と、期待する応答だけを扱う。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::Value;

/// 適合テスト code_page_1252.json と同じ入力。レイヤ Fläche の TEXT（ß・€・“”）と、割り当てのない 0x81 を持つ TEXT
const HEX: &str = "300a53454354494f4e0a320a4845414445520a390a24414341445645520a310a4143313031350a390a24445747434f4445504147450a330a414e53495f313235320a390a24494e53554e4954530a37300a340a390a2448414e44534545440a350a38300a300a454e445345430a300a53454354494f4e0a320a454e5449544945530a300a544558540a350a31410a380a466ce46368650a31300a302e300a32300a302e300a33300a302e300a34300a322e350a310a4d61df7374616220313a322080209378940a300a544558540a350a31420a380a300a31300a302e300a32300a352e300a33300a302e300a34300a322e350a310a4181420a300a454e445345430a300a454f460a";

fn bytes() -> Vec<u8> {
    (0..HEX.len())
        .step_by(2)
        .filter_map(|at| HEX.get(at..at + 2))
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

fn text(value: &Value, pointer: &str) -> String {
    match value.pointer(pointer) {
        Some(Value::String(text)) => text.clone(),
        other => panic!("{pointer}: {other:?}"),
    }
}

fn contents(session: &mut Session, doc: &str) -> Vec<(String, String, String)> {
    let (status, value) = call(
        session,
        &format!(r#"{{"v":1,"op":"entities","doc":"{doc}","fields":["id","layer","geometry"]}}"#),
    );
    assert_eq!(status, Status::Ok);
    let Some(Value::Array(items)) = value.pointer("/result/items") else {
        panic!("{value:?}")
    };
    items
        .iter()
        .map(|item| {
            (
                text(item, "/id"),
                text(item, "/layer"),
                text(item, "/geometry/content"),
            )
        })
        .collect()
}

#[test]
fn windows_1252_texts_and_names_survive_writing_back_and_reopening() {
    let mut session = Session::new(b"").unwrap();
    session.push(b"a", &bytes()).unwrap();
    assert_eq!(
        call(&mut session, r#"{"v":1,"op":"open","input":"a"}"#).0,
        Status::Ok
    );
    let read = contents(&mut session, "a");
    assert_eq!(
        read,
        [(
            "space/0/h1A".to_owned(),
            "Fläche".to_owned(),
            "Maßstab 1:2 € “x”".to_owned()
        )]
    );
    // Windows-1252 で表せない文字は、変更全体を拒む
    let add = |content: &str| {
        format!(
            r#"{{"v":1,"op":"apply","doc":"a","edits":[{{"kind":"add","place":"space/0","layer_id":"layer/0","geometry":{{"shape":"text","content":"{content}","position":[1,2],"height":2.5,"rotation":0}}}}]}}"#
        )
    };
    let (status, value) = call(&mut session, &add("日本"));
    assert_eq!(status, Status::Failed, "{value:?}");
    assert!(session.take(0).is_none());
    let (status, value) = call(&mut session, &add("Café – Ÿ"));
    assert_eq!(status, Status::Ok, "{value:?}");
    let file = session.take(0).unwrap();
    // 0xE9・0x96・0x9F の 1 バイトずつで書く
    assert!(file.windows(9).any(|w| w == b"Caf\xE9 \x96 \x9F\n"));
    session.push(b"b", &file).unwrap();
    assert_eq!(
        call(&mut session, r#"{"v":1,"op":"open","input":"b"}"#).0,
        Status::Ok
    );
    let mut expected = read;
    expected.push((
        "space/0/h80".to_owned(),
        "Fläche".to_owned(),
        "Café – Ÿ".to_owned(),
    ));
    assert_eq!(contents(&mut session, "b"), expected);
}
