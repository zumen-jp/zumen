//! 追加距離での位置・方向角・高さ（決定 0230）。

use crate::{Alignment, Horizontal, HorizontalKind, TransitionCurve, Vertical, VerticalKind};
use zumen_base::{Cx, Diagnostic, Kind, Stop, codes};
use zumen_crs::MapPoint;
use zumen_geom2d::Ramp;

/// 求めた値か、求められない理由の診断
pub type Found<T> = Result<T, Box<Diagnostic>>;

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
/// 理由の診断を返す。縦断線形の高さを計算できなければ、高さを `None` にし、理由を `cx` に知らせる。
///
/// 区間の上の点と高さを求める手間は、[`place_within`] と同じく予算に数え、予算で止まれば、止まった理由の診断を
/// 返す。区間をたどる回数（[`place_work`]）は、呼ぶ側が数える
pub fn place(
    alignment: &Alignment,
    distance: f64,
    cx: &mut Cx<'_>,
) -> Result<Placement, Box<Diagnostic>> {
    place_within(alignment, distance, cx).unwrap_or_else(|stop| Err(Box::new(stop.to_diagnostic())))
}

/// [`place`] と同じく位置を求める。区間の上の点と高さを求める手間（[`zumen_geom2d::along_work`] と、縦断の
/// クロソイドの反復）は、求める前に予算の作業（`Kind::Work`）に数え、予算で止まれば `Err` を返す（#727）。
/// 区間を初めからたどる回数は数えないので、呼ぶ側がその回数（[`place_work`]）を数える
pub fn place_within(
    alignment: &Alignment,
    distance: f64,
    cx: &mut Cx<'_>,
) -> Result<Found<Placement>, Stop> {
    let (index, segment, s) = match segment_at(&alignment.horizontal, distance) {
        Ok(found) => found,
        Err(diagnostic) => return Ok(Err(diagnostic)),
    };
    let (curvatures, ramp) = match curvature(index, segment) {
        Ok(found) => found,
        Err(diagnostic) => return Ok(Err(diagnostic)),
    };
    cx.check(
        Kind::Work,
        zumen_geom2d::along_work(curvatures, segment.length, ramp, s),
    )?;
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
        match height(&alignment.vertical, distance, cx)? {
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
    Ok(Ok(Placement {
        point: MapPoint { northing, easting },
        direction,
        height: profile.map(|(height, _)| height),
        gradient: profile.map(|(_, gradient)| gradient),
    }))
}

/// 区間の初めの点が、前の区間の終わりから計算した点と `tolerance`（メートル）より離れていれば、知らせる。
/// 計算できない区間の次は確かめない
pub fn check(alignment: &Alignment, tolerance: f64, cx: &mut Cx<'_>) {
    if let Err(stop) = check_within(alignment, tolerance, cx) {
        cx.report(stop.to_diagnostic());
    }
}

/// [`check`] と同じく確かめる。区間の終わりの点を求める手間（[`zumen_geom2d::along_work`]）は、求める前に予算の
/// 作業に数え、予算で止まれば `Err` を返す（#727）
pub fn check_within(alignment: &Alignment, tolerance: f64, cx: &mut Cx<'_>) -> Result<(), Stop> {
    for (index, pair) in alignment.horizontal.windows(2).enumerate() {
        let [previous, next] = pair else { continue };
        let Ok((curvatures, ramp)) = curvature(index, previous) else {
            continue;
        };
        cx.check(
            Kind::Work,
            zumen_geom2d::along_work(curvatures, previous.length, ramp, previous.length),
        )?;
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
    Ok(())
}

/// [`place`] を 1 回呼ぶときに区間をたどる回数の上限。予算の作業（`Kind::Work`）に数える量で、平面線形の区間を
/// 2 回（長さを足すのと、区間を探すの）と、縦断線形の区間を 1 回たどる（#560）。区間の上の点と高さを求める
/// 計算は含まず、[`place`] が求める前に数える（#727）
pub fn place_work(alignment: &Alignment) -> u64 {
    let count = |len: usize| u64::try_from(len).unwrap_or(u64::MAX);
    count(alignment.horizontal.len())
        .saturating_mul(2)
        .saturating_add(count(alignment.vertical.len()))
        .saturating_add(1)
}

/// 平面線形の長さ。区間をすべて足すので、作業は平面線形の区間の数に比例する
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
                // 3 次放物線は、直線と円弧の間だけを計算する
                TransitionCurve::CubicParabola
                    if start_radius.is_some() == end_radius.is_some() =>
                {
                    return Err(unsupported(index, "cubic_parabola"));
                }
                TransitionCurve::CubicParabola => Ramp::CubicParabola,
                TransitionCurve::Viennese => Ramp::Viennese,
                TransitionCurve::Other(name) => return Err(unsupported(index, name)),
            };
            let k1 = inverse(*start_radius, "start_radius")?;
            let k2 = inverse(*end_radius, "end_radius")?;
            Ok(([k1, k2], Some(ramp)))
        }
    }
}

/// 縦断線形の高さと勾配。範囲の外なら `None`。求める手間を予算に数える
fn height(
    segments: &[Vertical],
    distance: f64,
    cx: &mut Cx<'_>,
) -> Result<Found<Option<(f64, f64)>>, Stop> {
    for (index, segment) in segments.iter().enumerate() {
        if !(segment.length.is_finite() && segment.length > 0.0) {
            return Ok(Err(invalid(index, "length")));
        }
        let u = distance - segment.start_distance;
        if !(0.0..=segment.length).contains(&u) {
            continue;
        }
        return Ok(elevation_within(index, segment, u, cx)?.map(Some));
    }
    Ok(Ok(None))
}

/// 縦断線形の区間 `segment`（`index` 番目）の、初めから水平に `u` の所の高さと勾配。区間の値が使えなければ、
/// 理由の診断を返す。クロソイドの反復の手間は予算に数えないので、数えるなら [`elevation_within`] を使う
pub fn elevation(index: usize, segment: &Vertical, u: f64) -> Result<(f64, f64), Box<Diagnostic>> {
    // 数えないので止まらない
    evaluate(index, segment, u, &mut |_| Ok(())).unwrap_or_else(|_| Err(invalid(index, "length")))
}

/// [`elevation`] と同じく求める。クロソイドの反復で区間の上の点を求める手間（[`zumen_geom2d::along_work`]）は、
/// 求めるたびに予算の作業（`Kind::Work`）に数え、予算で止まれば `Err` を返す（#727）
pub fn elevation_within(
    index: usize,
    segment: &Vertical,
    u: f64,
    cx: &mut Cx<'_>,
) -> Result<Found<(f64, f64)>, Stop> {
    evaluate(index, segment, u, &mut |work| cx.check(Kind::Work, work))
}

/// [`elevation`] の計算。クロソイドの点を求める前に、その手間を `count` に渡す
fn evaluate(
    index: usize,
    segment: &Vertical,
    u: f64,
    count: &mut dyn FnMut(u64) -> Result<(), Stop>,
) -> Result<Found<(f64, f64)>, Stop> {
    let (h0, g0) = (segment.start_height, segment.start_gradient);
    Ok(match segment.kind {
        VerticalKind::ConstantGradient => Ok((h0 + g0 * u, g0)),
        VerticalKind::Parabola { end_gradient } => {
            let change = (end_gradient - g0) / segment.length;
            Ok((h0 + g0 * u + change * u * u / 2.0, g0 + change * u))
        }
        VerticalKind::CircularArc { radius } => {
            arc(h0, g0, radius, u).ok_or_else(|| invalid(index, "radius"))
        }
        VerticalKind::Clothoid {
            start_radius,
            end_radius,
        } => {
            // 縦断の面（追加距離と高さ）で、上に凸に曲がるものは時計回りなので、曲率を負にする
            let inverse = |radius: Option<f64>, field| match radius {
                None => Ok(0.0),
                Some(radius) if radius.is_finite() && radius != 0.0 => Ok(-1.0 / radius),
                Some(_) => Err(invalid(index, field)),
            };
            let curvatures = match (
                inverse(start_radius, "start_radius"),
                inverse(end_radius, "end_radius"),
            ) {
                (Ok(start), Ok(end)) => [start, end],
                (Err(diagnostic), _) | (_, Err(diagnostic)) => return Ok(Err(diagnostic)),
            };
            clothoid(h0, g0, curvatures, segment.length, u, count)?
                .ok_or_else(|| invalid(index, "length"))
        }
    })
}

/// 縦断の円弧の、初めから水平に `u` の所の高さと勾配。半径は上に凸のものを正にする。勾配の角を θ とすると、
/// sin θ = sin θ₀ − u / R、高さは h₀ + R (cos θ − cos θ₀)。円の上にない所なら `None`
fn arc(h0: f64, g0: f64, radius: f64, u: f64) -> Option<(f64, f64)> {
    if !(radius.is_finite() && radius != 0.0) {
        return None;
    }
    let cos0 = 1.0 / libm::hypot(1.0, g0);
    let sin = g0 * cos0 - u / radius;
    if sin.is_nan() || sin.abs() >= 1.0 {
        return None;
    }
    let cos = libm::sqrt(1.0 - sin * sin);
    Some((h0 + radius * (cos - cos0), sin / cos))
}

/// 縦断のクロソイドの、初めから水平に `u` の所の高さと勾配。曲率は、曲線の長さに沿って `curvatures` の初めから
/// 終わりへ変わり、水平の長さが `length` になるように曲線の長さを決める。解けなければ `None`。点を求める前に、
/// その手間を `count` に渡す（#727）
fn clothoid(
    h0: f64,
    g0: f64,
    curvatures: [f64; 2],
    length: f64,
    u: f64,
    count: &mut dyn FnMut(u64) -> Result<(), Stop>,
) -> Result<Option<(f64, f64)>, Stop> {
    let start = libm::atan(g0);
    let ramp = Some(Ramp::Clothoid);
    let mut at = |total: f64, s: f64| -> Result<([f64; 2], f64), Stop> {
        count(zumen_geom2d::along_work(curvatures, total, ramp, s))?;
        Ok(zumen_geom2d::along(
            [0.0, 0.0],
            start,
            curvatures,
            total,
            ramp,
            s,
        ))
    };
    // 曲線の長さを、水平の長さとの比で直していく
    let mut total = length * libm::hypot(1.0, g0);
    for _ in 0..64 {
        let ([x, _], _) = at(total, total)?;
        if !(x.is_finite() && x > 0.0) {
            return Ok(None);
        }
        let next = total * length / x;
        let done = (next - total).abs() <= 1e-12 * length;
        total = next;
        if done {
            break;
        }
    }
    // 水平に u の所の曲線の長さを、ニュートン法で解く。長さに対する水平の変わり方は cos θ
    let mut s = u * total / length;
    for _ in 0..64 {
        let ([x, _], angle) = at(total, s)?;
        let cos = libm::cos(angle);
        if cos.is_nan() || cos <= 0.0 {
            return Ok(None);
        }
        let step = (x - u) / cos;
        s -= step;
        if step.is_nan() || step.abs() <= 1e-12 * length {
            break;
        }
    }
    let ([_, y], angle) = at(total, s)?;
    let cos = libm::cos(angle);
    Ok((cos > 0.0 && y.is_finite()).then(|| (h0 + y, libm::sin(angle) / cos)))
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
