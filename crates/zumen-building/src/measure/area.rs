//! 水平断面を閉領域へ移し、重なりと穴を保って面積を求める。

use super::{Calculated, Options, Source, mesh, reserve};
use crate::{Building, evaluation::ElementMesh};
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_geom2d::topology::{self, Charge, Grid, Point, Region, RegionFill, RegionOperation};
use zumen_geom3d::{Failure, MeshSection, Plane, Xyz};

pub(super) fn charge(cx: &mut Cx<'_>, value: Charge) -> Result<(), zumen_base::Stop> {
    match value {
        Charge::Work(n) => cx.check(Kind::Work, n),
        Charge::Bytes(n) => cx.check(Kind::DecodedBytes, n),
        Charge::Points(n) => cx.check(Kind::Segments, n),
        _ => cx.check(Kind::Work, 0),
    }
}

pub(super) fn geometry(error: topology::Failure<zumen_base::Stop>) -> Failure {
    match error {
        topology::Failure::Stopped(stop) => Failure::Stopped(stop),
        topology::Failure::Geometry(_) => Failure::Invalid("measure_region"),
    }
}

pub(super) struct Section {
    pub grid: Grid,
    pub regions: Vec<Region>,
    pub sources: Vec<Source>,
}

pub(super) fn section(
    building: &Building,
    element: &ElementMesh,
    origin: Xyz,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Section, Failure> {
    if !element.complete || element.parts.is_empty() {
        return Err(Failure::Invalid("measure_incomplete_shape"));
    }
    let grid = Grid::new([0.0; 2], options.tolerance.length)
        .map_err(|_| Failure::Invalid("measure_grid"))?;
    let plane = Plane {
        origin: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
    };
    let mut contours = Vec::new();
    let mut sources = Vec::new();
    reserve(&mut sources, element.parts.len(), 64, cx)?;
    let mut snapped = 0u64;
    for part in &element.parts {
        cx.check(Kind::Work, 1)?;
        let mesh = mesh::local_mesh(part, origin, cx)?;
        let paths = MeshSection.paths(&mesh, &plane, cx)?;
        for path in paths {
            cx.check(Kind::Work, path.points.len() as u64)?;
            if !path.closed {
                return Err(Failure::Invalid("measure_open_contour"));
            }
            let mut points = Vec::new();
            reserve(&mut points, path.points.len(), 16, cx)?;
            for point in path.points {
                let value = grid
                    .snap(point)
                    .map_err(|_| Failure::Invalid("measure_grid"))?;
                snapped += u64::from(value.offset != [0.0, 0.0]);
                if points.last() != Some(&value.point) {
                    points.push(value.point);
                }
            }
            if points.first() == points.last() {
                points.pop();
            }
            if points.len() < 3 {
                return Err(Failure::Invalid("measure_collapsed_contour"));
            }
            reserve(&mut contours, 1, 24, cx)?;
            contours.push(points);
        }
        sources.push(Source {
            item: part.item,
            instance: building
                .geometry(element.element)
                .and_then(|g| g.data().nodes.get(part.item.0 as usize))
                .and_then(|n| n.source.instance),
            tolerance: part.tolerance,
            basis: super::SourceBasis::Evaluated,
        });
    }
    if contours.is_empty() {
        return Ok(Section {
            grid,
            regions: Vec::new(),
            sources,
        });
    }
    let mut borrowed = Vec::new();
    reserve(&mut borrowed, contours.len(), 16, cx)?;
    borrowed.extend(contours.iter().map(Vec::as_slice));
    let result = topology::boolean_regions(
        &borrowed,
        &[],
        RegionOperation::Union,
        RegionFill::NonZero,
        |v| charge(cx, v),
    )
    .map_err(geometry)?;
    if !result.discarded.is_empty() {
        return Err(Failure::Invalid("measure_collapsed_region"));
    }
    snapped += result.adjustments.len() as u64;
    if snapped != 0 {
        cx.check(Kind::Work, snapped)?;
        cx.report(
            Diagnostic::new(codes::BUILDING_MEASURE_APPROXIMATED)
                .with("spacing", grid.spacing())
                .with("points", snapped)
                .at(building.place(element.element)),
        );
        cx.check(Kind::Work, 0)?;
    }
    Ok(Section {
        grid,
        regions: result.regions,
        sources,
    })
}

fn twice_area(points: &[Point], cx: &mut Cx<'_>) -> Result<i128, Failure> {
    cx.check(Kind::Work, points.len() as u64)?;
    let mut total = 0i128;
    for (a, b) in points.iter().zip(points.iter().cycle().skip(1)) {
        let [ax, ay] = a.coordinates().map(i128::from);
        let [bx, by] = b.coordinates().map(i128::from);
        total = total
            .checked_add(ax * by - ay * bx)
            .ok_or(Failure::Invalid("measure_area_range"))?;
    }
    Ok(total)
}

pub(super) fn value(section: Section, cx: &mut Cx<'_>) -> Result<Calculated, Failure> {
    let mut twice = 0i128;
    for region in section.regions {
        twice = twice
            .checked_add(twice_area(&region.outer, cx)?)
            .ok_or(Failure::Invalid("measure_area_range"))?;
        for hole in region.holes {
            twice = twice
                .checked_add(twice_area(&hole, cx)?)
                .ok_or(Failure::Invalid("measure_area_range"))?;
        }
    }
    if twice < 0 {
        return Err(Failure::Invalid("measure_area_orientation"));
    }
    let si_value = twice as f64 / 2.0 * section.grid.spacing() * section.grid.spacing();
    if !si_value.is_finite() || (twice != 0 && si_value == 0.0) {
        return Err(Failure::Invalid("measure_area_range"));
    }
    Ok(Calculated {
        si_value,
        sources: section.sources,
        boundary: None,
    })
}
