//! JWW の文字の高さは用紙の mm で、entities は `height_on_paper` を付けて返す。返した形をそのまま置き換えに
//! 使える（#884）。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作入力と期待する応答だけを扱う。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

fn put(buffer: &mut [u8], at: usize, bytes: &[u8]) {
    buffer
        .get_mut(at..at + bytes.len())
        .unwrap()
        .copy_from_slice(bytes);
}

/// ヘッダーと線分 1 本の JWW 3.00。レイヤグループ 0 の縮尺を `scale` とする
fn jww(scale: f64) -> Vec<u8> {
    let mut header = vec![0u8; 4337];
    put(&mut header, 0, b"JwwData.");
    put(&mut header, 8, &300u32.to_le_bytes());
    put(&mut header, 21, &3u32.to_le_bytes());
    put(&mut header, 29, &scale.to_le_bytes());
    put(&mut header, 41, &3u32.to_le_bytes());
    let mut line = vec![0u8; 67];
    put(&mut line, 0, &1u16.to_le_bytes());
    put(&mut line, 2, &0xffffu16.to_le_bytes());
    put(&mut line, 4, &1u16.to_le_bytes());
    put(&mut line, 6, &8u16.to_le_bytes());
    put(&mut line, 8, b"CDataSen");
    put(&mut line, 20, &[1]);
    put(&mut line, 21, &1u16.to_le_bytes());
    put(&mut line, 45, &30f64.to_le_bytes());
    put(&mut line, 53, &20f64.to_le_bytes());
    header.extend(line);
    header
}

fn ok(session: &mut Session, request: &str) -> Value {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    let value = zumen_json::parse(&reply.response).unwrap();
    assert_eq!(reply.status, Status::Ok, "{value:?}");
    value
}

fn json(value: &Value) -> String {
    let mut writer = Writer::new();
    writer.value(value);
    String::from_utf8(writer.into_bytes()).unwrap()
}

fn open(bytes: &[u8]) -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"a.jww", bytes).unwrap();
    ok(&mut session, r#"{"v":1,"op":"open","input":"a.jww"}"#);
    session
}

const QUERY: &str =
    r#"{"v":1,"op":"entities","doc":"a.jww","fields":["id","kind","hash","geometry"]}"#;

fn items(value: &Value) -> Vec<Value> {
    let Some(Value::Array(items)) = value.pointer("/result/items") else {
        panic!("{value:?}")
    };
    items.clone()
}

fn number(value: &Value, pointer: &str) -> f64 {
    match value.pointer(pointer) {
        Some(Value::Number(number)) => number.to_f64().unwrap(),
        other => panic!("{pointer}: {other:?}"),
    }
}

#[test]
fn jww_text_heights_come_back_on_paper_and_can_be_replaced() {
    for scale in [1.0, 100.0] {
        let mut session = open(&jww(scale));
        // 点と同じ単位の高さ 300 と、用紙の 2.5 mm の文字を足す
        ok(
            &mut session,
            r#"{"v":1,"op":"apply","doc":"a.jww","edits":[
                {"kind":"add","place":"space/0","layer":"0-0","color":{"jww":1},"geometry":{"shape":"text","content":"Model","position":[12,8],"height":300,"rotation":0,"vertical":"bottom"}},
                {"kind":"add","place":"space/0","layer":"0-0","color":{"jww":1},"geometry":{"shape":"text","content":"Paper","position":[12,20],"height":2.5,"height_on_paper":true,"rotation":0,"vertical":"bottom"}}
            ]}"#,
        );
        let before = items(&ok(&mut session, QUERY));
        assert_eq!(before.len(), 3, "{scale}: {before:?}");
        let line = before[0].clone();
        for (item, height) in [(&before[1], 300.0 / scale), (&before[2], 2.5)] {
            assert_eq!(
                item.pointer("/geometry/height_on_paper"),
                Some(&Value::Bool(true)),
                "{scale}: {item:?}"
            );
            let got = number(item, "/geometry/height");
            assert!((got - height).abs() < 1e-9, "{scale}: {got} {height}");
        }
        // 返した形の内容だけを変えて置き換えると、高さはそのまま
        let text = &before[2];
        let geometry = json(text.pointer("/geometry").unwrap()).replace("\"Paper\"", "\"Moved\"");
        ok(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"a.jww","edits":[{{"kind":"replace","id":{},"hash":{},"geometry":{geometry}}}]}}"#,
                json(text.pointer("/id").unwrap()),
                json(text.pointer("/hash").unwrap()),
            ),
        );
        let written = session.take(0).unwrap();
        let after = items(&ok(&mut session, QUERY));
        assert_eq!(after[0], line, "{scale}");
        assert_eq!(after[1], before[1], "{scale}");
        assert_eq!(
            after[2].pointer("/geometry/content"),
            Some(&Value::String("Moved".into()))
        );
        assert_eq!(
            after[2].pointer("/geometry/height"),
            text.pointer("/geometry/height")
        );
        assert_eq!(items(&ok(&mut open(&written), QUERY)), after, "{scale}");
    }
}
