//! 切断位置を格子へ移し、元の線や円の上の区間として編集結果を返す。

use std::cmp::Ordering;

use super::arcs::filter;
use super::curves::{ROUND_WORK, Root, Roots, circle_roots, line_roots};
use super::{
    Arc, Charge, Circle, Direction, Error, Failure, Grid, Hit, Point, Ray, Segment, cross,
    difference,
};

/// 切断・延長の境界。線分と円弧は有限の範囲だけを使う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    /// 線分。
    Segment(Segment),
    /// 円。
    Circle(Circle),
    /// 円弧。
    Arc(Arc),
}

/// 延長する側の端点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// 始点を、始点から終点へ進む向きとは逆へ延ばす。
    Start,
    /// 終点を、始点から終点へ進む向きへ延ばす。
    End,
}

/// 採用した境界と、元の交点の丸め。利用側は境界と対象の出所を診断に付ける。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cut {
    /// 渡された境界の列での位置。同じ格子位置なら先の境界を選ぶ。
    pub boundary: u64,
    /// 元の交点を格子に丸めた点。
    pub intersection: Hit,
    /// 格子へ丸めた点を元の線・円の上へ投影したため、正確な切断位置と異なる。
    pub adjusted: bool,
}

/// 指定した位置を含む開区間を削った結果。入力を変更せず、元の向きの順で返す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trimmed<T> {
    /// 残す区間。最大 2 つ。切断点がなければ先頭に入力全体を返す。
    pub kept: [Option<T>; 2],
    /// 削った区間。切断点がない場合は `None`。
    pub removed: Option<T>,
    /// 削った区間の始点・終点を決めた境界。元の端点なら `None`。
    pub cuts: [Option<Cut>; 2],
}

/// 最初に当たる境界まで延ばした結果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extended<T> {
    /// 元の向きを保つ曲線。境界がなければ入力全体。
    pub curve: T,
    /// 採用した境界。延ばせる交点がなければ `None`。
    pub cut: Option<Cut>,
}

/// 元の線分を含む直線上の区間。端点を丸めた点で線の向きを変えない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentPiece {
    source: Segment,
    from: i128,
    to: i128,
    denominator: i128,
}

impl SegmentPiece {
    /// 長さのある線分全体を表す。
    pub fn new(source: Segment) -> Result<Self, Error> {
        let r = difference(source.end, source.start);
        let denominator = dot(r, r);
        if denominator == 0 {
            return Err(Error::Degenerate);
        }
        Ok(Self {
            source,
            from: 0,
            to: denominator,
            denominator,
        })
    }

    /// 向きを定める元の線分。
    pub fn source(self) -> Segment {
        self.source
    }

    /// 始点を 0、終点を 1 とした両端の有理数。各組は分子・正の分母。
    pub fn parameters(self) -> [[i128; 2]; 2] {
        [[self.from, self.denominator], [self.to, self.denominator]]
    }

    fn parameter(self, point: Point) -> i128 {
        dot(
            difference(point, self.source.start),
            difference(self.source.end, self.source.start),
        )
    }

    fn root(self, numerator: i128) -> Root {
        let Point([x, y]) = self.source.start;
        let [dx, dy] = difference(self.source.end, self.source.start);
        Root::rational(
            [
                i128::from(x) * self.denominator + dx * numerator,
                i128::from(y) * self.denominator + dy * numerator,
            ],
            self.denominator,
        )
    }

    /// 区間の端点を格子へ丸める。元の直線上の端点そのものは [`Self::positions`] で返す。
    pub fn endpoints<E>(
        self,
        mut check: impl FnMut(Charge) -> Result<(), E>,
    ) -> Result<[Hit; 2], Failure<E>> {
        check(Charge::Work(2 * ROUND_WORK + 2)).map_err(Failure::Stopped)?;
        let points = [
            self.root(self.from).hit().map_err(Failure::Geometry)?,
            self.root(self.to).hit().map_err(Failure::Geometry)?,
        ];
        check(Charge::Points(2)).map_err(Failure::Stopped)?;
        Ok(points)
    }

    /// 有理数のパラメーターから元の直線上の座標を求め、格子の原点と間隔で戻す。
    /// 浮動小数で座標を保てない場合は失敗する。
    pub fn positions<E>(
        self,
        grid: Grid,
        mut check: impl FnMut(Charge) -> Result<(), E>,
    ) -> Result<[crate::Xy; 2], Failure<E>> {
        check(Charge::Work(2 * ROUND_WORK + 8)).map_err(Failure::Stopped)?;
        let Point([x, y]) = self.source.start;
        let [dx, dy] = difference(self.source.end, self.source.start);
        let [ox, oy] = grid.origin();
        let position = |numerator| {
            let t = numerator as f64 / self.denominator as f64;
            let point = [
                ox + (x as f64 + dx as f64 * t) * grid.spacing(),
                oy + (y as f64 + dy as f64 * t) * grid.spacing(),
            ];
            let point = point.map(|value| if value == 0.0 { 0.0 } else { value });
            let snap = grid.position(self.root(numerator).hit()?.point)?;
            let [px, py] = point;
            let [sx, sy] = snap;
            if !point.into_iter().all(f64::is_finite)
                || libm::hypot(px - sx, py - sy) > grid.tolerance()
            {
                return Err(Error::Range);
            }
            Ok(point)
        };
        let points = [
            position(self.from).map_err(Failure::Geometry)?,
            position(self.to).map_err(Failure::Geometry)?,
        ];
        check(Charge::Points(2)).map_err(Failure::Stopped)?;
        Ok(points)
    }
}

fn dot([x, y]: [i128; 2], [u, v]: [i128; 2]) -> i128 {
    x * u + y * v
}

fn unchanged<T: Copy>(curve: T) -> Trimmed<T> {
    Trimmed {
        kept: [Some(curve), None],
        removed: None,
        cuts: [None, None],
    }
}

fn count_trim<E, T>(
    result: Trimmed<T>,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<Trimmed<T>, Failure<E>> {
    let pieces = result.kept.iter().filter(|piece| piece.is_some()).count()
        + usize::from(result.removed.is_some());
    check(Charge::Points(2 * pieces as u64)).map_err(Failure::Stopped)?;
    Ok(result)
}

/// 線分の指定位置を含む区間を削る。指定位置は元の直線へ垂直に投影する。
/// 内部の切断点がなければ入力を保つ。切断点そのものの指定は `Ambiguous`。
pub fn trim_segment<E>(
    source: Segment,
    boundaries: &[Boundary],
    pick: Point,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<Trimmed<SegmentPiece>, Failure<E>> {
    let whole = SegmentPiece::new(source).map_err(Failure::Geometry)?;
    let selected = whole.parameter(pick);
    if !(0..=whole.denominator).contains(&selected) {
        return Err(Failure::Geometry(Error::Range));
    }
    let (mut low, mut high) = (0, whole.denominator);
    let mut cuts = [None, None];
    let mut found = false;
    visit(
        Target::Line(source, true),
        boundaries,
        &mut check,
        |root, intersection, boundary| {
            let value = whole.parameter(intersection.point);
            if !(0..=whole.denominator).contains(&value) {
                return Err(Error::Precision);
            }
            if value == 0 || value == whole.denominator {
                let endpoint = if value == 0 { source.start } else { source.end };
                if along(root, source, endpoint)? != Ordering::Equal {
                    return Err(Error::Precision);
                }
                return Ok(());
            }
            let exact = along(root, source, pick)?;
            if exact == Ordering::Equal {
                return Err(Error::Ambiguous);
            }
            if value.cmp(&selected) != exact {
                return Err(Error::Precision);
            }
            found = true;
            let cut = Cut {
                boundary,
                intersection,
                adjusted: intersection.rounded,
            };
            if value < selected && value > low {
                low = value;
                cuts[0] = Some(cut);
            }
            if value > selected && value < high {
                high = value;
                cuts[1] = Some(cut);
            }
            Ok(())
        },
    )?;
    if !found {
        return count_trim(unchanged(whole), &mut check);
    }
    let prefix = (low > 0).then_some(SegmentPiece { to: low, ..whole });
    let suffix = (high < whole.denominator).then_some(SegmentPiece {
        from: high,
        ..whole
    });
    let kept = if prefix.is_some() {
        [prefix, suffix]
    } else {
        [suffix, None]
    };
    count_trim(
        Trimmed {
            kept,
            removed: Some(SegmentPiece {
                from: low,
                to: high,
                ..whole
            }),
            cuts,
        },
        &mut check,
    )
}

/// 線分の選んだ端点を、元の直線上で最初に当たる有限の境界まで延長する。
pub fn extend_segment<E>(
    source: Segment,
    end: End,
    boundaries: &[Boundary],
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<Extended<SegmentPiece>, Failure<E>> {
    let whole = SegmentPiece::new(source).map_err(Failure::Geometry)?;
    let mut result = Extended {
        curve: whole,
        cut: None,
    };
    let endpoint = match end {
        End::Start => source.start,
        End::End => source.end,
    };
    visit(
        Target::Line(source, false),
        boundaries,
        &mut check,
        |root, intersection, boundary| {
            let direction = along(root, source, endpoint)?;
            if direction
                != match end {
                    End::Start => Ordering::Less,
                    End::End => Ordering::Greater,
                }
            {
                return Ok(());
            }
            let value = whole.parameter(intersection.point);
            let beyond = match end {
                End::Start => value < 0,
                End::End => value > whole.denominator,
            };
            if !beyond {
                return Err(Error::Precision);
            }
            let closer = result.cut.is_none()
                || match end {
                    End::Start => value > result.curve.from,
                    End::End => value < result.curve.to,
                };
            if closer {
                match end {
                    End::Start => result.curve.from = value,
                    End::End => result.curve.to = value,
                }
                result.cut = Some(Cut {
                    boundary,
                    intersection,
                    adjusted: intersection.rounded,
                });
            }
            Ok(())
        },
    )?;
    // 投影後の端点も座標範囲内であることを、結果を返す前に確かめる。
    result.curve.endpoints(&mut check)?;
    Ok(result)
}

fn along(root: Root, source: Segment, origin: Point) -> Result<Ordering, Error> {
    let [x, y] = difference(source.end, source.start);
    root.side(origin, [y, -x])
}

fn angle_cmp(origin: Ray, a: Ray, b: Ray, direction: Direction) -> Ordering {
    match direction {
        Direction::CounterClockwise => (a < origin).cmp(&(b < origin)).then_with(|| a.cmp(&b)),
        Direction::Clockwise => (a > origin).cmp(&(b > origin)).then_with(|| b.cmp(&a)),
    }
}

fn ray_at(source: Arc, point: Point) -> Result<Ray, Error> {
    let [x, y] = difference(point, source.circle().center());
    Ray::new(
        i64::try_from(x).map_err(|_| Error::Range)?,
        i64::try_from(y).map_err(|_| Error::Range)?,
    )
}

fn on_ray(source: Arc, root: Root, ray: Ray) -> Result<bool, Error> {
    Arc::new(source.circle(), ray, ray, source.direction())?.contains_root(root)
}

fn piece(source: Arc, start: Ray, end: Ray) -> Result<Arc, Error> {
    Arc::new(source.circle(), start, end, source.direction())
}

/// 円弧の指定した向きを含む区間を削る。指定点から中心への向きで区間を選ぶ。
/// 1 周では異なる切断位置が 2 つ以上あるときだけ区間を削る。
pub fn trim_arc<E>(
    source: Arc,
    boundaries: &[Boundary],
    pick: Point,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<Trimmed<Arc>, Failure<E>> {
    if !source.is_full() && source.start() == source.end() {
        return Err(Failure::Geometry(Error::Degenerate));
    }
    check(Charge::Work(84)).map_err(Failure::Stopped)?;
    let pick = ray_at(source, pick).map_err(Failure::Geometry)?;
    if !source.contains_ray(pick, false) {
        return Err(Failure::Geometry(Error::Range));
    }
    let (mut low, mut high) = (source.start(), source.end());
    let (mut before, mut after) = (None, None);
    visit(
        Target::Arc(source),
        boundaries,
        &mut check,
        |root, intersection, boundary| {
            if !source.is_full()
                && (on_ray(source, root, source.start())? || on_ray(source, root, source.end())?)
            {
                return Ok(());
            }
            if on_ray(source, root, pick)? {
                return Err(Error::Ambiguous);
            }
            let ray = ray_at(source, intersection.point)?;
            if ray == pick || !source.contains_ray(ray, false) {
                return Err(Error::Precision);
            }
            let exact_side = root.side(source.circle().center(), pick.vector())?;
            let grid_side = cross(pick.vector(), ray.vector()).cmp(&0);
            if exact_side != Ordering::Equal
                && grid_side != Ordering::Equal
                && exact_side != grid_side
            {
                return Err(Error::Precision);
            }
            let cut = Cut {
                boundary,
                intersection,
                adjusted: intersection.rounded,
            };
            if source.is_full() {
                if after.is_none()
                    || angle_cmp(pick, ray, high, source.direction()) == Ordering::Less
                {
                    high = ray;
                    after = Some(cut);
                }
                if before.is_none()
                    || angle_cmp(pick, ray, low, source.direction()) == Ordering::Greater
                {
                    low = ray;
                    before = Some(cut);
                }
            } else {
                if ray == source.start() || ray == source.end() {
                    return Err(Error::Precision);
                }
                let order = angle_cmp(source.start(), ray, pick, source.direction());
                if order == Ordering::Less
                    && angle_cmp(source.start(), ray, low, source.direction()) == Ordering::Greater
                {
                    low = ray;
                    before = Some(cut);
                }
                if order == Ordering::Greater
                    && angle_cmp(source.start(), ray, high, source.direction()) == Ordering::Less
                {
                    high = ray;
                    after = Some(cut);
                }
            }
            Ok(())
        },
    )?;
    if before.is_none() && after.is_none() || source.is_full() && low == high {
        return count_trim(unchanged(source), &mut check);
    }
    let removed = piece(source, low, high).map_err(Failure::Geometry)?;
    let kept = if source.is_full() {
        [
            Some(piece(source, high, low).map_err(Failure::Geometry)?),
            None,
        ]
    } else {
        let prefix = if low != source.start() {
            Some(piece(source, source.start(), low).map_err(Failure::Geometry)?)
        } else {
            None
        };
        let suffix = if high != source.end() {
            Some(piece(source, high, source.end()).map_err(Failure::Geometry)?)
        } else {
            None
        };
        if prefix.is_some() {
            [prefix, suffix]
        } else {
            [suffix, None]
        }
    };
    count_trim(
        Trimmed {
            kept,
            removed: Some(removed),
            cuts: [before, after],
        },
        &mut check,
    )
}

/// 円弧の選んだ端点を、元の円上で最初に当たる境界まで延長する。
/// 反対側の端点に達したら 1 周とし、1 周を超えて延ばさない。
pub fn extend_arc<E>(
    source: Arc,
    end: End,
    boundaries: &[Boundary],
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<Extended<Arc>, Failure<E>> {
    check(Charge::Work(1)).map_err(Failure::Stopped)?;
    let mut result = Extended {
        curve: source,
        cut: None,
    };
    if source.is_full() {
        check(Charge::Points(2)).map_err(Failure::Stopped)?;
        return Ok(result);
    }
    let (origin, other, direction) = match end {
        End::End => (source.end(), source.start(), source.direction()),
        End::Start => (source.start(), source.end(), source.reversed().direction()),
    };
    let full = Arc::full(source.circle(), origin, direction).map_err(Failure::Geometry)?;
    let mut nearest = origin;
    visit(
        Target::Arc(full),
        boundaries,
        &mut check,
        |root, intersection, boundary| {
            let at_other = source.start() != source.end() && on_ray(source, root, other)?;
            if !at_other && source.contains_root(root)? {
                return Ok(());
            }
            let ray = if at_other {
                other
            } else {
                ray_at(source, intersection.point)?
            };
            if ray == origin || !at_other && source.contains_ray(ray, false) {
                return Err(Error::Precision);
            }
            if !at_other {
                let exact_side = root.side(source.circle().center(), origin.vector())?;
                let grid_side = cross(origin.vector(), ray.vector()).cmp(&0);
                if exact_side != Ordering::Equal
                    && grid_side != Ordering::Equal
                    && exact_side != grid_side
                {
                    return Err(Error::Precision);
                }
            }
            if result.cut.is_none() || angle_cmp(origin, ray, nearest, direction) == Ordering::Less
            {
                nearest = ray;
                result.curve = if at_other {
                    let anchor = match end {
                        End::Start => source.end(),
                        End::End => source.start(),
                    };
                    Arc::full(source.circle(), anchor, source.direction())?
                } else {
                    match end {
                        End::Start => piece(source, ray, source.end())?,
                        End::End => piece(source, source.start(), ray)?,
                    }
                };
                result.cut = Some(Cut {
                    boundary,
                    intersection,
                    adjusted: !at_other && intersection.rounded,
                });
            }
            Ok(())
        },
    )?;
    check(Charge::Points(2)).map_err(Failure::Stopped)?;
    Ok(result)
}

#[derive(Clone, Copy)]
enum Target {
    Line(Segment, bool),
    Arc(Arc),
}

fn visit<E>(
    target: Target,
    boundaries: &[Boundary],
    check: &mut impl FnMut(Charge) -> Result<(), E>,
    mut use_root: impl FnMut(Root, Hit, u64) -> Result<(), Error>,
) -> Result<(), Failure<E>> {
    let count = u64::try_from(boundaries.len()).map_err(|_| Failure::Geometry(Error::Range))?;
    check(Charge::Work(
        count
            .checked_add(1)
            .ok_or(Failure::Geometry(Error::Range))?,
    ))
    .map_err(Failure::Stopped)?;
    for (index, &boundary) in boundaries.iter().enumerate() {
        check(Charge::Work(0)).map_err(Failure::Stopped)?;
        let roots = roots(target, boundary, check)?;
        let roots = match roots {
            Roots::None => [None, None],
            Roots::One(root) | Roots::Tangent(root) => [Some(root), None],
            Roots::Two([first, second]) => [Some(first), Some(second)],
            Roots::Coincident => return Err(Failure::Geometry(Error::Ambiguous)),
        };
        for root in roots.into_iter().flatten() {
            // 根の丸めと、方向の正規化・最大 10 回の符号比較・区間の比較を先に数える。
            check(Charge::Work(ROUND_WORK + 128)).map_err(Failure::Stopped)?;
            let hit = root.hit().map_err(Failure::Geometry)?;
            check(Charge::Points(1)).map_err(Failure::Stopped)?;
            use_root(root, hit, index as u64).map_err(Failure::Geometry)?;
        }
    }
    Ok(())
}

fn roots<E>(
    target: Target,
    boundary: Boundary,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<Roots, Failure<E>> {
    check(Charge::Work(1)).map_err(Failure::Stopped)?;
    let (roots, first_arc, second_arc) = match (target, boundary) {
        (Target::Line(a, bounded), Boundary::Segment(b)) => (
            line_line(a, b, bounded).map_err(Failure::Geometry)?,
            None,
            None,
        ),
        (Target::Line(a, bounded), Boundary::Circle(b)) => (line_roots(a, b, bounded), None, None),
        (Target::Line(a, bounded), Boundary::Arc(b)) => {
            (line_roots(a, b.circle(), bounded), Some(b), None)
        }
        (Target::Arc(a), Boundary::Segment(b)) => (line_roots(b, a.circle(), true), Some(a), None),
        (Target::Arc(a), Boundary::Circle(b)) => (
            circle_roots(a.circle(), b).map_err(Failure::Geometry)?,
            Some(a),
            None,
        ),
        (Target::Arc(a), Boundary::Arc(b)) => (
            circle_roots(a.circle(), b.circle()).map_err(Failure::Geometry)?,
            Some(a),
            Some(b),
        ),
    };
    // 同じ円でも、離れた円弧と端点だけの接触は区別する。
    if matches!(roots, Roots::Coincident) {
        if let (Some(a), Some(b)) = (first_arc, second_arc) {
            let found = super::arc_intersection(a, b, &mut *check)?;
            match found {
                super::ArcIntersection::None => return Ok(Roots::None),
                super::ArcIntersection::Point(_) | super::ArcIntersection::Two(_) => {
                    // 4 つの端点について、2 円弧の範囲と重複を比較する。
                    check(Charge::Work(64)).map_err(Failure::Stopped)?;
                    let mut selected = [None, None];
                    for ray in [a.start(), a.end(), b.start(), b.end()] {
                        if a.contains_ray(ray, false)
                            && b.contains_ray(ray, false)
                            && !selected.contains(&Some(ray))
                        {
                            if let Some(slot) = selected.iter_mut().find(|slot| slot.is_none()) {
                                *slot = Some(ray);
                            } else {
                                return Err(Failure::Geometry(Error::Ambiguous));
                            }
                        }
                    }
                    return Ok(match selected {
                        [Some(a_ray), Some(b_ray)] => Roots::Two([
                            Root::on_ray(a.circle(), a_ray.vector()),
                            Root::on_ray(a.circle(), b_ray.vector()),
                        ]),
                        [Some(ray), None] => Roots::One(Root::on_ray(a.circle(), ray.vector())),
                        _ => Roots::None,
                    });
                }
                _ => {}
            }
        }
        return Err(Failure::Geometry(Error::Ambiguous));
    }
    match first_arc {
        Some(arc) => filter(roots, arc, second_arc, check),
        None => Ok(roots),
    }
}

fn line_line(a: Segment, b: Segment, bounded: bool) -> Result<Roots, Error> {
    let r = difference(a.end, a.start);
    let s = difference(b.end, b.start);
    let q = difference(b.start, a.start);
    let mut denominator = cross(r, s);
    let mut numerator = cross(q, s);
    let mut other = cross(q, r);
    if denominator == 0 {
        if other != 0 {
            return Ok(Roots::None);
        }
        if s != [0, 0] {
            if bounded {
                let (a0, a1) = super::ends(a);
                let (b0, b1) = super::ends(b);
                let (start, end) = (a0.max(b0), a1.min(b1));
                if start > end {
                    return Ok(Roots::None);
                }
                if start == end {
                    return Ok(Roots::One(Root::point(start)));
                }
            }
            return Err(Error::Ambiguous);
        }
        let projection = dot(q, r);
        return Ok(if !bounded || (0..=dot(r, r)).contains(&projection) {
            Roots::One(Root::point(b.start))
        } else {
            Roots::None
        });
    }
    if denominator < 0 {
        denominator = -denominator;
        numerator = -numerator;
        other = -other;
    }
    if !(0..=denominator).contains(&other) || bounded && !(0..=denominator).contains(&numerator) {
        return Ok(Roots::None);
    }
    let Point([x, y]) = a.start;
    let [dx, dy] = r;
    Ok(Roots::One(Root::rational(
        [
            i128::from(x) * denominator + dx * numerator,
            i128::from(y) * denominator + dy * numerator,
        ],
        denominator,
    )))
}
