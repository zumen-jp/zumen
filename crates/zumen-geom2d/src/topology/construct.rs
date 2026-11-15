//! 整数格子で方向と退化を判定し、作図する値を libm で求める。
//! 出力の座標・半径・距離は格子の間隔を単位とする。近い格子点も別に返す。

use std::f64::consts::{PI, TAU};

use super::{
    Arc, Charge, Circle, Direction, Error, Failure, Grid, Hit, MAX_COORDINATE, Point, Segment,
    cross, difference,
};
use crate::Xy;

/// 作図した座標と、再び位相を計算するときの近い格子点。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConstructionPoint {
    /// 格子の間隔を単位とした座標。格子へ丸める前の値。
    pub coordinates: Xy,
    /// 近い格子点と丸めの有無。
    pub nearest: Hit,
}

impl ConstructionPoint {
    fn new(coordinates: Xy) -> Result<Self, Error> {
        let coordinates = coordinates.map(positive_zero);
        if !coordinates.into_iter().all(f64::is_finite) {
            return Err(Error::NonFinite);
        }
        if coordinates
            .into_iter()
            .any(|v| v.abs() > MAX_COORDINATE as f64)
        {
            return Err(Error::Range);
        }
        let [x, y] = coordinates.map(libm::round);
        Ok(Self {
            coordinates,
            nearest: Hit {
                point: Point::new(x as i64, y as i64)?,
                rounded: coordinates != [x, y],
            },
        })
    }

    /// 指定した格子の原点と間隔で元の座標へ戻す。精度を保てない場合は失敗する。
    pub fn position(self, grid: Grid) -> Result<Xy, Error> {
        let [ox, oy] = grid.origin();
        let [x, y] = self.coordinates;
        let point = [ox + x * grid.spacing(), oy + y * grid.spacing()].map(positive_zero);
        let [sx, sy] = grid.position(self.nearest.point)?;
        let [px, py] = point;
        if !point.into_iter().all(f64::is_finite)
            || libm::hypot(px - sx, py - sy) > grid.tolerance()
        {
            return Err(Error::Range);
        }
        Ok(point)
    }
}

/// 丸める前の端点を持つ線分。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineGeometry {
    /// 始点。
    pub start: ConstructionPoint,
    /// 終点。
    pub end: ConstructionPoint,
}

/// 丸める前の中心と半径を持つ円。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CircleGeometry {
    /// 中心。
    pub center: ConstructionPoint,
    /// 格子の間隔を単位とした半径。0 は点。
    pub radius: f64,
}

impl CircleGeometry {
    fn new(center: Xy, radius: f64) -> Result<Self, Error> {
        finite(radius)?;
        if radius < 0.0 {
            return Err(Error::NoSolution);
        }
        let center = ConstructionPoint::new(center)?;
        if center
            .coordinates
            .into_iter()
            .any(|v| v.abs() + radius > MAX_COORDINATE as f64)
        {
            return Err(Error::Range);
        }
        Ok(Self {
            center,
            radius: positive_zero(radius),
        })
    }
}

/// 丸める前の円と角度を持つ円弧。角度は度で、反時計回りを正にする。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArcGeometry {
    /// 円の定義。
    pub circle: CircleGeometry,
    /// 始点。
    pub start: ConstructionPoint,
    /// 終点。
    pub end: ConstructionPoint,
    /// 始点の角度。-180 度以上 180 度以下。
    pub start_degrees: f64,
    /// 始点から進む角度。1 周まで。
    pub sweep_degrees: f64,
}

/// 2 本の線分が共有する角と、それぞれの反対側の端点。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Corner {
    vertex: Point,
    first: Point,
    second: Point,
}

impl Corner {
    /// 点に縮んだ線分や、同じ直線上の 2 本は受け付けない。
    pub fn new(vertex: Point, first: Point, second: Point) -> Result<Self, Error> {
        let a = difference(first, vertex);
        let b = difference(second, vertex);
        if a == [0, 0] || b == [0, 0] {
            return Err(Error::Degenerate);
        }
        if cross(a, b) == 0 {
            return Err(Error::NoSolution);
        }
        Ok(Self {
            vertex,
            first,
            second,
        })
    }

    /// 共通の角。
    pub fn vertex(self) -> Point {
        self.vertex
    }
    /// 1 本目の反対側の端点。
    pub fn first(self) -> Point {
        self.first
    }
    /// 2 本目の反対側の端点。
    pub fn second(self) -> Point {
        self.second
    }
}

/// 面取りで残す線分と、新しくつなぐ線分。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chamfer {
    /// 1 本目から 2 本目へ進む面取りの線分。
    pub join: LineGeometry,
    /// 1 本目は反対側の端点から、2 本目は反対側の端点へ向ける。全長を切る場合は `None`。
    pub kept: [Option<LineGeometry>; 2],
}

/// フィレットで残す線分と、接する円弧。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fillet {
    /// 1 本目から 2 本目へ進む接線の円弧。
    pub arc: ArcGeometry,
    /// 面取りと同じ向きで残す線分。
    pub kept: [Option<LineGeometry>; 2],
}

fn positive_zero(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}
fn finite(v: f64) -> Result<(), Error> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(Error::NonFinite)
    }
}
fn xy(Point([x, y]): Point) -> Xy {
    [x as f64, y as f64]
}
fn unit([x, y]: [i128; 2]) -> (Xy, f64) {
    let length = libm::hypot(x as f64, y as f64);
    ([x as f64 / length, y as f64 / length], length)
}
fn add([x, y]: Xy, [u, v]: Xy, scale: f64) -> Xy {
    [x + u * scale, y + v * scale]
}

/// 向きの左側を正として線分を平行に移す。座標を格子へ丸めず、同じ変位を両端に加える。
pub fn offset_segment<E>(
    source: Segment,
    distance: f64,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<LineGeometry, Failure<E>> {
    check(Charge::Work(16)).map_err(Failure::Stopped)?;
    finite(distance).map_err(Failure::Geometry)?;
    let r = difference(source.end, source.start);
    if r == [0, 0] {
        return Err(Failure::Geometry(Error::Degenerate));
    }
    let ([x, y], _) = unit(r);
    let displacement = [-y, x];
    let result = LineGeometry {
        start: ConstructionPoint::new(add(xy(source.start), displacement, distance))
            .map_err(Failure::Geometry)?,
        end: ConstructionPoint::new(add(xy(source.end), displacement, distance))
            .map_err(Failure::Geometry)?,
    };
    check(Charge::Points(2)).map_err(Failure::Stopped)?;
    Ok(result)
}

/// 外側を正として円をオフセットする。半径 0 は点、負の半径になる指定は `NoSolution`。
pub fn offset_circle<E>(
    source: Circle,
    distance: f64,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<CircleGeometry, Failure<E>> {
    check(Charge::Work(8)).map_err(Failure::Stopped)?;
    finite(distance).map_err(Failure::Geometry)?;
    let result = CircleGeometry::new(xy(source.center()), source.radius() as f64 + distance)
        .map_err(Failure::Geometry)?;
    check(Charge::Points(1)).map_err(Failure::Stopped)?;
    Ok(result)
}

/// 進む向きの左側を正として円弧をオフセットし、中心と角度範囲を保つ。
/// 半径が 0 以下になる円弧は `NoSolution`。
pub fn offset_arc<E>(
    source: Arc,
    distance: f64,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<ArcGeometry, Failure<E>> {
    check(Charge::Work(32)).map_err(Failure::Stopped)?;
    finite(distance).map_err(Failure::Geometry)?;
    let radius = source.circle().radius() as f64
        + match source.direction() {
            Direction::Clockwise => distance,
            Direction::CounterClockwise => -distance,
        };
    if radius <= 0.0 {
        return Err(Failure::Geometry(Error::NoSolution));
    }
    let circle =
        CircleGeometry::new(xy(source.circle().center()), radius).map_err(Failure::Geometry)?;
    let a = source.start().vector();
    let b = source.end().vector();
    let (u, _) = unit(a);
    let (v, _) = unit(b);
    let [ax, ay] = a;
    let [bx, by] = b;
    let start = libm::atan2(ay as f64, ax as f64);
    let mut sweep = libm::atan2(cross(a, b) as f64, (ax * bx + ay * by) as f64);
    match source.direction() {
        Direction::CounterClockwise if source.is_full() => sweep = TAU,
        Direction::Clockwise if source.is_full() => sweep = -TAU,
        Direction::CounterClockwise if sweep < 0.0 => sweep += TAU,
        Direction::Clockwise if sweep > 0.0 => sweep -= TAU,
        _ => {}
    }
    let result = ArcGeometry {
        circle,
        start: ConstructionPoint::new(add(circle.center.coordinates, u, radius))
            .map_err(Failure::Geometry)?,
        end: ConstructionPoint::new(add(circle.center.coordinates, v, radius))
            .map_err(Failure::Geometry)?,
        start_degrees: positive_zero(start * (180.0 / PI)),
        sweep_degrees: positive_zero(sweep * (180.0 / PI)),
    };
    check(Charge::Points(3)).map_err(Failure::Stopped)?;
    Ok(result)
}

fn contacts(
    corner: Corner,
    distances: [f64; 2],
) -> Result<([ConstructionPoint; 2], [Option<LineGeometry>; 2]), Error> {
    let (u, first_length) = unit(difference(corner.first, corner.vertex));
    let (v, second_length) = unit(difference(corner.second, corner.vertex));
    let [first, second] = distances;
    for distance in distances {
        finite(distance)?;
    }
    if first <= 0.0 || second <= 0.0 || first > first_length || second > second_length {
        return Err(Error::NoSolution);
    }
    let point = |end: Point, direction, distance, length| {
        ConstructionPoint::new(if distance == length {
            xy(end)
        } else {
            add(xy(corner.vertex), direction, distance)
        })
    };
    let first = point(corner.first, u, first, first_length)?;
    let second = point(corner.second, v, second, second_length)?;
    if first.nearest.point == second.nearest.point {
        return Err(Error::Precision);
    }
    let kept = [
        if distances[0] == first_length {
            None
        } else {
            Some(LineGeometry {
                start: ConstructionPoint::new(xy(corner.first))?,
                end: first,
            })
        },
        if distances[1] == second_length {
            None
        } else {
            Some(LineGeometry {
                start: second,
                end: ConstructionPoint::new(xy(corner.second))?,
            })
        },
    ];
    Ok(([first, second], kept))
}

/// 角から指定した正の距離だけ進んだ 2 点を結ぶ。距離が線分の全長を超えたら失敗する。
pub fn chamfer<E>(
    corner: Corner,
    distances: [f64; 2],
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<Chamfer, Failure<E>> {
    check(Charge::Work(32)).map_err(Failure::Stopped)?;
    let ([start, end], kept) = contacts(corner, distances).map_err(Failure::Geometry)?;
    check(Charge::Points(
        2 + 2 * kept.iter().filter(|v| v.is_some()).count() as u64,
    ))
    .map_err(Failure::Stopped)?;
    Ok(Chamfer {
        join: LineGeometry { start, end },
        kept,
    })
}

/// 2 本の線分の内側に接する、正の半径の円弧を作る。接点が全長を超えたら失敗する。
pub fn fillet<E>(
    corner: Corner,
    radius: f64,
    mut check: impl FnMut(Charge) -> Result<(), E>,
) -> Result<Fillet, Failure<E>> {
    check(Charge::Work(64)).map_err(Failure::Stopped)?;
    finite(radius).map_err(Failure::Geometry)?;
    if radius <= 0.0 {
        return Err(Failure::Geometry(Error::NoSolution));
    }
    let a = difference(corner.first, corner.vertex);
    let b = difference(corner.second, corner.vertex);
    let [ax, ay] = a;
    let [bx, by] = b;
    let cross = cross(a, b);
    let dot = (ax * bx + ay * by) as f64;
    let cross_abs = cross.abs() as f64;
    let length_product = libm::hypot(dot, cross_abs);
    // cot(theta/2)。鈍角では近い 2 値の差を使わず、外積から計算する。
    let ratio = if dot >= 0.0 {
        (length_product + dot) / cross_abs
    } else {
        cross_abs / (length_product - dot)
    };
    let distance = radius * ratio;
    let ([start, end], kept) = contacts(corner, [distance; 2]).map_err(Failure::Geometry)?;
    let ([ux, uy], _) = unit(a);
    let side = cross.signum() as f64;
    let center = add(start.coordinates, [-side * uy, side * ux], radius);
    let circle = CircleGeometry::new(center, radius).map_err(Failure::Geometry)?;
    let arc = ArcGeometry {
        circle,
        start,
        end,
        start_degrees: positive_zero(libm::atan2(-side * ux, side * uy) * (180.0 / PI)),
        sweep_degrees: -side * libm::atan2(cross_abs, -dot) * (180.0 / PI),
    };
    // 大きな座標の丸めで接線の条件を格子の 1/4 より失う場合は、結果を出さない。
    let [cx, cy] = center;
    let (second_unit, _) = unit(b);
    for (point, [vx, vy]) in [(start, [ux, uy]), (end, second_unit)] {
        let [x, y] = point.coordinates;
        if (libm::hypot(x - cx, y - cy) - radius).abs() > 0.25
            || ((x - cx) * vx + (y - cy) * vy).abs() > 0.25
        {
            return Err(Failure::Geometry(Error::Precision));
        }
    }
    check(Charge::Points(
        3 + 2 * kept.iter().filter(|v| v.is_some()).count() as u64,
    ))
    .map_err(Failure::Stopped)?;
    Ok(Fillet { arc, kept })
}
