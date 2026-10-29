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

/// 文書を開く。この境界で読めない版（R2000 でない DWG など）は `false`
#[cfg(test)]
fn open(session: &mut Session, input: &str) -> bool {
    let (status, _) = call(
        session,
        &format!(
            r#"{{"v":1,"op":"open","input":"{input}","budget":{{"input_bytes":"unlimited","decoded_bytes":"unlimited","records":"unlimited","work":"unlimited"}}}}"#
        ),
    );
    status == Status::Ok
}

/// 表の項目の応答
#[cfg(test)]
fn tables(session: &mut Session, doc: &str) -> Value {
    let (status, response) = call(
        session,
        &format!(
            r#"{{"v":1,"op":"tables","doc":"{doc}","budget":{{"work":"unlimited","elements":"unlimited"}}}}"#
        ),
    );
    assert_eq!(status, Status::Ok, "{doc}: {response:?}");
    response.pointer("/result").unwrap().clone()
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
        if !open(&mut session, &name) {
            // この境界で読めない版は、読む試験が扱う
            continue;
        }
        let before = entities(&mut session, &name);
        let tables_before = tables(&mut session, &name);
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
        assert!(open(&mut session, "again"), "{name}");
        let after = entities(&mut session, "again");
        // 表の項目の ID とハッシュも変わらない
        assert_eq!(tables(&mut session, "again"), tables_before, "{name}");
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

/// 試料の DXF の実寸の空間の線を、`tables` が返したレイヤの ID で別のレイヤに移すと、開き直した線が
/// そのレイヤにある（決定 0139・0156）。DWG はレイヤを書き戻せないので、`change.edit.unsupported` になる
#[test]
fn drawings_move_lines_to_layers_named_by_id() {
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
        if !open(&mut session, &name) {
            continue;
        }
        let Some((id, _, hash)) =
            entities(&mut session, &name)
                .into_iter()
                .find(|(id, kind, _)| {
                    kind == "line" && id.starts_with("space/0/h") && id.matches('/').count() == 2
                })
        else {
            continue;
        };
        let (_, listed) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"entities","doc":"{name}","fields":["layer"],"limit":10000,"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited"}}}}"#
            ),
        );
        let Some(Value::Array(items)) = listed.pointer("/result/items") else {
            panic!("{name}");
        };
        let current = items
            .iter()
            .find(|item| text(item, "/id") == Some(id.as_str()))
            .and_then(|item| text(item, "/layer"))
            .map(str::to_string);
        let table = tables(&mut session, &name);
        let Some(Value::Array(layers)) = table.pointer("/layers") else {
            panic!("{name}");
        };
        // ハンドルを持ち、今のレイヤと違うレイヤ
        let Some(layer) = layers.iter().find(|layer| {
            text(layer, "/id").is_some_and(|id| id.starts_with("layer/h"))
                && text(layer, "/name").map(str::to_string) != current
        }) else {
            continue;
        };
        let (layer_id, layer_name) = (text(layer, "/id").unwrap(), text(layer, "/name").unwrap());
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"restyle","id":"{id}","hash":"{hash}","layer_id":"{layer_id}"}}],"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}}}}"#
            ),
        );
        if name.to_ascii_lowercase().ends_with(".dwg") {
            assert_eq!(status, Status::Failed, "{name}");
            assert!(
                format!("{response:?}").contains("change.edit.unsupported"),
                "{name}: {response:?}"
            );
            continue;
        }
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let (_, listed) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"entities","doc":"{name}","fields":["layer","hash"],"limit":10000,"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited"}}}}"#
            ),
        );
        let Some(Value::Array(items)) = listed.pointer("/result/items") else {
            panic!("{name}");
        };
        let moved = items
            .iter()
            .find(|item| text(item, "/id") == Some(id.as_str()))
            .unwrap();
        assert_eq!(text(moved, "/layer"), Some(layer_name), "{name}");
        assert_eq!(
            text(moved, "/hash"),
            text(&response, "/result/applied/0/hash"),
            "{name}"
        );
        checked += 1;
    }
    assert!(drawings().is_empty() || checked > 0);
}

/// 要素の ID と形の値。要素が多い図面も、すべて読む
#[cfg(test)]
fn shapes(session: &mut Session, doc: &str) -> Vec<(String, Option<Value>)> {
    let request = format!(
        r#"{{"v":1,"op":"entities","doc":"{doc}","fields":["geometry"],"limit":10000,"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited"}}}}"#
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
            found.push((
                text(item, "/id").unwrap().to_string(),
                item.pointer("/geometry").cloned(),
            ));
        }
        offset += items.len();
        if response.pointer("/result/next_offset").is_none() {
            return found;
        }
    }
}

/// 試料の実寸の空間の線の形を、境界の `replace` で置き換え、`add` で線を足すと、書き戻したファイルを開き直した
/// 要素の形とハッシュが、送った形と `apply` が返したものと同じになり、ほかの要素は変わらない（決定 0139）
#[test]
fn drawings_take_back_shapes_replaced_and_added_through_the_boundary() {
    let mut checked = [0, 0];
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
        if !open(&mut session, &name) {
            continue;
        }
        let before = entities(&mut session, &name);
        let Some((id, _, hash)) = before.iter().find(|(id, kind, _)| {
            kind == "line" && id.starts_with("space/0/h") && id.matches('/').count() == 2
        }) else {
            continue;
        };
        let replaced = r#"{"shape":"line","start":[-3.5,2.25],"end":[40,17.75]}"#;
        // R12 の DXF は LWPOLYLINE を足せないので、どの版でも足せる円弧を足す
        let added =
            r#"{"shape":"arc","center":[5,6],"radius":7.5,"start_angle":30,"end_angle":210}"#;
        let request = format!(
            r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}},{{"kind":"add","place":"space/0","geometry":{added}}}],"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}}}}"#
        );
        let (status, response) = call(&mut session, &request);
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let file = session.take(0).unwrap();
        let applied: Vec<(String, String)> = (0..2)
            .map(|index| {
                let field = |key: &str| {
                    text(&response, &format!("/result/applied/{index}/{key}"))
                        .unwrap_or_else(|| panic!("{name}: {response:?}"))
                        .to_string()
                };
                (field("id"), field("hash"))
            })
            .collect();
        assert_eq!(&applied[0].0, id, "{name}");
        session.push(b"again", &file).unwrap();
        assert!(open(&mut session, "again"), "{name}");
        let after = entities(&mut session, "again");
        assert_eq!(after.len(), before.len() + 1, "{name}");
        for (was_id, _, was_hash) in &before {
            let (_, _, hash) = after
                .iter()
                .find(|(id, _, _)| id == was_id)
                .unwrap_or_else(|| panic!("{name}: {was_id}"));
            if was_id != id {
                assert_eq!(was_hash, hash, "{name}: {was_id}");
            }
        }
        for (id, hash) in &applied {
            assert!(
                after
                    .iter()
                    .any(|(was, _, was_hash)| was == id && was_hash == hash),
                "{name}: {id}"
            );
        }
        let geometry = shapes(&mut session, "again");
        for (id, expected) in [(&applied[0].0, replaced), (&applied[1].0, added)] {
            let found = geometry
                .iter()
                .find(|(was, _)| was == id)
                .and_then(|(_, geometry)| geometry.clone());
            let expected = zumen_json::parse(expected.as_bytes()).unwrap();
            // DWG は角度をラジアンで持つので、度の値は末の桁がずれうる
            assert!(
                found.as_ref().is_some_and(|found| close(found, &expected)),
                "{name}: {id}: {found:?}"
            );
        }
        checked[usize::from(name.to_ascii_lowercase().ends_with(".dwg"))] += 1;
    }
    assert!(
        drawings().is_empty() || checked.iter().all(|n| *n > 0),
        "{checked:?}"
    );
}

/// 数の差が小さいほかは同じ値
#[cfg(test)]
fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b): (f64, f64) = (a.as_str().parse().unwrap(), b.as_str().parse().unwrap());
            (a - b).abs() <= 1e-9 * a.abs().max(1.0)
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| close(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, a), (kb, b))| ka == kb && close(a, b))
        }
        _ => a == b,
    }
}
