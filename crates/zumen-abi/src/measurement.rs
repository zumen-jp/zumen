//! 実寸の要素の計測。線幅と表示用の変換を含めず、元の曲線の定義を使う。

use std::f64::consts::{PI, TAU};
use zumen_base::{Cx, Kind, Stop};
use zumen_drawing::{Edge, Length, ModelPoint, Shape, SpaceId};
use zumen_geom2d::{
    self as geom, Xy,
    measure::{self, Metric, Piece},
    topology::{Charge, Failure},
};

mod sample;

const MAX_POINTS: usize = geom::MAX_STEPS;

pub(crate) enum Error {
    Geometry(&'static str),
    Stopped(Stop),
}

impl From<Stop> for Error {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

pub(crate) struct Path {
    pub pieces: Vec<Piece>,
    pub closed: bool,
    pub approximate: bool,
}

pub(crate) struct Scalar {
    pub value: f64,
    pub approximate: bool,
}

pub(crate) fn charge(cx: &mut Cx<'_>, value: Charge) -> Result<(), Stop> {
    match value {
        Charge::Work(n) => cx.check(Kind::Work, n),
        Charge::Points(n) => cx.check(Kind::Segments, n),
        Charge::Bytes(n) => cx.check(Kind::DecodedBytes, n),
        _ => cx.check(Kind::Work, 0),
    }
}

fn geometry(error: geom::topology::Error) -> Error {
    use geom::topology::Error as E;
    Error::Geometry(match error {
        E::NoSolution => "not-closed",
        E::NonFinite => "invalid-curve",
        E::Degenerate => "empty-curve",
        E::Allocation => "allocation",
        _ => "numeric-range",
    })
}

fn failure(error: Failure<Stop>) -> Error {
    match error {
        Failure::Geometry(error) => geometry(error),
        Failure::Stopped(stop) => Error::Stopped(stop),
    }
}

fn reserve<T>(output: &mut Vec<T>, count: usize, slot: u64, cx: &mut Cx<'_>) -> Result<(), Error> {
    cx.check(Kind::DecodedBytes, (count as u64).saturating_mul(slot))?;
    output
        .try_reserve_exact(count)
        .map_err(|_| Error::Geometry("allocation"))
}

fn point(point: ModelPoint, space: SpaceId) -> Result<Xy, Error> {
    if point.space != space {
        return Err(Error::Geometry("space-mismatch"));
    }
    let result = [point.x, point.y];
    if !result.into_iter().all(f64::is_finite) {
        return Err(Error::Geometry("invalid-curve"));
    }
    Ok(result)
}

fn length(length: Length, space: SpaceId) -> Result<f64, Error> {
    match length {
        Length::Model(value) if value.space == space && value.value.is_finite() => Ok(value.value),
        _ => Err(Error::Geometry("space-mismatch")),
    }
}

fn angles(start: f64, end: f64, ccw: bool) -> Result<(f64, f64), Error> {
    if !start.is_finite() || !end.is_finite() {
        return Err(Error::Geometry("invalid-curve"));
    }
    Ok((
        measure::start_radians(start),
        geom::sweep(start, end, ccw) * (PI / 180.0),
    ))
}

fn radius(radius: Length, space: SpaceId) -> Result<f64, Error> {
    let radius = length(radius, space)?;
    if radius < 0.0 {
        return Err(Error::Geometry("invalid-curve"));
    }
    Ok(radius)
}

fn one(piece: Piece, closed: bool, cx: &mut Cx<'_>) -> Result<Path, Error> {
    let mut pieces = Vec::new();
    reserve(&mut pieces, 1, 64, cx)?;
    pieces.push(piece);
    Ok(Path {
        pieces,
        closed,
        approximate: false,
    })
}

pub(crate) fn point_path(value: Xy, cx: &mut Cx<'_>) -> Result<Path, Error> {
    if !value.into_iter().all(f64::is_finite) {
        return Err(Error::Geometry("invalid-point"));
    }
    one(
        Piece::Line {
            start: value,
            end: value,
        },
        false,
        cx,
    )
}

pub(crate) fn path(
    shape: &Shape<ModelPoint>,
    space: SpaceId,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<Path, Error> {
    cx.check(Kind::Work, 64)?;
    match shape {
        Shape::Line { start, end } | Shape::Curve(Edge::Line { start, end }) => one(
            Piece::Line {
                start: point(*start, space)?,
                end: point(*end, space)?,
            },
            false,
            cx,
        ),
        Shape::Point(value) => point_path(point(value.position, space)?, cx),
        Shape::Circle {
            center,
            radius: value,
        } => one(
            Piece::Arc {
                center: point(*center, space)?,
                radius: radius(*value, space)?,
                start: 0.0,
                sweep: TAU,
            },
            true,
            cx,
        ),
        Shape::Curve(Edge::Arc {
            center,
            radius: value,
            start,
            end,
            counterclockwise,
        }) => {
            let (start, sweep) = angles(*start, *end, *counterclockwise)?;
            one(
                Piece::Arc {
                    center: point(*center, space)?,
                    radius: radius(*value, space)?,
                    start,
                    sweep,
                },
                sweep.abs() == TAU,
                cx,
            )
        }
        Shape::Polyline(polyline) => {
            let n = polyline.vertices.len();
            if n == 0 || n > MAX_POINTS {
                return Err(Error::Geometry("input-limit"));
            }
            cx.check(Kind::Work, 64 * n as u64)?;
            let mut pieces = Vec::new();
            reserve(&mut pieces, n, 64, cx)?;
            for pair in polyline.vertices.windows(2) {
                if let [a, b] = pair {
                    cx.check(Kind::Work, 0)?;
                    pieces.push(
                        measure::bulge_piece(
                            point(a.point, space)?,
                            point(b.point, space)?,
                            a.bulge,
                        )
                        .map_err(geometry)?,
                    );
                }
            }
            if let (Some(first), Some(last)) = (polyline.vertices.first(), polyline.vertices.last())
                && (polyline.closed || n == 1)
            {
                pieces.push(
                    measure::bulge_piece(
                        point(last.point, space)?,
                        point(first.point, space)?,
                        last.bulge,
                    )
                    .map_err(geometry)?,
                );
            }
            Ok(Path {
                pieces,
                closed: polyline.closed,
                approximate: false,
            })
        }
        Shape::Curve(edge) => sample::edge(edge, space, tolerance, cx),
        _ => Err(Error::Geometry("unsupported-shape")),
    }
}

pub(crate) fn scalar(
    shape: &Shape<ModelPoint>,
    space: SpaceId,
    metric: Metric,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<Scalar, Error> {
    cx.check(Kind::Work, 64)?;
    let analytic = match (metric, shape) {
        (
            Metric::Length,
            Shape::Curve(Edge::Clothoid {
                origin,
                parameter,
                angle,
                start,
                end,
                ..
            }),
        ) => {
            point(*origin, space)?;
            if length(*parameter, space)? <= 0.0 || !angle.is_finite() {
                return Err(Error::Geometry("invalid-curve"));
            }
            Some((length(*end, space)? - length(*start, space)?).abs())
        }
        (
            Metric::Area,
            Shape::Curve(Edge::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            }),
        ) => {
            let (_, sweep) = angles(*start, *end, *counterclockwise)?;
            if sweep.abs() != TAU {
                return Err(Error::Geometry("not-closed"));
            }
            let center = point(*center, space)?;
            let major = point(*major, space)?;
            if !ratio.is_finite() || *ratio == 0.0 {
                return Err(Error::Geometry("invalid-curve"));
            }
            let a = libm::hypot(major[0] - center[0], major[1] - center[1]);
            Some(a * (a * ratio.abs() * PI))
        }
        _ => None,
    };
    if let Some(value) = analytic {
        if !value.is_finite() {
            return Err(Error::Geometry("numeric-range"));
        }
        return Ok(Scalar {
            value: if value == 0.0 { 0.0 } else { value },
            approximate: false,
        });
    }
    let path = path(shape, space, tolerance, cx)?;
    let value = measure::path_metric(&path.pieces, path.closed, metric, |value| charge(cx, value))
        .map_err(failure)?;
    Ok(Scalar {
        value,
        approximate: path.approximate,
    })
}

pub(crate) fn closest(
    first: &Path,
    second: &Path,
    cx: &mut Cx<'_>,
) -> Result<measure::Closest, Error> {
    measure::closest_paths(&first.pieces, &second.pieces, |value| charge(cx, value))
        .map_err(failure)
}
