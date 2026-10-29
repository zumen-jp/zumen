//! `ZUMEN_SAMPLES` のフォルダーにある DXF・DWG で、表のセルの書式と、表のスタイルの行の種類ごとの書式を移す
//! （決定 0151）。フォルダーを指定しないときは何も確かめない。図面は Git に入れない。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{
    Border, CellFormat, Color, ColorCode, ColorSystem, Drawing2D, Horizontal, Inherit, Length,
    LineWeight, ModelPoint, PaperLength, Shape, Table, Vertical,
};
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

#[cfg(test)]
fn tables(drawing: &Drawing2D) -> Vec<&Table<ModelPoint>> {
    drawing
        .spaces
        .iter()
        .flat_map(|space| &space.elements)
        .filter_map(|element| match &element.shape {
            Shape::Table(table) => Some(table),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
fn aci(value: u32) -> Option<Inherit<Color>> {
    Some(Inherit::Own(Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value,
        }),
        rgb: None,
    }))
}

/// 試料の `sample_AC10*` の図面（表を持たない R12 の AC1009 を除く）の最初の表は、7 行 3 列で、5 行目の左の
/// セルが 3 列 2 行に広がる。そのセルは、位置合わせ・文字の色・背景の色（塗らない）・高さと、左と上の枠（色 63、
/// 0.40 mm）を上書きする。右の列のセルは右の枠も上書きする。どの版の DWG と DXF でも、セルの書式が同じになる。
/// DXF の表のスタイルは、データ・タイトル・見出しの行の書式を持つ（表を描いたブロックの文字と合う）。
/// R2004 までの DWG の TABLESTYLE も、DXF と同じ書式になる。R2010 からの DWG の読み手は、セルのスタイルと
/// 内容の書式と TABLESTYLE をまだ読まないので、セルの書式は空、表の書式は `None` になる
#[test]
fn cell_formats_and_table_styles_are_lifted() {
    let mut cells: BTreeMap<String, Vec<CellFormat>> = BTreeMap::new();
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some((drawing, _)) = lift(&path) else {
            continue;
        };
        let tables = tables(&drawing);
        let table = tables.first().unwrap();
        assert_eq!(table.cells.len(), 21, "{name}");
        let dwg_r2010 = name.ends_with(".dwg")
            && ["sample_AC1024", "sample_AC1027", "sample_AC1032"]
                .iter()
                .any(|v| name.starts_with(v));
        if dwg_r2010 {
            assert!(
                table
                    .cells
                    .iter()
                    .all(|cell| cell.format == CellFormat::default()),
                "{name}"
            );
            assert_eq!(table.formats, None, "{name}");
            checked += 1;
            continue;
        }
        let merged = &table.cells[12].format;
        assert_eq!(
            merged.alignment,
            Some((Horizontal::Center, Vertical::Middle)),
            "{name}"
        );
        assert_eq!(merged.text_color, aci(7), "{name}");
        assert_eq!(
            (merged.fill, &merged.fill_color),
            (Some(false), &aci(165)),
            "{name}"
        );
        let border = Border {
            color: aci(63),
            weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.4)))),
            visible: None,
        };
        for at in [12, 13, 14, 15, 16, 17] {
            let borders = &table.cells[at].format.borders;
            assert_eq!(
                (&borders.left, &borders.top),
                (&border, &border),
                "{name} {at}"
            );
            let right = if at % 3 == 2 {
                border.clone()
            } else {
                Border::default()
            };
            assert_eq!(borders.right, right, "{name} {at}");
        }
        assert_eq!(table.cells[18].format.borders.top, border, "{name}");
        {
            let formats = table.formats.as_deref().unwrap();
            assert!(formats.title && formats.header, "{name}");
            let height = |format: &CellFormat| match format.text_height {
                Some(Length::Model(length)) => length.value,
                _ => panic!("{name}"),
            };
            assert_eq!(height(&formats.title_format), 0.25, "{name}");
            assert_eq!(height(&formats.header_format), 0.18, "{name}");
            assert_eq!(height(&formats.data_format), 0.18, "{name}");
            assert_eq!(
                formats.data_format.alignment,
                Some((Horizontal::Center, Vertical::Top)),
                "{name}"
            );
            assert_eq!(formats.data_format.fill, Some(false), "{name}");
        }
        let version = name.get(..13).unwrap().to_owned();
        // ASCII の DXF の実数は丸められているので、高さは 1e-9 の桁で比べる
        let formats: Vec<_> = table
            .cells
            .iter()
            .map(|cell| {
                let mut format = cell.format.clone();
                if let Some(Length::Model(length)) = &mut format.text_height {
                    length.value = (length.value * 1e9).round();
                }
                format
            })
            .collect();
        match cells.get(&version) {
            Some(other) => assert_eq!(&formats, other, "{name}"),
            None => {
                cells.insert(version, formats);
            }
        }
        checked += 1;
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(checked == 0 || checked >= 11, "{checked}");
}
