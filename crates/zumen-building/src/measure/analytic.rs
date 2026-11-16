//! 単独の押し出しで閉包を確認できたとき、元の寸法を計測に使う。

use crate::{
    AreaProfileKind, Building, GeometryDefinition, ProfileExtrusion,
    evaluation::{self, ElementMesh, MeshPart},
};
use zumen_base::{Cx, Kind};
use zumen_geom3d::{Failure, Transform, Xyz};

pub(super) fn no_openings(
    building: &Building,
    apply: bool,
    cx: &mut Cx<'_>,
) -> Result<bool, Failure> {
    if !apply {
        return Ok(true);
    }
    for (_, relation) in building.components.opening.iter() {
        cx.check(Kind::Work, 1)?;
        // 分解された親の開口もあり得る。関係がある文書では元寸法へ戻さない。
        if matches!(relation, crate::Opening::Voids(_)) {
            return Ok(false);
        }
    }
    Ok(true)
}

struct Extrusion<'a> {
    value: &'a ProfileExtrusion,
    factor: f64,
    area: f64,
    part: &'a MeshPart,
    position: Transform,
}

fn extrusion<'a>(
    building: &'a Building,
    mesh: &'a ElementMesh,
    cx: &mut Cx<'_>,
) -> Result<Option<Extrusion<'a>>, Failure> {
    cx.check(Kind::Work, 1)?;
    if !mesh.complete {
        return Ok(None);
    }
    let [part] = mesh.parts.as_slice() else {
        return Ok(None);
    };
    let node = building
        .geometry(mesh.element)
        .and_then(|g| g.data().nodes.get(part.item.0 as usize))
        .ok_or(Failure::Invalid("measure_reference"))?;
    let GeometryDefinition::Extrusion {
        definition,
        length_unit,
        ..
    } = &node.definition
    else {
        return Ok(None);
    };
    if !zumen_geom3d::ManifoldKernel.is_solid(&part.mesh, cx)? {
        return Ok(None);
    }
    let factor = evaluation::length_scale(
        length_unit
            .as_ref()
            .ok_or(Failure::Invalid("measure_unit_missing"))?,
    )?;
    let area = match &definition.profile.kind {
        AreaProfileKind::Rectangle { x_dim, y_dim } => x_dim * y_dim,
        AreaProfileKind::Circle { radius } => core::f64::consts::PI * radius * radius,
        AreaProfileKind::RectangleHollow {
            x_dim,
            y_dim,
            wall_thickness,
            inner_fillet_radius,
            outer_fillet_radius,
        } if inner_fillet_radius.is_none_or(|v| v == 0.0)
            && outer_fillet_radius.is_none_or(|v| v == 0.0) =>
        {
            2.0 * wall_thickness * (x_dim + y_dim - 2.0 * wall_thickness)
        }
        AreaProfileKind::IShape {
            overall_width,
            overall_depth,
            web_thickness,
            flange_thickness,
            fillet_radius,
            flange_edge_radius,
            flange_slope,
        } if fillet_radius.is_none_or(|v| v == 0.0)
            && flange_edge_radius.is_none_or(|v| v == 0.0)
            && flange_slope.as_ref().is_none_or(|v| v.value == 0.0) =>
        {
            2.0 * overall_width * flange_thickness
                + (overall_depth - 2.0 * flange_thickness) * web_thickness
        }
        _ => return Ok(None),
    } * factor
        * factor;
    if !area.is_finite() || area <= 0.0 {
        return Err(Failure::Invalid("measure_area_range"));
    }
    let position = evaluation::values::placement(definition.position.as_ref(), factor)?;
    Ok(Some(Extrusion {
        value: definition,
        factor,
        area,
        part,
        position,
    }))
}

fn norm(p: Xyz) -> f64 {
    libm::hypot(libm::hypot(p[0], p[1]), p[2])
}

pub(super) fn volume(
    building: &Building,
    mesh: &ElementMesh,
    cx: &mut Cx<'_>,
) -> Result<Option<f64>, Failure> {
    let Some(shape) = extrusion(building, mesh, cx)? else {
        return Ok(None);
    };
    let height = shape.value.depth * shape.factor * shape.value.direction[2].abs()
        / norm(shape.value.direction);
    let value = shape.area * height * shape.part.placement.determinant().abs();
    if !value.is_finite() || value <= 0.0 {
        return Err(Failure::Invalid("measure_volume"));
    }
    Ok(Some(value))
}

pub(super) fn area(
    building: &Building,
    mesh: &ElementMesh,
    origin: Xyz,
    cx: &mut Cx<'_>,
) -> Result<Option<f64>, Failure> {
    let Some(shape) = extrusion(building, mesh, cx)? else {
        return Ok(None);
    };
    let mut local = shape.part.placement;
    for (row, coordinate) in local.0.iter_mut().zip(origin) {
        row[3] -= coordinate;
    }
    let frame = shape.position.then(&local);
    let (x, y) = (frame.column(0), frame.column(1));
    if x[2] != 0.0 || y[2] != 0.0 {
        return Ok(None);
    }
    let d = shape
        .value
        .direction
        .map(|v| v / norm(shape.value.direction));
    let z0 = frame.column(3)[2];
    let z1 = z0 + frame.vector(d)[2] * shape.value.depth * shape.factor;
    if !z0.is_finite() || !z1.is_finite() || z0 == z1 {
        return Ok(None);
    }
    let value = if z0.min(z1) <= 0.0 && z0.max(z1) >= 0.0 {
        shape.area * (x[0] * y[1] - x[1] * y[0]).abs()
    } else {
        0.0
    };
    if !value.is_finite() {
        return Err(Failure::Invalid("measure_area_range"));
    }
    Ok(Some(value))
}
