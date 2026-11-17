//! 実寸の要素のオフセット・トリム・延長・面取り・フィレット。
//!
//! 計測と同じ経路を交点と同じ整数格子へ移し、`zumen_geom2d::topology` の作図と編集を使う。
//! 元の端点・中心・半径を保つ部分は元の値を返し、作図した点と切断点だけを格子から戻す。

use std::f64::consts::PI;
use zumen_base::{Cx, Stop};
use zumen_geom2d::{
    Xy,
    measure::Piece,
    topology::{self as t, Arc, Direction, Grid, Point, Ray, Segment, SegmentPiece},
};

use crate::{
    intersection::Curve,
    measurement::{self, Error},
};

/// 作図の種類。距離と半径は空間の長さの単位。
#[derive(Clone, Copy)]
pub(crate) enum Operation {
    Offset(f64),
    Trim(Point),
    Extend(t::End),
    Chamfer([f64; 2]),
    Fillet(f64),
}

/// 返す形。円弧は反時計回りに start から end（度）へ進む。
pub(crate) enum Shape {
    Line(Xy, Xy),
    Circle(Xy, f64),
    Arc {
        center: Xy,
        radius: f64,
        start: f64,
        end: f64,
    },
}

/// 採用した境界と切断点。boundary は境界の列での位置。
pub(crate) struct Cut {
    pub boundary: u64,
    pub position: Xy,
    pub rounded: bool,
    pub adjusted: bool,
}

/// target、second の順に置き換える形。置き換えない要素は `None`。
pub(crate) type Replace = [Option<Vec<Shape>>; 2];

/// 作図の結果。
pub(crate) struct Built {
    pub replace: Replace,
    pub add: Option<Shape>,
    pub cuts: Vec<Cut>,
}

/// 作図に使う要素。piece は元の座標の経路、curve は格子へ移した同じ曲線。
#[derive(Clone, Copy)]
pub(crate) struct Source<'a> {
    pub piece: &'a Piece,
    pub curve: Curve,
}

fn reason(error: t::Error) -> Error {
    Error::Geometry(match error {
        t::Error::NoSolution => "no-solution",
        t::Error::Degenerate => "empty-curve",
        t::Error::Ambiguous => "ambiguous-pick",
        t::Error::Precision => "precision",
        t::Error::NonFinite => "invalid-curve",
        t::Error::Allocation => "allocation",
        t::Error::Tolerance => "tolerance",
        _ => "numeric-range",
    })
}

fn failure(error: t::Failure<Stop>) -> Error {
    match error {
        t::Failure::Geometry(error) => reason(error),
        t::Failure::Stopped(stop) => Error::Stopped(stop),
    }
}

fn positive_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn degrees(radians: f64) -> f64 {
    positive_zero(radians * (180.0 / PI))
}

/// 中心からの向きの角度。0 度以上 360 度未満にする。
fn ray_degrees(ray: Ray) -> f64 {
    let [x, y] = ray.components();
    let value = degrees(libm::atan2(y as f64, x as f64));
    if value < 0.0 { value + 360.0 } else { value }
}

fn point(grid: Grid, value: t::ConstructionPoint) -> Result<Xy, Error> {
    value.position(grid).map_err(reason)
}

fn line_ends(piece: &Piece) -> Result<(Xy, Xy), Error> {
    match *piece {
        Piece::Line { start, end } => Ok((start, end)),
        Piece::Arc { .. } => Err(Error::Geometry("unsupported-shape")),
    }
}

/// 線分の区間。元の端点に当たる側は元の座標を返す。
fn segment_shape(
    piece: SegmentPiece,
    original: (Xy, Xy),
    grid: Grid,
    cx: &mut Cx<'_>,
) -> Result<Shape, Error> {
    let [[from, denominator], [to, _]] = piece.parameters();
    let [start, end] = piece
        .positions(grid, |charge| measurement::charge(cx, charge))
        .map_err(failure)?;
    Ok(Shape::Line(
        if from == 0 { original.0 } else { start },
        if to == denominator { original.1 } else { end },
    ))
}

/// 円弧の区間。中心と半径、元の端点に当たる角度は元の値を返す。
fn arc_shape(piece: Arc, source: Arc, original: &Piece) -> Result<Shape, Error> {
    let Piece::Arc {
        center,
        radius,
        start,
        sweep,
    } = *original
    else {
        return Err(Error::Geometry("unsupported-shape"));
    };
    if piece.is_full() {
        return Ok(Shape::Circle(center, radius));
    }
    let first = if !source.is_full() && piece.start() == source.start() {
        degrees(start)
    } else {
        ray_degrees(piece.start())
    };
    let last = if !source.is_full() && piece.end() == source.end() {
        degrees(start + sweep)
    } else {
        ray_degrees(piece.end())
    };
    let (start, end) = match piece.direction() {
        Direction::CounterClockwise => (first, last),
        Direction::Clockwise => (last, first),
    };
    Ok(Shape::Arc {
        center,
        radius,
        start,
        end,
    })
}

fn constructed_arc(value: t::ArcGeometry, grid: Grid) -> Result<Shape, Error> {
    let radius = value.circle.radius * grid.spacing();
    if !radius.is_finite() || radius <= 0.0 {
        return Err(Error::Geometry("no-solution"));
    }
    let start = value.start_degrees;
    let end = start + value.sweep_degrees;
    let (start, end) = if value.sweep_degrees < 0.0 {
        (end, start)
    } else {
        (start, end)
    };
    // 始まりを 0 度以上 360 度未満にそろえる。
    let (start, end) = if start < 0.0 {
        (start + 360.0, end + 360.0)
    } else {
        (start, end)
    };
    Ok(Shape::Arc {
        center: point(grid, value.circle.center)?,
        radius,
        start: positive_zero(start),
        end: positive_zero(end),
    })
}

fn cut(grid: Grid, value: t::Cut) -> Result<Cut, Error> {
    Ok(Cut {
        boundary: value.boundary,
        position: grid.position(value.intersection.point).map_err(reason)?,
        rounded: value.intersection.rounded,
        adjusted: value.adjusted,
    })
}

/// 円は X の正の向きから反時計回りの 1 周として編集する。
fn as_arc(curve: Curve) -> Result<Option<Arc>, Error> {
    match curve {
        Curve::Segment(_) => Ok(None),
        Curve::Circle(circle) => Arc::full(circle, Ray::X, Direction::CounterClockwise)
            .map(Some)
            .map_err(reason),
        Curve::Arc(arc) => Ok(Some(arc)),
    }
}

fn offset(target: Source<'_>, distance: f64, grid: Grid, cx: &mut Cx<'_>) -> Result<Shape, Error> {
    let distance = distance / grid.spacing();
    let check = |charge| measurement::charge(cx, charge);
    match target.curve {
        Curve::Segment(segment) => {
            let value = t::offset_segment(segment, distance, check).map_err(failure)?;
            Ok(Shape::Line(
                point(grid, value.start)?,
                point(grid, value.end)?,
            ))
        }
        Curve::Circle(circle) => {
            let value = t::offset_circle(circle, distance, check).map_err(failure)?;
            let radius = value.radius * grid.spacing();
            if !radius.is_finite() || radius <= 0.0 {
                return Err(Error::Geometry("no-solution"));
            }
            Ok(Shape::Circle(point(grid, value.center)?, radius))
        }
        Curve::Arc(arc) => {
            // 外側を正にする。反時計回りの円弧の左は内側。
            let distance = match arc.direction() {
                Direction::CounterClockwise => -distance,
                Direction::Clockwise => distance,
            };
            constructed_arc(t::offset_arc(arc, distance, check).map_err(failure)?, grid)
        }
    }
}

fn trim(
    target: Source<'_>,
    boundaries: &[t::Boundary],
    pick: Point,
    grid: Grid,
    cx: &mut Cx<'_>,
) -> Result<(Vec<Shape>, Vec<Cut>), Error> {
    // 対象の外の指定位置は、範囲の失敗と区別して返す。
    let pick_failure = |error| match error {
        t::Failure::Geometry(t::Error::Range) => Error::Geometry("pick-outside"),
        error => failure(error),
    };
    let mut shapes = Vec::new();
    let mut cuts = Vec::new();
    match as_arc(target.curve)? {
        None => {
            let Curve::Segment(segment) = target.curve else {
                return Err(Error::Geometry("unsupported-shape"));
            };
            let result = t::trim_segment(segment, boundaries, pick, |charge| {
                measurement::charge(cx, charge)
            })
            .map_err(pick_failure)?;
            if result.removed.is_none() {
                return Err(Error::Geometry("no-intersection"));
            }
            let original = line_ends(target.piece)?;
            for piece in result.kept.into_iter().flatten() {
                shapes.push(segment_shape(piece, original, grid, cx)?);
            }
            for value in result.cuts.into_iter().flatten() {
                cuts.push(cut(grid, value)?);
            }
        }
        Some(arc) => {
            let result = t::trim_arc(arc, boundaries, pick, |charge| {
                measurement::charge(cx, charge)
            })
            .map_err(pick_failure)?;
            if result.removed.is_none() {
                return Err(Error::Geometry("no-intersection"));
            }
            for piece in result.kept.into_iter().flatten() {
                shapes.push(arc_shape(piece, arc, target.piece)?);
            }
            for value in result.cuts.into_iter().flatten() {
                cuts.push(cut(grid, value)?);
            }
        }
    }
    Ok((shapes, cuts))
}

fn extend(
    target: Source<'_>,
    boundaries: &[t::Boundary],
    end: t::End,
    grid: Grid,
    cx: &mut Cx<'_>,
) -> Result<(Shape, Cut), Error> {
    match target.curve {
        Curve::Segment(segment) => {
            let result = t::extend_segment(segment, end, boundaries, |charge| {
                measurement::charge(cx, charge)
            })
            .map_err(failure)?;
            let value = result.cut.ok_or(Error::Geometry("no-intersection"))?;
            let shape = segment_shape(result.curve, line_ends(target.piece)?, grid, cx)?;
            Ok((shape, cut(grid, value)?))
        }
        Curve::Circle(_) => Err(Error::Geometry("unsupported-shape")),
        Curve::Arc(arc) => {
            let result = t::extend_arc(arc, end, boundaries, |charge| {
                measurement::charge(cx, charge)
            })
            .map_err(failure)?;
            let value = result.cut.ok_or(Error::Geometry("no-intersection"))?;
            Ok((
                arc_shape(result.curve, arc, target.piece)?,
                cut(grid, value)?,
            ))
        }
    }
}

/// 2 本の線分が格子で共有する端点。target の終点、target の始点の順に探す。
/// 返す値は角と、角が target の終点か、second の始点か。
fn shared(first: Segment, second: Segment) -> Option<(Point, bool, bool)> {
    [
        (first.end, second.start, true, true),
        (first.end, second.end, true, false),
        (first.start, second.start, false, true),
        (first.start, second.end, false, false),
    ]
    .into_iter()
    .find(|(a, b, _, _)| a == b)
    .map(|(vertex, _, at_end, at_start)| (vertex, at_end, at_start))
}

fn corner(
    target: Source<'_>,
    second: Source<'_>,
    operation: Operation,
    grid: Grid,
    cx: &mut Cx<'_>,
) -> Result<(Replace, Shape), Error> {
    let (Curve::Segment(a), Curve::Segment(b)) = (target.curve, second.curve) else {
        return Err(Error::Geometry("unsupported-shape"));
    };
    let (vertex, first_at_end, second_at_start) =
        shared(a, b).ok_or(Error::Geometry("no-corner"))?;
    let (a_start, a_end) = line_ends(target.piece)?;
    let (b_start, b_end) = line_ends(second.piece)?;
    let (first_far, first_original) = if first_at_end {
        (a.start, a_start)
    } else {
        (a.end, a_end)
    };
    let (second_far, second_original) = if second_at_start {
        (b.end, b_end)
    } else {
        (b.start, b_start)
    };
    let value = t::Corner::new(vertex, first_far, second_far).map_err(reason)?;
    let check = |charge| measurement::charge(cx, charge);
    let (kept, add) = match operation {
        Operation::Chamfer([first, second]) => {
            let spacing = grid.spacing();
            let result =
                t::chamfer(value, [first / spacing, second / spacing], check).map_err(failure)?;
            let join = Shape::Line(
                point(grid, result.join.start)?,
                point(grid, result.join.end)?,
            );
            (result.kept, join)
        }
        Operation::Fillet(radius) => {
            let result = t::fillet(value, radius / grid.spacing(), check).map_err(failure)?;
            (result.kept, constructed_arc(result.arc, grid)?)
        }
        _ => return Err(Error::Geometry("unsupported-shape")),
    };
    let [first_kept, second_kept] = kept;
    // 残す線分は元の向きに戻し、反対側の端点は元の座標を返す。
    let first = match first_kept {
        None => Vec::new(),
        Some(line) => {
            let contact = point(grid, line.end)?;
            vec![if first_at_end {
                Shape::Line(first_original, contact)
            } else {
                Shape::Line(contact, first_original)
            }]
        }
    };
    let second = match second_kept {
        None => Vec::new(),
        Some(line) => {
            let contact = point(grid, line.start)?;
            vec![if second_at_start {
                Shape::Line(contact, second_original)
            } else {
                Shape::Line(second_original, contact)
            }]
        }
    };
    Ok(([Some(first), Some(second)], add))
}

/// 作図の結果を求める。boundaries は格子へ移した境界の曲線。
pub(crate) fn construct(
    target: Source<'_>,
    second: Option<Source<'_>>,
    boundaries: &[t::Boundary],
    operation: Operation,
    grid: Grid,
    cx: &mut Cx<'_>,
) -> Result<Built, Error> {
    let mut built = Built {
        replace: [None, None],
        add: None,
        cuts: Vec::new(),
    };
    match operation {
        Operation::Offset(distance) => built.add = Some(offset(target, distance, grid, cx)?),
        Operation::Trim(pick) => {
            let (shapes, cuts) = trim(target, boundaries, pick, grid, cx)?;
            built.replace[0] = Some(shapes);
            built.cuts = cuts;
        }
        Operation::Extend(end) => {
            let (shape, value) = extend(target, boundaries, end, grid, cx)?;
            built.replace[0] = Some(vec![shape]);
            built.cuts.push(value);
        }
        Operation::Chamfer(_) | Operation::Fillet(_) => {
            let second = second.ok_or(Error::Geometry("no-corner"))?;
            let (replace, add) = corner(target, second, operation, grid, cx)?;
            built.replace = replace;
            built.add = Some(add);
        }
    }
    Ok(built)
}
