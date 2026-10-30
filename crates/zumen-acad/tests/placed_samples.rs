//! 試料の DXF で、モデル空間に置いたブロックの中の図形の継承を、配置で解く（決定 0143）。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。

mod support;

use std::path::PathBuf;
use zumen_acad::Dxf;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Drawing2D, Inherit, Inheritor, Shape};
use zumen_sdk::{Codec, Lift};

/// フォルダーの中の DXF
#[cfg(test)]
fn dxfs() -> Vec<PathBuf> {
    let mut found: Vec<_> = support::sample_files()
        .into_iter()
        .filter(|path| path.extension().is_some_and(|e| e == "dxf"))
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
    Dxf::read(&src, &mut cx).ok()?.lift(&mut cx).ok()
}

/// 配置の色がブロックから継承しなければ、配置の中のブロックから継承する図形の色は配置で解ける。
/// レイヤ 0 の図形のレイヤからの継承は、配置のレイヤで解く
#[test]
fn byblock_in_placed_blocks_is_resolved_by_the_placement() {
    let paths = dxfs();
    let (mut byblock, mut on_zero) = (0, 0);
    for path in &paths {
        let Some(drawing) = lift(path) else { continue };
        for placement in drawing.spaces.iter().flat_map(|s| &s.elements) {
            let Shape::Symbol(symbol) = &placement.shape else {
                continue;
            };
            let Some(definition) = drawing.symbol(&symbol.key) else {
                continue;
            };
            let outer = drawing.color_of(placement);
            for element in &definition.space.elements {
                let chain = [Inheritor::from(element), Inheritor::from(placement)];
                let placed = drawing.color_placed(&chain);
                if element.style.color == Inherit::Block {
                    byblock += 1;
                    assert_eq!(placed, outer, "{}", path.display());
                }
                let zero = drawing
                    .layer(element.layer)
                    .is_some_and(|layer| layer.follows_placement);
                if zero && element.style.color == Inherit::Layer {
                    on_zero += 1;
                    let by_placement_layer = drawing.color_placed(&[Inheritor {
                        style: &element.style,
                        layer: placement.layer,
                    }]);
                    assert_eq!(placed, by_placement_layer, "{}", path.display());
                }
            }
        }
    }
    // 試料には、どちらも千件ほどある
    assert!(!paths.iter().any(|path| support::received(path)) || (byblock > 0 && on_zero > 0));
}
