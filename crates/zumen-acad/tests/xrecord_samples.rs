//! `ZUMEN_SAMPLES` のフォルダーにある DXF・DWG で、図形の拡張辞書の XRECORD を、形式固有の情報に移す
//! （決定 0148）。フォルダーを指定しないときは何も確かめない。図面は Git に入れない。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_drawing::{Drawing2D, Field, FieldValue};
use zumen_sdk::{Codec, Lift};

/// フォルダーの中の、ASCII の DXF と DWG
#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
fn drawings() -> Vec<PathBuf> {
    let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) else {
        return Vec::new();
    };
    let mut found: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "dxf" || e == "dwg"))
        .filter(|path| !path.to_string_lossy().ends_with("_binary.dxf"))
        .collect();
    found.sort();
    found
}

#[cfg(test)]
fn lift(path: &Path) -> Option<(Drawing2D, Diagnostics)> {
    let src = std::fs::read(path).unwrap();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let drawing = if path.extension().is_some_and(|e| e == "dwg") {
        Dwg::read(&src, &mut cx).ok()?.lift(&mut cx).ok()?
    } else {
        Dxf::read(&src, &mut cx).ok()?.lift(&mut cx).ok()?
    };
    Some((drawing, diag))
}

/// 項目の名前と、値の並び
type Records = Vec<(String, Vec<Field>)>;

/// 図形の XRECORD の、グループ・コードと値の並び。ハンドルは、DXF を書くときに付け直されることがあるので除く
fn values(drawing: &Drawing2D) -> Records {
    drawing
        .extensions
        .iter()
        .filter(|extension| extension.namespace == "dxf.xrecord")
        .map(|extension| {
            let fields = extension
                .fields
                .iter()
                .filter(|field| field.key != "handle")
                .cloned()
                .collect();
            (extension.name.clone(), fields)
        })
        .collect()
}

/// 試料の `sample_AC10*` の図面（R12 の AC1009 を除く）の表は、版の往復のための XRECORD
/// （`ACAD_XREC_ROUNDTRIP`）を拡張辞書に持つ。DWG と DXF で同じ値になる。R2010 からの DWG の XRECORD の
/// 文字列は UTF-16LE で、R2010 からの表の XRECORD は `ACAD_ROUNDTRIP_2008_TABLE_ENTITY` で始まる。
/// 文字の注釈の尺度（`AcDbContextDataManager`）、寸法の関連づけ（`ACAD_DIMASSOC`）、ブロックの切り抜き
/// （`ACAD_FILTER`）などの XRECORD でない項目は、移さずに診断する
#[test]
fn xrecords_of_extension_dictionaries_become_extensions() {
    let mut by_version: BTreeMap<String, Vec<Records>> = BTreeMap::new();
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some((drawing, diag)) = lift(&path) else {
            continue;
        };
        let found = values(&drawing);
        assert!(
            found.iter().any(|(key, _)| key == "ACAD_XREC_ROUNDTRIP"),
            "{name}"
        );
        if name.as_str() >= "sample_AC1024" {
            let tables: Vec<_> = found
                .iter()
                .filter(|(key, _)| key == "ACAD_XREC_ROUNDTRIP")
                .collect();
            assert_eq!(tables.len(), 2, "{name}");
            for (_, fields) in tables {
                assert_eq!(
                    fields.get(..2),
                    Some(
                        [
                            Field {
                                key: "code",
                                value: FieldValue::Integer(102)
                            },
                            Field {
                                key: "text",
                                value: FieldValue::Text(
                                    "ACAD_ROUNDTRIP_2008_TABLE_ENTITY".to_owned()
                                )
                            }
                        ]
                        .as_slice()
                    ),
                    "{name}"
                );
            }
        }
        let dropped: Vec<String> = diag
            .iter()
            .filter(|d| d.code() == codes::DXF_EXTENSION_DICTIONARY_DROPPED)
            .map(|d| format!("{:?}", d.params()))
            .collect();
        for key in [
            "AcDbContextDataManager",
            "ACAD_DIMASSOC",
            "AcDbBlockRepresentation",
        ] {
            assert!(dropped.iter().any(|d| d.contains(key)), "{name} {key}");
        }
        let version = name.get(7..13).unwrap().to_owned();
        by_version.entry(version).or_default().push(found);
    }
    // DWG と DXF の両方があれば、同じ値になる
    let mut compared = 0;
    for (version, found) in &by_version {
        if let [dwg, dxf] = found.as_slice() {
            assert_eq!(dwg, dxf, "{version}");
            compared += 1;
        }
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(by_version.is_empty() || compared >= 5, "{compared}");
}
