//! 試料の DXF と DWG を、配置をたどって読む（決定 0159）。
//! 配置の中の要素の色・線種・線幅が、決定 0143 の関数で配置を通して解いた値と同じになる。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。
#![cfg(feature = "drawing")]

mod support;

use std::collections::BTreeMap;
use std::path::PathBuf;
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{
    Drawing2D, Element, Inherit, Inheritor, Item, LineWeight, ModelPoint, Origin, Shape,
};
use zumen_sdk::{Codec, Lift};

/// フォルダーの中の DXF と DWG
#[cfg(test)]
fn samples() -> Vec<PathBuf> {
    let mut found: Vec<_> = support::sample_files()
        .into_iter()
        .filter(|path| path.extension().is_some_and(|e| e == "dxf" || e == "dwg"))
        .filter(|path| !path.to_string_lossy().ends_with("_binary.dxf"))
        .collect();
    found.sort();
    found
}

#[cfg(test)]
fn lift(path: &PathBuf) -> Option<Drawing2D> {
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

/// 一段の配置の中の要素は、要素と配置の並びを 0143 の関数で解き、残った継承を文書の既定で解いた値になる。
/// 用紙からもたどれる
#[test]
fn walking_placements_matches_the_placed_functions() {
    let paths = samples();
    let (mut drawings, mut inside) = (0, 0);
    for path in &paths {
        let Some(drawing) = lift(path) else { continue };
        drawings += 1;
        let name = path.display();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        for space in &drawing.spaces {
            let visits = drawing
                .walk(Origin::Space(space.id), &mut cx)
                .unwrap_or_else(|stop| panic!("{name}: {stop:?}"));
            let placements: BTreeMap<&str, &Element<ModelPoint>> = visits
                .iter()
                .filter(|visit| visit.path.is_empty())
                .filter_map(|visit| match visit.item {
                    Item::Model(element) => Some((visit.id.as_str(), element)),
                    _ => None,
                })
                .filter(|(_, element)| matches!(element.shape, Shape::Symbol(_)))
                .collect();
            for visit in visits.iter().filter(|visit| visit.path.len() == 1) {
                let Item::Model(element) = visit.item else {
                    continue;
                };
                let placement = placements[visit.path[0].id.as_str()];
                let chain = [Inheritor::from(element), Inheritor::from(placement)];
                let color = match drawing.color_placed(&chain) {
                    Inherit::Own(color) => Some(color),
                    Inherit::Block => drawing.default_color,
                    Inherit::Layer => None,
                };
                let line_type = match drawing.line_type_placed(&chain) {
                    Inherit::Own(line_type) => Some(line_type),
                    Inherit::Block => drawing.default_line_type,
                    Inherit::Layer => None,
                };
                let line_weight = match drawing.line_weight_placed(&chain) {
                    Some(Inherit::Own(LineWeight::Width(width))) => Some(width),
                    Some(Inherit::Own(LineWeight::Default) | Inherit::Block) => {
                        drawing.default_line_weight
                    }
                    Some(Inherit::Layer) | None => None,
                };
                let zero = drawing
                    .layer(element.layer)
                    .is_some_and(|layer| layer.follows_placement);
                let layer = if zero { placement.layer } else { element.layer };
                assert_eq!(visit.color, color, "{name}: {}", visit.id);
                assert_eq!(visit.line_type, line_type, "{name}: {}", visit.id);
                assert_eq!(visit.line_weight, line_weight, "{name}: {}", visit.id);
                assert_eq!(visit.layer, layer, "{name}: {}", visit.id);
                inside += 1;
            }
        }
        for sheet in &drawing.sheets {
            drawing
                .walk(Origin::Sheet(sheet.id), &mut cx)
                .unwrap_or_else(|stop| panic!("{name}: {stop:?}"));
        }
    }
    // 試料では、34 枚の図面の配置の中に 7 千ほどの要素がある
    assert!(!paths.iter().any(|path| support::received(path)) || (drawings > 0 && inside > 0));
}
