//! 試料の DXF と DWG の寸法の、保存された測定値（DXF の 42）を、定義点から測った値と
//! 比べる。長さは図面の単位、角度はラジアンであることを確かめる（docs/formats/dxf.md の推定した扱い）。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。
#![cfg(feature = "drawing")]

mod support;

use std::f64::consts::PI;
use std::path::PathBuf;
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Axis, DimensionKind, Drawing2D, ModelPoint, Shape};
use zumen_sdk::{Codec, Lift};

/// フォルダーの中の DXF と、読める版の DWG
#[cfg(test)]
fn drawings() -> Vec<PathBuf> {
    let mut found: Vec<_> = support::sample_files()
        .into_iter()
        .filter(|path| {
            let Some(extension) = path.extension() else {
                return false;
            };
            let src = std::fs::read(path).unwrap();
            if extension == "dwg" {
                [b"AC1015", b"AC1018", b"AC1024", b"AC1027", b"AC1032"]
                    .iter()
                    .any(|version| src.starts_with(*version))
            } else {
                extension == "dxf"
            }
        })
        .collect();
    found.sort();
    found
}

/// 図面モデルへ移したもの。読めないか移せない図面は `None`
#[cfg(test)]
fn lifted(path: &PathBuf) -> Option<Drawing2D> {
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

/// 2 点の距離
#[cfg(test)]
fn distance(a: &ModelPoint, b: &ModelPoint) -> f64 {
    libm::hypot(b.x - a.x, b.y - a.y)
}

/// `a` と `b` の向きのなす角度（0 から π のラジアン）
#[cfg(test)]
fn between(a: (f64, f64), b: (f64, f64)) -> f64 {
    libm::atan2(a.0 * b.1 - a.1 * b.0, a.0 * b.0 + a.1 * b.1).abs()
}

/// `from` から `to` への向き
#[cfg(test)]
fn toward(from: &ModelPoint, to: &ModelPoint) -> (f64, f64) {
    (to.x - from.x, to.y - from.y)
}

/// 定義点から測った値。2 本の線の角度は、寸法の弧の点で、なす角度とその補角のどちらかが決まるので、
/// どちらも返す。円弧の長さは、両端の間で寸法の弧の点の側の弧の長さとする
#[cfg(test)]
fn measured(kind: &DimensionKind<ModelPoint>) -> Vec<f64> {
    match kind {
        DimensionKind::Linear {
            first,
            second,
            rotation,
            ..
        } => {
            let (x, y) = toward(first, second);
            let r = rotation.to_radians();
            vec![(x * libm::cos(r) + y * libm::sin(r)).abs()]
        }
        DimensionKind::Aligned { first, second, .. } => vec![distance(first, second)],
        DimensionKind::Angular {
            vertex,
            first,
            second,
            ..
        } => vec![between(toward(vertex, first), toward(vertex, second))],
        DimensionKind::AngularLines { first, second, .. } => {
            let [a, b] = first;
            let [c, d] = second;
            let angle = between(toward(a, b), toward(c, d));
            vec![angle, PI - angle]
        }
        DimensionKind::Radius { center, chord, .. } => vec![distance(center, chord)],
        DimensionKind::Diameter {
            chord, opposite, ..
        } => vec![distance(chord, opposite)],
        DimensionKind::Ordinate {
            origin,
            feature,
            axis,
            ..
        } => vec![match axis {
            Axis::X => (feature.x - origin.x).abs(),
            Axis::Y => (feature.y - origin.y).abs(),
        }],
        DimensionKind::ArcLength {
            center,
            first,
            second,
            arc,
        } => {
            // 反時計回りの角度（0 から 2π）
            let sweep = |a: (f64, f64), b: (f64, f64)| {
                let angle = libm::atan2(a.0 * b.1 - a.1 * b.0, a.0 * b.0 + a.1 * b.1);
                if angle < 0.0 { angle + 2.0 * PI } else { angle }
            };
            let (a, b, c) = (
                toward(center, first),
                toward(center, second),
                toward(center, arc),
            );
            let angle = sweep(a, b);
            let angle = if sweep(a, c) <= angle {
                angle
            } else {
                2.0 * PI - angle
            };
            vec![distance(center, first) * angle]
        }
    }
}

/// 保存された測定値は、最後に寸法を描いたときの値で、後で定義点を動かしたり、寸法でないものと一緒に
/// 尺度を変えたりすると、定義点から測った値と違うことがある（試料の farmhouse.dwg は 0.99985 倍、
/// mechanical_example-imperial.dwg は 17 のうち 9 が違う）。そこで、どの図面も 1 つ以上の寸法が測った値と
/// 合うことと、同じ図面の DWG と DXF の組（`sample_` で始まるもの）はすべてが合うこと、角度の寸法が
/// ラジアンで合うことを確かめる
#[test]
fn stored_dimension_measurements_are_lengths_in_drawing_units_and_angles_in_radians() {
    let mut angles = 0;
    for path in drawings() {
        let Some(drawing) = lifted(&path) else {
            continue;
        };
        let strict = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("sample_"));
        let (mut checked, mut matched) = (0, 0);
        for element in drawing.spaces.iter().flat_map(|space| &space.elements) {
            let Shape::Dimension(dimension) = &element.shape else {
                continue;
            };
            let Some(stored) = dimension.measurement else {
                continue;
            };
            let candidates = measured(&dimension.kind);
            if candidates.is_empty() {
                continue;
            }
            checked += 1;
            let exact = candidates
                .iter()
                .any(|value| (value - stored.value).abs() <= 1e-9 * stored.value.abs().max(1.0));
            assert!(
                exact || !strict,
                "{} {:?}: {} {candidates:?}",
                path.display(),
                dimension.kind,
                stored.value
            );
            if exact {
                matched += 1;
                if matches!(
                    dimension.kind,
                    DimensionKind::Angular { .. } | DimensionKind::AngularLines { .. }
                ) {
                    angles += 1;
                }
            }
        }
        assert!(checked == 0 || matched > 0, "{}", path.display());
    }
    // 角度の寸法を持つ試料があれば、ラジアンで合ったものがある
    let any = drawings().iter().any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("sample_"))
    });
    assert!(!any || angles > 0);
}
