//! 軸または材料の向きと、一定厚の矩形断面が一致する壁の中心線。
//!
//! buildingSMART 公開説明 IfcMaterialLayerSetUsage「Attribute use definition」。
//! 基準軸と層の基準線・積層方向は別なので、Axis をそのまま壁芯にしない。
//! https://github.com/buildingSMART/IFC4.x-development/blob/ifc4.3-main/docs/schemas/resource/IfcMaterialResource/Entities/IfcMaterialLayerSetUsage.md

use super::{Options, area, curves, reserve};
use crate::{
    Building, Coordinates, CurveKind, ElementRef, GeometryDefinition, GeometryLinks, MaterialKind,
    evaluation::ElementMesh,
};
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_geom3d::{Failure, Xy, Xyz, sub};

/// 中心線の方向の根拠。どちらも壁の断面との一致を検査する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallBasis {
    /// 名指した Axis の方向と壁の形。
    AxisAndShape,
    /// 材料層の方向・全厚と壁の形。
    LayersAndShape,
}

/// 一定厚の直線壁から求めた、断面内の中心線。
#[derive(Clone, Debug, PartialEq)]
pub struct WallCenter {
    /// 元の壁の要素。
    pub element: ElementRef,
    /// 元の壁の永続 ID。
    pub id: crate::Id,
    /// 計測の原点を引いたメートルの端点。
    pub ends: [Xy; 2],
    /// 壁の全厚。メートル。
    pub thickness: f64,
    /// 方向と厚さの根拠。
    pub basis: WallBasis,
}

fn xy(point: Coordinates) -> Xyz {
    match point {
        Coordinates::Two([x, y]) => [x, y, 0.0],
        Coordinates::Three(p) => p,
    }
}

fn direction(value: Xyz) -> Result<Xy, Failure> {
    let length = libm::hypot(value[0], value[1]);
    if !value.into_iter().all(f64::is_finite)
        || length <= 0.0
        || !length.is_finite()
        || value[2].abs() > length * 1e-12
    {
        return Err(Failure::Unsupported("measure_wall_direction"));
    }
    let sign = if value[0] < 0.0 || (value[0] == 0.0 && value[1] < 0.0) {
        -1.0
    } else {
        1.0
    };
    Ok([value[0] / length * sign, value[1] / length * sign])
}

fn dot(a: Xy, b: Xy) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn axis(building: &Building, element: ElementRef, cx: &mut Cx<'_>) -> Result<Xy, Failure> {
    let curves = curves::named(building, element, "Axis", cx)?;
    let [curve] = curves.as_slice() else {
        return Err(Failure::Unsupported("measure_wall_axis_count"));
    };
    let item = curve
        .graph
        .item(curve.graph.root)
        .ok_or(Failure::Invalid("measure_wall_axis"))?;
    let vector = match &item.kind {
        CurveKind::Polyline(points) => {
            let [a, b] = points.as_slice() else {
                return Err(Failure::Unsupported("measure_wall_axis_curve"));
            };
            sub(xy(*b), xy(*a))
        }
        CurveKind::Line {
            point,
            direction,
            magnitude,
        } => {
            if !xy(*point).into_iter().all(f64::is_finite)
                || !magnitude.is_finite()
                || *magnitude <= 0.0
            {
                return Err(Failure::Invalid("measure_wall_axis"));
            }
            xy(*direction)
        }
        _ => return Err(Failure::Unsupported("measure_wall_axis_curve")),
    };
    direction(curve.transform.vector(vector))
}

fn material(building: &Building, element: ElementRef) -> Option<&crate::Material> {
    building.components.material.get(element).or_else(|| {
        building
            .components
            .r#type
            .get(element)
            .and_then(|kind| building.components.material.get(kind.target))
    })
}

fn thickness(
    building: &Building,
    element: ElementRef,
    cx: &mut Cx<'_>,
) -> Result<Option<f64>, Failure> {
    let Some(material) = material(building, element).filter(|m| m.kind == MaterialKind::Layers)
    else {
        return Ok(None);
    };
    let mut sum = 0.0;
    for layer in &material.parts {
        cx.check(Kind::Work, 1)?;
        let value = layer
            .thickness
            .as_ref()
            .ok_or(Failure::Invalid("measure_layer_thickness"))?;
        let factor = crate::evaluation::length_scale(
            value
                .unit
                .as_ref()
                .ok_or(Failure::Invalid("measure_unit_missing"))?,
        )?;
        if !value.value.is_finite() || value.value < 0.0 {
            return Err(Failure::Invalid("measure_layer_thickness"));
        }
        sum += value.value * factor;
    }
    if !sum.is_finite() || sum <= 0.0 {
        return Err(Failure::Invalid("measure_layer_thickness"));
    }
    Ok(Some(sum))
}

fn material_direction(
    building: &Building,
    element: ElementRef,
    cx: &mut Cx<'_>,
) -> Result<Xy, Failure> {
    let material =
        material(building, element).ok_or(Failure::Unsupported("measure_wall_material"))?;
    if material.direction.as_deref() != Some("AXIS2") {
        return Err(Failure::Unsupported("measure_wall_layer_direction"));
    }
    let data = building
        .geometry(element)
        .ok_or(Failure::Invalid("measure_shape_missing"))?
        .data();
    let mut context = None;
    for node in &data.nodes {
        cx.check(Kind::Work, 1)?;
        if !matches!(&node.links, GeometryLinks::Representation { identifier:Some(id),.. } if id=="Body")
        {
            continue;
        }
        let GeometryDefinition::Context {
            definition,
            length_unit,
        } = &node.definition
        else {
            return Err(Failure::Invalid("measure_wall_context"));
        };
        let crate::ContextPlacement::Space(position) =
            &definition.geometric.world_coordinate_system
        else {
            return Err(Failure::Unsupported("measure_wall_context"));
        };
        let factor = crate::evaluation::length_scale(
            length_unit
                .as_ref()
                .ok_or(Failure::Invalid("measure_unit_missing"))?,
        )?;
        let value = crate::evaluation::values::placement(Some(position), factor)?;
        if context.is_some_and(|previous| previous != value) {
            return Err(Failure::Invalid("measure_wall_context"));
        }
        context = Some(value);
    }
    let transform = curves::world(building, element, cx)?
        .then(&context.ok_or(Failure::Invalid("measure_wall_context"))?);
    direction(transform.column(0))
}

fn one(
    building: &Building,
    element: ElementRef,
    meshes: &[ElementMesh],
    origin: Xyz,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<WallCenter, Failure> {
    let element_mesh = meshes
        .get(element.0 as usize)
        .ok_or(Failure::Invalid("measure_wall_element"))?;
    let section = area::section(building, element_mesh, origin, options, cx)?;
    let [region] = section.regions.as_slice() else {
        return Err(Failure::Unsupported("measure_wall_sections"));
    };
    if !region.holes.is_empty() {
        return Err(Failure::Unsupported("measure_wall_holes"));
    }
    let layers = thickness(building, element, cx)?;
    let (axis, basis) = match axis(building, element, cx) {
        Ok(axis) => (axis, WallBasis::AxisAndShape),
        Err(Failure::Stopped(stop)) => return Err(Failure::Stopped(stop)),
        Err(_) if layers.is_some() => (
            material_direction(building, element, cx)?,
            WallBasis::LayersAndShape,
        ),
        Err(error) => return Err(error),
    };
    let normal = [-axis[1], axis[0]];
    let mut points = Vec::new();
    reserve(&mut points, region.outer.len(), 16, cx)?;
    for point in &region.outer {
        cx.check(Kind::Work, 1)?;
        let p = section
            .grid
            .position(*point)
            .map_err(|_| Failure::Invalid("measure_grid"))?;
        points.push([dot(p, axis), dot(p, normal)]);
    }
    let first = points
        .first()
        .copied()
        .ok_or(Failure::Invalid("measure_wall_section"))?;
    let (mut lo, mut hi) = (first, first);
    for &p in &points {
        cx.check(Kind::Work, 1)?;
        for ((min, max), value) in lo.iter_mut().zip(&mut hi).zip(p) {
            *min = min.min(value);
            *max = max.max(value);
        }
    }
    let width = hi[1] - lo[1];
    let length = hi[0] - lo[0];
    let tolerance = options.tolerance.length;
    if width <= tolerance || length <= tolerance || !width.is_finite() || !length.is_finite() {
        return Err(Failure::Invalid("measure_wall_section"));
    }
    // 全周が矩形の境界にあり、囲む面積も一致することを確かめる。凹みを矩形で補わない。
    let mut twice = 0.0;
    for (a, b) in points.iter().zip(points.iter().cycle().skip(1)) {
        cx.check(Kind::Work, 1)?;
        if (a[0] - lo[0]).min(hi[0] - a[0]) > tolerance
            && (a[1] - lo[1]).min(hi[1] - a[1]) > tolerance
        {
            return Err(Failure::Unsupported("measure_wall_not_rectangular"));
        }
        twice += (a[0] - lo[0]) * (b[1] - lo[1]) - (a[1] - lo[1]) * (b[0] - lo[0]);
    }
    if (twice / 2.0 - width * length).abs() > tolerance * (width + length) * 2.0
        || layers.is_some_and(|t| (t - width).abs() > tolerance)
    {
        return Err(Failure::Invalid("measure_wall_thickness_mismatch"));
    }
    let center = (lo[1] + hi[1]) / 2.0;
    let point = |s| {
        [
            axis[0] * s + normal[0] * center,
            axis[1] * s + normal[1] * center,
        ]
    };
    Ok(WallCenter {
        element,
        id: building
            .id(element)
            .ok_or(Failure::Invalid("measure_wall_element"))?,
        ends: [point(lo[0]), point(hi[0])],
        thickness: width,
        basis,
    })
}

pub(super) fn gather(
    building: &Building,
    elements: &[ElementRef],
    meshes: &[ElementMesh],
    origin: Xyz,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Vec<WallCenter>, Failure> {
    let mut out = Vec::new();
    reserve(&mut out, elements.len(), 112, cx)?;
    for &element in elements {
        match one(building, element, meshes, origin, options, cx) {
            Ok(wall) => out.push(wall),
            Err(Failure::Stopped(stop)) => return Err(Failure::Stopped(stop)),
            Err(error) => {
                let reason = match &error {
                    Failure::Invalid(s) | Failure::Unsupported(s) => *s,
                    Failure::Failed(s) => s,
                    Failure::Stopped(_) => "stopped",
                };
                cx.report(
                    Diagnostic::new(codes::BUILDING_MEASURE_FAILED)
                        .with("reason", reason)
                        .at(building.place(element)),
                );
                cx.check(Kind::Work, 0)?;
            }
        }
    }
    Ok(out)
}
