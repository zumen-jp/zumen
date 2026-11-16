//! 名指した表現の曲線を、配置と元項目を保って計測する。

use super::{Calculated, Options, Source, reserve};
use crate::{
    Building, ContextPlacement, Coordinates, CurveGraph, CurveKind, CurveRef, ElementRef,
    GeometryData, GeometryDefinition, GeometryLinks, GeometryRef, evaluation,
};
use zumen_base::{Cx, Key, Kind};
use zumen_geom3d::{Failure, MeshChanges, Transform, Xyz, dot, sub};

fn coordinates(point: Coordinates) -> Xyz {
    match point {
        Coordinates::Two([x, y]) => [x, y, 0.0],
        Coordinates::Three(p) => p,
    }
}

fn norm(v: Xyz) -> f64 {
    libm::hypot(libm::hypot(v[0], v[1]), v[2])
}

pub(super) fn document_scale(building: &Building, cx: &mut Cx<'_>) -> Result<f64, Failure> {
    cx.check(Kind::Work, building.document.units.len() as u64)?;
    let mut units = building
        .document
        .units
        .iter()
        .filter(|u| u.kind == "LENGTHUNIT");
    let first = evaluation::length_scale(
        units
            .next()
            .ok_or(Failure::Invalid("measure_unit_missing"))?,
    )?;
    for unit in units {
        if evaluation::length_scale(unit)? != first {
            return Err(Failure::Invalid("measure_unit_conflict"));
        }
    }
    Ok(first)
}

pub(super) fn world(
    building: &Building,
    element: ElementRef,
    cx: &mut Cx<'_>,
) -> Result<Transform, Failure> {
    let factor = document_scale(building, cx)?;
    let value = match building.world_placement(element, cx)? {
        Some(value) => value,
        None if building.components.placement.get(element).is_none() => crate::Transform::IDENTITY,
        None => return Err(Failure::Invalid("measure_placement")),
    };
    let mut value = Transform(value.0);
    for row in &mut value.0 {
        row[3] *= factor;
    }
    if !value.is_finite() || !value.determinant().is_finite() || value.determinant() == 0.0 {
        return Err(Failure::Invalid("measure_placement"));
    }
    Ok(value)
}

pub(super) struct Curve<'a> {
    pub item: GeometryRef,
    pub graph: &'a CurveGraph,
    pub transform: Transform,
    pub instance: Option<u64>,
}

fn walk<'a>(
    data: &'a GeometryData,
    at: GeometryRef,
    transform: Transform,
    out: &mut Vec<Curve<'a>>,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    cx.enter()?;
    let result = (|| {
        cx.check(Kind::Work, 1)?;
        let node = data
            .nodes
            .get(at.0 as usize)
            .ok_or(Failure::Invalid("measure_reference"))?;
        match &node.links {
            GeometryLinks::Definition(items)
            | GeometryLinks::Representation { items, .. }
            | GeometryLinks::Set { items, .. } => {
                for &child in items {
                    walk(data, child, transform, out, cx)?;
                }
            }
            GeometryLinks::Map(child) => walk(data, *child, transform, out, cx)?,
            GeometryLinks::Mapped(child) => {
                let GeometryDefinition::Mapping {
                    definition,
                    length_unit,
                } = &node.definition
                else {
                    return Err(Failure::Invalid("measure_mapping"));
                };
                let factor = evaluation::length_scale(
                    length_unit
                        .as_ref()
                        .ok_or(Failure::Invalid("measure_unit_missing"))?,
                )?;
                let mapping = evaluation::values::mapping(definition, factor)?;
                walk(data, *child, mapping.then(&transform), out, cx)?;
            }
            GeometryLinks::Leaf => {
                let GeometryDefinition::Curve(graph) = &node.definition else {
                    return Err(Failure::Unsupported("measure_curve"));
                };
                reserve(out, 1, 160, cx)?;
                out.push(Curve {
                    item: at,
                    graph,
                    transform,
                    instance: node.source.instance,
                });
            }
            GeometryLinks::Boolean { .. } => {
                return Err(Failure::Unsupported("measure_curve_boolean"));
            }
        }
        Ok(())
    })();
    cx.leave();
    result
}

pub(super) fn named<'a>(
    building: &'a Building,
    element: ElementRef,
    name: &str,
    cx: &mut Cx<'_>,
) -> Result<Vec<Curve<'a>>, Failure> {
    let data = building
        .geometry(element)
        .ok_or(Failure::Invalid("measure_shape_missing"))?
        .data();
    let root = data.root.ok_or(Failure::Invalid("measure_shape_missing"))?;
    let root_node = data
        .nodes
        .get(root.0 as usize)
        .ok_or(Failure::Invalid("measure_reference"))?;
    let representations = match &root_node.links {
        GeometryLinks::Definition(items) => items.as_slice(),
        _ => std::slice::from_ref(&root),
    };
    let placement = world(building, element, cx)?;
    let mut out = Vec::new();
    for &at in representations {
        cx.check(Kind::Work, name.len() as u64 + 1)?;
        let node = data
            .nodes
            .get(at.0 as usize)
            .ok_or(Failure::Invalid("measure_reference"))?;
        if !matches!(&node.links, GeometryLinks::Representation { identifier: Some(id), .. } if id == name)
        {
            continue;
        }
        let context = match &node.definition {
            GeometryDefinition::Context {
                definition,
                length_unit,
            } => {
                let factor = evaluation::length_scale(
                    length_unit
                        .as_ref()
                        .ok_or(Failure::Invalid("measure_unit_missing"))?,
                )?;
                match &definition.geometric.world_coordinate_system {
                    ContextPlacement::Space(p) => evaluation::values::placement(Some(p), factor)?,
                    ContextPlacement::Plane(p) => Transform::from_axes(
                        [p.location[0] * factor, p.location[1] * factor, 0.0],
                        Some([0.0, 0.0, 1.0]),
                        p.reference_direction.map(|[x, y]| [x, y, 0.0]),
                    )
                    .ok_or(Failure::Invalid("measure_context"))?,
                }
            }
            GeometryDefinition::Links => Transform::IDENTITY,
            _ => return Err(Failure::Invalid("measure_context")),
        };
        walk(data, at, placement.then(&context), &mut out, cx)?;
    }
    if out.is_empty() {
        return Err(Failure::Unsupported("measure_representation"));
    }
    Ok(out)
}

fn curve_length(
    graph: &CurveGraph,
    at: CurveRef,
    transform: Transform,
    options: Options<'_>,
    changes: &mut MeshChanges,
    cx: &mut Cx<'_>,
) -> Result<f64, Failure> {
    cx.enter()?;
    let result = (|| {
        cx.check(Kind::Work, 1)?;
        let kind = &graph
            .item(at)
            .ok_or(Failure::Invalid("measure_curve_reference"))?
            .kind;
        if let CurveKind::Composite { segments, .. } = kind {
            // 不連続な区間の間に、書かれていない接続線を足さない。
            let mut sum = 0.0;
            for segment in segments {
                sum += curve_length(graph, segment.curve, transform, options, changes, cx)?;
            }
            return Ok(sum);
        }
        if let CurveKind::Circle { position, radius } = kind {
            if !coordinates(position.location)
                .into_iter()
                .all(f64::is_finite)
            {
                return Err(Failure::Invalid("measure_curve_position"));
            }
            let factor = evaluation::length_scale(
                graph
                    .length_unit
                    .as_ref()
                    .ok_or(Failure::Invalid("measure_unit_missing"))?,
            )?;
            let frame = Transform::from_axes(
                [0.0; 3],
                position.axis,
                position.reference_direction.map(coordinates),
            )
            .ok_or(Failure::Invalid("measure_curve_axis"))?;
            let x = transform.vector(frame.column(0));
            let y = transform.vector(frame.column(1));
            let (sx, sy) = (norm(x), norm(y));
            // 円の平面で同じ尺度なら、弦で近似せず元の半径から求める。
            if sx == sy && dot(x, y) == 0.0 {
                let value = core::f64::consts::TAU * radius * factor * sx;
                if !value.is_finite() || value <= 0.0 {
                    return Err(Failure::Invalid("measure_circle"));
                }
                return Ok(value);
            }
        }
        let scale = norm([
            norm(transform.column(0)),
            norm(transform.column(1)),
            norm(transform.column(2)),
        ]);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(Failure::Invalid("measure_placement"));
        }
        let points =
            evaluation::curves::evaluate(graph, at, options.tolerance.chord / scale, changes, cx)?;
        cx.check(Kind::Work, points.len() as u64)?;
        let mut length = 0.0;
        for pair in points.windows(2) {
            let [a, b] = pair else {
                return Err(Failure::Invalid("measure_curve_points"));
            };
            length += norm(transform.vector(sub(*b, *a)));
        }
        Ok(length)
    })();
    cx.leave();
    result
}

pub(super) fn length(
    building: &Building,
    element: ElementRef,
    representation: &str,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Calculated, Failure> {
    let curves = named(building, element, representation, cx)?;
    let mut sources = Vec::new();
    reserve(&mut sources, curves.len(), 64, cx)?;
    let mut si_value = 0.0;
    for curve in curves {
        let mut changes = MeshChanges::default();
        si_value += curve_length(
            curve.graph,
            curve.graph.root,
            curve.transform,
            options,
            &mut changes,
            cx,
        )?;
        let mut place = building.place(element);
        if let Some(instance) = curve.instance {
            place = place.with_key(Key::Instance(instance));
        }
        changes.report(Some(&place), options.tolerance, cx)?;
        sources.push(Source {
            item: curve.item,
            instance: curve.instance,
            tolerance: options.tolerance,
            basis: if changes.approximated_curves == 0 {
                super::SourceBasis::Definition
            } else {
                super::SourceBasis::Evaluated
            },
        });
    }
    Ok(Calculated {
        si_value,
        sources,
        boundary: None,
    })
}
