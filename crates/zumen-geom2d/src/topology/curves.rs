//! 整数格子の円と線分の交点。二次方程式の根の符号と丸めを、固定長の整数で比較する。

use std::cmp::Ordering;

use super::wide::{SignedWide, Wide};
use super::{Charge, Error, Failure, Hit, MAX_COORDINATE, Point, Segment, cross, difference};

/// 同じ格子上の中心と、格子の間隔を単位とする半径を持つ円。
/// 半径 0 は中心の 1 点として扱う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Circle {
    center: Point,
    radius: i64,
}

impl Circle {
    /// 円全体が格子の座標範囲に収まるものを作る。負の半径は受け付けない。
    pub fn new(center: Point, radius: i64) -> Result<Self, Error> {
        let Point([x, y]) = center;
        let maximum = MAX_COORDINATE - x.abs().max(y.abs());
        if !(0..=maximum).contains(&radius) {
            return Err(Error::Range);
        }
        Ok(Self { center, radius })
    }

    /// 中心。
    pub fn center(self) -> Point {
        self.center
    }

    /// 格子の間隔を単位とする半径。
    pub fn radius(self) -> i64 {
        self.radius
    }
}

/// 円を含む組の交わり。異なる交点が同じ格子点へ丸められても、2 点を保つ。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurveIntersection {
    /// 交わらない。
    None,
    /// 1 点で交わる。端点だけの接触と半径 0 を含み、接点は `Tangent` で返す。
    Point(Hit),
    /// 半径が正の円と線分、または異なる 2 円が接する点。
    Tangent(Hit),
    /// 2 点で交わる。格子点の X、Y、丸めの有無の辞書順で返す。
    Two([Hit; 2]),
    /// 半径が正で同じ円どうし。交点の列に展開しない。
    Coincident,
}

/// 線分と円の交点。線分の端点で接する場合も含む。
/// 判定に `Work(1)`、返す点ごとに最大 86 回の整数式の比較と、点数を数える。
pub fn segment_circle_intersection<E>(
    segment: Segment,
    circle: Circle,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<CurveIntersection, Failure<E>> {
    check(Charge::Work(1)).map_err(Failure::Stopped)?;
    finish(segment_roots(segment, circle), &mut check)
}

/// 円どうしの交点。外接・内接・同じ中心・半径 0 を区別する。
/// 予算の数え方は [`segment_circle_intersection`] と同じ。
pub fn circle_intersection<E>(
    a: Circle,
    b: Circle,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<CurveIntersection, Failure<E>> {
    check(Charge::Work(1)).map_err(Failure::Stopped)?;
    let roots = circle_roots(a, b).map_err(Failure::Geometry)?;
    finish(roots, &mut check)
}

// 各座標について、符号・範囲・二分探索・丸めの有無を比べる回数の上限。
pub(super) const ROUND_WORK: u64 = 2 * (MAX_COORDINATE.ilog2() as u64 + 4);

#[derive(Clone, Copy)]
pub(super) struct Root {
    // 点は (a + b sqrt(d)) / denominator。分母は正。
    a: [i128; 2],
    b: [i128; 2],
    d: Wide,
    denominator: i128,
}

pub(super) enum Roots {
    None,
    One(Root),
    Tangent(Root),
    Two([Root; 2]),
    Coincident,
}

impl Root {
    fn point(Point([x, y]): Point) -> Self {
        Self {
            a: [i128::from(x), i128::from(y)],
            b: [0, 0],
            d: Wide::ZERO,
            denominator: 1,
        }
    }

    pub(super) fn hit(self) -> Result<Hit, Error> {
        let [ax, ay] = self.a;
        let [bx, by] = self.b;
        let (x, rounded_x) = round(ax, bx, self.d, self.denominator)?;
        let (y, rounded_y) = round(ay, by, self.d, self.denominator)?;
        Ok(Hit {
            point: Point::new(x, y)?,
            rounded: rounded_x || rounded_y,
        })
    }

    pub(super) fn on_ray(circle: Circle, [x, y]: [i128; 2]) -> Self {
        let denominator = x * x + y * y;
        let radius = i128::from(circle.radius);
        let Point([cx, cy]) = circle.center;
        Self {
            a: [denominator * i128::from(cx), denominator * i128::from(cy)],
            b: [x, y],
            d: Wide::product((radius * radius) as u128, denominator as u128),
            denominator,
        }
    }

    // ray × (root - center) の符号。定数項は 166 bit、二乗の比較は 512 bit に収まる。
    pub(super) fn side(self, center: Point, [x, y]: [i128; 2]) -> Result<Ordering, Error> {
        let Point([cx, cy]) = center;
        let [ax, ay] = self.a;
        let [bx, by] = self.b;
        let ax = ax - self.denominator * i128::from(cx);
        let ay = ay - self.denominator * i128::from(cy);
        let a = SignedWide::difference_of_products(x, ay, y, ax).ok_or(Error::Range)?;
        let b = x * by - y * bx;
        let a_sign = a.sign();
        if b == 0 || self.d == Wide::ZERO {
            return Ok(a_sign);
        }
        let b_sign = b.cmp(&0);
        if a_sign == Ordering::Equal || a_sign == b_sign {
            return Ok(b_sign);
        }
        let comparison = a
            .magnitude
            .full_product(a.magnitude)
            .cmp(&Wide::square(b).full_product(self.d));
        Ok(if a.negative {
            comparison.reverse()
        } else {
            comparison
        })
    }
}

fn dot([x, y]: [i128; 2], [u, v]: [i128; 2]) -> i128 {
    x * u + y * v
}

pub(super) fn segment_roots(segment: Segment, circle: Circle) -> Roots {
    let r = difference(segment.end, segment.start);
    let p = difference(segment.start, circle.center);
    let a = dot(r, r);
    let radius = i128::from(circle.radius);
    if a == 0 {
        return if dot(p, p) == radius * radius {
            Roots::One(Root::point(segment.start))
        } else {
            Roots::None
        };
    }
    // |p + t r|² = radius²。t = (-b ± sqrt(d)) / a。
    // d = a radius² - cross(r, p)² を、近い値の差も含めて整数で求める。
    let b = dot(r, p);
    let Some(d) =
        Wide::product(a as u128, (radius * radius) as u128).checked_sub(Wide::square(cross(r, p)))
    else {
        return Roots::None;
    };
    let Point([x, y]) = segment.start;
    let [rx, ry] = r;
    let numerators = [a * i128::from(x) - rx * b, a * i128::from(y) - ry * b];
    let root = |sign| Root {
        a: numerators,
        b: [sign * rx, sign * ry],
        d,
        denominator: a,
    };
    // 0 <= t <= 1 を、根を浮動小数にせずに比較する。
    let minus = b <= 0 && d <= Wide::square(b) && (a + b >= 0 || d >= Wide::square(a + b));
    let plus = (b <= 0 || d >= Wide::square(b)) && a + b >= 0 && d <= Wide::square(a + b);
    match (minus, plus && d != Wide::ZERO) {
        (true, true) => Roots::Two([root(-1), root(1)]),
        (true, false) if d == Wide::ZERO && circle.radius > 0 => Roots::Tangent(root(-1)),
        (true, false) => Roots::One(root(-1)),
        (false, true) => Roots::One(root(1)),
        (false, false) => Roots::None,
    }
}

pub(super) fn circle_roots(a: Circle, b: Circle) -> Result<Roots, Error> {
    let q = difference(b.center, a.center);
    let d2 = dot(q, q);
    let (r1, r2) = (i128::from(a.radius), i128::from(b.radius));
    if d2 == 0 {
        return Ok(if r1 != r2 {
            Roots::None
        } else if r1 == 0 {
            Roots::One(Root::point(a.center))
        } else {
            Roots::Coincident
        });
    }
    if d2 > (r1 + r2) * (r1 + r2) || d2 < (r1 - r2) * (r1 - r2) {
        return Ok(Roots::None);
    }
    // 円を差し引いて求めた共通弦の中点と、中心を結ぶ線に垂直な成分。
    let factor = d2 + r1 * r1 - r2 * r2;
    let d = Wide::product((4 * r1 * r1) as u128, d2 as u128)
        .checked_sub(Wide::square(factor))
        .ok_or(Error::Range)?;
    let denominator = 2 * d2;
    let Point([x, y]) = a.center;
    let [qx, qy] = q;
    let root = Root {
        a: [
            denominator * i128::from(x) + factor * qx,
            denominator * i128::from(y) + factor * qy,
        ],
        b: [-qy, qx],
        d,
        denominator,
    };
    Ok(if d == Wide::ZERO && r1 > 0 && r2 > 0 {
        Roots::Tangent(root)
    } else if d == Wide::ZERO {
        Roots::One(root)
    } else {
        Roots::Two([
            root,
            Root {
                b: [qy, -qx],
                ..root
            },
        ])
    })
}

/// a + b sqrt(d) の符号。逆符号の 2 項だけを二乗して比較する。
fn sign(a: i128, b: i128, d: Wide) -> Result<Ordering, Error> {
    if d == Wide::ZERO || b == 0 {
        return Ok(a.cmp(&0));
    }
    if a == 0 || a.signum() == b.signum() {
        return Ok(b.cmp(&0));
    }
    let absolute_b = b.unsigned_abs();
    let b2 = absolute_b.checked_mul(absolute_b).ok_or(Error::Range)?;
    let right = d.checked_mul(b2).ok_or(Error::Range)?;
    let comparison = Wide::square(a).cmp(&right);
    Ok(if a > 0 {
        comparison
    } else {
        comparison.reverse()
    })
}

fn round(mut a: i128, mut b: i128, d: Wide, denominator: i128) -> Result<(i64, bool), Error> {
    let negative = match sign(a, b, d)? {
        Ordering::Equal => return Ok((0, false)),
        Ordering::Less => true,
        Ordering::Greater => false,
    };
    if negative {
        a = -a;
        b = -b;
    }
    if sign(a - denominator * i128::from(MAX_COORDINATE), b, d)? == Ordering::Greater {
        return Err(Error::Range);
    }
    let (mut low, mut high) = (0, MAX_COORDINATE);
    while low < high {
        let middle = low + (high - low) / 2;
        // 正の値を middle + 1/2 と比べ、半分なら大きい整数を選ぶ。
        let comparison = sign(2 * a - denominator * i128::from(2 * middle + 1), 2 * b, d)?;
        if comparison == Ordering::Less {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let rounded = sign(a - denominator * i128::from(low), b, d)? != Ordering::Equal;
    Ok((if negative { -low } else { low }, rounded))
}

pub(super) fn finish<E>(
    roots: Roots,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<CurveIntersection, Failure<E>> {
    let mut hit = |root: Root| {
        check(Charge::Work(ROUND_WORK)).map_err(Failure::Stopped)?;
        root.hit().map_err(Failure::Geometry)
    };
    let (result, points) = match roots {
        Roots::None => (CurveIntersection::None, 0),
        Roots::Coincident => (CurveIntersection::Coincident, 0),
        Roots::One(root) => (CurveIntersection::Point(hit(root)?), 1),
        Roots::Tangent(root) => (CurveIntersection::Tangent(hit(root)?), 1),
        Roots::Two([first, second]) => {
            let (mut a, mut b) = (hit(first)?, hit(second)?);
            if (a.point, a.rounded) > (b.point, b.rounded) {
                std::mem::swap(&mut a, &mut b);
            }
            (CurveIntersection::Two([a, b]), 2)
        }
    };
    check(Charge::Points(points)).map_err(Failure::Stopped)?;
    Ok(result)
}
