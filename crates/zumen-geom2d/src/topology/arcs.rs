//! 円弧の範囲は根を丸める前に比べ、同じ円上の重なりは角度順の区間で求める。

use std::cmp::Ordering;

use super::curves::{ROUND_WORK, Root, Roots, circle_roots, finish, segment_roots};
use super::{
    Charge, Circle, CurveIntersection, Error, Failure, Hit, MAX_COORDINATE, Segment, cross,
};

/// 円の中心からの向き。長さには意味を持たせず、成分の最大公約数でそろえる。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ray([i64; 2]);

impl Ray {
    /// X の正の向き。
    pub const X: Self = Self([1, 0]);

    /// 各成分の絶対値は `2 * MAX_COORDINATE` まで。0 の向きは受け付けない。
    /// 正規化の反復は、成分の 41 bit の範囲に対して最大 84 回に制限する。
    pub fn new(x: i64, y: i64) -> Result<Self, Error> {
        let limit = 2 * MAX_COORDINATE;
        if !(-limit..=limit).contains(&x) || !(-limit..=limit).contains(&y) {
            return Err(Error::Range);
        }
        if x == 0 && y == 0 {
            return Err(Error::Degenerate);
        }
        let (mut a, mut b) = (x.abs(), y.abs());
        for _ in 0..84 {
            if b == 0 {
                return Ok(Self([x / a, y / a]));
            }
            (a, b) = (b, a % b);
        }
        Err(Error::Range)
    }

    /// 正規化した整数の成分。
    pub fn components(self) -> [i64; 2] {
        self.0
    }

    pub(super) fn vector(self) -> [i128; 2] {
        [i128::from(self.0[0]), i128::from(self.0[1])]
    }

    fn half(self) -> bool {
        let Self([x, y]) = self;
        y < 0 || (y == 0 && x < 0)
    }
}

impl Ord for Ray {
    /// X の正の向きから反時計回りの角度順。
    fn cmp(&self, other: &Self) -> Ordering {
        self.half()
            .cmp(&other.half())
            .then_with(|| cross(self.vector(), other.vector()).cmp(&0).reverse())
    }
}

impl PartialOrd for Ray {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 円弧を始点から終点へ進む向き。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// 反時計回り。
    CounterClockwise,
    /// 時計回り。
    Clockwise,
}

/// 中心・半径と、両端の向きで定義する円弧。両端を含む。
/// 向きの成分は円周上の点でなくてもよく、元の円の半径は変えない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arc {
    circle: Circle,
    start: Ray,
    end: Ray,
    direction: Direction,
    full: bool,
}

impl Arc {
    /// 同じ向きの両端は 1 点の円弧。1 周は [`Self::full`] で明示する。
    pub fn new(circle: Circle, start: Ray, end: Ray, direction: Direction) -> Result<Self, Error> {
        if circle.radius() == 0 {
            return Err(Error::Degenerate);
        }
        Ok(Self {
            circle,
            start,
            end,
            direction,
            full: false,
        })
    }

    /// 指定した始点と向きで 1 周する円弧。
    pub fn full(circle: Circle, start: Ray, direction: Direction) -> Result<Self, Error> {
        Ok(Self {
            full: true,
            ..Self::new(circle, start, start, direction)?
        })
    }

    /// 円弧を含む円。
    pub fn circle(self) -> Circle {
        self.circle
    }
    /// 始点の向き。
    pub fn start(self) -> Ray {
        self.start
    }
    /// 終点の向き。
    pub fn end(self) -> Ray {
        self.end
    }
    /// 始点から終点へ進む向き。
    pub fn direction(self) -> Direction {
        self.direction
    }
    /// 1 周するか。
    pub fn is_full(self) -> bool {
        self.full
    }

    /// 同じ範囲を逆向きでたどる円弧。
    pub fn reversed(self) -> Self {
        Self {
            start: self.end,
            end: self.start,
            direction: match self.direction {
                Direction::CounterClockwise => Direction::Clockwise,
                Direction::Clockwise => Direction::CounterClockwise,
            },
            ..self
        }
    }

    pub(super) fn ccw(self) -> (Ray, Ray) {
        match self.direction {
            Direction::CounterClockwise => (self.start, self.end),
            Direction::Clockwise => (self.end, self.start),
        }
    }

    pub(super) fn contains_ray(self, ray: Ray, after: bool) -> bool {
        if self.full {
            return true;
        }
        let (start, end) = self.ccw();
        if start == end {
            return !after && ray == start;
        }
        let below_end = if after { ray < end } else { ray <= end };
        if start < end {
            start <= ray && below_end
        } else {
            start <= ray || below_end
        }
    }

    pub(super) fn contains_root(self, root: Root) -> Result<bool, Error> {
        if self.full {
            return Ok(true);
        }
        let (start, end) = self.ccw();
        let (start, end) = (start.vector(), end.vector());
        let first = root.side(self.circle.center(), start)?;
        let last = root.side(self.circle.center(), end)?.reverse();
        Ok(match cross(start, end).cmp(&0) {
            Ordering::Greater => first != Ordering::Less && last != Ordering::Less,
            Ordering::Less => first != Ordering::Less || last != Ordering::Less,
            Ordering::Equal => {
                let [x, y] = start;
                let [u, v] = end;
                if x * u + y * v < 0 {
                    first != Ordering::Less
                } else {
                    first == Ordering::Equal
                        && root.side(self.circle.center(), [y, -x])? == Ordering::Greater
                }
            }
        })
    }

    /// 両端を格子へ丸めた点。1 周も始点と終点の 2 点として数える。
    pub fn endpoints<E>(
        self,
        mut check: impl FnMut(Charge) -> Result<(), E>,
    ) -> Result<[Hit; 2], Failure<E>> {
        check(Charge::Work(2 * ROUND_WORK + 2)).map_err(Failure::Stopped)?;
        let start = Root::on_ray(self.circle, self.start.vector())
            .hit()
            .map_err(Failure::Geometry)?;
        let end = Root::on_ray(self.circle, self.end.vector())
            .hit()
            .map_err(Failure::Geometry)?;
        check(Charge::Points(2)).map_err(Failure::Stopped)?;
        Ok([start, end])
    }
}

/// 円弧を含む交わり。重なる円弧は反時計回りで、始点の角度順に返す。
/// 1 周の重なりの始点は X の正の向き。離れた交点は格子点の辞書順。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArcIntersection {
    /// 交わらない。
    None,
    /// 1 点で交わる。
    Point(Hit),
    /// 異なる円が接する点。
    Tangent(Hit),
    /// 離れた 2 点で交わる。同じ格子点へ丸めても 2 点を保つ。
    Two([Hit; 2]),
    /// 1 区間で重なる。
    Overlap(Arc),
    /// 1 区間の重なりと、それに含まれない 1 点。
    OverlapAndPoint(Arc, Hit),
    /// 離れた 2 区間で重なる。
    TwoOverlaps([Arc; 2]),
}

/// 線分と円弧の交点。角度の内外は、交点を格子へ丸める前に判定する。
pub fn segment_arc_intersection<E>(
    segment: Segment,
    arc: Arc,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<CurveIntersection, Failure<E>> {
    check(Charge::Work(1)).map_err(Failure::Stopped)?;
    let roots = filter(segment_roots(segment, arc.circle), arc, None, &mut check)?;
    finish(roots, &mut check)
}

/// 円と円弧の交わり。同じ円なら円弧の範囲を返す。
pub fn circle_arc_intersection<E>(
    circle: Circle,
    arc: Arc,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<ArcIntersection, Failure<E>> {
    check(Charge::Work(1)).map_err(Failure::Stopped)?;
    let roots = circle_roots(circle, arc.circle).map_err(Failure::Geometry)?;
    if matches!(roots, Roots::Coincident) {
        let full =
            Arc::full(circle, Ray::X, Direction::CounterClockwise).map_err(Failure::Geometry)?;
        return coincident(full, arc, &mut check);
    }
    let roots = filter(roots, arc, None, &mut check)?;
    discrete(finish(roots, &mut check)?)
}

/// 円弧どうしの交点・接点・重なり。入力順と各円弧の逆向きによらない結果を返す。
pub fn arc_intersection<E>(
    a: Arc,
    b: Arc,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<ArcIntersection, Failure<E>> {
    check(Charge::Work(1)).map_err(Failure::Stopped)?;
    let roots = circle_roots(a.circle, b.circle).map_err(Failure::Geometry)?;
    if matches!(roots, Roots::Coincident) {
        return coincident(a, b, &mut check);
    }
    let roots = filter(roots, a, Some(b), &mut check)?;
    discrete(finish(roots, &mut check)?)
}

pub(super) fn filter<E>(
    roots: Roots,
    a: Arc,
    b: Option<Arc>,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<Roots, Failure<E>> {
    let mut contains = |root| {
        // 円弧ごとに最大 3 回の符号比較。短絡しても予約した作業量は戻さない。
        check(Charge::Work(if b.is_some() { 6 } else { 3 })).map_err(Failure::Stopped)?;
        let first = a.contains_root(root).map_err(Failure::Geometry)?;
        let second = b
            .map(|arc| arc.contains_root(root))
            .transpose()
            .map_err(Failure::Geometry)?
            .unwrap_or(true);
        Ok::<_, Failure<E>>(first && second)
    };
    Ok(match roots {
        Roots::One(root) => {
            if contains(root)? {
                Roots::One(root)
            } else {
                Roots::None
            }
        }
        Roots::Tangent(root) => {
            if contains(root)? {
                Roots::Tangent(root)
            } else {
                Roots::None
            }
        }
        Roots::Two([first, second]) => match (contains(first)?, contains(second)?) {
            (true, true) => Roots::Two([first, second]),
            (true, false) => Roots::One(first),
            (false, true) => Roots::One(second),
            (false, false) => Roots::None,
        },
        other => other,
    })
}

fn discrete<E>(value: CurveIntersection) -> Result<ArcIntersection, Failure<E>> {
    Ok(match value {
        CurveIntersection::None => ArcIntersection::None,
        CurveIntersection::Point(hit) => ArcIntersection::Point(hit),
        CurveIntersection::Tangent(hit) => ArcIntersection::Tangent(hit),
        CurveIntersection::Two(hits) => ArcIntersection::Two(hits),
        CurveIntersection::Coincident => return Err(Failure::Geometry(Error::Range)),
    })
}

fn coincident<E>(
    a: Arc,
    b: Arc,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<ArcIntersection, Failure<E>> {
    // 4 本の向きの整列、各点と開区間の判定、最大 4 区間ずつの走査を先に数える。
    check(Charge::Work(64)).map_err(Failure::Stopped)?;
    let mut input = [a.start, a.end, b.start, b.end];
    input.sort_unstable();
    let mut unique = [Ray::X; 4];
    let mut length = 0;
    let mut previous = None;
    let range = || Failure::Geometry(Error::Range);
    for ray in input {
        if previous != Some(ray) {
            *unique.get_mut(length).ok_or_else(range)? = ray;
            length += 1;
            previous = Some(ray);
        }
    }
    let rays = unique.get(..length).ok_or_else(range)?;
    let mut selected = [false; 4];
    for (slot, &ray) in selected.iter_mut().zip(rays) {
        *slot = a.contains_ray(ray, true) && b.contains_ray(ray, true);
    }
    let selected = selected.get(..length).ok_or_else(range)?;
    if selected.iter().all(|&value| value) {
        check(Charge::Points(2)).map_err(Failure::Stopped)?;
        return Ok(ArcIntersection::Overlap(
            Arc::full(a.circle, Ray::X, Direction::CounterClockwise).map_err(Failure::Geometry)?,
        ));
    }
    let mut spans = [None; 2];
    let mut contacts = [None; 2];
    let (mut span_count, mut point_count) = (0, 0);
    for (i, &ray) in rays.iter().enumerate() {
        let before = *selected.get((i + length - 1) % length).ok_or_else(range)?;
        let after = *selected.get(i).ok_or_else(range)?;
        if after && !before {
            let mut end = ray;
            for offset in 1..=length {
                let index = (i + offset) % length;
                if !selected.get(index).copied().ok_or_else(range)? {
                    end = *rays.get(index).ok_or_else(range)?;
                    break;
                }
            }
            let span = Arc::new(a.circle, ray, end, Direction::CounterClockwise)
                .map_err(Failure::Geometry)?;
            *spans.get_mut(span_count).ok_or_else(range)? = Some(span);
            span_count += 1;
        } else if !after && !before && a.contains_ray(ray, false) && b.contains_ray(ray, false) {
            check(Charge::Work(ROUND_WORK + 1)).map_err(Failure::Stopped)?;
            let hit = Root::on_ray(a.circle, ray.vector())
                .hit()
                .map_err(Failure::Geometry)?;
            *contacts.get_mut(point_count).ok_or_else(range)? = Some(hit);
            point_count += 1;
        }
    }
    let result = match (spans, contacts) {
        ([None, None], [None, None]) => ArcIntersection::None,
        ([None, None], [Some(hit), None]) => ArcIntersection::Point(hit),
        ([None, None], [Some(mut first), Some(mut second)]) => {
            if (first.point, first.rounded) > (second.point, second.rounded) {
                std::mem::swap(&mut first, &mut second);
            }
            ArcIntersection::Two([first, second])
        }
        ([Some(span), None], [None, None]) => ArcIntersection::Overlap(span),
        ([Some(span), None], [Some(hit), None]) => ArcIntersection::OverlapAndPoint(span, hit),
        ([Some(first), Some(second)], [None, None]) => {
            ArcIntersection::TwoOverlaps([first, second])
        }
        _ => return Err(range()),
    };
    check(Charge::Points(2 * span_count as u64 + point_count as u64)).map_err(Failure::Stopped)?;
    Ok(result)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;
    use crate::topology::Point;

    fn any_small_raw_ray() -> Ray {
        let x: i8 = kani::any();
        let y: i8 = kani::any();
        kani::assume(x != 0 || y != 0);
        Ray([i64::from(x), i64::from(y)])
    }

    #[kani::proof]
    #[kani::solver(kissat)]
    #[kani::unwind(8)]
    fn verify_ray_new_normalizes_and_rejects_zero() {
        let x: i8 = kani::any();
        let y: i8 = kani::any();
        kani::assume((-12..=12).contains(&x) && (-12..=12).contains(&y));
        match Ray::new(i64::from(x), i64::from(y)) {
            Ok(ray) => {
                assert!(x != 0 || y != 0);
                let [rx, ry] = ray.components();
                assert!(rx != 0 || ry != 0);
                // 元のベクトルと同じ向き（外積 0・内積正）
                assert_eq!(
                    i128::from(x) * i128::from(ry),
                    i128::from(y) * i128::from(rx)
                );
                assert!(i128::from(x) * i128::from(rx) + i128::from(y) * i128::from(ry) > 0);
            }
            Err(Error::Degenerate) => {
                assert!(x == 0 && y == 0);
            }
            Err(_) => unreachable!(),
        }
    }

    #[kani::proof]
    #[kani::solver(kissat)]
    fn verify_ray_ord_is_antisymmetric_and_reflexive() {
        let a = any_small_raw_ray();
        let b = any_small_raw_ray();
        assert_eq!(a.cmp(&a), Ordering::Equal);
        assert_eq!(a.cmp(&b), b.cmp(&a).reverse());
    }

    /// `Ray` の全順序公理（`Ord` と `Eq` の整合）だけを抽象化して `Arc::contains_ray` の区間論理を高速検証するスタブ
    #[allow(dead_code)]
    fn stub_ray_cmp(a: &Ray, b: &Ray) -> Ordering {
        a.0.cmp(&b.0)
    }

    #[kani::proof]
    #[kani::stub(Ray::cmp, stub_ray_cmp)]
    fn verify_arc_reversed_preserves_contains_ray() {
        let c = Circle::new(Point::new(0, 0).unwrap(), 10).unwrap();
        let start = Ray([i64::from(kani::any::<u8>()), 0]);
        let end = Ray([i64::from(kani::any::<u8>()), 0]);
        let probe = Ray([i64::from(kani::any::<u8>()), 0]);
        let after: bool = kani::any();
        let dir = if kani::any() {
            Direction::CounterClockwise
        } else {
            Direction::Clockwise
        };

        let arc = Arc::new(c, start, end, dir).unwrap();
        assert_eq!(arc.reversed().reversed(), arc);
        assert_eq!(arc.ccw(), arc.reversed().ccw());
        assert_eq!(
            arc.contains_ray(probe, after),
            arc.reversed().contains_ray(probe, after)
        );
        assert!(arc.contains_ray(start, false));
        assert!(arc.contains_ray(end, false));

        // 異なる 2 端点の補円弧どうしは半開区間（after = true）で全周を重複なく分割する
        if start != end {
            let complement = Arc::new(c, end, start, dir).unwrap();
            assert_ne!(
                arc.contains_ray(probe, true),
                complement.contains_ray(probe, true)
            );
        }

        let full = Arc::full(c, start, dir).unwrap();
        assert!(full.contains_ray(probe, after));
        assert!(full.reversed().contains_ray(probe, after));
    }
}
