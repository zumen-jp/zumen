//! `ZUMEN_SAMPLES` のフォルダーにある DXF・DWG で、レイアウトごとの用紙を確かめる（決定 0145）。
//! フォルダーを指定しないときは何も確かめない。図面は Git に入れない。

use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::Drawing2D;
use zumen_sdk::{Codec, Lift};

#[cfg(test)]
#[allow(
    clippy::disallowed_methods,
    reason = "環境変数を読むのは試験だけで、コアは読まない（AGENTS.md の図面と秘密）"
)]
/// フォルダーの中の、ASCII の DXF と DWG
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

/// 試料の `sample_AC10*` の図面（レイアウトを持たない R12 の AC1009 を除く）は、今のレイアウト MyLayout と、
/// ほかのレイアウト Layout1・Layout2 を持つ。どの版の DWG・DXF でも、今のレイアウトを先に、ほかをタブの順に
/// 並べた 3 枚の用紙になる。ほかのレイアウトは、用紙そのもののビューポートを除く 1 つのビューを持つ
#[test]
fn each_layout_of_the_samples_becomes_a_sheet() {
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some(drawing) = lift(&path) else { continue };
        let names: Vec<_> = drawing
            .sheets
            .iter()
            .map(|sheet| sheet.name.as_deref())
            .collect();
        assert_eq!(
            names,
            [Some("MyLayout"), Some("Layout1"), Some("Layout2")],
            "{name}"
        );
        for sheet in &drawing.sheets[1..] {
            assert_eq!(sheet.views.len(), 1, "{name}");
            assert_eq!(sheet.views[0].sheet, sheet.id, "{name}");
            let size = sheet.size.unwrap().map(|l| l.0);
            // レター（8.5 × 11 インチ）の範囲
            assert!(
                (size[0] - 279.4).abs() < 1e-3 && (size[1] - 215.9).abs() < 1e-3,
                "{name}"
            );
        }
        checked += 1;
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(checked == 0 || checked >= 11, "{checked}");
}

/// 図形のないレイアウトも、名前と大きさだけの用紙になる（決定 0153）。`arc_2000.dwg` などの AutoCAD の
/// 試料は、モデルのタブを今のレイアウトとし、図形のない Layout1・Layout2 を持つ。今のレイアウトの用紙は
/// 作らず、ほかの 2 つのレイアウトが、中身のない用紙になる
#[test]
fn empty_layouts_become_empty_sheets() {
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !matches!(
            name.as_str(),
            "arc_2000.dwg"
                | "line_2000.dwg"
                | "jp-yokosuka-minami-survey-plan.dwg"
                | "jp-yokosuka-minami-survey-plan.dxf"
        ) {
            continue;
        }
        let Some(drawing) = lift(&path) else { continue };
        let sheets: Vec<_> = drawing
            .sheets
            .iter()
            .map(|sheet| (sheet.id.0, sheet.name.as_deref()))
            .collect();
        assert_eq!(
            sheets,
            [(1, Some("Layout1")), (2, Some("Layout2"))],
            "{name}"
        );
        for sheet in &drawing.sheets {
            assert!(
                sheet.elements.is_empty() && sheet.views.is_empty(),
                "{name}"
            );
        }
        checked += 1;
    }
    assert!(checked == 0 || checked == 4, "{checked}");
}

/// 図形のある用紙と図形のない用紙が混じるとき、用紙はタブの順に並び、中身のない用紙の ID は中身の
/// ある用紙の後に付く。`features-r2018.dxf` は、図形のない Layout1 と、図形のある A3-PLAN を持つ
#[test]
fn empty_sheets_take_ids_after_the_filled_ones() {
    for path in drawings() {
        if path.file_name().is_none_or(|n| n != "features-r2018.dxf") {
            continue;
        }
        let drawing = lift(&path).unwrap();
        let sheets: Vec<_> = drawing
            .sheets
            .iter()
            .map(|sheet| (sheet.id.0, sheet.name.as_deref(), sheet.elements.is_empty()))
            .collect();
        assert_eq!(
            sheets,
            [(2, Some("Layout1"), true), (1, Some("A3-PLAN"), false)]
        );
    }
}
