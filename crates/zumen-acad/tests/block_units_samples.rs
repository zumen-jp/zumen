//! 試料の DXF・DWG で、ブロックのレコードの挿す単位を、記号の定義の空間の単位に移す（決定 0144）。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。

mod support;

use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Drawing2D, Shape, Unit};
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
fn lift(path: &Path) -> Option<Drawing2D> {
    let src = std::fs::read(path).unwrap();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    if path.extension().is_some_and(|e| e == "dwg") {
        Dwg::read(&src, &mut cx).ok()?.lift(&mut cx).ok()
    } else {
        Dxf::read(&src, &mut cx).ok()?.lift(&mut cx).ok()
    }
}

/// 試料の `sample_AC10*` の図面（単位を持たない R12 の AC1009 を除く）は inch で、mm と m のブロックを持つ。どの版の DWG・DXF でも、定義の単位は
/// ブロックのレコードの挿す単位になる。単位の換算は配置の尺度が持つ。mm のブロックを尺度 1/25.4 で挿した配置が
/// あり、定義の 1 mm が図面の 1 mm になる
#[test]
fn block_record_units_become_the_unit_of_the_definition() {
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some(drawing) = lift(&path) else { continue };
        let unit = |key: &str| {
            drawing
                .symbols
                .iter()
                .find(|symbol| symbol.key.name == key)
                .map(|symbol| symbol.space.unit)
        };
        assert_eq!(drawing.spaces[0].unit, Unit::Inch, "{name}");
        assert_eq!(unit("MyBlock"), Some(Unit::Millimeter), "{name}");
        assert_eq!(unit("my_block"), Some(Unit::Millimeter), "{name}");
        assert_eq!(unit("my_block_v2"), Some(Unit::Meter), "{name}");
        let converted = drawing.spaces[0].elements.iter().any(|element| {
            matches!(&element.shape, Shape::Symbol(symbol)
                if symbol.key.name == "my_block"
                    && symbol.scale.iter().all(|s| (s * 25.4 - 1.0).abs() < 1e-9))
        });
        assert!(converted, "{name}");
        checked += 1;
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(checked == 0 || checked >= 11, "{checked}");
}
