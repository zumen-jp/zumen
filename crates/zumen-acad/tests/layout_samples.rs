//! サンプルの DXF・DWG で、レイアウトごとの用紙を確かめる（決定 0145）。同じサンプルに、HANDSEED の大きさ（バイト）が
//! 増える数の線を足して書き戻す（#701）。
//! サンプルは公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。
#![cfg(feature = "drawing")]

mod support;

use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, DwgFile, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Changes, Drawing2D, Edit, Element, ModelPoint, Placed, Shape, id};
use zumen_sdk::{Apply, Codec, Lift};

#[cfg(test)]
/// フォルダーの中の、ASCII の DXF と DWG
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

/// サンプルの `sample_AC10*` の図面（レイアウトを持たない R12 の AC1009 を除く）は、今のレイアウト MyLayout と、
/// ほかのレイアウト Layout1・Layout2 を持つ。どのバージョンの DWG・DXF でも、今のレイアウトを先に、ほかをタブの順に
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
/// サンプルは、モデルのタブを今のレイアウトとし、図形のない Layout1・Layout2 を持つ。今のレイアウトの用紙は
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

/// AutoCAD で作ったサンプル（#316）の用紙の大きさは、LAYOUT の範囲を、印刷の尺度と単位で mm にした値になる
/// （決定 0229）。ヘッダーの `$INSUNITS` はどれも mm だが、A3-1to2 の範囲は 840 × 594、Letter-inch の範囲は
/// 11 × 8.5 だった
#[test]
fn autocad_layout_sheets_take_millimeters_from_plot_settings() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg");
    for version in ["r2000", "r2004", "r2018"] {
        for path in [
            dir.join(format!("zumen-layout-{version}.dwg")),
            dir.join(format!("zumen-layout-{version}_ascii.dxf")),
        ] {
            let at = path.display();
            let drawing = lift(&path).unwrap_or_else(|| panic!("{at}"));
            for (name, width, height) in [
                ("A3-1to1", 420.0, 297.0),
                ("A3-1to2", 420.0, 297.0),
                ("Letter-inch", 279.4, 215.9),
            ] {
                let sheet = drawing
                    .sheets
                    .iter()
                    .find(|sheet| sheet.name.as_deref() == Some(name))
                    .unwrap_or_else(|| panic!("{at}: {name}"));
                let [w, h] = sheet.size.unwrap_or_else(|| panic!("{at}: {name}"));
                // サンプルの Letter-inch の範囲は、11 × 8.5 よりわずかに小さい値で保存されている
                assert!(
                    (w.0 - width).abs() < 1e-3 && (h.0 - height).abs() < 1e-3,
                    "{at}: {name} {w:?} {h:?}"
                );
            }
        }
    }
}

#[cfg(test)]
fn read_dwg(src: &[u8]) -> DwgFile {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    Dwg::read(
        src,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap()
}

#[cfg(test)]
fn lift_dwg(file: &DwgFile) -> Drawing2D {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    file.lift(&mut Cx::new(&mut budget, &mut diag, &CancelToken::new()))
        .unwrap()
}

/// 実寸の空間に `element` を足す変更
#[cfg(test)]
fn add_to_model(element: Element<ModelPoint>) -> Edit {
    Edit::Add {
        place: "space/0".to_owned(),
        element: Box::new(Placed::Model(element)),
    }
}

/// 実寸の空間の要素の ID（決定 0139）
#[cfg(test)]
fn model_ids(drawing: &Drawing2D) -> Vec<String> {
    let space = &drawing.spaces[0];
    let place = id::space(space.id);
    let mut namer = id::Namer::new(&place);
    space
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| namer.next(index, element))
        .collect()
}

/// 公開サンプルの R2018 に、HANDSEED が 0xFFFF をまたいで 3 バイトになる数の線を、1 回の apply で足す。
/// 書き戻したファイルを読み直すと、足した線と HANDSEED が合い、元の要素の ID と形は変わらない
#[test]
fn lines_crossing_two_byte_handles_are_written_back_to_autocad_dwgs() {
    // R2000・R2004 の書き直しは、合成の DWG で確かめる（tests/dwg.rs）。ここでは、#701 で断っていた数（64,353 本）を足す
    let version = "r2018";
    {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/dwg")
            .join(format!("zumen-layout-{version}.dwg"));
        let src = std::fs::read(&path).unwrap();
        let mut file = read_dwg(&src);
        let seed = file.handle_seed().unwrap();
        assert!(seed < 0x1_0000, "{version} {seed:#x}");
        let before = lift_dwg(&file);
        let layer = before.spaces[0].elements[0].layer;
        let template = before.spaces[0].elements[0].style;
        // HANDSEED を 0x10000（3 バイト）に進める数
        let count = 0x1_0000 - seed;
        let line = |i: u64| Element {
            source: zumen_base::Ref::default(),
            layer,
            style: template,
            shape: Shape::Line {
                start: ModelPoint {
                    space: before.spaces[0].id,
                    x: 0.0,
                    y: i as f64,
                },
                end: ModelPoint {
                    space: before.spaces[0].id,
                    x: 10.0,
                    y: i as f64,
                },
            },
        };
        let edits = (0..count).map(|i| add_to_model(line(i))).collect();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let result = file.apply(
            &Changes {
                document: None,
                edits,
            },
            &mut cx,
        );
        assert!(
            result.is_ok(),
            "{version}: {:?}",
            diag.iter().map(|d| d.code()).collect::<Vec<_>>()
        );
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, &mut cx).unwrap();

        let again = read_dwg(&out);
        assert_eq!(again.handle_seed(), Some(seed + count), "{version}");
        let after = lift_dwg(&again);
        let (old, new) = after.spaces[0]
            .elements
            .split_at(before.spaces[0].elements.len());
        // 図形の並びをたどるハンドルを直したレコードは、置く所（バイト範囲）だけが変わる
        let without_bytes = |elements: &[Element<ModelPoint>]| {
            elements
                .iter()
                .map(|e| {
                    let mut e = e.clone();
                    let key = e.source.key();
                    e.source = zumen_base::Ref::default();
                    (key, e)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            without_bytes(old),
            without_bytes(&before.spaces[0].elements),
            "{version}"
        );
        assert_eq!(
            model_ids(&after)[..old.len()],
            model_ids(&before)[..],
            "{version}"
        );
        assert_eq!(new.len() as u64, count, "{version}");
        for (i, element) in new.iter().enumerate() {
            assert_eq!(element.shape, line(i as u64).shape, "{version} {i}");
        }
        assert_eq!(count, 64_353);
    }
}
