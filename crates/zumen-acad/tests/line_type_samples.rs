//! 試料の DXF・DWG で、線種の列の要素に添える形と文字を、線種の定義に移す（決定 0150）。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。
#![cfg(feature = "drawing")]

mod support;

use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_drawing::{Drawing2D, Font, FontKind, LineTypeMark, MarkContent, MarkRotation};
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

#[cfg(test)]
fn marks<'a>(drawing: &'a Drawing2D, name: &str) -> &'a [LineTypeMark] {
    drawing
        .line_types
        .iter()
        .find(|line_type| line_type.name == name)
        .and_then(|line_type| line_type.definition.as_ref())
        .map(|definition| definition.marks.as_slice())
        .unwrap_or_default()
}

#[cfg(test)]
fn shape(number: u16) -> MarkContent {
    MarkContent::Shape {
        font: Some(Font {
            name: "ltypeshp.shx".to_owned(),
            kind: FontKind::Shape,
        }),
        number,
    }
}

/// 試料の `sample_AC10*` の図面（線種の要素を持たない R12 の AC1009 を除く）は、文字を添える GAS_LINE と、
/// 形を添える TRACKS・ZIGZAG を持つ。どの版の DWG・DXF でも、同じ形と文字になる。R2013 からは、文字に
/// 逆さにしない印（74 の 8）が付く
#[test]
fn shapes_and_text_of_line_types_become_marks() {
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some((drawing, diag)) = lift(&path) else {
            continue;
        };
        let upright = name.starts_with("sample_AC1027") || name.starts_with("sample_AC1032");
        assert_eq!(
            marks(&drawing, "GAS_LINE"),
            [LineTypeMark {
                index: 1,
                content: MarkContent::Text {
                    content: "GAS".to_owned(),
                    style: Some("Standard".to_owned()),
                },
                scale: 0.1,
                rotation: 0.0,
                rotation_basis: if upright {
                    MarkRotation::Upright
                } else {
                    MarkRotation::Relative
                },
                offset: [-0.1, -0.05],
            }],
            "{name}"
        );
        // 文字の高さ。Standard は高さを決めないので、倍率が高さになる（決定 0166）
        let gas = &marks(&drawing, "GAS_LINE")[0];
        let standard = drawing.text_style("Standard").unwrap().definition.as_ref();
        assert_eq!(
            standard.map(|definition| definition.height),
            Some(None),
            "{name}"
        );
        assert_eq!(drawing.mark_text_height(gas), Some(0.1), "{name}");
        assert_eq!(
            marks(&drawing, "TRACKS"),
            [LineTypeMark {
                index: 0,
                content: shape(130),
                scale: 0.25,
                rotation: 0.0,
                rotation_basis: MarkRotation::Relative,
                offset: [0.0, 0.0],
            }],
            "{name}"
        );
        let zigzag: Vec<_> = marks(&drawing, "ZIGZAG")
            .iter()
            .map(|mark| (mark.index, &mark.content, mark.rotation, mark.offset))
            .collect();
        assert_eq!(
            zigzag,
            [
                (1, &shape(131), 0.0, [-0.2, 0.0]),
                (2, &shape(131), 180.0, [0.2, 0.0])
            ],
            "{name}"
        );
        assert!(
            diag.iter()
                .all(|d| d.code() != codes::DXF_LINE_TYPE_ELEMENT_DROPPED),
            "{name}"
        );
        checked += 1;
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(checked == 0 || checked >= 11, "{checked}");
}

/// 形の番号が 0 で、形のファイルを指さない要素（74 の 4）は、形を添えない。読めない要素ともしない
#[test]
fn shapes_without_a_number_are_not_marks() {
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("jp-yokosuka-minami-survey") {
            continue;
        }
        let Some((drawing, diag)) = lift(&path) else {
            continue;
        };
        for line_type in &drawing.line_types {
            let marks = line_type.definition.as_ref().map(|d| d.marks.len());
            assert!(matches!(marks, None | Some(0)), "{name} {}", line_type.name);
        }
        assert!(
            diag.iter()
                .all(|d| d.code() != codes::DXF_LINE_TYPE_ELEMENT_DROPPED),
            "{name}"
        );
    }
}
