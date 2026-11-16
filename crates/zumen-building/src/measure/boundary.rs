//! 壁芯と室の頂点の距離による推定と、一定厚の直線壁に沿う境界の換算。

use super::{
    Area, Boundary, BoundaryDecision, BoundaryEvidence, Calculated, InputBoundary, WallCenter,
    area, reserve,
};
use crate::{Building, ElementRef};
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_geom2d::topology::{self, Grid, Point, RegionFill, RegionOperation};
use zumen_geom3d::{Failure, Xy};

// #424 の推定規則。表示や形状評価の許容差と混同しない。
const INFERENCE_DISTANCE: f64 = 0.001;

fn cross(a: Xy, b: Xy) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn sub(a: Xy, b: Xy) -> Xy {
    [a[0] - b[0], a[1] - b[1]]
}
fn dot(a: Xy, b: Xy) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn simplified(points: &[Point], cx: &mut Cx<'_>) -> Result<Vec<Point>, Failure> {
    let mut out = Vec::new();
    reserve(&mut out, points.len(), 16, cx)?;
    let Some(&last) = points.last() else {
        return Err(Failure::Invalid("measure_empty_boundary"));
    };
    let mut previous = last;
    for (&point, &next) in points.iter().zip(points.iter().cycle().skip(1)) {
        cx.check(Kind::Work, 1)?;
        let [px, py] = previous.coordinates().map(i128::from);
        let [x, y] = point.coordinates().map(i128::from);
        let [nx, ny] = next.coordinates().map(i128::from);
        let a = [x - px, y - py];
        let b = [nx - x, ny - y];
        if a[0] * b[1] - a[1] * b[0] != 0 || a[0] * b[0] + a[1] * b[1] <= 0 {
            out.push(point);
        }
        previous = point;
    }
    if out.len() < 3 {
        return Err(Failure::Invalid("measure_collapsed_boundary"));
    }
    Ok(out)
}

fn remember(
    decision: &mut BoundaryDecision,
    wall: &WallCenter,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    cx.check(Kind::Work, decision.walls.len() as u64)?;
    if !decision.walls.iter().any(|w| w.element == wall.element) {
        reserve(&mut decision.walls, 1, 112, cx)?;
        decision.walls.push(wall.clone());
    }
    Ok(())
}

fn distance(point: Xy, wall: &WallCenter) -> Option<f64> {
    let d = sub(wall.ends[1], wall.ends[0]);
    let length = libm::hypot(d[0], d[1]);
    let unit = [d[0] / length, d[1] / length];
    let p = sub(point, wall.ends[0]);
    let s = dot(p, unit);
    let margin = wall.thickness / 2.0 + INFERENCE_DISTANCE;
    (s >= -margin && s <= length + margin).then(|| cross(p, unit).abs())
}

fn infer(
    contours: &[Vec<Point>],
    grid: Grid,
    walls: &[WallCenter],
    decision: &mut BoundaryDecision,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    for contour in contours {
        for &point in contour {
            let p = grid
                .position(point)
                .map_err(|_| Failure::Invalid("measure_grid"))?;
            let (mut center, mut inside) = (false, false);
            for wall in walls {
                cx.check(Kind::Work, 1)?;
                if let Some(d) = distance(p, wall) {
                    let c = d <= INFERENCE_DISTANCE;
                    let i = (d - wall.thickness / 2.0).abs() <= INFERENCE_DISTANCE;
                    if c || i {
                        remember(decision, wall, cx)?;
                    }
                    center |= c;
                    inside |= i;
                }
            }
            decision.vertices += 1;
            decision.center_vertices += u64::from(center);
            decision.inside_vertices += u64::from(inside);
        }
    }
    let center = decision.center_vertices > decision.vertices / 2;
    let inside = decision.inside_vertices > decision.vertices / 2;
    decision.source = match (center, inside) {
        (true, false) => Boundary::WallCenter,
        (false, true) => Boundary::Inside,
        _ => return Err(Failure::Unsupported("measure_boundary_unknown")),
    };
    decision.evidence = BoundaryEvidence::Inferred;
    Ok(())
}

fn edge_wall<'a>(
    a: Xy,
    b: Xy,
    walls: &'a [WallCenter],
    source: Boundary,
    explicit: bool,
    cx: &mut Cx<'_>,
) -> Result<&'a WallCenter, Failure> {
    let d = sub(b, a);
    let length = libm::hypot(d[0], d[1]);
    if !length.is_finite() || length <= 0.0 {
        return Err(Failure::Invalid("measure_boundary_edge"));
    }
    let unit = [d[0] / length, d[1] / length];
    let outward = [unit[1], -unit[0]];
    let midpoint = [a[0] + d[0] / 2.0, a[1] + d[1] / 2.0];
    let mut best: Option<(&WallCenter, f64)> = None;
    let mut ambiguous = false;
    for wall in walls {
        cx.check(Kind::Work, 1)?;
        let wd = sub(wall.ends[1], wall.ends[0]);
        let wl = libm::hypot(wd[0], wd[1]);
        let wu = [wd[0] / wl, wd[1] / wl];
        if cross(unit, wu).abs() > (INFERENCE_DISTANCE / length).min(1e-6) {
            continue;
        }
        let first = dot(sub(a, wall.ends[0]), wu);
        let last = dot(sub(b, wall.ends[0]), wu);
        let margin = wall.thickness / 2.0 + INFERENCE_DISTANCE;
        if first.min(last) < -margin || first.max(last) > wl + margin {
            continue;
        }
        let signed = dot(sub(wall.ends[0], midpoint), outward);
        let center = signed.abs();
        let inside = (signed - wall.thickness / 2.0).abs();
        let score = if explicit {
            center.min(inside)
        } else if source == Boundary::Inside {
            inside
        } else {
            center
        };
        if score > INFERENCE_DISTANCE {
            continue;
        }
        if let Some((previous, previous_score)) = best {
            if (score - previous_score).abs() <= 1e-9 {
                // 同じ線・厚さの重複は入力順で決める。異なる壁を恣意的に選ばない。
                let equivalent = (wall.thickness - previous.thickness).abs() <= 1e-9
                    && cross(sub(wall.ends[0], previous.ends[0]), wu).abs() <= 1e-9;
                ambiguous |= !equivalent;
            } else if score < previous_score {
                best = Some((wall, score));
                ambiguous = false;
            }
        } else {
            best = Some((wall, score));
        }
    }
    if ambiguous {
        return Err(Failure::Unsupported("measure_wall_ambiguous"));
    }
    best.map(|(wall, _)| wall)
        .ok_or(Failure::Unsupported("measure_wall_edge_missing"))
}

fn convert(
    contour: &[Point],
    grid: Grid,
    walls: &[WallCenter],
    decision: &mut BoundaryDecision,
    cx: &mut Cx<'_>,
) -> Result<(Vec<Point>, u64), Failure> {
    let mut lines = Vec::new();
    reserve(&mut lines, contour.len(), 32, cx)?;
    let sign = if decision.source == Boundary::Inside {
        1.0
    } else {
        -1.0
    };
    for (&a, &b) in contour.iter().zip(contour.iter().cycle().skip(1)) {
        let a = grid
            .position(a)
            .map_err(|_| Failure::Invalid("measure_grid"))?;
        let b = grid
            .position(b)
            .map_err(|_| Failure::Invalid("measure_grid"))?;
        let wall = edge_wall(
            a,
            b,
            walls,
            decision.source,
            decision.evidence == BoundaryEvidence::Explicit,
            cx,
        )?;
        remember(decision, wall, cx)?;
        let d = sub(b, a);
        let length = libm::hypot(d[0], d[1]);
        let distance = sign * wall.thickness / 2.0;
        lines.push((
            [
                a[0] + d[1] / length * distance,
                a[1] - d[0] / length * distance,
            ],
            d,
        ));
    }
    let mut out = Vec::new();
    reserve(&mut out, contour.len(), 16, cx)?;
    let mut previous = *lines
        .last()
        .ok_or(Failure::Invalid("measure_empty_boundary"))?;
    let mut snapped = 0;
    for &(point, direction) in &lines {
        cx.check(Kind::Work, 1)?;
        let denominator = cross(previous.1, direction);
        if denominator == 0.0 {
            return Err(Failure::Unsupported("measure_parallel_boundary"));
        }
        let t = cross(sub(point, previous.0), direction) / denominator;
        let intersection = [
            previous.0[0] + previous.1[0] * t,
            previous.0[1] + previous.1[1] * t,
        ];
        let value = grid
            .snap(intersection)
            .map_err(|_| Failure::Invalid("measure_grid"))?;
        snapped += u64::from(value.offset != [0.0, 0.0]);
        out.push(value.point);
        previous = (point, direction);
    }
    for ((&a, &b), &(_, d)) in out.iter().zip(out.iter().cycle().skip(1)).zip(&lines) {
        cx.check(Kind::Work, 1)?;
        let a = grid
            .position(a)
            .map_err(|_| Failure::Invalid("measure_grid"))?;
        let b = grid
            .position(b)
            .map_err(|_| Failure::Invalid("measure_grid"))?;
        if dot(sub(b, a), d) <= 0.0 {
            return Err(Failure::Invalid("measure_collapsed_boundary"));
        }
    }
    cx.check(Kind::Segments, out.len() as u64)?;
    Ok((out, snapped))
}

pub(super) fn measure(
    building: &Building,
    element: ElementRef,
    mut section: area::Section,
    walls: &[WallCenter],
    request: Area<'_>,
    cx: &mut Cx<'_>,
) -> Result<Calculated, Failure> {
    let mut decision = BoundaryDecision {
        source: Boundary::Shape,
        evidence: BoundaryEvidence::Shape,
        vertices: 0,
        center_vertices: 0,
        inside_vertices: 0,
        walls: Vec::new(),
    };
    if request.boundary != Boundary::Shape {
        let mut contours = Vec::new();
        for region in &section.regions {
            reserve(&mut contours, 1 + region.holes.len(), 24, cx)?;
            contours.push(simplified(&region.outer, cx)?);
            for hole in &region.holes {
                contours.push(simplified(hole, cx)?);
            }
        }
        match request.input {
            InputBoundary::Auto => infer(&contours, section.grid, walls, &mut decision, cx)?,
            InputBoundary::WallCenter | InputBoundary::Inside => {
                decision.source = if request.input == InputBoundary::WallCenter {
                    Boundary::WallCenter
                } else {
                    Boundary::Inside
                };
                decision.evidence = BoundaryEvidence::Explicit;
            }
        }
        if decision.source != request.boundary {
            let mut converted = Vec::new();
            reserve(&mut converted, contours.len(), 24, cx)?;
            let mut snapped = 0;
            for contour in contours {
                let (points, count) = convert(&contour, section.grid, walls, &mut decision, cx)?;
                converted.push(points);
                snapped += count;
            }
            let mut borrowed = Vec::new();
            reserve(&mut borrowed, converted.len(), 16, cx)?;
            borrowed.extend(converted.iter().map(Vec::as_slice));
            let result = topology::boolean_regions(
                &borrowed,
                &[],
                RegionOperation::Union,
                RegionFill::NonZero,
                |v| area::charge(cx, v),
            )
            .map_err(area::geometry)?;
            if !result.discarded.is_empty() {
                return Err(Failure::Invalid("measure_collapsed_boundary"));
            }
            snapped += result.adjustments.len() as u64;
            if snapped != 0 {
                cx.report(
                    Diagnostic::new(codes::BUILDING_MEASURE_APPROXIMATED)
                        .with("spacing", section.grid.spacing())
                        .with("points", snapped)
                        .at(building.place(element)),
                );
                cx.check(Kind::Work, 0)?;
            }
            section.regions = result.regions;
        }
    }
    let mut result = area::value(section, cx)?;
    result.boundary = Some(decision);
    Ok(result)
}
