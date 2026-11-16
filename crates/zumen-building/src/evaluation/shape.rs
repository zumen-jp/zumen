//! 建物が所有する元の形を、形式を知らない 3D の形へ写す。

use super::{
    curves,
    values::{placement, reserve, unit},
};
use crate::*;
use zumen_base::{Cx, Kind};
use zumen_geom3d::{
    Curve, Face, Failure, MeshChanges, Plane, Profile, Shape, Tolerance, Transform, Xyz, scale,
};

fn polygon(points: impl IntoIterator<Item = [f64; 2]>, cx: &mut Cx) -> Result<Curve, Failure> {
    let mut out = Vec::new();
    for p in points {
        cx.check(Kind::Work, 1)?;
        reserve(&mut out, 1, 16, cx)?;
        out.push(p);
    }
    Ok(Curve::Polygon(out))
}

fn rectangle(x: f64, y: f64, cx: &mut Cx) -> Result<Curve, Failure> {
    if !(x.is_finite() && y.is_finite() && x > 0.0 && y > 0.0) {
        return Err(Failure::Invalid("profile_dimensions"));
    }
    let (x, y) = (x / 2.0, y / 2.0);
    polygon([[-x, -y], [x, -y], [x, y], [-x, y]], cx)
}

fn boundary(
    graph: Option<&CurveGraph>,
    reference: CurveRef,
    tol: Tolerance,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Curve, Failure> {
    let graph = graph.ok_or(Failure::Invalid("profile_curve"))?;
    let mut points = curves::evaluate(graph, reference, tol.chord, changes, cx)?;
    if points.iter().any(|p| p[2].abs() > tol.length) {
        return Err(Failure::Invalid("profile_plane"));
    }
    if points.len() < 4
        || points
            .first()
            .zip(points.last())
            .is_none_or(|(a, b)| zumen_geom3d::length(zumen_geom3d::sub(*a, *b)) > tol.length)
    {
        return Err(Failure::Invalid("profile_closed"));
    }
    // 閉じる点は先頭から許容差内にあることを上で確かめた。重ねて辺にしない。
    points.pop();
    polygon(points.into_iter().map(|p| [p[0], p[1]]), cx)
}

fn profile(
    value: &AreaProfile,
    graph: Option<&CurveGraph>,
    factor: f64,
    tol: Tolerance,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Profile, Failure> {
    let mut holes = Vec::new();
    let outer = match &value.kind {
        AreaProfileKind::ArbitraryClosed { boundary: at } => {
            boundary(graph, *at, tol, changes, cx)?
        }
        AreaProfileKind::ArbitraryWithVoids {
            boundary: at,
            holes: inner,
        } => {
            reserve(&mut holes, inner.len(), 48, cx)?;
            for &inner in inner {
                holes.push(boundary(graph, inner, tol, changes, cx)?);
            }
            boundary(graph, *at, tol, changes, cx)?
        }
        AreaProfileKind::Rectangle { x_dim, y_dim } => {
            rectangle(x_dim * factor, y_dim * factor, cx)?
        }
        AreaProfileKind::Circle { radius } => Curve::Circle {
            center: [0.0; 2],
            radius: radius * factor,
        },
        AreaProfileKind::RectangleHollow {
            x_dim,
            y_dim,
            wall_thickness,
            inner_fillet_radius,
            outer_fillet_radius,
        } => {
            if inner_fillet_radius.is_some_and(|v| v != 0.0)
                || outer_fillet_radius.is_some_and(|v| v != 0.0)
            {
                return Err(Failure::Unsupported("rounded_profile"));
            }
            let (x, y, t) = (x_dim * factor, y_dim * factor, wall_thickness * factor);
            if !t.is_finite() || t <= 0.0 {
                return Err(Failure::Invalid("profile_thickness"));
            }
            reserve(&mut holes, 1, 48, cx)?;
            holes.push(rectangle(x - 2.0 * t, y - 2.0 * t, cx)?);
            rectangle(x, y, cx)?
        }
        AreaProfileKind::IShape {
            overall_width,
            overall_depth,
            web_thickness,
            flange_thickness,
            fillet_radius,
            flange_edge_radius,
            flange_slope,
        } => {
            if fillet_radius.is_some_and(|v| v != 0.0)
                || flange_edge_radius.is_some_and(|v| v != 0.0)
                || flange_slope.as_ref().is_some_and(|v| v.value != 0.0)
            {
                return Err(Failure::Unsupported("rounded_profile"));
            }
            let (x, y, w, t) = (
                overall_width * factor / 2.0,
                overall_depth * factor / 2.0,
                web_thickness * factor / 2.0,
                flange_thickness * factor,
            );
            if [x, y, w, t].iter().any(|v| !v.is_finite() || *v <= 0.0) || w >= x || t >= y {
                return Err(Failure::Invalid("profile_dimensions"));
            }
            polygon(
                [
                    [-x, -y],
                    [x, -y],
                    [x, -y + t],
                    [w, -y + t],
                    [w, y - t],
                    [x, y - t],
                    [x, y],
                    [-x, y],
                    [-x, y - t],
                    [-w, y - t],
                    [-w, -y + t],
                    [-x, -y + t],
                ],
                cx,
            )?
        }
    };
    let mut result = Profile { outer, holes };
    if let Some(p) = &value.position {
        let reference = p.reference_direction.map(|[x, y]| [x, y, 0.0]);
        let frame = Transform::from_axes(
            [p.location[0] * factor, p.location[1] * factor, 0.0],
            None,
            reference,
        )
        .ok_or(Failure::Invalid("profile_position"))?;
        for curve in std::iter::once(&mut result.outer).chain(&mut result.holes) {
            match curve {
                Curve::Circle { center, .. } => {
                    let p = frame.point([center[0], center[1], 0.0]);
                    *center = [p[0], p[1]];
                }
                Curve::Polygon(points) => {
                    cx.check(Kind::Work, points.len() as u64)?;
                    for p in points {
                        let q = frame.point([p[0], p[1], 0.0]);
                        *p = [q[0], q[1]];
                    }
                }
            }
        }
    }
    Ok(result)
}

fn face(graph: &FaceGraph, value: &PolygonFace, factor: f64, cx: &mut Cx) -> Result<Face, Failure> {
    let mut outer = None;
    let mut holes = Vec::new();
    reserve(&mut holes, value.bounds.len(), 24, cx)?;
    for at in &value.bounds {
        cx.check(Kind::Work, 1)?;
        let bound = graph
            .bounds
            .get(at.0 as usize)
            .ok_or(Failure::Invalid("face_bound"))?;
        let polygon = graph
            .loops
            .get(bound.polygon.0 as usize)
            .ok_or(Failure::Invalid("face_loop"))?;
        let mut points = Vec::new();
        reserve(&mut points, polygon.points.len(), 24, cx)?;
        for at in &polygon.points {
            cx.check(Kind::Work, 1)?;
            points.push(scale(
                *graph
                    .points
                    .get(at.0 as usize)
                    .ok_or(Failure::Invalid("face_point"))?,
                factor,
            ));
        }
        if !bound.orientation {
            points.reverse();
        }
        if bound.outer || value.bounds.len() == 1 {
            if outer.is_some() {
                return Err(Failure::Invalid("face_outer"));
            }
            outer = Some(points);
        } else {
            holes.push(points);
        }
    }
    Ok(Face {
        outer: outer.ok_or(Failure::Unsupported("face_outer"))?,
        holes,
    })
}

fn faces(graph: &FaceGraph, factor: f64, cx: &mut Cx) -> Result<Shape, Failure> {
    let mut shapes = Vec::new();
    reserve(&mut shapes, graph.roots.len(), 256, cx)?;
    for at in &graph.roots {
        let set = graph
            .sets
            .get(at.0 as usize)
            .ok_or(Failure::Invalid("face_set"))?;
        let mut faces = Vec::new();
        reserve(&mut faces, set.faces.len(), 48, cx)?;
        for at in &set.faces {
            cx.check(Kind::Work, 1)?;
            faces.push(face(
                graph,
                graph
                    .faces
                    .get(at.0 as usize)
                    .ok_or(Failure::Invalid("face"))?,
                factor,
                cx,
            )?);
        }
        shapes.push(Shape::FaceSet {
            faces,
            closed: set.closed == Some(true),
        });
    }
    Ok(Shape::Group(shapes))
}

fn indexed(value: &IndexedFaceSet, factor: f64, cx: &mut Cx) -> Result<Shape, Failure> {
    let index = |i: u32| -> Result<u32, Failure> {
        let i = i.checked_sub(1).ok_or(Failure::Invalid("face_index"))?;
        let i = if let Some(pn) = &value.point_index {
            pn.get(i as usize)
                .copied()
                .and_then(|v| v.checked_sub(1))
                .ok_or(Failure::Invalid("point_index"))?
        } else {
            i
        };
        if i as usize >= value.points.len() {
            return Err(Failure::Invalid("face_index"));
        }
        Ok(i)
    };
    match &value.faces {
        IndexedFaces::Triangles { indices, .. } => {
            let mut positions = Vec::new();
            reserve(&mut positions, value.points.len(), 24, cx)?;
            cx.check(Kind::Work, value.points.len() as u64)?;
            positions.extend(value.points.iter().map(|p| scale(*p, factor)));
            let mut triangles = Vec::new();
            reserve(&mut triangles, indices.len(), 12, cx)?;
            for t in indices {
                cx.check(Kind::Work, 1)?;
                triangles.push([index(t[0])?, index(t[1])?, index(t[2])?]);
            }
            Ok(Shape::Triangles {
                positions,
                triangles,
                closed: value.closed == Some(true),
            })
        }
        IndexedFaces::Polygons { indices, faces } => {
            let ring = |indices: &[u32], cx: &mut Cx| -> Result<Vec<Xyz>, Failure> {
                let mut out = Vec::new();
                reserve(&mut out, indices.len(), 24, cx)?;
                for &i in indices {
                    cx.check(Kind::Work, 1)?;
                    out.push(scale(
                        *value
                            .points
                            .get(index(i)? as usize)
                            .ok_or(Failure::Invalid("point"))?,
                        factor,
                    ));
                }
                Ok(out)
            };
            let mut out = Vec::new();
            reserve(&mut out, indices.len(), 48, cx)?;
            for at in indices {
                let f = faces
                    .get(at.0 as usize)
                    .ok_or(Failure::Invalid("polygon"))?;
                let mut holes = Vec::new();
                reserve(&mut holes, f.holes.len(), 24, cx)?;
                for h in &f.holes {
                    holes.push(ring(h, cx)?);
                }
                out.push(Face {
                    outer: ring(&f.boundary, cx)?,
                    holes,
                });
            }
            Ok(Shape::FaceSet {
                faces: out,
                closed: value.closed == Some(true),
            })
        }
    }
}

pub(super) fn half_space(value: &PlaneHalfSpace, factor: f64) -> Result<Plane, Failure> {
    let frame = placement(Some(&value.plane), factor)?;
    Ok(Plane {
        origin: frame.column(3),
        normal: scale(frame.column(2), if value.agreement { 1.0 } else { -1.0 }),
    })
}

pub(super) fn evaluate(
    value: &GeometryDefinition,
    tol: Tolerance,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Shape, Failure> {
    cx.check(Kind::Work, 1)?;
    match value {
        GeometryDefinition::Extrusion {
            definition,
            boundary,
            length_unit,
        } => {
            let factor = unit(length_unit.as_ref(), "LENGTHUNIT")?;
            Ok(Shape::Extrusion {
                profile: profile(
                    &definition.profile,
                    boundary.as_ref(),
                    factor,
                    tol,
                    changes,
                    cx,
                )?,
                position: placement(definition.position.as_ref(), factor)?,
                direction: definition.direction,
                depth: definition.depth * factor,
            })
        }
        GeometryDefinition::Revolution {
            definition,
            boundary,
            length_unit,
        } => {
            let factor = unit(length_unit.as_ref(), "LENGTHUNIT")?;
            let angle = (definition.angle.value
                * unit(definition.angle.unit.as_ref(), "PLANEANGLEUNIT")?)
            .to_degrees();
            Ok(Shape::Revolution {
                profile: profile(
                    &definition.profile,
                    boundary.as_ref(),
                    factor,
                    tol,
                    changes,
                    cx,
                )?,
                position: placement(definition.position.as_ref(), factor)?,
                axis_origin: scale(definition.axis.location, factor),
                axis_direction: definition.axis.direction,
                angle,
            })
        }
        GeometryDefinition::SweptDisk {
            definition,
            directrix,
        } => {
            let factor = unit(directrix.length_unit.as_ref(), "LENGTHUNIT")?;
            Ok(Shape::SweptDisk {
                directrix: curves::interval(
                    directrix,
                    definition.directrix,
                    [definition.start_parameter, definition.end_parameter],
                    tol.chord,
                    changes,
                    cx,
                )?,
                radius: definition.radius * factor,
                inner_radius: definition.inner_radius.map(|r| r * factor),
            })
        }
        GeometryDefinition::Faces {
            definition,
            length_unit,
        } => faces(definition, unit(length_unit.as_ref(), "LENGTHUNIT")?, cx),
        GeometryDefinition::Tessellation {
            definition,
            length_unit,
        } => indexed(definition, unit(length_unit.as_ref(), "LENGTHUNIT")?, cx),
        GeometryDefinition::HalfSpace {
            definition,
            length_unit,
            ..
        } => {
            if matches!(definition.limit, HalfSpaceLimit::Polygonal { .. }) {
                return Err(Failure::Unsupported("bounded_half_space"));
            }
            Ok(Shape::HalfSpace {
                plane: half_space(definition, unit(length_unit.as_ref(), "LENGTHUNIT")?)?,
            })
        }
        _ => Err(Failure::Unsupported("shape")),
    }
}

pub(super) fn kind(value: &GeometryDefinition) -> &str {
    match value {
        GeometryDefinition::Links => "links",
        GeometryDefinition::Context { .. } => "context",
        GeometryDefinition::Curve(_) => "curve",
        GeometryDefinition::Extrusion { .. } => "extrusion",
        GeometryDefinition::Revolution { .. } => "revolution",
        GeometryDefinition::SweptDisk { .. } => "swept_disk",
        GeometryDefinition::Faces { .. } => "faces",
        GeometryDefinition::Tessellation { .. } => "tessellation",
        GeometryDefinition::HalfSpace { .. } => "half_space",
        GeometryDefinition::Mapping { .. } => "mapping",
        GeometryDefinition::Unsupported(name) => name.as_str(),
    }
}
