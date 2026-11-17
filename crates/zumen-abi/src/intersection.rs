//! 実寸の 2 要素の交点・接点・重なり。
//!
//! 計測と同じ経路（線分と円弧、ほかの曲線は折れ線）を、許容差から作る整数格子へ移し、
//! `zumen_geom2d::topology` の整数式で全組を比べる。元の要素と ID は変更しない。

use std::f64::consts::TAU;
use zumen_base::{Cx, Kind};
use zumen_geom2d::{
    Xy,
    measure::Piece,
    topology::{
        self as t, Arc, ArcIntersection, Circle, CurveIntersection, Direction, Grid, Hit,
        MAX_COORDINATE, Point, Ray, Segment, SegmentIntersection,
    },
};

use crate::measurement::{self, Error, Path};

// 32 bit と 64 bit で同じ予算になるよう、1 件の予約量を固定する。
const CURVE_BYTES: u64 = 64;
const HIT_BYTES: u64 = 32;
const KEY_BYTES: u64 = 96;
const RESULT_POINT_BYTES: u64 = 64;
const RESULT_OVERLAP_BYTES: u64 = 128;

/// 格子へ移した経路の 1 区間。
#[derive(Clone, Copy)]
pub(crate) enum Curve {
    Segment(Segment),
    Circle(Circle),
    Arc(Arc),
}

/// 返す交点。
pub(crate) struct Crossing {
    pub position: Xy,
    pub tangent: bool,
    pub rounded: bool,
}

/// 返す重なり。円弧は反時計回りに start から end へ進む。
pub(crate) enum Overlap {
    Segment {
        start: Xy,
        end: Xy,
    },
    Arc {
        center: Xy,
        radius: f64,
        start: Xy,
        end: Xy,
    },
    Circle {
        center: Xy,
        radius: f64,
    },
}

pub(crate) struct Found {
    pub points: Vec<Crossing>,
    pub overlaps: Vec<Overlap>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Mark {
    point: Point,
    tangent: bool,
    rounded: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Segment(Point, Point),
    Arc {
        center: Point,
        radius: i64,
        start: (Point, bool),
        end: (Point, bool),
    },
    Circle(Point, i64),
}

fn geometry(error: t::Error) -> Error {
    Error::Geometry(match error {
        t::Error::NonFinite => "invalid-curve",
        t::Error::Allocation => "allocation",
        t::Error::Tolerance => "tolerance",
        _ => "numeric-range",
    })
}

fn failure(error: t::Failure<zumen_base::Stop>) -> Error {
    match error {
        t::Failure::Geometry(error) => geometry(error),
        t::Failure::Stopped(stop) => Error::Stopped(stop),
    }
}

fn reserve<T>(output: &mut Vec<T>, count: u64, slot: u64, cx: &mut Cx<'_>) -> Result<(), Error> {
    cx.check(Kind::DecodedBytes, count.saturating_mul(slot))?;
    let count = usize::try_from(count).map_err(|_| Error::Geometry("input-limit"))?;
    output
        .try_reserve(count)
        .map_err(|_| Error::Geometry("allocation"))
}

fn anchor(path: &Path) -> Option<Xy> {
    path.pieces.first().map(|piece| match *piece {
        Piece::Line { start, .. } => start,
        Piece::Arc { center, .. } => center,
    })
}

/// 両方の経路に共通の格子。原点は各経路の最初の点のうち X、Y の辞書順で小さいほうを
/// 整数へ丸めた点とし、first と second を入れ替えても同じ格子にする。
pub(crate) fn grid(first: &Path, second: &Path, tolerance: f64) -> Result<Grid, Error> {
    let origin = match (anchor(first), anchor(second)) {
        (Some(a), Some(b)) => {
            let order = a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1]));
            if order.is_le() { a } else { b }
        }
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => [0.0, 0.0],
    };
    Grid::new(origin.map(libm::round).map(|v| v + 0.0), tolerance).map_err(geometry)
}

fn snap(grid: Grid, point: Xy) -> Result<Point, Error> {
    grid.snap(point).map(|snap| snap.point).map_err(geometry)
}

fn curve(piece: &Piece, grid: Grid) -> Result<Curve, Error> {
    match *piece {
        Piece::Line { start, end } => Ok(Curve::Segment(Segment {
            start: snap(grid, start)?,
            end: snap(grid, end)?,
        })),
        Piece::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let center = snap(grid, center)?;
            let r = libm::round(radius / grid.spacing());
            if !r.is_finite() || !(0.0..=MAX_COORDINATE as f64).contains(&r) {
                return Err(Error::Geometry("numeric-range"));
            }
            let circle = Circle::new(center, r as i64).map_err(geometry)?;
            if r == 0.0 || sweep.abs() >= TAU {
                return Ok(Curve::Circle(circle));
            }
            let ray = |angle: f64| {
                Ray::new(
                    libm::round(r * libm::cos(angle)) as i64,
                    libm::round(r * libm::sin(angle)) as i64,
                )
                .map_err(geometry)
            };
            let direction = if sweep < 0.0 {
                Direction::Clockwise
            } else {
                Direction::CounterClockwise
            };
            Arc::new(circle, ray(start)?, ray(start + sweep)?, direction)
                .map(Curve::Arc)
                .map_err(geometry)
        }
    }
}

/// 経路の各区間を格子へ移す。元の座標から格子点までのずれは許容差以内に限る。
pub(crate) fn curves(path: &Path, grid: Grid, cx: &mut Cx<'_>) -> Result<Vec<Curve>, Error> {
    let n = path.pieces.len() as u64;
    cx.check(Kind::Work, n.saturating_mul(8))?;
    let mut output = Vec::new();
    reserve(&mut output, n, CURVE_BYTES, cx)?;
    for piece in &path.pieces {
        cx.check(Kind::Work, 0)?;
        output.push(curve(piece, grid)?);
    }
    Ok(output)
}

struct Collect {
    marks: Vec<Mark>,
    keys: Vec<Key>,
}

impl Collect {
    fn hit(&mut self, hit: Hit, tangent: bool, cx: &mut Cx<'_>) -> Result<(), Error> {
        reserve(&mut self.marks, 1, HIT_BYTES, cx)?;
        self.marks.push(Mark {
            point: hit.point,
            tangent,
            rounded: hit.rounded,
        });
        Ok(())
    }

    fn key(&mut self, key: Key, cx: &mut Cx<'_>) -> Result<(), Error> {
        reserve(&mut self.keys, 1, KEY_BYTES, cx)?;
        self.keys.push(key);
        Ok(())
    }

    fn curve(&mut self, found: CurveIntersection, cx: &mut Cx<'_>) -> Result<(), Error> {
        match found {
            CurveIntersection::Point(hit) => self.hit(hit, false, cx),
            CurveIntersection::Tangent(hit) => self.hit(hit, true, cx),
            CurveIntersection::Two([a, b]) => {
                self.hit(a, false, cx)?;
                self.hit(b, false, cx)
            }
            _ => Ok(()),
        }
    }

    fn arc(&mut self, arc: Arc, cx: &mut Cx<'_>) -> Result<(), Error> {
        let circle = arc.circle();
        if arc.is_full() {
            return self.key(Key::Circle(circle.center(), circle.radius()), cx);
        }
        let [start, end] = arc
            .endpoints(|value| measurement::charge(cx, value))
            .map_err(failure)?;
        self.key(
            Key::Arc {
                center: circle.center(),
                radius: circle.radius(),
                start: (start.point, start.rounded),
                end: (end.point, end.rounded),
            },
            cx,
        )
    }

    fn arcs(&mut self, found: ArcIntersection, cx: &mut Cx<'_>) -> Result<(), Error> {
        match found {
            ArcIntersection::Point(hit) => self.hit(hit, false, cx),
            ArcIntersection::Tangent(hit) => self.hit(hit, true, cx),
            ArcIntersection::Two([a, b]) => {
                self.hit(a, false, cx)?;
                self.hit(b, false, cx)
            }
            ArcIntersection::Overlap(arc) => self.arc(arc, cx),
            ArcIntersection::OverlapAndPoint(arc, hit) => {
                self.arc(arc, cx)?;
                self.hit(hit, false, cx)
            }
            ArcIntersection::TwoOverlaps([a, b]) => {
                self.arc(a, cx)?;
                self.arc(b, cx)
            }
            _ => Ok(()),
        }
    }

    fn pair(&mut self, a: Curve, b: Curve, cx: &mut Cx<'_>) -> Result<(), Error> {
        use Curve as C;
        match (a, b) {
            (C::Segment(a), C::Segment(b)) => {
                match t::segment_intersection(a, b, |value| measurement::charge(cx, value))
                    .map_err(failure)?
                {
                    SegmentIntersection::Point(hit) => self.hit(hit, false, cx),
                    SegmentIntersection::Overlap(s) => self.key(Key::Segment(s.start, s.end), cx),
                    _ => Ok(()),
                }
            }
            (C::Segment(s), C::Circle(c)) | (C::Circle(c), C::Segment(s)) => {
                let found =
                    t::segment_circle_intersection(s, c, |value| measurement::charge(cx, value))
                        .map_err(failure)?;
                self.curve(found, cx)
            }
            (C::Segment(s), C::Arc(arc)) | (C::Arc(arc), C::Segment(s)) => {
                let found =
                    t::segment_arc_intersection(s, arc, |value| measurement::charge(cx, value))
                        .map_err(failure)?;
                self.curve(found, cx)
            }
            (C::Circle(a), C::Circle(b)) => {
                let found = t::circle_intersection(a, b, |value| measurement::charge(cx, value))
                    .map_err(failure)?;
                if found == CurveIntersection::Coincident {
                    return self.key(Key::Circle(a.center(), a.radius()), cx);
                }
                self.curve(found, cx)
            }
            (C::Circle(c), C::Arc(arc)) | (C::Arc(arc), C::Circle(c)) => {
                let found =
                    t::circle_arc_intersection(c, arc, |value| measurement::charge(cx, value))
                        .map_err(failure)?;
                self.arcs(found, cx)
            }
            (C::Arc(a), C::Arc(b)) => {
                let found = t::arc_intersection(a, b, |value| measurement::charge(cx, value))
                    .map_err(failure)?;
                self.arcs(found, cx)
            }
        }
    }
}

fn sort_work(n: usize) -> u64 {
    let n = n as u64;
    n.saturating_mul(u64::from(n.max(1).ilog2()) + 1)
}

fn position(grid: Grid, point: Point) -> Result<Xy, Error> {
    grid.position(point).map_err(geometry)
}

/// 2 つの経路の全組を比べる。交点は格子点の順に並べ、同じ格子点の交点を 1 つにまとめる。
/// まとめた点は、すべてが接点のときだけ接点、すべてが丸めたときだけ丸めたものとする。
/// 重なりの端点と同じ格子点の交点は返さない。重なりは区間の組ごとで、つながる区間をつながない。
pub(crate) fn intersect(
    first: &[Curve],
    second: &[Curve],
    grid: Grid,
    cx: &mut Cx<'_>,
) -> Result<Found, Error> {
    let pairs = (first.len() as u64).saturating_mul(second.len() as u64);
    cx.check(Kind::Work, pairs)?;
    let mut collect = Collect {
        marks: Vec::new(),
        keys: Vec::new(),
    };
    for &a in first {
        for &b in second {
            cx.check(Kind::Work, 0)?;
            collect.pair(a, b, cx)?;
        }
    }
    let Collect {
        mut marks,
        mut keys,
    } = collect;
    cx.check(Kind::Work, sort_work(marks.len()) + sort_work(keys.len()))?;
    marks.sort_unstable();
    keys.sort_unstable();
    keys.dedup();
    let mut ends = Vec::new();
    reserve(&mut ends, 2 * keys.len() as u64, 16, cx)?;
    for key in &keys {
        match *key {
            Key::Segment(a, b) => ends.extend([a, b]),
            Key::Arc { start, end, .. } => ends.extend([start.0, end.0]),
            Key::Circle(..) => {}
        }
    }
    cx.check(Kind::Work, sort_work(ends.len()) + sort_work(marks.len()))?;
    ends.sort_unstable();
    let mut points: Vec<Crossing> = Vec::new();
    let mut last: Option<Mark> = None;
    let mut merged = Vec::new();
    reserve(&mut merged, marks.len() as u64, HIT_BYTES, cx)?;
    for mark in marks {
        match &mut last {
            Some(current) if current.point == mark.point => {
                current.tangent &= mark.tangent;
                current.rounded &= mark.rounded;
            }
            _ => {
                if let Some(done) = last.replace(mark) {
                    merged.push(done);
                }
            }
        }
    }
    merged.extend(last);
    merged.retain(|mark| ends.binary_search(&mark.point).is_err());
    reserve(&mut points, merged.len() as u64, RESULT_POINT_BYTES, cx)?;
    for mark in merged {
        cx.check(Kind::Work, 1)?;
        points.push(Crossing {
            position: position(grid, mark.point)?,
            tangent: mark.tangent,
            rounded: mark.rounded,
        });
    }
    let mut overlaps = Vec::new();
    reserve(&mut overlaps, keys.len() as u64, RESULT_OVERLAP_BYTES, cx)?;
    let radius = |r: i64| r as f64 * grid.spacing();
    for key in keys {
        cx.check(Kind::Work, 1)?;
        overlaps.push(match key {
            Key::Segment(a, b) => Overlap::Segment {
                start: position(grid, a)?,
                end: position(grid, b)?,
            },
            Key::Arc {
                center,
                radius: r,
                start,
                end,
            } => Overlap::Arc {
                center: position(grid, center)?,
                radius: radius(r),
                start: position(grid, start.0)?,
                end: position(grid, end.0)?,
            },
            Key::Circle(center, r) => Overlap::Circle {
                center: position(grid, center)?,
                radius: radius(r),
            },
        });
    }
    Ok(Found { points, overlaps })
}
