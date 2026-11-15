//! 許容差より細かな整数格子上の位相計算。
//!
//! [`Grid`] の原点と間隔は、計算結果と一緒に記録する。同じ計算の点には同じ格子を使う。
//! 元の曲線の定義は変更しない。入力を格子へ移すずれは [`Snap::offset`]、交点を格子へ
//! 丸めたことは [`Hit::rounded`] で返す。利用側は、形の変更を元の要素の出所とともに診断する。
//!
//! 線分の向き・接触・重なりは整数で判定し、交点だけを最後に格子へ丸める。
//! 点の各座標を [`MAX_COORDINATE`] までに制限し、線分の積と交点の分子を `i128` に収める。
//! 円の判別式と根の比較は固定長の 256 bit の整数で行い、接点と格子への丸めを区別する。

use crate::Xy;

mod curves;
mod wide;

pub use curves::{Circle, CurveIntersection, circle_intersection, segment_circle_intersection};

/// 格子の各座標の絶対値の上限。差の外積は 83 bit、交点の分子の計算は 125 bit 未満に収まる。
pub const MAX_COORDINATE: i64 = (1 << 40) - 1;

/// 1 回の全組の計算で予約する結果の上限。32 bit の `isize` に収め、環境ごとの範囲判定をそろえる。
pub const MAX_RESULT_BYTES: u64 = (1 << 31) - 1;

/// 幾何演算で受け付けない入力、または確保できない結果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// 許容差が正の有限値でないか、間隔が 0 に丸められる。
    Tolerance,
    /// 原点か入力に有限でない座標がある。
    NonFinite,
    /// 座標・個数・確保量が、表せる範囲を超える。
    Range,
    /// 予算内でも、実際のメモリを確保できなかった。
    Allocation,
}

/// 計算する前に利用側の予算へ数える量。`Work(0)` でも中断を確認する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Charge {
    /// 作業量。線分の組の判定 1 回を 1 とする。
    Work(u64),
    /// 結果の点数。重なった区間は両端の 2 点として数える。
    Points(u64),
    /// 確保する結果の最大バイト数。
    Bytes(u64),
}

/// 入力・確保の失敗と、利用側の予算・中断による停止を分ける。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure<E> {
    /// 入力か確保の失敗。
    Geometry(Error),
    /// 利用側の検査が返した停止。途中の結果は返さない。
    Stopped(E),
}

/// 実数の座標と整数格子との対応。間隔は許容差の 1/4 とする。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    origin: Xy,
    tolerance: f64,
    spacing: f64,
}

impl Grid {
    /// 原点と正の許容差から格子を作る。途中の除算も同じ順序で行う。
    pub fn new(origin: Xy, tolerance: f64) -> Result<Self, Error> {
        if !origin.into_iter().all(f64::is_finite) {
            return Err(Error::NonFinite);
        }
        let spacing = tolerance / 4.0;
        if !tolerance.is_finite() || tolerance <= 0.0 || spacing == 0.0 {
            return Err(Error::Tolerance);
        }
        Ok(Self {
            origin,
            tolerance,
            spacing,
        })
    }

    /// 元の座標系の原点。
    pub fn origin(self) -> Xy {
        self.origin
    }

    /// 利用側が指定した許容差。
    pub fn tolerance(self) -> f64 {
        self.tolerance
    }

    /// 位相の計算に使う格子の間隔。
    pub fn spacing(self) -> f64 {
        self.spacing
    }

    /// 点を近い格子点へ移す。ちょうど半分なら、原点から遠いほうに丸める。
    /// 元の座標に戻すと許容差を超える場合も、黙って精度を落とさず `Range` にする。
    pub fn snap(self, [x, y]: Xy) -> Result<Snap, Error> {
        let [ox, oy] = self.origin;
        let quantize = |value: f64, origin: f64| {
            if !value.is_finite() {
                return Err(Error::NonFinite);
            }
            let coordinate = libm::round((value - origin) / self.spacing);
            if !coordinate.is_finite() || coordinate.abs() > MAX_COORDINATE as f64 {
                return Err(Error::Range);
            }
            Ok(coordinate as i64)
        };
        let point = Point([quantize(x, ox)?, quantize(y, oy)?]);
        let [px, py] = self.position(point)?;
        let offset = [px - x, py - y];
        let [dx, dy] = offset;
        if libm::hypot(dx, dy) > self.tolerance {
            return Err(Error::Range);
        }
        Ok(Snap { point, offset })
    }

    /// 格子点を元の座標へ戻す。有限でない結果は返さない。負の 0 は正の 0 にそろえる。
    pub fn position(self, Point([x, y]): Point) -> Result<Xy, Error> {
        let [ox, oy] = self.origin;
        let positive_zero = |value| if value == 0.0 { 0.0 } else { value };
        let point = [
            positive_zero(ox + x as f64 * self.spacing),
            positive_zero(oy + y as f64 * self.spacing),
        ];
        let [px, py] = point;
        if !point.into_iter().all(f64::is_finite)
            || libm::round((px - ox) / self.spacing) != x as f64
            || libm::round((py - oy) / self.spacing) != y as f64
        {
            return Err(Error::Range);
        }
        Ok(point)
    }
}

/// 整数格子の点。X、Y の順の辞書順で比較する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point([i64; 2]);

impl Point {
    /// 各座標が `-MAX_COORDINATE..=MAX_COORDINATE` に収まる点を作る。
    pub fn new(x: i64, y: i64) -> Result<Self, Error> {
        if [x, y]
            .into_iter()
            .all(|v| (-MAX_COORDINATE..=MAX_COORDINATE).contains(&v))
        {
            Ok(Self([x, y]))
        } else {
            Err(Error::Range)
        }
    }

    /// X と Y の整数の座標。
    pub fn coordinates(self) -> [i64; 2] {
        self.0
    }
}

/// 格子へ移した点と、元の座標でのずれ。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    /// 格子の点。
    pub point: Point,
    /// 格子の点を元の座標へ戻した値から、入力の座標を引いた値。
    pub offset: Xy,
}

/// 同じ格子上の 2 点を結ぶ線分。両端が同じなら 1 点として扱う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    /// 始点。
    pub start: Point,
    /// 終点。
    pub end: Point,
}

/// 交点を格子へ丸めた位置と、丸めの有無。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// 近い格子点。半分なら原点から遠いほう。
    pub point: Point,
    /// 正確な交点が格子点ではない。
    pub rounded: bool,
}

/// 線分どうしの交わり。線分を逆向きにしても、同じ結果を返す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentIntersection {
    /// 交わらない。
    None,
    /// 1 点で交わる。端点だけの接触を含む。
    Point(Hit),
    /// 長さを持つ区間で重なる。両端は X、Y の辞書順に並べる。
    Overlap(Segment),
}

impl SegmentIntersection {
    fn points(self) -> u64 {
        match self {
            Self::None => 0,
            Self::Point(_) => 1,
            Self::Overlap(_) => 2,
        }
    }
}

/// 1 組の線分を比べる。判定の前に `Work(1)`、結果を返す前に点数を数える。
/// 確保をせず、入力の線分も変更しない。
pub fn segment_intersection<E>(
    a: Segment,
    b: Segment,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<SegmentIntersection, Failure<E>> {
    check(Charge::Work(1)).map_err(Failure::Stopped)?;
    let result = intersect(a, b).map_err(Failure::Geometry)?;
    check(Charge::Points(result.points())).map_err(Failure::Stopped)?;
    Ok(result)
}

/// 入力の位置で指した線分の組と、その交わり。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairIntersection {
    /// 入力の線分の位置。小さいほうが先。
    pub pair: [u64; 2],
    /// 交点か重なる区間。`None` の組は結果に入れない。
    pub intersection: SegmentIntersection,
}

// メモリの予算も 32 bit と 64 bit で同じにする。実際の型の大きさが予約を超えないことは試験で確認する。
const PAIR_RESERVED_BYTES: u64 = 64;

/// 線分の全組を入力の位置の順で比べる。結果は組の辞書順に並ぶ。
///
/// 確保の前に線分の数と全組の数の和を作業量に数え、全組が交わる場合のバイト数を予約する。
/// 点数は実際に交わった組ごとに数える。組ごとに `Work(0)` で中断も確認する。
/// どこで停止しても部分結果は返さない。結果の上限が大きい入力は、呼ぶ側で空間別に分ける。
pub fn segment_intersections<E>(
    segments: &[Segment],
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<Vec<PairIntersection>, Failure<E>> {
    let range = || Failure::Geometry(Error::Range);
    let n = u64::try_from(segments.len()).map_err(|_| range())?;
    let pairs = n.checked_mul(n.saturating_sub(1)).ok_or_else(range)? / 2;
    check(Charge::Work(n.checked_add(pairs).ok_or_else(range)?)).map_err(Failure::Stopped)?;
    let bytes = pairs.checked_mul(PAIR_RESERVED_BYTES).ok_or_else(range)?;
    if bytes > MAX_RESULT_BYTES {
        return Err(range());
    }
    check(Charge::Bytes(bytes)).map_err(Failure::Stopped)?;
    let capacity = usize::try_from(pairs).map_err(|_| range())?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(capacity)
        .map_err(|_| Failure::Geometry(Error::Allocation))?;
    for (first, &a) in segments.iter().enumerate() {
        for (second, &b) in segments.iter().enumerate().skip(first + 1) {
            check(Charge::Work(0)).map_err(Failure::Stopped)?;
            let intersection = intersect(a, b).map_err(Failure::Geometry)?;
            if intersection != SegmentIntersection::None {
                check(Charge::Points(intersection.points())).map_err(Failure::Stopped)?;
                result.push(PairIntersection {
                    pair: [first as u64, second as u64],
                    intersection,
                });
            }
        }
    }
    Ok(result)
}

fn difference(Point([x, y]): Point, Point([ox, oy]): Point) -> [i128; 2] {
    [
        i128::from(x) - i128::from(ox),
        i128::from(y) - i128::from(oy),
    ]
}

fn cross([x, y]: [i128; 2], [u, v]: [i128; 2]) -> i128 {
    x * v - y * u
}

fn ends(segment: Segment) -> (Point, Point) {
    (
        segment.start.min(segment.end),
        segment.start.max(segment.end),
    )
}

fn exact(point: Point) -> SegmentIntersection {
    SegmentIntersection::Point(Hit {
        point,
        rounded: false,
    })
}

fn intersect(a: Segment, b: Segment) -> Result<SegmentIntersection, Error> {
    let r = difference(a.end, a.start);
    let s = difference(b.end, b.start);
    let q = difference(b.start, a.start);
    if r == [0, 0] {
        let (low, high) = ends(b);
        return Ok(if cross(q, s) == 0 && low <= a.start && a.start <= high {
            exact(a.start)
        } else {
            SegmentIntersection::None
        });
    }
    let mut denominator = cross(r, s);
    let mut numerator = cross(q, s);
    let mut other = cross(q, r);
    if denominator == 0 {
        if other != 0 {
            return Ok(SegmentIntersection::None);
        }
        let (a0, a1) = ends(a);
        let (b0, b1) = ends(b);
        let (start, end) = (a0.max(b0), a1.min(b1));
        return Ok(match start.cmp(&end) {
            std::cmp::Ordering::Greater => SegmentIntersection::None,
            std::cmp::Ordering::Equal => exact(start),
            std::cmp::Ordering::Less => SegmentIntersection::Overlap(Segment { start, end }),
        });
    }
    if denominator < 0 {
        denominator = -denominator;
        numerator = -numerator;
        other = -other;
    }
    if !(0..=denominator).contains(&numerator) || !(0..=denominator).contains(&other) {
        return Ok(SegmentIntersection::None);
    }
    let Point([x, y]) = a.start;
    let [dx, dy] = r;
    let nx = i128::from(x) * denominator + dx * numerator;
    let ny = i128::from(y) * denominator + dy * numerator;
    let rounded = |value: i128| -> Result<i64, Error> {
        let quotient = value / denominator;
        let remainder = value % denominator;
        let away = if remainder.abs() * 2 >= denominator {
            value.signum()
        } else {
            0
        };
        i64::try_from(quotient + away).map_err(|_| Error::Range)
    };
    Ok(SegmentIntersection::Point(Hit {
        point: Point::new(rounded(nx)?, rounded(ny)?)?,
        rounded: nx % denominator != 0 || ny % denominator != 0,
    }))
}
