//! 試料の DXF・DWG で、MULTILEADER の線の色・線種・線幅と注記の色を移す（決定 0146）。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。
#![cfg(feature = "drawing")]

mod support;

use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{
    CalloutContent, Color, ColorCode, ColorSystem, Drawing2D, Inherit, LineTypeId, LineWeight,
    PaperLength, Shape, Style,
};
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

/// モデル空間の引出線つきの注記の、線の指定と注記の色と、注記が記号か
#[cfg(test)]
fn callouts(drawing: &Drawing2D) -> Vec<(Style, Inherit<Color>, bool)> {
    drawing
        .spaces
        .iter()
        .flat_map(|space| &space.elements)
        .filter_map(|element| match &element.shape {
            Shape::Callout(callout) => Some((
                callout.line_style,
                callout.content_color,
                matches!(callout.content, CalloutContent::Symbol { .. }),
            )),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
fn aci(value: u32) -> Inherit<Color> {
    Inherit::Own(Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value,
        }),
        rgb: None,
    })
}

/// 試料の `made-mleader-*.dxf` は、線の色（色の番号 1・RGB・レイヤ）、線種（表の線種・ByLayer）、線幅
/// （0.35 mm）、文字の色（色の番号 6）、記号の色（色の番号 5）を明示した注記を持つ。記号の注記は属性の値を持ち、
/// 図形の所有者（330）を属性の定義と取り違えずに移す
#[test]
fn callouts_keep_the_style_of_their_lines_and_content() {
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("made-mleader-") {
            continue;
        }
        let drawing = lift(&path).unwrap();
        let dashed = drawing
            .line_types
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case("DASHED"))
            .map(|t| t.id)
            .unwrap();
        let style = |color: Inherit<Color>,
                     line_type: Inherit<LineTypeId>,
                     line_weight: Inherit<LineWeight>| Style {
            color,
            line_type,
            line_type_scale: 1.0,
            line_weight: Some(line_weight),
            transparency: Inherit::Block,
        };
        let rgb = Inherit::Own(Color {
            code: None,
            rgb: Some([255, 128, 0]),
        });
        let width = Inherit::Own(LineWeight::Width(PaperLength(0.35)));
        assert_eq!(
            callouts(&drawing),
            [
                (
                    style(Inherit::Block, Inherit::Layer, Inherit::Block),
                    Inherit::Block,
                    false
                ),
                (style(aci(1), Inherit::Block, width), Inherit::Block, false),
                (
                    style(Inherit::Block, Inherit::Own(dashed), Inherit::Block),
                    aci(6),
                    false
                ),
                (
                    style(Inherit::Block, Inherit::Layer, Inherit::Block),
                    Inherit::Block,
                    false
                ),
                (
                    style(Inherit::Layer, Inherit::Layer, Inherit::Block),
                    Inherit::Block,
                    false
                ),
                (style(rgb, Inherit::Block, Inherit::Block), aci(5), true),
                (
                    style(Inherit::Block, Inherit::Block, Inherit::Block),
                    Inherit::Block,
                    false
                ),
                (
                    style(Inherit::Block, Inherit::Layer, Inherit::Block),
                    Inherit::Block,
                    false
                ),
            ],
            "{name}"
        );
        checked += 1;
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(checked == 0 || checked >= 2, "{checked}");
}

/// 試料の `sample_AC10*` の図面（R12 の AC1009 を除く）の注記は、線も注記も注記の要素に従う。DWG と DXF で
/// 同じになる
#[test]
fn dwg_and_dxf_callouts_follow_the_callout() {
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some(drawing) = lift(&path) else { continue };
        let found = callouts(&drawing);
        assert!(!found.is_empty(), "{name}");
        for (style, content, _) in found {
            assert_eq!((style, content), (Style::BLOCK, Inherit::Block), "{name}");
        }
        checked += 1;
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(checked == 0 || checked >= 11, "{checked}");
}
