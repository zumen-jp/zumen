//! 試料の DXF・DWG で、拡張辞書の XRECORD を、決定 0157 の振り分けで形式固有の情報に移す（決定 0148・0152）。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。
#![cfg(feature = "drawing")]

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_drawing::{Drawing2D, Target};
use zumen_sdk::{Codec, Lift};

/// フォルダーの中の、ASCII の DXF と DWG
#[cfg(test)]
fn drawings() -> Vec<PathBuf> {
    let mut found: Vec<_> = support::sample_files()
        .into_iter()
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

/// 付く先が `wanted` の XRECORD の、項目の名前
fn names(drawing: &Drawing2D, wanted: impl Fn(&Target) -> bool) -> Vec<String> {
    drawing
        .extensions
        .iter()
        .filter(|extension| extension.namespace == "dxf.xrecord" && wanted(&extension.target))
        .map(|extension| extension.name.clone())
        .collect()
}

/// 図面モデルに移さなかった拡張辞書の項目の名前。重ねない
fn unsupported(diag: &Diagnostics) -> BTreeSet<String> {
    diag.iter()
        .filter(|d| d.code() == codes::DXF_EXTENSION_DICTIONARY_UNSUPPORTED)
        .map(|d| format!("{:?}", d.params()))
        .collect()
}

/// 試料の `sample_AC10*` の図面（R12 の AC1009 を除く）は、版の往復のための XRECORD（`ACAD_XREC_ROUNDTRIP`。
/// R2000 と R2004 の文字は `ACAD_MTEXT_RT`・`ACAD_MTEXT_2008_RT` も）と、注釈の尺度
/// （`AcDbContextDataManager`）・寸法の関連づけ（`ACAD_DIMASSOC`）・動的ブロックの表し方
/// （`AcDbBlockRepresentation`）を拡張辞書に持つ。どれも図面モデルに移さず、診断する（決定 0157）。
/// 同じ版の DWG と DXF で、移さなかった項目の名前が同じになる
#[test]
fn xrecords_without_model_fields_stay_in_the_file() {
    let mut by_version: BTreeMap<String, Vec<BTreeSet<String>>> = BTreeMap::new();
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some((drawing, diag)) = lift(&path) else {
            continue;
        };
        let kept = names(&drawing, |_| true);
        assert_eq!(kept, Vec::<String>::new(), "{name}");
        let dropped = unsupported(&diag);
        for key in [
            "ACAD_XREC_ROUNDTRIP",
            "AcDbContextDataManager",
            "ACAD_DIMASSOC",
            "AcDbBlockRepresentation",
        ] {
            assert!(dropped.iter().any(|d| d.contains(key)), "{name} {key}");
        }
        let version = name.get(7..13).unwrap().to_owned();
        by_version.entry(version).or_default().push(dropped);
    }
    // DWG と DXF の両方があれば、同じ名前になる
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

/// 画層の拡張辞書のアプリケーションの XRECORD は、画層に付く形式固有の情報になる（決定 0152）。
/// `farmhouse.dwg` の画層は、画層の通知の控え（`ADSK_XREC_LAYER_RECONCILED`）を形式固有の情報にし、
/// ビューごとの色の上書き（`ADSK_XREC_LAYER_COLOR_OVR`）は図面モデルの欄にする値なので移さずに診断する
/// （決定 0157）。値はグループ・コードを付けて並べる
#[test]
fn xrecords_of_layers_become_extensions() {
    for path in drawings() {
        if path.file_name().is_none_or(|n| n != "farmhouse.dwg") {
            continue;
        }
        let (drawing, diag) = lift(&path).unwrap();
        let layers = names(&drawing, |target| matches!(target, Target::Layer(_)));
        assert!(!layers.is_empty());
        assert!(layers.iter().all(|n| n == "ADSK_XREC_LAYER_RECONCILED"));
        for extension in &drawing.extensions {
            assert!(extension.is_current(&drawing));
            if extension.namespace == "dxf.xrecord" {
                assert!(matches!(extension.target, Target::Layer(_)));
                assert!(extension.fields.iter().all(|field| field.code.is_some()));
            }
        }
        assert!(
            unsupported(&diag)
                .iter()
                .any(|d| d.contains("ADSK_XREC_LAYER_COLOR_OVR"))
        );
    }
}
