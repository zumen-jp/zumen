//! 公開レイヤ ID による除外は、SVG・PNG とも描画単位に適用する。
mod fixtures;

use fixtures::{Outcome, mixed, options, pixels};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_render::Error;
use zumen_scene::layout as l;

#[allow(clippy::unwrap_used)] // 合成シーンの列がなければテストを失敗させる。
fn layered(layers: &[u32]) -> Vec<u8> {
    let mut scene = mixed();
    scene.0.get_mut(l::UNIT_LAYERS).unwrap().clear();
    scene.u(l::UNIT_LAYERS, layers);
    scene.string(l::LAYER_IDS, "layer/hidden");
    scene.string(l::LAYER_IDS, "layer/hidden");
    scene.pack()
}

fn run(png: bool, bytes: &[u8], hidden: &[&str], limits: Limits, cancelled: bool) -> Outcome {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    if cancelled {
        cancel.cancel();
    }
    let render = if png {
        zumen_render::png_with_hidden_layers
    } else {
        zumen_render::svg_with_hidden_layers
    };
    let result = render(
        bytes,
        &options(),
        &hidden.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>(),
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    );
    (
        result,
        diagnostics.into_vec(),
        Kind::ALL
            .iter()
            .map(|&kind| (kind, budget.used(kind)))
            .collect(),
    )
}

#[test]
fn empty_filter_preserves_bytes_diagnostics_and_budget() {
    let input = layered(&[0; 8]);
    for png in [false, true] {
        let original = if png {
            fixtures::run_png
        } else {
            fixtures::run
        };
        assert_eq!(
            original(&input, &options(), Limits::default(), false),
            run(png, &input, &[], Limits::default(), false)
        );
    }
}

#[test]
fn hidden_units_produce_no_paint_or_render_diagnostic() {
    // 同じ公開 ID の行をすべて除く。非表示の未描画の参照も診断しない。
    let input = layered(&[0, 1, 0, 1, 0, 1, 0, 1]);
    for png in [false, true] {
        let (result, diagnostics, _) =
            run(png, &input, &["layer/hidden"], Limits::default(), false);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let bytes = result.unwrap();
        if png {
            assert!(pixels(&bytes).2.iter().all(|&v| v == 255));
        } else {
            let svg = std::str::from_utf8(&bytes).unwrap();
            for color in ["#ff0000", "#0000ff", "#00a000", "#ff8000"] {
                assert!(!svg.contains(color), "{svg}");
            }
        }
        let repeated = run(
            png,
            &input,
            &["layer/hidden", "layer/hidden"],
            Limits::default(),
            false,
        );
        assert_eq!(repeated.0.unwrap(), bytes);
        assert_eq!(repeated.1, diagnostics);
    }
}

#[test]
fn units_without_a_layer_remain_visible() {
    let input = layered(&[0, l::NONE, 0, 0, 0, 0, 0, 0]);
    let (svg, diagnostics, _) = run(false, &input, &["layer/hidden"], Limits::default(), false);
    assert!(
        std::str::from_utf8(&svg.unwrap())
            .unwrap()
            .contains("#0000ff")
    );
    assert!(diagnostics.is_empty());
    let (png, diagnostics, _) = run(true, &input, &["layer/hidden"], Limits::default(), false);
    let (width, _, rgba) = pixels(&png.unwrap());
    assert_eq!(
        &rgba[(50 * width + 50) * 4..(50 * width + 51) * 4],
        &[0, 0, 255, 255]
    );
    assert_eq!(&rgba[..4], &[255; 4]);
    assert!(diagnostics.is_empty());
}

#[test]
fn unknown_layers_stop_without_an_image_and_retry_is_identical() {
    let input = layered(&[0; 8]);
    for png in [false, true] {
        let baseline = run(png, &input, &["layer/hidden"], Limits::default(), false);
        let bad = run(png, &input, &["layer/missing"], Limits::default(), false);
        assert!(matches!(bad.0, Err(Error::Options)));
        assert_eq!(bad.1.len(), 1);
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let stopped = run(
                png,
                &input,
                &["layer/hidden"],
                Limits::default().with(kind, Limit::Finite(1)),
                false,
            );
            assert!(matches!(stopped.0, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind));
        }
        assert!(matches!(
            run(png, &input, &["layer/hidden"], Limits::default(), true).0,
            Err(Error::Stopped(Stop::Cancelled))
        ));
        assert_eq!(
            run(png, &input, &["layer/hidden"], Limits::default(), false),
            baseline
        );
    }
}
