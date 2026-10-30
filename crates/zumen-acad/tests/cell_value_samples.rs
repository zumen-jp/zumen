//! 試料の DXF・DWG で、表のセルの値の型・単位・表示の書式を移す（決定 0147）。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。
#![cfg(feature = "drawing")]

mod support;

use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{CellData, CellValue, DateTime, Drawing2D, Shape, ValueUnit};
use zumen_sdk::{Codec, Lift};

/// フォルダーの中の、2007 からの版の ASCII の DXF と、2010 からの版の DWG
#[cfg(test)]
fn drawings() -> Vec<PathBuf> {
    let mut found: Vec<_> = support::sample_files()
        .into_iter()
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            ["AC1021_ascii.dxf", "AC1024", "AC1027", "AC1032"]
                .iter()
                .any(|version| name.starts_with(&format!("sample_{version}")))
                && (name.ends_with("_ascii.dxf") || name.ends_with(".dwg"))
        })
        .collect();
    found.sort();
    found
}

#[cfg(test)]
fn lift(path: &Path) -> Drawing2D {
    let src = std::fs::read(path).unwrap();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    if path.extension().is_some_and(|e| e == "dwg") {
        Dwg::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap()
    } else {
        Dxf::read(&src, &mut cx).unwrap().lift(&mut cx).unwrap()
    }
}

/// 試料の表は、文字列のほかに、実数・角度・点・日時・整数・百分率・通貨の値を持つ。DXF の 2007 からの版と、DWG の
/// 2010 からの版で、同じ値・単位・書式になる。角度はラジアンで持つ（表示する文字は 60）。DXF の実数は文字の
/// 精度で丸まるので、近さで比べる
#[test]
fn table_cells_keep_typed_values_in_dwg_and_dxf() {
    let typed = |data, unit, format: Option<&str>| CellValue {
        data,
        unit,
        format: format.map(str::to_owned),
    };
    let expected = [
        typed(CellData::Real(0.5), ValueUnit::None, None),
        typed(
            CellData::Real(60f64.to_radians()),
            ValueUnit::Angle,
            Some("%au0"),
        ),
        typed(CellData::Point([0.0; 3]), ValueUnit::None, None),
        typed(
            CellData::Date(DateTime {
                year: 2024,
                month: 11,
                day: 15,
                hour: 0,
                minute: 0,
                second: 0,
                millisecond: 0,
            }),
            ValueUnit::None,
            None,
        ),
        typed(CellData::Integer(5), ValueUnit::None, None),
        typed(
            CellData::Real(35.0),
            ValueUnit::Percentage,
            Some("%lu2%pr2%ps[,%]"),
        ),
        typed(
            CellData::Real(100.0),
            ValueUnit::Currency,
            Some("%lu2%pr2%ps[$,]"),
        ),
    ];
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let drawing = lift(&path);
        let values: Vec<CellValue> = drawing
            .spaces
            .iter()
            .flat_map(|space| &space.elements)
            .filter_map(|element| match &element.shape {
                Shape::Table(table) => Some(table),
                _ => None,
            })
            .flat_map(|table| &table.cells)
            .filter_map(|cell| cell.value.clone())
            .collect();
        assert_eq!(values.len(), 19, "{name}");
        let typed: Vec<_> = values
            .into_iter()
            .filter(|value| !matches!(value.data, CellData::Text(_)))
            .collect();
        assert_eq!(typed.len(), expected.len(), "{name}");
        for (found, expected) in typed.iter().zip(&expected) {
            assert_eq!(
                (&found.unit, &found.format),
                (&expected.unit, &expected.format),
                "{name}"
            );
            match (&found.data, &expected.data) {
                (CellData::Real(found), CellData::Real(expected)) => {
                    assert!((found - expected).abs() < 1e-12, "{name} {found}");
                }
                (found, expected) => assert_eq!(found, expected, "{name}"),
            }
        }
        checked += 1;
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(checked == 0 || checked >= 7, "{checked}");
}
