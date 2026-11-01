//! 追加距離での位置・方向角・高さ（決定 0230）。

use crate::{Alignment, Horizontal, HorizontalKind, TransitionCurve, Vertical, VerticalKind};
use zumen_base::{Cx, Diagnostic, codes};
use zumen_crs::MapPoint;
use zumen_geom2d::Ramp;

/// 線形の上の 1 つの所
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// 平面の点
    pub point: MapPoint,
    /// 接線の方向角（北から時計回りのラジアン）
    pub direction: f64,
    /// 縦断線形の高さ（メートル）。縦断線形がないか、範囲の外なら `None`
    pub height: Option<f64>,
    /// 縦断線形の勾配。高さと同じく、なければ `None`
    pub gradient: Option<f64>,
}

/// 追加距離 `distance` での位置。平面線形の範囲の外か、区間の値が使えないか、曲線の種類から計算できなければ、
/// 理由の診断を返す。縦断線形の高さを計算できなければ、高さを `None` にし、理由を `cx` に知らせる
pub fn place(
    alignment: &Alignment,
    distance: f64,
    cx: &mut Cx<'_>,
) -> Result<Placement, Box<Diagnostic>> {
    let (index, segment, s) = segment_at(&alignment.horizontal, distance)?;
    let (curvatures, ramp) = curvature(index, segment)?;
    let ([northing, easting], direction) = zumen_geom2d::along(
        [segment.start.northing, segment.start.easting],
        segment.direction,
        curvatures,
        segment.length,
        ramp,
        s,
    );
    let profile = if alignment.vertical.is_empty() {
        None
    } else {
        match height(&alignment.vertical, distance) {
            Ok(Some(found)) => Some(found),
            Ok(None) => {
                cx.report(Diagnostic::new(codes::CIVIL_PROFILE_MISSING).with("distance", distance));
                None
            }
            Err(diagnostic) => {
                cx.report(*diagnostic);
                None
            }
        }
    };
    Ok(Placement {
        point: MapPoint { northing, easting },
        direction,
        height: profile.map(|(height, _)| height),
        gradient: profile.map(|(_, gradient)| gradient),
    })
}

/// 区間の初めの点が、前の区間の終わりから計算した点と `tolerance`（メートル）より離れていれば、知らせる。
/// 計算できない区間の次は確かめない
pub fn check(alignment: &Alignment, tolerance: f64, cx: &mut Cx<'_>) {
    for (index, pair) in alignment.horizontal.windows(2).enumerate() {
        let [previous, next] = pair else { continue };
        let Ok((curvatures, ramp)) = curvature(index, previous) else {
            continue;
        };
        let ([northing, easting], _) = zumen_geom2d::along(
            [previous.start.northing, previous.start.easting],
            previous.direction,
            curvatures,
            previous.length,
            ramp,
            previous.length,
        );
        let gap = libm::hypot(next.start.northing - northing, next.start.easting - easting);
        if gap > tolerance {
            cx.report(
                Diagnostic::new(codes::CIVIL_SEGMENT_START_INVALID)
                    .with("index", (index + 1) as u64)
                    .with("gap", gap)
                    .with("tolerance", tolerance),
            );
        }
    }
}

/// 平面線形の長さ
pub fn length(alignment: &Alignment) -> f64 {
    alignment
        .horizontal
        .iter()
        .map(|segment| segment.length)
        .sum()
}

/// 追加距離を含む区間と、その区間の初めからの長さ。区間の境の所は前の区間にする
fn segment_at(
    segments: &[Horizontal],
    distance: f64,
) -> Result<(usize, &Horizontal, f64), Box<Diagnostic>> {
    let total: f64 = segments.iter().map(|segment| segment.length).sum();
    let outside = || {
        Box::new(
            Diagnostic::new(codes::CIVIL_DISTANCE_MISSING)
                .with("distance", distance)
                .with("length", total),
        )
    };
    if !(distance.is_finite() && distance >= 0.0 && distance <= total) {
        return Err(outside());
    }
    let mut start = 0.0;
    for (index, segment) in segments.iter().enumerate() {
        if !(segment.length.is_finite() && segment.length > 0.0) {
            return Err(invalid(index, "length"));
        }
        let end = start + segment.length;
        if distance <= end || index + 1 == segments.len() {
            return Ok((index, segment, (distance - start).min(segment.length)));
        }
        start = end;
    }
    Err(outside())
}

/// 区間（`index` は診断に書く番号）の初めと終わりの曲率と、緩和曲線の曲率の変わり方。曲率は、右へ曲がる
/// ものを正にする。値が使えないか、曲線の種類から計算できなければ、理由の診断を返す
pub fn curvature(
    index: usize,
    segment: &Horizontal,
) -> Result<([f64; 2], Option<Ramp>), Box<Diagnostic>> {
    let inverse = |radius: Option<f64>, field: &'static str| match radius {
        None => Ok(0.0),
        Some(radius) if radius.is_finite() && radius != 0.0 => Ok(1.0 / radius),
        Some(_) => Err(invalid(index, field)),
    };
    match &segment.kind {
        HorizontalKind::Line => Ok(([0.0, 0.0], None)),
        HorizontalKind::Arc { radius } => {
            let k = inverse(Some(*radius), "radius")?;
            Ok(([k, k], None))
        }
        HorizontalKind::Transition {
            start_radius,
            end_radius,
            curve,
        } => {
            let ramp = match curve {
                TransitionCurve::Clothoid => Ramp::Clothoid,
                TransitionCurve::Bloss => Ramp::Bloss,
                TransitionCurve::Cosine => Ramp::Cosine,
                TransitionCurve::SineHalfWave => Ramp::SineHalfWave,
                TransitionCurve::Helmert => Ramp::Helmert,
                TransitionCurve::CubicParabola => return Err(unsupported(index, "cubic_parabola")),
                TransitionCurve::Viennese => return Err(unsupported(index, "viennese")),
                TransitionCurve::Other(name) => return Err(unsupported(index, name)),
            };
            let k1 = inverse(*start_radius, "start_radius")?;
            let k2 = inverse(*end_radius, "end_radius")?;
            Ok(([k1, k2], Some(ramp)))
        }
    }
}

/// 縦断線形の高さと勾配。範囲の外なら `None`
fn height(segments: &[Vertical], distance: f64) -> Result<Option<(f64, f64)>, Box<Diagnostic>> {
    for (index, segment) in segments.iter().enumerate() {
        if !(segment.length.is_finite() && segment.length > 0.0) {
            return Err(invalid(index, "length"));
        }
        let u = distance - segment.start_distance;
        if !(0.0..=segment.length).contains(&u) {
            continue;
        }
        let (h0, g0) = (segment.start_height, segment.start_gradient);
        return match segment.kind {
            VerticalKind::ConstantGradient => Ok(Some((h0 + g0 * u, g0))),
            VerticalKind::Parabola { end_gradient } => {
                let change = (end_gradient - g0) / segment.length;
                Ok(Some((h0 + g0 * u + change * u * u / 2.0, g0 + change * u)))
            }
            VerticalKind::CircularArc { .. } => Err(unsupported(index, "circular_arc")),
            VerticalKind::Clothoid { .. } => Err(unsupported(index, "clothoid")),
        };
    }
    Ok(None)
}

fn invalid(index: usize, field: &'static str) -> Box<Diagnostic> {
    Box::new(
        Diagnostic::new(codes::CIVIL_SEGMENT_INVALID)
            .with("index", index as u64)
            .with("field", field),
    )
}

fn unsupported(index: usize, curve: &str) -> Box<Diagnostic> {
    Box::new(
        Diagnostic::new(codes::CIVIL_CURVE_UNSUPPORTED)
            .with("index", index as u64)
            .with("curve", curve.to_owned()),
    )
}
