//! 曲線の数値近似。計測の値の誤差上限とは分け、応答に近似したことを残す。

use super::*;
use zumen_drawing::{KnotParameterization, Spline};

fn segments(points: Vec<Xy>, closed: bool, cx: &mut Cx<'_>) -> Result<Path, Error> {
    if points.len() < 2 || points.len() > MAX_POINTS + 1 {
        return Err(Error::Geometry("sampling-limit"));
    }
    cx.check(Kind::Work, points.len() as u64)?;
    cx.check(Kind::Segments, points.len() as u64)?;
    if points.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error::Geometry("numeric-range"));
    }
    let mut pieces = Vec::new();
    reserve(&mut pieces, points.len(), 64, cx)?;
    for pair in points.windows(2) {
        if let &[start, end] = pair {
            pieces.push(Piece::Line { start, end });
        }
    }
    if closed
        && let (Some(&first), Some(&last)) = (points.first(), points.last())
        && first != last
    {
        pieces.push(Piece::Line {
            start: last,
            end: first,
        });
    }
    Ok(Path {
        pieces,
        closed,
        approximate: true,
    })
}

fn ellipse(
    center: Xy,
    major: Xy,
    ratio: f64,
    start: f64,
    sweep: f64,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<Path, Error> {
    if !ratio.is_finite() || ratio == 0.0 {
        return Err(Error::Geometry("invalid-curve"));
    }
    let u = [major[0] - center[0], major[1] - center[1]];
    let v = [-u[1] * ratio, u[0] * ratio];
    let radius = libm::hypot(u[0], u[1]) * ratio.abs().max(1.0);
    if !radius.is_finite() || radius <= 0.0 {
        return Err(Error::Geometry("invalid-curve"));
    }
    // 媒介変数の 2 階微分のノルムの上限から弦の刻みを決める。
    let step = libm::sqrt(8.0 * (tolerance / radius)).min(PI / 2.0);
    let count = libm::ceil(sweep.abs() / step);
    if !count.is_finite() || count > MAX_POINTS as f64 {
        return Err(Error::Geometry("sampling-limit"));
    }
    let count = count.max(1.0) as usize;
    cx.check(Kind::Work, 32 * (count as u64 + 1))?;
    let mut points = Vec::new();
    reserve(&mut points, count + 1, 16, cx)?;
    for i in 0..=count {
        cx.check(Kind::Work, 0)?;
        let angle = start + sweep * (i as f64 / count as f64);
        let (c, s) = (libm::cos(angle), libm::sin(angle));
        points.push([
            center[0] + u[0] * c + v[0] * s,
            center[1] + u[1] * c + v[1] * s,
        ]);
    }
    let closed = sweep.abs() == TAU;
    if closed
        && let Some(first) = points.first().copied()
        && let Some(last) = points.last_mut()
    {
        *last = first;
    }
    segments(points, closed, cx)
}

fn spline(
    input: &Spline<ModelPoint>,
    space: SpaceId,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<Path, Error> {
    const MAX_CONTROL: usize = 4096;
    if input.control.len() > MAX_CONTROL
        || input.fit.len() > MAX_CONTROL
        || input.knots.len() > MAX_CONTROL + 33
        || input.weights.len() > MAX_CONTROL
        || input.degree > geom::MAX_DEGREE
    {
        return Err(Error::Geometry("input-limit"));
    }
    let n =
        (input.control.len() + input.fit.len() + input.knots.len() + input.weights.len()) as u64;
    cx.check(Kind::Work, 8 * n)?;
    // 制御点、同次座標、de Boor の作業領域、分割のスタックと出力の成長分を予約する。
    cx.check(Kind::DecodedBytes, 128 * (n + MAX_POINTS as u64 + 256))?;
    if input.weights.iter().any(|w| !w.is_finite() || *w <= 0.0)
        || input.knots.iter().any(|k| !k.is_finite())
    {
        return Err(Error::Geometry("invalid-curve"));
    }
    let collect = |points: &[ModelPoint]| -> Result<Vec<Xy>, Error> {
        points.iter().map(|&p| point(p, space)).collect()
    };
    let closed = input.closed || input.periodic;
    let output = if input.control.is_empty() {
        let fit = collect(&input.fit)?;
        cx.check(Kind::Work, 128 * (fit.len() as u64 + 1).pow(2))?;
        let spacing = match input.knot_parameterization {
            Some(KnotParameterization::SquareRoot) => geom::Spacing::SquareRoot,
            Some(KnotParameterization::Uniform) => geom::Spacing::Uniform,
            _ => geom::Spacing::Chord,
        };
        let (knots, control) = geom::interpolate(
            &fit,
            input.start_tangent,
            input.end_tangent,
            spacing,
            closed,
        )
        .ok_or(Error::Geometry("invalid-curve"))?;
        geom::try_spline_with_work(3, &knots, &control, &[], tolerance, |work| {
            cx.check(Kind::Work, work)
        })?
    } else {
        let control = collect(&input.control)?;
        geom::try_spline_with_work(
            input.degree,
            &input.knots,
            &control,
            &input.weights,
            tolerance,
            |work| cx.check(Kind::Work, work),
        )?
    }
    .ok_or(Error::Geometry("invalid-curve"))?;
    if output.len() >= MAX_POINTS {
        return Err(Error::Geometry("sampling-limit"));
    }
    segments(output, closed, cx)
}

pub(super) fn edge(
    edge: &Edge<ModelPoint>,
    space: SpaceId,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<Path, Error> {
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(Error::Geometry("invalid-tolerance"));
    }
    match edge {
        Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        } => {
            let (start, sweep) = angles(*start, *end, *counterclockwise)?;
            ellipse(
                point(*center, space)?,
                point(*major, space)?,
                *ratio,
                start,
                sweep,
                tolerance,
                cx,
            )
        }
        Edge::Spline(input) => spline(input, space, tolerance, cx),
        Edge::Clothoid {
            origin,
            parameter,
            angle,
            counterclockwise,
            start,
            end,
        } => {
            let origin = point(*origin, space)?;
            let parameter = length(*parameter, space)?;
            let (start, end) = (length(*start, space)?, length(*end, space)?);
            if parameter <= 0.0 || !angle.is_finite() {
                return Err(Error::Geometry("invalid-curve"));
            }
            let curvature = start.abs().max(end.abs()) / (parameter * parameter);
            let step = libm::sqrt(8.0 * tolerance / curvature);
            let span = (end - start).abs();
            if !span.is_finite()
                || !curvature.is_finite()
                || libm::ceil(span.max(start.abs()) / step) > MAX_POINTS as f64
            {
                return Err(Error::Geometry("sampling-limit"));
            }
            cx.check(
                Kind::Work,
                geom::clothoid_work(parameter, start, end, tolerance),
            )?;
            let count = geom::clothoid_point_count(parameter, start, end, tolerance);
            cx.check(Kind::DecodedBytes, 32 * count)?;
            let points = geom::clothoid(
                origin,
                parameter,
                *angle,
                *counterclockwise,
                start,
                end,
                tolerance,
            );
            segments(points, false, cx)
        }
        Edge::Line { .. } | Edge::Arc { .. } => Err(Error::Geometry("unsupported-shape")),
    }
}
