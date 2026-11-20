//! R2000・R2007 の拡張データ段組み MTEXT と R2018 の埋め込みオブジェクト段組み MTEXT を
//! 公開操作（`open`・`drawing`・`entities`）で読み、要素数・関連・診断を確かめる回帰試験（#904）。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, str as json_str, uint};

#[cfg(test)]
fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

#[cfg(test)]
fn open_and_entities(name: &str, bytes: &[u8]) -> (Value, Value, Value) {
    let mut session = Session::new(b"").unwrap();
    session.push(name.as_bytes(), bytes).unwrap();
    let (status, open_res) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"open","input":"{name}"}}"#),
    );
    assert_eq!(status, Status::Ok, "{name}: {open_res:?}");

    let (status, draw_res) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"drawing","doc":"{name}"}}"#),
    );
    assert_eq!(status, Status::Ok, "{name}: {draw_res:?}");

    let (status, ent_res) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"entities","doc":"{name}","fields":["geometry","source"]}}"#),
    );
    assert_eq!(status, Status::Ok, "{name}: {ent_res:?}");
    (open_res, draw_res, ent_res)
}

#[cfg(test)]
fn column_diagnostics(root: &Value) -> Vec<(u64, Vec<u64>)> {
    let Some(Value::Array(diags)) = root.pointer("/diagnostics") else {
        return Vec::new();
    };
    diags
        .iter()
        .filter(|diag| {
            diag.pointer("/code").and_then(|v| json_str(v).ok())
                == Some("dxf.mtext-columns.unsupported")
        })
        .map(|diag| {
            let count = diag
                .pointer("/count")
                .and_then(|v| uint(v, 0, u64::MAX).ok())
                .unwrap_or_else(|| panic!("missing count: {diag:?}"));
            let records = match diag.pointer("/at") {
                Some(Value::Array(items)) => items
                    .iter()
                    .filter_map(|p| p.pointer("/record").and_then(|v| uint(v, 0, u64::MAX).ok()))
                    .collect(),
                _ => Vec::new(),
            };
            (count, records)
        })
        .collect()
}

/// R2000・R2007 の `ACAD` 拡張データ段組み MTEXT 2 種が続く列（`2E`・`2F`・`31`・`32`・`34`・`35`）を
/// 個別要素として返さず、R2018 の `101` 形式と同じく 0 要素・3 件の `dxf.mtext-columns.unsupported` になる。
#[test]
fn public_column_mtext_samples_report_three_groups_across_r2000_r2007_and_r2018() {
    let r2000 = include_bytes!("../../../samples/dxf/ezdxf-mtext-columns-r2000.dxf");
    let r2007 = include_bytes!("../../../samples/dxf/ezdxf-mtext-columns-r2007.dxf");
    let r2018 = include_bytes!("../../../samples/dxf/ezdxf-mtext-columns-r2018.dxf");

    for (name, bytes, expected_records) in [
        ("r2000.dxf", &r2000[..], vec![58, 61, 64]),
        ("r2007.dxf", &r2007[..], vec![58, 61, 64]),
        ("r2018.dxf", &r2018[..], vec![58, 59, 60]),
    ] {
        let (open_res, draw_res, ent_res) = open_and_entities(name, bytes);
        assert_eq!(
            ent_res.pointer("/result/items"),
            Some(&Value::Array(Vec::new())),
            "{name}: {ent_res:?}"
        );
        assert_eq!(
            draw_res
                .pointer("/result/spaces/0/elements/formatted_texts")
                .and_then(|v| uint(v, 0, u64::MAX).ok()),
            Some(0),
            "{name}: {draw_res:?}"
        );
        assert_eq!(
            column_diagnostics(&draw_res),
            vec![(3, expected_records.clone())],
            "{name}"
        );
        assert_eq!(
            column_diagnostics(&ent_res),
            vec![(3, expected_records)],
            "{name}"
        );
        let _ = open_res;
    }
}

/// 同じ空間に段組みでない MTEXT（`75 = 0` や `ACAD_MTEXT_DEFINED_HEIGHT` のみ）や別種類の図形があるとき、
/// 段組みの親と `1005` で結ばれた続く列の MTEXT だけを除き、通常の文字と別種類の図形はそのまま返す。
#[test]
fn preserves_non_column_mtext_and_non_mtext_entities_around_linked_columns() {
    let dxf = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nMTEXT\n5\n2D\n8\n0\n10\n0.0\n20\n0.0\n40\n2.5\n41\n20.0\n71\n1\n1\ncol1\n\
1001\nACAD\n\
1000\nACAD_MTEXT_COLUMN_INFO_BEGIN\n1070\n75\n1070\n1\n1070\n76\n1070\n2\n1000\nACAD_MTEXT_COLUMN_INFO_END\n\
1000\nACAD_MTEXT_COLUMNS_BEGIN\n1070\n47\n1070\n2\n1005\n2E\n1005\n50\n1000\nACAD_MTEXT_COLUMNS_END\n\
0\nMTEXT\n5\n2E\n8\n0\n10\n21.0\n20\n0.0\n40\n2.5\n41\n20.0\n71\n1\n1\ncol2\n\
0\nMTEXT\n5\n40\n8\n0\n10\n50.0\n20\n0.0\n40\n2.5\n41\n20.0\n71\n1\n1\nplain\n\
1001\nACAD\n\
1000\nACAD_MTEXT_COLUMN_INFO_BEGIN\n1070\n75\n1070\n0\n1000\nACAD_MTEXT_COLUMN_INFO_END\n\
1000\nACAD_MTEXT_DEFINED_HEIGHT_BEGIN\n1070\n46\n1040\n30.0\n1000\nACAD_MTEXT_DEFINED_HEIGHT_END\n\
0\nLINE\n5\n50\n8\n0\n10\n0.0\n20\n0.0\n11\n10.0\n21\n0.0\n\
0\nENDSEC\n0\nEOF\n";

    let (_, draw_res, ent_res) = open_and_entities("mixed.dxf", dxf);
    let Some(Value::Array(items)) = ent_res.pointer("/result/items") else {
        panic!("{ent_res:?}");
    };
    let ids: Vec<_> = items
        .iter()
        .filter_map(|item| item.pointer("/id").and_then(|v| json_str(v).ok()))
        .collect();
    assert_eq!(ids, vec!["space/0/h40", "space/0/h50"]);
    assert_eq!(
        items[0]
            .pointer("/geometry/content")
            .and_then(|v| json_str(v).ok()),
        Some("plain")
    );
    assert_eq!(column_diagnostics(&draw_res), vec![(1, vec![3])]);
    assert_eq!(column_diagnostics(&ent_res), vec![(1, vec![3])]);
}
