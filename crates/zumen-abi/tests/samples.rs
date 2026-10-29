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

/// 試料の実寸の空間の楕円の形を、境界の `replace` で置き換えると、開き直した楕円の形とハッシュが、送った形と
/// `apply` が返したものと同じになる（決定 0139）
#[test]
fn drawings_take_back_ellipses_replaced_through_the_boundary() {
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
        let ellipse = shapes(&mut session, &name)
            .into_iter()
            .find(|(id, geometry)| {
                id.starts_with("space/0/h")
                    && id.matches('/').count() == 2
                    && geometry
                        .as_ref()
                        .is_some_and(|g| text(g, "/shape") == Some("ellipse"))
            });
        let Some((id, _)) = ellipse else {
            continue;
        };
        let (_, _, hash) = entities(&mut session, &name)
            .into_iter()
            .find(|(was, _, _)| *was == id)
            .unwrap();
        let replaced = r#"{"shape":"ellipse","center":[10,20],"major":[13,24],"ratio":0.4,"start_angle":45,"end_angle":300}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replaced}}}],"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let file = session.take(0).unwrap();
        session.push(b"again", &file).unwrap();
        assert!(open(&mut session, "again"), "{name}");
        let applied = text(&response, "/result/applied/0/hash").unwrap();
        assert!(
            entities(&mut session, "again")
                .iter()
                .any(|(was, _, was_hash)| *was == id && was_hash == applied),
            "{name}: {id}"
        );
        let found = shapes(&mut session, "again")
            .into_iter()
            .find(|(was, _)| *was == id)
            .and_then(|(_, geometry)| geometry);
        let expected = zumen_json::parse(replaced.as_bytes()).unwrap();
        assert!(
            found.as_ref().is_some_and(|found| close(found, &expected)),
            "{name}: {id}: {found:?}"
        );
        checked[usize::from(name.to_ascii_lowercase().ends_with(".dwg"))] += 1;
    }
    assert!(
        drawings().is_empty() || checked.iter().all(|n| *n > 0),
        "{checked:?}"
    );
}

/// 試料の実寸の空間に、境界の `add` で文字を足すと、開き直した文字の形とハッシュが、送った形と `apply` が
/// 返したものと同じになる（決定 0139）。2007 より前の版は ASCII の文字だけを書くので、日本語の文字は
/// `change.edit.unsupported` になり、ASCII の文字なら足せる
#[test]
fn drawings_take_back_texts_added_through_the_boundary() {
    let mut japanese = [0, 0];
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
        // レイヤ「0」を持たない図面（Jw_cad が書いた R12 の DXF など）もあるので、表の最初のレイヤを ID で指す
        let table = tables(&mut session, &name);
        let layer_id = text(&table, "/layers/0/id").unwrap().to_string();
        let add = |session: &mut Session, content: &str| {
            let geometry = format!(
                r#"{{"shape":"text","content":"{content}","position":[12.5,-4],"height":3.5,"rotation":15}}"#
            );
            let (status, response) = call(
                session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"add","place":"space/0","layer_id":"{layer_id}","geometry":{geometry}}}],"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}}}}"#
                ),
            );
            (status, response, geometry)
        };
        let (status, response, geometry) = match add(&mut session, "通り芯 X1") {
            (Status::Ok, response, geometry) => {
                japanese[usize::from(name.to_ascii_lowercase().ends_with(".dwg"))] += 1;
                (Status::Ok, response, geometry)
            }
            (_, response, _) => {
                assert!(
                    format!("{response:?}").contains("change.edit.unsupported"),
                    "{name}: {response:?}"
                );
                add(&mut session, "GRID X1")
            }
        };
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let file = session.take(0).unwrap();
        let id = text(&response, "/result/applied/0/id").unwrap().to_string();
        let hash = text(&response, "/result/applied/0/hash")
            .unwrap()
            .to_string();
        session.push(b"again", &file).unwrap();
        assert!(open(&mut session, "again"), "{name}");
        assert!(
            entities(&mut session, "again")
                .iter()
                .any(|(was, _, was_hash)| *was == id && *was_hash == hash),
            "{name}: {id}"
        );
        let found = shapes(&mut session, "again")
            .into_iter()
            .find(|(was, _)| *was == id)
            .and_then(|(_, geometry)| geometry)
            .unwrap_or_else(|| panic!("{name}: {id}"));
        // 足した文字の文字スタイルは、図面の Standard の名前で読み直す
        let style = text(&found, "/style").unwrap_or_else(|| panic!("{name}: {found:?}"));
        assert!(style.eq_ignore_ascii_case("Standard"), "{name}: {style}");
        let style_id = text(&found, "/style_id").unwrap_or_else(|| panic!("{name}: {found:?}"));
        let expected = format!(
            r#"{},"style":"{style}","style_id":"{style_id}"}}"#,
            geometry.trim_end_matches('}')
        );
        let expected = zumen_json::parse(expected.as_bytes()).unwrap();
        assert!(close(&found, &expected), "{name}: {found:?}");
    }
    assert!(
        drawings().is_empty() || japanese.iter().all(|n| *n > 0),
        "{japanese:?}"
    );
}

/// 試料の実寸の空間の 2 点の間に合わせる文字を、境界の `replace` で 2 点だけ動かし、`add` で 2 点の間に合わせる
/// 文字を足すと、開き直した形とハッシュが、送った形と `apply` が返したものと同じになる（決定 0139）。DWG の試料は
/// 2 点の間に合わせる文字を持たないので、足した文字を置き換えて確かめる
#[test]
fn drawings_take_back_texts_fitted_between_two_points() {
    let budget = r#""budget":{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}"#;
    let (mut replaced, mut added, mut moved, mut skewed) = ([0, 0], [0, 0], [0, 0], 0);
    for path in drawings() {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(' ', "_");
        let dwg = usize::from(name.to_ascii_lowercase().ends_with(".dwg"));
        let mut session = Session::new(b"").unwrap();
        session
            .push(name.as_bytes(), &std::fs::read(&path).unwrap())
            .unwrap();
        if !open(&mut session, &name) {
            continue;
        }
        let check = |session: &mut Session, doc: &str, response: &Value, expected: &Value| {
            let file = session.take(0).unwrap();
            let id = text(response, "/result/applied/0/id").unwrap().to_string();
            let hash = text(response, "/result/applied/0/hash")
                .unwrap()
                .to_string();
            session.push(doc.as_bytes(), &file).unwrap();
            assert!(open(session, doc), "{name}");
            assert!(
                entities(session, doc)
                    .iter()
                    .any(|(found, _, found_hash)| *found == id && *found_hash == hash),
                "{name}: {id}"
            );
            let found = shapes(session, doc)
                .into_iter()
                .find(|(found, _)| *found == id)
                .and_then(|(_, geometry)| geometry);
            assert!(
                found.as_ref().is_some_and(|found| close(found, expected)),
                "{name}: {id}: {found:?}"
            );
        };
        let fitted = shapes(&mut session, &name)
            .into_iter()
            .find(|(id, geometry)| {
                id.starts_with("space/0/h")
                    && id.matches('/').count() == 2
                    && geometry.as_ref().is_some_and(|g| {
                        text(g, "/shape") == Some("text") && g.pointer("/end").is_some()
                    })
            });
        if let Some((id, Some(geometry))) = fitted {
            let (_, _, hash) = entities(&mut session, &name)
                .into_iter()
                .find(|(found, _, _)| *found == id)
                .unwrap();
            let number =
                |pointer: &str| zumen_json::number(geometry.pointer(pointer).unwrap()).unwrap();
            let style = text(&geometry, "/style").unwrap();
            let style_id = text(&geometry, "/style_id").unwrap();
            let moved = format!(
                r#"{{"shape":"text","content":"GRID X1","position":[{},{}],"end":[{},{}],"fit":"{}","height":{},"rotation":{},"style":"{style}","style_id":"{style_id}"}}"#,
                number("/position/0") + 5.0,
                number("/position/1") - 2.5,
                number("/end/0") + 5.0,
                number("/end/1") - 2.5,
                text(&geometry, "/fit").unwrap(),
                number("/height"),
                number("/rotation"),
            );
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{moved}}}],{budget}}}"#
                ),
            );
            if status == Status::Ok {
                check(
                    &mut session,
                    "replaced",
                    &response,
                    &zumen_json::parse(moved.as_bytes()).unwrap(),
                );
                replaced[dwg] += 1;
            } else {
                // 回転が 2 点の向きと合わない文字は、置き換えない
                assert!(
                    format!("{response:?}").contains("/edits/0/geometry/rotation"),
                    "{name}: {response:?}"
                );
                skewed += 1;
            }
        }
        let table = tables(&mut session, &name);
        let layer_id = text(&table, "/layers/0/id").unwrap().to_string();
        let adding = r#"{"shape":"text","content":"GRID X1","position":[10,0],"end":[10,8],"fit":"aligned","height":2,"rotation":90}"#;
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"add","place":"space/0","layer_id":"{layer_id}","geometry":{adding}}}],{budget}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let file = session.take(0).unwrap();
        session.push(b"added", &file).unwrap();
        assert!(open(&mut session, "added"), "{name}");
        let id = text(&response, "/result/applied/0/id").unwrap().to_string();
        let hash = text(&response, "/result/applied/0/hash")
            .unwrap()
            .to_string();
        assert!(
            entities(&mut session, "added")
                .iter()
                .any(|(found, _, found_hash)| *found == id && *found_hash == hash),
            "{name}: {id}"
        );
        let found = shapes(&mut session, "added")
            .into_iter()
            .find(|(found, _)| *found == id)
            .and_then(|(_, geometry)| geometry)
            .unwrap_or_else(|| panic!("{name}: {id}"));
        // 足した文字の文字スタイルは、図面の Standard の名前で読み直す
        let style = text(&found, "/style").unwrap_or_else(|| panic!("{name}: {found:?}"));
        let style_id = text(&found, "/style_id").unwrap_or_else(|| panic!("{name}: {found:?}"));
        let expected = format!(
            r#"{},"style":"{style}","style_id":"{style_id}"}}"#,
            adding.trim_end_matches('}')
        );
        let expected = zumen_json::parse(expected.as_bytes()).unwrap();
        assert!(close(&found, &expected), "{name}: {found:?}");
        added[dwg] += 1;
        // 足した文字も、2 点を動かして置き換える。DWG の試料は 2 点の間に合わせる文字を持たないため
        let moving = format!(
            r#"{{"shape":"text","content":"GRID X2","position":[-6,1],"end":[-12,1],"fit":"aligned","height":2,"rotation":180,"style":"{style}","style_id":"{style_id}"}}"#
        );
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"added","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{moving}}}],{budget}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        check(
            &mut session,
            "moved",
            &response,
            &zumen_json::parse(moving.as_bytes()).unwrap(),
        );
        moved[dwg] += 1;
    }
    assert!(
        drawings().is_empty()
            || (replaced[0] > 0 && added.iter().all(|n| *n > 0) && moved.iter().all(|n| *n > 0)),
        "{replaced:?} {added:?} {moved:?} {skewed}"
    );
}

/// 試料の実寸の空間の書式つきの文字を、境界の `replace` で内容と位置だけ替え、`add` で書式つきの文字を足すと、
/// 開き直した形とハッシュが、送った形と `apply` が返したものと同じになる（決定 0139）。2007 より前の版は
/// ASCII の文字だけを書くので、日本語の内容は `change.edit.unsupported` になり、ASCII の内容なら足せる
#[test]
fn drawings_take_back_formatted_texts_through_the_boundary() {
    let number = |value: &Value, pointer: &str| zumen_json::number(value.pointer(pointer)?).ok();
    let budget = r#""budget":{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}"#;
    let (mut replaced, mut japanese) = ([0, 0], [0, 0]);
    for path in drawings() {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(' ', "_");
        let dwg = usize::from(name.to_ascii_lowercase().ends_with(".dwg"));
        let mut session = Session::new(b"").unwrap();
        session
            .push(name.as_bytes(), &std::fs::read(&path).unwrap())
            .unwrap();
        if !open(&mut session, &name) {
            continue;
        }
        // 置き換え。位置合わせ・枠の幅・回転・文字スタイルは読んだ値のまま送る
        let formatted = shapes(&mut session, &name)
            .into_iter()
            .find(|(id, geometry)| {
                id.starts_with("space/0/h")
                    && id.matches('/').count() == 2
                    && geometry
                        .as_ref()
                        .is_some_and(|g| text(g, "/shape") == Some("formatted_text"))
            });
        if let Some((id, Some(was))) = formatted {
            let (_, _, hash) = entities(&mut session, &name)
                .into_iter()
                .find(|(found, _, _)| *found == id)
                .unwrap();
            let width =
                number(&was, "/width").map_or(String::new(), |w| format!(r#","width":{w}"#));
            let geometry = format!(
                r#"{{"shape":"formatted_text","content":"GRID\\PX2","position":[7.5,-2],"height":{},"rotation":{},"attachment":"{}"{width},"style":"{}","style_id":"{}"}}"#,
                number(&was, "/height").unwrap(),
                number(&was, "/rotation").unwrap(),
                text(&was, "/attachment").unwrap(),
                text(&was, "/style").unwrap(),
                text(&was, "/style_id").unwrap(),
            );
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}],{budget}}}"#
                ),
            );
            assert_eq!(status, Status::Ok, "{name}: {response:?}");
            let file = session.take(0).unwrap();
            let applied = text(&response, "/result/applied/0/hash")
                .unwrap()
                .to_string();
            session.push(b"again", &file).unwrap();
            assert!(open(&mut session, "again"), "{name}");
            assert!(
                entities(&mut session, "again")
                    .iter()
                    .any(|(found, _, found_hash)| *found == id && *found_hash == applied),
                "{name}: {id}"
            );
            let found = shapes(&mut session, "again")
                .into_iter()
                .find(|(found, _)| *found == id)
                .and_then(|(_, geometry)| geometry);
            let expected = zumen_json::parse(geometry.as_bytes()).unwrap();
            assert!(
                found.as_ref().is_some_and(|found| close(found, &expected)),
                "{name}: {id}: {found:?}"
            );
            replaced[dwg] += 1;
        }
        // 追加。足す先のレイヤは、表の最初のレイヤを ID で指す
        let table = tables(&mut session, &name);
        let layer_id = text(&table, "/layers/0/id").unwrap().to_string();
        let add = |session: &mut Session, content: &str| {
            let geometry = format!(
                r#"{{"shape":"formatted_text","content":"{content}","position":[12.5,-4],"height":3.5,"rotation":15,"attachment":"middle_center","width":40}}"#
            );
            let (status, response) = call(
                session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"add","place":"space/0","layer_id":"{layer_id}","geometry":{geometry}}}],{budget}}}"#
                ),
            );
            (status, response, geometry)
        };
        let (status, response, geometry) = match add(&mut session, r"通り芯\\PX1") {
            (Status::Ok, response, geometry) => {
                japanese[dwg] += 1;
                (Status::Ok, response, geometry)
            }
            (_, response, _) => {
                assert!(
                    format!("{response:?}").contains("change.edit.unsupported"),
                    "{name}: {response:?}"
                );
                add(&mut session, r"GRID\\PX1")
            }
        };
        // R12 の DXF は MTEXT を持たない
        if status != Status::Ok {
            assert!(
                format!("{response:?}").contains("change.edit.unsupported"),
                "{name}: {response:?}"
            );
            continue;
        }
        let file = session.take(0).unwrap();
        let id = text(&response, "/result/applied/0/id").unwrap().to_string();
        let hash = text(&response, "/result/applied/0/hash")
            .unwrap()
            .to_string();
        session.push(b"added", &file).unwrap();
        assert!(open(&mut session, "added"), "{name}");
        assert!(
            entities(&mut session, "added")
                .iter()
                .any(|(found, _, found_hash)| *found == id && *found_hash == hash),
            "{name}: {id}"
        );
        let found = shapes(&mut session, "added")
            .into_iter()
            .find(|(found, _)| *found == id)
            .and_then(|(_, geometry)| geometry)
            .unwrap_or_else(|| panic!("{name}: {id}"));
        let style = text(&found, "/style").unwrap_or_else(|| panic!("{name}: {found:?}"));
        assert!(style.eq_ignore_ascii_case("Standard"), "{name}: {style}");
        let style_id = text(&found, "/style_id").unwrap_or_else(|| panic!("{name}: {found:?}"));
        let expected = format!(
            r#"{},"style":"{style}","style_id":"{style_id}"}}"#,
            geometry.trim_end_matches('}')
        );
        let expected = zumen_json::parse(expected.as_bytes()).unwrap();
        assert!(close(&found, &expected), "{name}: {found:?}");
    }
    assert!(
        drawings().is_empty()
            || (japanese.iter().all(|n| *n > 0) && replaced.iter().all(|n| *n > 0)),
        "{replaced:?} {japanese:?}"
    );
}

/// 試料の実寸の空間の記号を、境界の `replace` で置く点・尺度・回転だけ替え、同じ定義の記号を `add` で足すと、
/// 開き直した形とハッシュが、送った形と `apply` が返したものと同じになる（決定 0139）。属性を持つ記号の
/// 置く点などは、属性が付いてこないので、`call.value.invalid` で `/edits/0/geometry` を指して変えない。属性の
/// 定義を持つ定義の記号を足すと、定義から属性が作られ、渡したタグの値はその値になる
#[test]
fn drawings_take_back_symbols_through_the_boundary() {
    let budget = r#""budget":{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}"#;
    let (mut replaced, mut added, mut attributed, mut valued) = ([0, 0], [0, 0], 0, [0, 0]);
    for path in drawings() {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(' ', "_");
        let dwg = usize::from(name.to_ascii_lowercase().ends_with(".dwg"));
        let mut session = Session::new(b"").unwrap();
        session
            .push(name.as_bytes(), &std::fs::read(&path).unwrap())
            .unwrap();
        if !open(&mut session, &name) {
            continue;
        }
        let symbols: Vec<(String, String)> = shapes(&mut session, &name)
            .into_iter()
            .filter(|(id, geometry)| {
                id.starts_with("space/0/h")
                    && id.matches('/').count() == 2
                    && geometry.as_ref().is_some_and(|g| {
                        text(g, "/shape") == Some("symbol") && g.pointer("/grid").is_none()
                    })
            })
            .filter_map(|(id, geometry)| Some((id, text(&geometry?, "/name")?.to_string())))
            .collect();
        let hashes = entities(&mut session, &name);
        let check =
            |session: &mut Session, doc: &str, response: &Value, id: &str, geometry: &str| {
                let file = session.take(0).unwrap();
                let hash = text(response, "/result/applied/0/hash")
                    .unwrap()
                    .to_string();
                session.push(doc.as_bytes(), &file).unwrap();
                assert!(open(session, doc), "{name}");
                assert!(
                    entities(session, doc)
                        .iter()
                        .any(|(found, _, found_hash)| *found == id && *found_hash == hash),
                    "{name}: {id}"
                );
                let found = shapes(session, doc)
                    .into_iter()
                    .find(|(found, _)| *found == id)
                    .and_then(|(_, geometry)| geometry);
                let expected = zumen_json::parse(geometry.as_bytes()).unwrap();
                // 送らなかった属性は、定義から作られたものとして比べない
                let same = match (&found, &expected) {
                    (Some(Value::Object(found)), Value::Object(expected)) => {
                        let kept: Vec<_> = found
                            .iter()
                            .filter(|(key, _)| *key != "attributes" || expected.get(key).is_some())
                            .collect();
                        kept.len() == expected.len()
                            && kept
                                .iter()
                                .zip(expected.iter())
                                .all(|((ka, a), (kb, b))| *ka == kb && close(a, b))
                    }
                    _ => false,
                };
                assert!(same, "{name}: {id}: {found:?}");
                found.unwrap()
            };
        for (id, definition) in &symbols {
            let hash = &hashes.iter().find(|(found, _, _)| found == id).unwrap().2;
            let geometry = format!(
                r#"{{"shape":"symbol","name":"{definition}","position":[12.5,-4],"scale":[1.5,-2],"rotation":15}}"#
            );
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{geometry}}}],{budget}}}"#
                ),
            );
            if status != Status::Ok {
                let failed = (0..)
                    .map_while(|at| response.pointer(&format!("/diagnostics/{at}")))
                    .find(|d| text(d, "/severity") == Some("error"))
                    .unwrap();
                assert_eq!(
                    (text(failed, "/code"), text(failed, "/params/pointer")),
                    (Some("call.value.invalid"), Some("/edits/0/geometry")),
                    "{name}: {failed:?}"
                );
                attributed += 1;
                continue;
            }
            check(&mut session, "replaced", &response, id, &geometry);
            replaced[dwg] += 1;
            break;
        }
        let Some((_, definition)) = symbols.first() else {
            continue;
        };
        let geometry = format!(
            r#"{{"shape":"symbol","name":"{definition}","position":[-3,7.25],"scale":[0.5,0.5],"rotation":-30}}"#
        );
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"add","place":"space/0","geometry":{geometry}}}],{budget}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let id = text(&response, "/result/applied/0/id").unwrap().to_string();
        let found = check(&mut session, "added", &response, &id, &geometry);
        added[dwg] += 1;
        // 属性の定義を持つ定義なら、1 つ目のタグの値を替えて足す
        let Some(tag) = text(&found, "/attributes/0/tag") else {
            continue;
        };
        let valued_geometry = geometry.replacen(
            "\"rotation\"",
            &format!(r#""attributes":[{{"tag":"{tag}","value":"T1"}}],"rotation""#),
            1,
        );
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"add","place":"space/0","geometry":{valued_geometry}}}],{budget}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let id = text(&response, "/result/applied/0/id").unwrap().to_string();
        let found = check(&mut session, "valued", &response, &id, &geometry);
        let Some(Value::Array(attributes)) = found.pointer("/attributes") else {
            panic!("{name}: {found:?}");
        };
        assert!(
            attributes
                .iter()
                .any(|attribute| text(attribute, "/tag") == Some(tag)
                    && text(attribute, "/value") == Some("T1")),
            "{name}: {found:?}"
        );
        valued[dwg] += 1;
    }
    assert!(
        drawings().is_empty()
            || (replaced.iter().all(|n| *n > 0)
                && added.iter().all(|n| *n > 0)
                && valued.iter().all(|n| *n > 0)
                && attributed > 0),
        "{replaced:?} {added:?} {valued:?} {attributed}"
    );
}

/// 試料の実寸の空間の、当てはめる点で決まるスプラインを、境界の `replace` で当てはめる点を動かして置き換え、
/// `add` で当てはめる点のスプラインを足すと、開き直した形とハッシュが、送った形と `apply` が返したものと同じに
/// なる（決定 0139）。R12 の DXF には足せず `change.edit.unsupported` に、R2013 から後の DWG には足せず
/// `dwg.change.unsupported` になる
#[test]
fn drawings_take_back_splines_fitted_through_points() {
    let budget = r#""budget":{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}"#;
    let adding = r#"{"shape":"spline","degree":3,"knots":[],"control":[],"fit":[[0,0],[2,3],[5,1],[8,4]],"start_tangent":[1,1],"end_tangent":[1,-0.5],"closed":false,"periodic":false}"#;
    let moving = r#"{"shape":"spline","degree":3,"knots":[],"control":[],"fit":[[1,-1],[3,2],[7,0]],"closed":false,"periodic":false}"#;
    let (mut replaced, mut added, mut moved, mut refused) = ([0, 0], [0, 0], [0, 0], [0, 0]);
    for path in drawings() {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(' ', "_");
        let dwg = usize::from(name.to_ascii_lowercase().ends_with(".dwg"));
        let mut session = Session::new(b"").unwrap();
        session
            .push(name.as_bytes(), &std::fs::read(&path).unwrap())
            .unwrap();
        if !open(&mut session, &name) {
            continue;
        }
        let check = |session: &mut Session, doc: &str, response: &Value, geometry: &str| {
            let file = session.take(0).unwrap();
            let id = text(response, "/result/applied/0/id").unwrap().to_string();
            let hash = text(response, "/result/applied/0/hash")
                .unwrap()
                .to_string();
            session.push(doc.as_bytes(), &file).unwrap();
            assert!(open(session, doc), "{name}");
            assert!(
                entities(session, doc)
                    .iter()
                    .any(|(found, _, found_hash)| *found == id && *found_hash == hash),
                "{name}: {id}"
            );
            let found = shapes(session, doc)
                .into_iter()
                .find(|(found, _)| *found == id)
                .and_then(|(_, geometry)| geometry);
            let expected = zumen_json::parse(geometry.as_bytes()).unwrap();
            assert!(
                found.as_ref().is_some_and(|found| close(found, &expected)),
                "{name}: {id}: {found:?}"
            );
        };
        let fitted = shapes(&mut session, &name)
            .into_iter()
            .find(|(id, geometry)| {
                id.starts_with("space/0/h")
                    && id.matches('/').count() == 2
                    && geometry.as_ref().is_some_and(|g| {
                        text(g, "/shape") == Some("spline") && g.pointer("/fit").is_some()
                    })
            });
        if let Some((id, Some(geometry))) = fitted {
            let (_, _, hash) = entities(&mut session, &name)
                .into_iter()
                .find(|(found, _, _)| *found == id)
                .unwrap();
            // 当てはめる点を動かし、ほかの値は元のまま送る
            let Some(Value::Array(points)) = geometry.pointer("/fit") else {
                panic!("{name}: {geometry:?}");
            };
            let fit: Vec<String> = points
                .iter()
                .map(|point| {
                    let at = |index: usize| {
                        zumen_json::number(point.pointer(&format!("/{index}")).unwrap()).unwrap()
                    };
                    format!("[{},{}]", at(0) + 3.0, at(1) - 2.0)
                })
                .collect();
            let tangent = |key: &str| {
                geometry
                    .pointer(&format!("/{key}"))
                    .map_or(String::new(), |_| {
                        let at = |index: usize| {
                            zumen_json::number(
                                geometry.pointer(&format!("/{key}/{index}")).unwrap(),
                            )
                            .unwrap()
                        };
                        format!(r#","{key}":[{},{}]"#, at(0), at(1))
                    })
            };
            let flag = |key: &str| geometry.pointer(&format!("/{key}")) == Some(&Value::Bool(true));
            let moved = format!(
                r#"{{"shape":"spline","degree":{},"knots":[],"control":[],"fit":[{}]{}{},"closed":{},"periodic":{}}}"#,
                zumen_json::number(geometry.pointer("/degree").unwrap()).unwrap(),
                fit.join(","),
                tangent("start_tangent"),
                tangent("end_tangent"),
                flag("closed"),
                flag("periodic"),
            );
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{moved}}}],{budget}}}"#
                ),
            );
            assert_eq!(status, Status::Ok, "{name}: {response:?}");
            check(&mut session, "replaced", &response, &moved);
            replaced[dwg] += 1;
        }
        let table = tables(&mut session, &name);
        let layer_id = text(&table, "/layers/0/id").unwrap().to_string();
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"add","place":"space/0","layer_id":"{layer_id}","geometry":{adding}}}],{budget}}}"#
            ),
        );
        if status != Status::Ok {
            let code = if dwg == 1 {
                "dwg.change.unsupported"
            } else {
                "change.edit.unsupported"
            };
            assert!(
                format!("{response:?}").contains(code),
                "{name}: {response:?}"
            );
            refused[dwg] += 1;
            continue;
        }
        check(&mut session, "added", &response, adding);
        added[dwg] += 1;
        // 足したスプラインも、当てはめる点を動かして置き換える。R2013 より前の DWG の試料は、当てはめる点の
        // スプラインを持たないため
        let id = text(&response, "/result/applied/0/id").unwrap().to_string();
        let hash = text(&response, "/result/applied/0/hash")
            .unwrap()
            .to_string();
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"added","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{moving}}}],{budget}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        check(&mut session, "moved", &response, moving);
        moved[dwg] += 1;
    }
    assert!(
        drawings().is_empty()
            || (replaced.iter().all(|n| *n > 0)
                && added.iter().all(|n| *n > 0)
                && moved.iter().all(|n| *n > 0)),
        "{replaced:?} {added:?} {moved:?} {refused:?}"
    );
}

/// 試料の実寸の空間の、制御点で決まるスプラインを、境界の `replace` で有理のスプラインに置き換え、`add` で
/// スプラインを足すと、開き直した形とハッシュが、送った形と `apply` が返したものと同じになる（決定 0139）。
/// R12 の DXF は SPLINE を持たないので、足すと `change.edit.unsupported` になる
#[test]
fn drawings_take_back_splines_through_the_boundary() {
    let budget = r#""budget":{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}"#;
    let replacing = r#"{"shape":"spline","degree":2,"knots":[0,0,0,1,1,1],"control":[[10,0],[10,10],[0,10]],"weights":[1,0.7071067811865476,1],"closed":false,"periodic":false}"#;
    let adding = r#"{"shape":"spline","degree":3,"knots":[0,0,0,0,0.5,1,1,1,1],"control":[[0,0],[2,4],[5,5],[8,1],[12,0]],"closed":false,"periodic":false}"#;
    let (mut replaced, mut added) = ([0, 0], [0, 0]);
    for path in drawings() {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(' ', "_");
        let dwg = usize::from(name.to_ascii_lowercase().ends_with(".dwg"));
        let mut session = Session::new(b"").unwrap();
        session
            .push(name.as_bytes(), &std::fs::read(&path).unwrap())
            .unwrap();
        if !open(&mut session, &name) {
            continue;
        }
        let check = |session: &mut Session, doc: &str, response: &Value, geometry: &str| {
            let file = session.take(0).unwrap();
            let id = text(response, "/result/applied/0/id").unwrap().to_string();
            let hash = text(response, "/result/applied/0/hash")
                .unwrap()
                .to_string();
            session.push(doc.as_bytes(), &file).unwrap();
            assert!(open(session, doc), "{name}");
            assert!(
                entities(session, doc)
                    .iter()
                    .any(|(found, _, found_hash)| *found == id && *found_hash == hash),
                "{name}: {id}"
            );
            let found = shapes(session, doc)
                .into_iter()
                .find(|(found, _)| *found == id)
                .and_then(|(_, geometry)| geometry);
            let expected = zumen_json::parse(geometry.as_bytes()).unwrap();
            assert!(
                found.as_ref().is_some_and(|found| close(found, &expected)),
                "{name}: {id}: {found:?}"
            );
        };
        let spline = shapes(&mut session, &name)
            .into_iter()
            .find(|(id, geometry)| {
                id.starts_with("space/0/h")
                    && id.matches('/').count() == 2
                    && geometry.as_ref().is_some_and(|g| {
                        text(g, "/shape") == Some("spline") && g.pointer("/fit").is_none()
                    })
            });
        if let Some((id, _)) = spline {
            let (_, _, hash) = entities(&mut session, &name)
                .into_iter()
                .find(|(found, _, _)| *found == id)
                .unwrap();
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{replacing}}}],{budget}}}"#
                ),
            );
            assert_eq!(status, Status::Ok, "{name}: {response:?}");
            check(&mut session, "replaced", &response, replacing);
            replaced[dwg] += 1;
        }
        let table = tables(&mut session, &name);
        let layer_id = text(&table, "/layers/0/id").unwrap().to_string();
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"add","place":"space/0","layer_id":"{layer_id}","geometry":{adding}}}],{budget}}}"#
            ),
        );
        if status != Status::Ok {
            assert!(
                format!("{response:?}").contains("change.edit.unsupported"),
                "{name}: {response:?}"
            );
            continue;
        }
        check(&mut session, "added", &response, adding);
        added[dwg] += 1;
    }
    assert!(
        drawings().is_empty() || (replaced.iter().all(|n| *n > 0) && added.iter().all(|n| *n > 0)),
        "{replaced:?} {added:?}"
    );
}

/// 試料の寸法は、`entities` が形の値を返し、その形の値は `replace` で受ける値になる。寸法を描いた部品を
/// 作り直せないので、書き戻しは `change.edit.unsupported` で断り、ファイルを変えない（決定 0139）
#[test]
fn drawings_return_dimensions_but_do_not_write_them() {
    let mut returned = 0;
    for path in drawings() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"in", &std::fs::read(&path).unwrap()).unwrap();
        if !open(&mut session, "in") {
            continue;
        }
        let seen = entities(&mut session, "in");
        let found = shapes(&mut session, "in");
        let dimensions: Vec<_> = found
            .iter()
            .zip(&seen)
            .filter_map(|((id, geometry), (_, _, hash))| {
                let geometry = geometry.as_ref()?;
                (text(geometry, "/shape") == Some("dimension")).then_some((id, hash, geometry))
            })
            .collect();
        returned += dimensions.len();
        for (id, hash, geometry) in dimensions.into_iter().take(5) {
            // 文字の出し方だけを替える。種類と定義点はそのまま
            let mut out = zumen_json::Writer::new();
            out.value(geometry);
            let sent = String::from_utf8(out.into_bytes()).unwrap();
            let label = text(geometry, "/label").unwrap();
            let content = match text(geometry, "/content") {
                Some(content) => {
                    let mut out = zumen_json::Writer::new();
                    out.value(&Value::String(content.to_string()));
                    format!(
                        r#","content":{}"#,
                        String::from_utf8(out.into_bytes()).unwrap()
                    )
                }
                None => String::new(),
            };
            let other = if label == "hidden" {
                "measurement"
            } else {
                "hidden"
            };
            let sent = sent.replace(
                &format!(r#""label":"{label}"{content}"#),
                &format!(r#""label":"{other}""#),
            );
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"in","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{sent}}}],"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited"}}}}"#
                ),
            );
            let code = response
                .pointer("/diagnostics")
                .and_then(|d| match d {
                    Value::Array(items) => items
                        .iter()
                        .filter(|item| text(item, "/severity") == Some("error"))
                        .find_map(|item| text(item, "/code").map(str::to_string)),
                    _ => None,
                })
                .unwrap_or_default();
            assert_eq!(status, Status::Failed, "{} {sent}", path.display());
            assert_eq!(
                code,
                "change.edit.unsupported",
                "{} {sent} {response:?}",
                path.display()
            );
        }
        assert_eq!(shapes(&mut session, "in"), found, "{}", path.display());
    }
    assert!(drawings().is_empty() || returned > 0);
}

/// 試料の属性を持つ記号（表題欄など）の属性の値を `replace` で変えると、ATTRIB の値だけが書き直され、読み直した
/// 形の値は変えたものと同じになる。ほかの要素の形の値は変わらない。書式つきの値を持つ属性は値を変えられない
#[test]
fn drawings_take_back_attribute_values_changed_through_the_boundary() {
    let json = |value: &Value| {
        let mut out = zumen_json::Writer::new();
        out.value(value);
        String::from_utf8(out.into_bytes()).unwrap()
    };
    // DXF と DWG それぞれで書き換えた数
    let mut changed = [0, 0];
    for path in drawings() {
        let mut session = Session::new(b"").unwrap();
        session.push(b"in", &std::fs::read(&path).unwrap()).unwrap();
        if !open(&mut session, "in") {
            continue;
        }
        let symbols: Vec<(String, String, Value)> = shapes(&mut session, "in")
            .into_iter()
            .zip(entities(&mut session, "in"))
            .filter_map(|((id, geometry), (_, _, hash))| {
                let geometry = geometry?;
                (text(&geometry, "/shape") == Some("symbol")
                    && text(&geometry, "/attributes/0/tag").is_some())
                .then_some((id, hash, geometry))
            })
            .take(3)
            .collect();
        for (id, hash, geometry) in symbols {
            let before = shapes(&mut session, "in");
            let sent = json(&geometry);
            let attribute = geometry.pointer("/attributes/0").unwrap();
            let value = text(attribute, "/value").unwrap();
            let fresh = format!("{value}-1");
            let old = json(attribute);
            let new = old.replacen(
                &json(&Value::String(value.to_string())),
                &json(&Value::String(fresh.clone())),
                1,
            );
            let sent = sent.replacen(&old, &new, 1);
            let (status, response) = call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"apply","doc":"in","edits":[{{"kind":"replace","id":"{id}","hash":"{hash}","geometry":{sent}}}],"budget":{{"work":"unlimited","elements":"unlimited","records":"unlimited"}}}}"#
                ),
            );
            if status != Status::Ok {
                // 書式つきの値を持つ属性と、2007 より前の DXF の ASCII でない値は書けない
                assert_eq!(status, Status::Failed, "{} {sent}", path.display());
                assert_eq!(shapes(&mut session, "in"), before, "{}", path.display());
                continue;
            }
            assert_eq!(text(&response, "/result/applied/0/id"), Some(id.as_str()));
            let expected: Vec<(String, Option<String>)> = before
                .iter()
                .map(|(other, geometry)| {
                    let geometry = match (other == &id, geometry) {
                        (true, _) => Some(sent.clone()),
                        (false, geometry) => geometry.as_ref().map(json),
                    };
                    (other.clone(), geometry)
                })
                .collect();
            let after: Vec<(String, Option<String>)> = shapes(&mut session, "in")
                .iter()
                .map(|(other, geometry)| (other.clone(), geometry.as_ref().map(json)))
                .collect();
            assert_eq!(after, expected, "{}", path.display());
            changed[usize::from(path.extension().is_some_and(|e| e == "dwg"))] += 1;
        }
    }
    assert!(
        drawings().is_empty() || changed.iter().all(|count| *count > 0),
        "{changed:?}"
    );
}

/// 試料の文字と書式つきの文字は、`tables` の文字スタイルを ID（`style_id`）で指す。その ID だけで文字を足すと、
/// 書き戻したファイルを開き直した文字が、同じ文字スタイルを指す（決定 0158）
#[test]
fn drawings_refer_to_text_styles_by_their_ids() {
    let budget = r#""budget":{"work":"unlimited","elements":"unlimited","records":"unlimited","decoded_bytes":"unlimited"}"#;
    let (mut referred, mut added) = ([0, 0], [0, 0]);
    for path in drawings() {
        let name = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(' ', "_");
        let dwg = usize::from(name.to_ascii_lowercase().ends_with(".dwg"));
        let mut session = Session::new(b"").unwrap();
        session
            .push(name.as_bytes(), &std::fs::read(&path).unwrap())
            .unwrap();
        if !open(&mut session, &name) {
            continue;
        }
        let table = tables(&mut session, &name);
        let layer_id = text(&table, "/layers/0/id").unwrap().to_string();
        let Some(Value::Array(styles)) = table.pointer("/text_styles").cloned() else {
            panic!("{name}");
        };
        let items: Vec<(String, String)> = styles
            .iter()
            .map(|item| {
                (
                    text(item, "/id").unwrap().to_string(),
                    text(item, "/name").unwrap().to_string(),
                )
            })
            .collect();
        let unique = |id: &str| {
            let (_, found) = items.iter().find(|(item, _)| item == id)?;
            let same = items
                .iter()
                .filter(|(_, other)| other.eq_ignore_ascii_case(found))
                .count();
            (same == 1).then(|| found.clone())
        };
        let mut chosen = None;
        for (id, geometry) in shapes(&mut session, &name) {
            let Some(geometry) = geometry else { continue };
            if !matches!(text(&geometry, "/shape"), Some("text" | "formatted_text")) {
                continue;
            }
            // 要素が名前で指すだけの文字スタイルも、定義のない項目として表にあるので、ID は必ず決まる
            let style = text(&geometry, "/style").unwrap();
            let style_id = text(&geometry, "/style_id").unwrap_or_else(|| panic!("{name}: {id}"));
            let (_, found) = items
                .iter()
                .find(|(item, _)| item == style_id)
                .unwrap_or_else(|| panic!("{name}: {id} {style_id}"));
            assert!(found.eq_ignore_ascii_case(style), "{name}: {id}");
            referred[dwg] += 1;
            if chosen.is_none() && id.starts_with("space/0/") {
                chosen = unique(style_id).map(|found| (style_id.to_string(), found));
            }
        }
        let Some((style_id, style)) = chosen else {
            continue;
        };
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"apply","doc":"{name}","edits":[{{"kind":"add","place":"space/0","layer_id":"{layer_id}","geometry":{{"shape":"text","content":"STYLE","position":[1,2],"height":2.5,"rotation":0,"style_id":"{style_id}"}}}}],{budget}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{name}: {response:?}");
        let id = text(&response, "/result/applied/0/id").unwrap().to_string();
        let file = session.take(0).unwrap();
        session.push(b"again", &file).unwrap();
        assert!(open(&mut session, "again"), "{name}");
        let geometry = shapes(&mut session, "again")
            .into_iter()
            .find(|(found, _)| *found == id)
            .and_then(|(_, geometry)| geometry)
            .unwrap_or_else(|| panic!("{name}: {id}"));
        // DWG の文字は、文字スタイルの名前を元の図形の書き方（STANDARD など）で返すことがある
        assert!(
            text(&geometry, "/style").is_some_and(|found| found.eq_ignore_ascii_case(&style)),
            "{name}"
        );
        assert_eq!(
            text(&geometry, "/style_id"),
            Some(style_id.as_str()),
            "{name}"
        );
        added[dwg] += 1;
    }
    if !drawings().is_empty() {
        assert!(referred.iter().all(|count| *count > 0), "{referred:?}");
        assert!(added.iter().all(|count| *count > 0), "{added:?}");
    }
}
