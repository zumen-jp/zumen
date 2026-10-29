//! `ZUMEN_SAMPLES` のフォルダーにある DXF と DWG を、境界の `apply` で書き戻す。
//! フォルダーを指定しないときは何も確かめない。図面は Git に入れない。

use std::path::PathBuf;

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::Value;

/// 試料の DXF と DWG
#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn drawings() -> Vec<PathBuf> {
    let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) else {
        return Vec::new();
    };
    let mut found: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("dxf") || e.eq_ignore_ascii_case("dwg"))
        })
        .collect();
    found.sort();
    found
}

#[cfg(test)]
fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

#[cfg(test)]
fn text<'a>(value: &'a Value, pointer: &str) -> Option<&'a str> {
    match value.pointer(pointer) {
        Some(Value::String(text)) => Some(text),
        _ => None,
    }
}

/// 要素の ID・種類・ハッシュ。要素が多い図面も、すべて読む
#[cfg(test)]
fn entities(session: &mut Session, doc: &str) -> Vec<(String, String, String)> {
    let request = format!(
        r#"{{"v":1,"op":"entities","doc":"{doc}","fields":["kind","hash"],"limit":10000,"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited"}}}}"#
    );
    let mut found = Vec::new();
    let mut offset = 0;
    loop {
        let paged = request.replacen("\"limit\"", &format!("\"offset\":{offset},\"limit\""), 1);
        let (status, response) = call(session, &paged);
        assert_eq!(status, Status::Ok, "{doc}: {response:?}");
        let Some(Value::Array(items)) = response.pointer("/result/items") else {
            panic!("{doc}: {response:?}");
        };
        for item in items {
            let field = |name: &str| {
                text(item, &format!("/{name}"))
                    .unwrap_or_default()
                    .to_string()
            };
            found.push((field("id"), field("kind"), field("hash")));
        }
        offset += items.len();
        if response.pointer("/result/next_offset").is_none() {
            return found;
        }
    }
}

/// 試料の実寸の空間の線を `apply` で動かすと、書き戻したファイルを開き直した要素の ID とハッシュが、
/// `apply` が返したものと同じになり、ほかの要素は変わらない（決定 0139・0156）
#[test]
fn drawings_take_back_lines_moved_through_the_boundary() {
    let mut checked = 0;
    for path in drawings() {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(' ', "_");
        let mut session = Session::new(b"").unwrap();
        session
            .push(name.as_bytes(), &std::fs::read(&path).unwrap())
            .unwrap();
        let (status, _) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"open","input":"{name}","budget":{{"input_bytes":"unlimited","decoded_bytes":"unlimited","records":"unlimited","work":"unlimited"}}}}"#
            ),
        );
        if status != Status::Ok {
            // この境界で読めない版（R2000 でない DWG など）は、読む試験が扱う
            continue;
        }
        let before = entities(&mut session, &name);
        // 1 つのレコードが 1 つの要素を作る、ハンドルを持つ線
        let Some((id, _, hash)) = before.iter().find(|(id, kind, _)| {
            kind == "line" && id.starts_with("space/0/h") && id.matches('/').count() == 2
        }) else {
            continue;
        };
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"transform","id":"{id}","hash":"{hash}","matrix":[1,0,12.5,0,1,-7.25]}}],"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}}}}"#
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let file = session.take(0).unwrap();
        let applied = (
            text(&response, "/result/applied/0/id").unwrap().to_string(),
            text(&response, "/result/applied/0/hash")
                .unwrap()
                .to_string(),
        );
        assert_eq!(&applied.0, id, "{name}");
        assert_ne!(&applied.1, hash, "{name}");
        // 書き戻したファイルを開き直す
        session.push(b"again", &file).unwrap();
        let (status, response) = call(
            &mut session,
            r#"{"v":1,"op":"open","input":"again","budget":{"input_bytes":"unlimited","decoded_bytes":"unlimited","records":"unlimited","work":"unlimited"}}"#,
        );
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let after = entities(&mut session, "again");
        assert_eq!(after.len(), before.len(), "{name}");
        for ((was_id, _, was_hash), (id, _, hash)) in before.iter().zip(&after) {
            assert_eq!(was_id, id, "{name}");
            if *id == applied.0 {
                assert_eq!(hash, &applied.1, "{name}: {id}");
            } else {
                assert_eq!(was_hash, hash, "{name}: {id}");
            }
        }
        checked += 1;
    }
    assert!(drawings().is_empty() || checked > 0);
}
