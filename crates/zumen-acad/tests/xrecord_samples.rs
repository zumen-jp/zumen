//! `ZUMEN_SAMPLES` のフォルダーにある DXF・DWG で、図形の拡張辞書の XRECORD を、形式固有の情報に移す
//! （決定 0148）。フォルダーを指定しないときは何も確かめない。図面は Git に入れない。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_drawing::{Basis, Drawing2D, Field, FieldValue, Target};
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
    records(drawing, |target| {
        matches!(target, Target::Model { .. } | Target::Paper { .. })
    })
}

/// 付く先が `wanted` の XRECORD の、項目の名前と値の並び。ハンドルは除く
fn records(drawing: &Drawing2D, wanted: impl Fn(&Target) -> bool) -> Records {
    drawing
        .extensions
        .iter()
        .filter(|extension| extension.namespace == "dxf.xrecord" && wanted(&extension.target))
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

/// 試料の `sample_AC10*` の図面の画層と線種の拡張辞書の XRECORD は、画層と線種に付く（決定 0152）。
/// R2007 までの図面は、注釈の尺度の画層（`0 @ 1`・`Layer_color_80 @ 1`）に `ASDK_XREC_ANNO_SCALE_INFO` を持ち、
/// DWG と DXF で同じ値になる。版の往復のための `ACAD_XREC_ROUNDTRIP` は形式ごとに違い、R2010 までの DXF は
/// 線種 GAS_LINE に、R2000 の DWG は真の色と色見本帳の色を持つ画層に持つ
#[test]
fn xrecords_of_layers_and_line_types_become_extensions() {
    let mut by_version: BTreeMap<String, Vec<Vec<(String, String)>>> = BTreeMap::new();
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some((drawing, _)) = lift(&path) else {
            continue;
        };
        let mut found = Vec::new();
        for extension in &drawing.extensions {
            if extension.namespace != "dxf.xrecord" {
                continue;
            }
            let owner = match extension.target {
                Target::Layer(id) => drawing.layer(id).unwrap().name.clone(),
                Target::LineType(id) => drawing.line_type(id).unwrap().name.clone(),
                _ => continue,
            };
            assert!(extension.is_current(&drawing), "{name}");
            assert!(
                matches!(extension.basis, Basis::Layer(_) | Basis::LineType(_)),
                "{name}"
            );
            found.push((owner, extension.name.clone()));
        }
        let dxf = name.ends_with(".dxf");
        let mut expected = Vec::new();
        if dxf && name.as_str() < "sample_AC1027" {
            expected.push(("GAS_LINE".to_owned(), "ACAD_XREC_ROUNDTRIP".to_owned()));
        }
        if name == "sample_AC1015.dwg" {
            for layer in ["Layer_true_color", "Layer_color_book", "color_125_33_79"] {
                expected.push((layer.to_owned(), "ACAD_XREC_ROUNDTRIP".to_owned()));
            }
        }
        if name.as_str() < "sample_AC1024" {
            for layer in ["0 @ 1", "Layer_color_80 @ 1"] {
                expected.push((layer.to_owned(), "ASDK_XREC_ANNO_SCALE_INFO".to_owned()));
            }
        }
        assert_eq!(found, expected, "{name}");
        let version = name.get(7..13).unwrap().to_owned();
        let values = records(&drawing, |target| matches!(target, Target::Layer(_)));
        by_version.entry(version).or_default().push(
            values
                .into_iter()
                .filter(|(key, _)| key == "ASDK_XREC_ANNO_SCALE_INFO")
                .map(|(key, fields)| (key, format!("{fields:?}")))
                .collect(),
        );
    }
    for (version, found) in &by_version {
        if let [dwg, dxf] = found.as_slice() {
            assert_eq!(dwg, dxf, "{version}");
        }
    }
}
