//! 曲線の定義からの計測。座標と長さは入力の単位、角度はラジアン。
//! 折れ線近似が必要な曲線は呼び出し側で分割し、近似したことを結果と診断に残す。

use crate::{
    Xy,
    topology::{Charge, Error, Failure},
};
use std::f64::consts::{PI, TAU};

/// 長さを持たない点も、両端が同じ線分として扱う。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    /// 線分。
    Line {
        /// 始点。
        start: Xy,
        /// 終点。
        end: Xy,
    },
    /// 円弧。正の sweep は反時計回り。絶対値が TAU なら円。
    Arc {
        /// 中心。
        center: Xy,
        /// 0 以上の半径。
        radius: f64,
        /// 始点の角度。
        start: f64,
        /// 符号付きの中心角。絶対値は TAU 以下。
        sweep: f64,
    },
}

/// 1 つの経路に対する計測。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// 経路の長さ。
    Length,
    /// 閉じた経路の周長。
    Perimeter,
    /// 閉じた経路の面積。
    Area,
}

/// 2 つの経路の境界上の最短距離。閉領域の内部は距離の対象に含めない。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Closest {
    /// 境界間の距離。
    pub distance: f64,
    /// 最初の経路上の点。
    pub first: Xy,
    /// もう一方の経路上の点。
    pub second: Xy,
}

fn sub(a: Xy, b: Xy) -> Xy {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: Xy, b: Xy) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: Xy, b: Xy) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn norm(a: Xy) -> f64 {
    libm::hypot(a[0], a[1])
}
fn at(a: Xy, direction: Xy, distance: f64) -> Xy {
    [
        a[0] + direction[0] * distance,
        a[1] + direction[1] * distance,
    ]
}
fn finite(point: Xy) -> bool {
    point.into_iter().all(f64::is_finite)
}
fn zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

impl Piece {
    fn valid(self) -> bool {
        match self {
            Self::Line { start, end } => finite(start) && finite(end),
            Self::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                finite(center)
                    && radius.is_finite()
                    && radius >= 0.0
                    && start.is_finite()
                    && sweep.is_finite()
                    && sweep.abs() <= TAU
            }
        }
    }
    fn endpoints(self) -> [Xy; 2] {
        match self {
            Self::Line { start, end } => [start, end],
            Self::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let point = |angle| at(center, [libm::cos(angle), libm::sin(angle)], radius);
                let first = point(start);
                [
                    first,
                    if sweep.abs() == TAU {
                        first
                    } else {
                        point(start + sweep)
                    },
                ]
            }
        }
    }
    fn contains_angle(self, angle: f64) -> bool {
        match self {
            Self::Arc { start, sweep, .. } => {
                if sweep.abs() == TAU {
                    return true;
                }
                let delta = if sweep >= 0.0 {
                    angle - start
                } else {
                    start - angle
                };
                delta.rem_euclid(TAU) <= sweep.abs()
            }
            Self::Line { .. } => false,
        }
    }
    fn contains_direction(self, direction: Xy) -> bool {
        self.contains_angle(libm::atan2(direction[1], direction[0]))
    }
}

// 小さな弧では、theta と sin(theta) の差を直接引いて有効桁を失わない。
fn arc_segment_factor(theta: f64) -> f64 {
    if theta.abs() < 0.001 {
        let square = theta * theta;
        theta * square * (1.0 / 6.0 - square * (1.0 / 120.0 - square / 5040.0))
    } else if theta.abs() == TAU {
        theta
    } else {
        theta - libm::sin(theta)
    }
}

/// 経路の長さ、閉じた経路の周長・面積を求める。
/// 面積は向き付きの線積分の絶対値で、自己交差の重なりを塗り規則で統合した面積とは異なる。
/// `closed` の経路は、最後の端から最初の端までの線分を含める。
pub fn path_metric<E>(
    pieces: &[Piece],
    closed: bool,
    metric: Metric,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<f64, Failure<E>> {
    check(Charge::Work(
        32_u64
            .saturating_mul(pieces.len() as u64)
            .saturating_add(32),
    ))
    .map_err(Failure::Stopped)?;
    if !closed && metric != Metric::Length {
        return Err(Failure::Geometry(Error::NoSolution));
    }
    let Some(first) = pieces.first() else {
        return Err(Failure::Geometry(Error::Degenerate));
    };
    let origin = first.endpoints()[0];
    let mut end = origin;
    let mut sum = 0.0;
    let mut correction = 0.0;
    for &piece in pieces {
        check(Charge::Work(0)).map_err(Failure::Stopped)?;
        if !piece.valid() {
            return Err(Failure::Geometry(Error::NonFinite));
        }
        let [a, b] = piece.endpoints();
        if !finite(a) || !finite(b) {
            return Err(Failure::Geometry(Error::Range));
        }
        let value = match (metric, piece) {
            (Metric::Area, Piece::Line { .. }) => cross(sub(a, origin), sub(b, origin)) / 2.0,
            (Metric::Area, Piece::Arc { radius, sweep, .. }) => {
                cross(sub(a, origin), sub(b, origin)) / 2.0
                    + radius * (radius * (arc_segment_factor(sweep) / 2.0))
            }
            (_, Piece::Line { .. }) => norm(sub(b, a)),
            (_, Piece::Arc { radius, sweep, .. }) => radius * sweep.abs(),
        };
        let adjusted = value - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
        end = b;
    }
    if closed && metric != Metric::Area {
        sum += norm(sub(end, origin));
    }
    if !sum.is_finite() {
        return Err(Failure::Geometry(Error::Range));
    }
    Ok(zero(if metric == Metric::Area {
        sum.abs()
    } else {
        sum
    }))
}

fn candidate(first: Xy, second: Xy) -> Closest {
    Closest {
        distance: norm(sub(second, first)),
        first,
        second,
    }
}

fn improve(best: &mut Closest, first: Xy, second: Xy) {
    let next = candidate(first, second);
    if next.distance < best.distance {
        *best = next;
    }
}

fn project_line(point: Xy, start: Xy, end: Xy) -> Xy {
    let delta = sub(end, start);
    let length = norm(delta);
    if length == 0.0 {
        return start;
    }
    let direction = [delta[0] / length, delta[1] / length];
    at(
        start,
        direction,
        dot(sub(point, start), direction).clamp(0.0, length),
    )
}

fn point_piece(point: Xy, piece: Piece) -> Xy {
    match piece {
        Piece::Line { start, end } => project_line(point, start, end),
        Piece::Arc { center, radius, .. } => {
            let [a, b] = piece.endpoints();
            let delta = sub(point, center);
            let length = norm(delta);
            if length > 0.0 && piece.contains_direction(delta) {
                at(center, [delta[0] / length, delta[1] / length], radius)
            } else if norm(sub(point, a)) <= norm(sub(point, b)) {
                a
            } else {
                b
            }
        }
    }
}

fn line_line(a: Piece, b: Piece, best: &mut Closest) {
    let [a0, a1] = a.endpoints();
    let [b0, b1] = b.endpoints();
    let (r, s) = (sub(a1, a0), sub(b1, b0));
    let (rl, sl) = (norm(r), norm(s));
    if rl == 0.0 || sl == 0.0 {
        return;
    }
    let (r, s) = ([r[0] / rl, r[1] / rl], [s[0] / sl, s[1] / sl]);
    let denominator = cross(r, s);
    if denominator != 0.0 {
        let q = sub(b0, a0);
        let t = cross(q, s) / denominator;
        let u = cross(q, r) / denominator;
        if (0.0..=rl).contains(&t) && (0.0..=sl).contains(&u) {
            let point = at(a0, r, t);
            improve(best, point, point);
        }
    }
}

fn line_arc(line: Piece, arc: Piece, reversed: bool, best: &mut Closest) {
    let Piece::Arc { center, radius, .. } = arc else {
        return;
    };
    let [start, end] = line.endpoints();
    let delta = sub(end, start);
    let length = norm(delta);
    if length == 0.0 {
        return;
    }
    let direction = [delta[0] / length, delta[1] / length];
    let t = dot(sub(center, start), direction);
    let foot = at(start, direction, t);
    let normal = [-direction[1], direction[0]];
    if (0.0..=length).contains(&t) {
        for sign in [-1.0, 1.0] {
            let radial = [normal[0] * sign, normal[1] * sign];
            if arc.contains_direction(radial) {
                let point = at(center, radial, radius);
                if reversed {
                    improve(best, point, foot);
                } else {
                    improve(best, foot, point);
                }
            }
        }
    }
    let h = cross(sub(center, start), direction).abs();
    if h <= radius && radius > 0.0 {
        let ratio = h / radius;
        let half = radius * libm::sqrt((1.0 - ratio) * (1.0 + ratio));
        for distance in [t - half, t + half] {
            if (0.0..=length).contains(&distance) {
                let point = at(start, direction, distance);
                if arc.contains_direction(sub(point, center)) {
                    improve(best, point, point);
                }
            }
        }
    }
}

fn arc_arc(a: Piece, b: Piece, best: &mut Closest) {
    let (
        Piece::Arc {
            center: ca,
            radius: ra,
            ..
        },
        Piece::Arc {
            center: cb,
            radius: rb,
            ..
        },
    ) = (a, b)
    else {
        return;
    };
    let delta = sub(cb, ca);
    let distance = norm(delta);
    if distance == 0.0 {
        if ra == rb {
            for point in a.endpoints().into_iter().chain(b.endpoints()) {
                let direction = sub(point, ca);
                if a.contains_direction(direction) && b.contains_direction(direction) {
                    improve(best, point, point);
                }
            }
        }
        return;
    }
    let unit = [delta[0] / distance, delta[1] / distance];
    for sa in [-1.0, 1.0] {
        let da = [unit[0] * sa, unit[1] * sa];
        if !a.contains_direction(da) {
            continue;
        }
        for sb in [-1.0, 1.0] {
            let db = [unit[0] * sb, unit[1] * sb];
            if b.contains_direction(db) {
                improve(best, at(ca, da, ra), at(cb, db, rb));
            }
        }
    }
    let scale = ra.max(rb).max(distance);
    let (r, s, d) = (ra / scale, rb / scale, distance / scale);
    if d > r + s || d < (r - s).abs() {
        return;
    }
    let x = ((r - s) * (r + s) + d * d) / (2.0 * d);
    let y = libm::sqrt(((r - x) * (r + x)).max(0.0));
    for sign in [-1.0, 1.0] {
        let offset = [
            (unit[0] * x - unit[1] * y * sign) * scale,
            (unit[1] * x + unit[0] * y * sign) * scale,
        ];
        let point = at(ca, offset, 1.0);
        if a.contains_direction(offset) && b.contains_direction(sub(point, cb)) {
            improve(best, point, point);
        }
    }
}

/// 経路間の最短距離。候補を入力順で調べ、同じ距離なら先の候補を保つ。
/// 各組の前に中断を確認し、すべての組の作業量を先に数える。動的な確保はしない。
pub fn closest_paths<E>(
    first: &[Piece],
    second: &[Piece],
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<Closest, Failure<E>> {
    let pairs = (first.len() as u64)
        .checked_mul(second.len() as u64)
        .and_then(|n| n.checked_mul(128))
        .ok_or(Failure::Geometry(Error::Range))?;
    check(Charge::Work(
        pairs
            .saturating_add(first.len() as u64)
            .saturating_add(second.len() as u64),
    ))
    .map_err(Failure::Stopped)?;
    if first.is_empty() || second.is_empty() {
        return Err(Failure::Geometry(Error::Degenerate));
    }
    if first.iter().chain(second).any(|p| !p.valid()) {
        return Err(Failure::Geometry(Error::NonFinite));
    }
    for &piece in first.iter().chain(second) {
        let [a, b] = piece.endpoints();
        if !finite(a) || !finite(b) || !norm(sub(b, a)).is_finite() {
            return Err(Failure::Geometry(Error::Range));
        }
        if let Piece::Arc { center, radius, .. } = piece
            && center
                .into_iter()
                .any(|value| !(value - radius).is_finite() || !(value + radius).is_finite())
        {
            return Err(Failure::Geometry(Error::Range));
        }
    }
    let mut best = Closest {
        distance: f64::INFINITY,
        first: [0.0; 2],
        second: [0.0; 2],
    };
    for &a in first {
        for &b in second {
            check(Charge::Work(0)).map_err(Failure::Stopped)?;
            for pa in a.endpoints() {
                for pb in b.endpoints() {
                    if !finite(pa) || !finite(pb) || !norm(sub(pa, pb)).is_finite() {
                        return Err(Failure::Geometry(Error::Range));
                    }
                }
            }
            for point in a.endpoints() {
                improve(&mut best, point, point_piece(point, b));
            }
            for point in b.endpoints() {
                improve(&mut best, point_piece(point, a), point);
            }
            match (a, b) {
                (Piece::Line { .. }, Piece::Line { .. }) => line_line(a, b, &mut best),
                (Piece::Line { .. }, Piece::Arc { .. }) => line_arc(a, b, false, &mut best),
                (Piece::Arc { .. }, Piece::Line { .. }) => line_arc(b, a, true, &mut best),
                (Piece::Arc { .. }, Piece::Arc { .. }) => arc_arc(a, b, &mut best),
            }
        }
    }
    if !best.distance.is_finite() || !finite(best.first) || !finite(best.second) {
        return Err(Failure::Geometry(Error::Range));
    }
    best.distance = zero(best.distance);
    best.first = best.first.map(zero);
    best.second = best.second.map(zero);
    check(Charge::Points(2)).map_err(Failure::Stopped)?;
    Ok(best)
}

/// 膨らみを持つ辺を、元の円弧として表す。始点と終点が同じときは点。
pub fn bulge_piece(start: Xy, end: Xy, bulge: f64) -> Result<Piece, Error> {
    if !finite(start) || !finite(end) || !bulge.is_finite() {
        return Err(Error::NonFinite);
    }
    let chord = sub(end, start);
    let length = norm(chord);
    if !length.is_finite() {
        return Err(Error::Range);
    }
    if bulge == 0.0 || length == 0.0 {
        return Ok(Piece::Line { start, end });
    }
    let sweep = 4.0 * libm::atan(bulge);
    let radius = (length / 2.0 / libm::sin(sweep / 2.0)).abs();
    let height = length / 4.0 * (1.0 / bulge - bulge);
    let unit = [chord[0] / length, chord[1] / length];
    let middle = at(start, chord, 0.5);
    let center = at(middle, [-unit[1], unit[0]], height);
    let offset = sub(start, center);
    let piece = Piece::Arc {
        center,
        radius,
        start: libm::atan2(offset[1], offset[0]),
        sweep,
    };
    if !piece.valid() || radius == 0.0 || sweep.abs() >= TAU {
        return Err(Error::Range);
    }
    Ok(piece)
}

/// 度をラジアンへ移す。大きい角度は先に 1 周以内へそろえる。
pub fn start_radians(degrees: f64) -> f64 {
    degrees.rem_euclid(360.0) * (PI / 180.0)
}
