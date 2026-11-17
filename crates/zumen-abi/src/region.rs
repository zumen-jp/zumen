//! 実寸の要素の集合が囲む閉領域と、閉じた要素どうしの集合演算。
//!
//! 計測と同じ経路を折れ線にし、許容差から作る整数格子へ移して
//! `zumen_geom2d::topology` の閉領域の演算に渡す。元の要素と ID は変更しない。

use std::f64::consts::PI;
use zumen_base::{Cx, Kind};
use zumen_geom2d::{
    Xy,
    measure::Piece,
    topology::{
        self as t, Grid, MAX_REGION_EDGES, Point, RegionDiscard, RegionFill, RegionLocation,
        RegionOperand, RegionOperation, Segment,
    },
};

use crate::{
    intersection,
    measurement::{self, Error, Path},
};

// 32 bit と 64 bit で同じ予算になるよう、1 件の予約量を固定する。
const POINT_BYTES: u64 = 16;
const RESULT_POINT_BYTES: u64 = 64;
const RESULT_ITEM_BYTES: u64 = 128;

/// 格子へ移した 1 要素の折れ線。閉じた要素は最後の点を最初の点と重ねない。
pub(crate) struct Polyline {
    pub points: Vec<Point>,
    pub closed: bool,
}

/// 求め方。
#[derive(Clone, Copy)]
pub(crate) enum Mode {
    /// 要素の線が囲む面。
    Detect,
    /// 閉じた要素どうしの集合演算。
    Boolean(RegionOperation, RegionFill),
}

/// 返す閉領域。外周は反時計回り、穴は時計回り。
pub(crate) struct Area {
    pub outer: Vec<Xy>,
    pub holes: Vec<Vec<Xy>>,
    pub area: f64,
}

/// 要素の位置で示す、丸めた分割点と使わなかった部分。
pub(crate) enum Note {
    Rounded { element: usize, position: Xy },
    Degenerate { element: usize },
    Open { element: usize, start: Xy, end: Xy },
}

pub(crate) struct Found {
    pub regions: Vec<Area>,
    pub notes: Vec<Note>,
}

fn geometry(error: t::Error) -> Error {
    Error::Geometry(match error {
        t::Error::NonFinite => "invalid-curve",
        t::Error::Allocation => "allocation",
        t::Error::Tolerance => "tolerance",
        t::Error::Precision => "precision",
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

/// 要素の集合に共通の格子。
pub(crate) fn grid(paths: &[Path], tolerance: f64) -> Result<Grid, Error> {
    intersection::shared_grid(paths, tolerance)
}

/// 円弧の弦の数。弦と円弧の距離（矢高）を許容差の半分以下にする。
fn chords(radius: f64, sweep: f64, tolerance: f64) -> Result<usize, Error> {
    let ratio = (tolerance / 2.0 / radius).min(1.0);
    let step = (2.0 * libm::acos(1.0 - ratio)).min(PI / 2.0);
    let count = libm::ceil(sweep.abs() / step);
    if !count.is_finite() || count > MAX_REGION_EDGES as f64 {
        return Err(Error::Geometry("input-limit"));
    }
    Ok((count as usize).max(1))
}

fn push(points: &mut Vec<Point>, point: Point, cx: &mut Cx<'_>) -> Result<(), Error> {
    cx.check(Kind::Work, 0)?;
    if points.last() != Some(&point) {
        if points.len() >= MAX_REGION_EDGES {
            return Err(Error::Geometry("input-limit"));
        }
        cx.check(Kind::Segments, 1)?;
        reserve(points, 1, POINT_BYTES, cx)?;
        points.push(point);
    }
    Ok(())
}

/// 経路を折れ線にして格子へ移す。円弧を弦に分けたときは真を返す。
pub(crate) fn polyline(
    path: &Path,
    grid: Grid,
    cx: &mut Cx<'_>,
) -> Result<(Polyline, bool), Error> {
    cx.check(Kind::Work, (path.pieces.len() as u64).saturating_mul(8))?;
    let tolerance = grid.tolerance();
    let mut points = Vec::new();
    let mut approximate = false;
    let snap = |point| intersection::snap(grid, point);
    for piece in &path.pieces {
        match *piece {
            Piece::Line { start, end } => {
                push(&mut points, snap(start)?, cx)?;
                push(&mut points, snap(end)?, cx)?;
            }
            Piece::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                if !radius.is_finite() || !sweep.is_finite() || radius < 0.0 {
                    return Err(Error::Geometry("invalid-curve"));
                }
                if radius == 0.0 {
                    push(&mut points, snap(center)?, cx)?;
                    continue;
                }
                approximate = true;
                let count = chords(radius, sweep, tolerance)?;
                cx.check(Kind::Work, 16 * (count as u64 + 1))?;
                for i in 0..=count {
                    let angle = start + sweep * (i as f64 / count as f64);
                    let point = [
                        center[0] + radius * libm::cos(angle),
                        center[1] + radius * libm::sin(angle),
                    ];
                    push(&mut points, snap(point)?, cx)?;
                }
            }
        }
    }
    if path.closed && points.len() > 1 && points.first() == points.last() {
        points.pop();
    }
    Ok((
        Polyline {
            points,
            closed: path.closed,
        },
        approximate || path.approximate,
    ))
}

fn area_twice(points: &[Point]) -> i128 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(&a, &b)| {
            let [x, y] = a.coordinates().map(i128::from);
            let [u, v] = b.coordinates().map(i128::from);
            x * v - y * u
        })
        .sum()
}

fn positions(points: &[Point], grid: Grid, cx: &mut Cx<'_>) -> Result<Vec<Xy>, Error> {
    let mut output = Vec::new();
    reserve(&mut output, points.len() as u64, RESULT_POINT_BYTES, cx)?;
    for &point in points {
        cx.check(Kind::Work, 0)?;
        output.push(grid.position(point).map_err(geometry)?);
    }
    Ok(output)
}

/// 閉領域を求める。`subject` と `clip` は要素の位置の列で、検出では `clip` を使わない。
/// 結果の要素の位置は、`subject`、`clip` を続けた列の位置。
pub(crate) fn regions(
    subject: &[Polyline],
    clip: &[Polyline],
    mode: Mode,
    grid: Grid,
    cx: &mut Cx<'_>,
) -> Result<Found, Error> {
    let mut owners = Vec::new();
    let result = match mode {
        Mode::Detect => {
            let count = subject
                .iter()
                .map(|line| line.points.len() + usize::from(line.closed))
                .sum::<usize>();
            if count > MAX_REGION_EDGES {
                return Err(Error::Geometry("input-limit"));
            }
            let mut lines = Vec::new();
            reserve(&mut lines, count as u64, 32, cx)?;
            reserve(&mut owners, count as u64, 8, cx)?;
            for (element, line) in subject.iter().enumerate() {
                let close = line.closed && line.points.len() > 2;
                let mut previous = if close { line.points.last() } else { None };
                for point in &line.points {
                    cx.check(Kind::Work, 0)?;
                    if let Some(&start) = previous {
                        lines.push(Segment { start, end: *point });
                        owners.push(element);
                    }
                    previous = Some(point);
                }
            }
            t::detect_regions(&lines, |charge| measurement::charge(cx, charge))
        }
        Mode::Boolean(operation, fill) => {
            let mut a = Vec::new();
            let mut b = Vec::new();
            reserve(&mut a, subject.len() as u64, 16, cx)?;
            reserve(&mut b, clip.len() as u64, 16, cx)?;
            a.extend(subject.iter().map(|line| line.points.as_slice()));
            b.extend(clip.iter().map(|line| line.points.as_slice()));
            t::boolean_regions(&a, &b, operation, fill, |charge| {
                measurement::charge(cx, charge)
            })
        }
    }
    .map_err(failure)?;
    let owner = |location: RegionLocation| -> usize {
        match location.operand {
            RegionOperand::Lines => location
                .edge
                .and_then(|edge| owners.get(edge as usize).copied())
                .unwrap_or(0),
            RegionOperand::Subject => location.contour as usize,
            RegionOperand::Clip => subject.len() + location.contour as usize,
        }
    };
    let spacing = grid.spacing();
    let mut regions = Vec::new();
    reserve(
        &mut regions,
        result.regions.len() as u64,
        RESULT_ITEM_BYTES,
        cx,
    )?;
    for region in &result.regions {
        cx.check(Kind::Work, 4 + region.holes.len() as u64)?;
        let twice = region
            .holes
            .iter()
            .fold(area_twice(&region.outer), |sum, hole| {
                sum + area_twice(hole)
            });
        let mut holes = Vec::new();
        reserve(&mut holes, region.holes.len() as u64, RESULT_ITEM_BYTES, cx)?;
        for hole in &region.holes {
            holes.push(positions(hole, grid, cx)?);
        }
        regions.push(Area {
            outer: positions(&region.outer, grid, cx)?,
            holes,
            area: twice as f64 / 2.0 * spacing * spacing,
        });
    }
    let mut rounded = Vec::new();
    reserve(&mut rounded, result.adjustments.len() as u64, 32, cx)?;
    for adjustment in &result.adjustments {
        cx.check(Kind::Work, 0)?;
        rounded.push((owner(adjustment.source), adjustment.point));
    }
    cx.check(Kind::Work, (rounded.len() as u64).saturating_mul(8))?;
    rounded.sort_unstable();
    rounded.dedup();
    let mut notes = Vec::new();
    reserve(
        &mut notes,
        (rounded.len() + result.discarded.len()) as u64,
        RESULT_ITEM_BYTES,
        cx,
    )?;
    for (element, point) in rounded {
        cx.check(Kind::Work, 0)?;
        notes.push(Note::Rounded {
            element,
            position: grid.position(point).map_err(geometry)?,
        });
    }
    for discard in &result.discarded {
        cx.check(Kind::Work, 0)?;
        notes.push(match *discard {
            RegionDiscard::Degenerate(location) => Note::Degenerate {
                element: owner(location),
            },
            RegionDiscard::Open { source, segment } => Note::Open {
                element: owner(source),
                start: grid.position(segment.start).map_err(geometry)?,
                end: grid.position(segment.end).map_err(geometry)?,
            },
        });
    }
    Ok(Found { regions, notes })
}
