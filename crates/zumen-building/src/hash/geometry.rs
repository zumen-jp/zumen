//! 形の出所を除き、最初に参照する順でグラフ内の番号を付け直して符号化する。
//! 共有する子を展開しないので、深い写像と共有の多いグラフでも作業量は項目と辺の数に比例する。

use super::{Canon, Encode, Hash};
use crate::*;
use zumen_base::{Cx, Kind};

mod values;

type Result<T> = core::result::Result<T, GeometryError>;

fn entry<T>(values: &[T], index: usize) -> Result<&T> {
    values.get(index).ok_or(GeometryError::Reference)
}

fn reserve(cx: &mut Cx, count: usize, bytes: usize) -> Result<()> {
    if count > i32::MAX as usize / bytes {
        return Err(GeometryError::Reference);
    }
    cx.check(Kind::DecodedBytes, (count as u64) * (bytes as u64))?;
    Ok(())
}

struct Order {
    indices: Vec<u32>,
    map: Vec<u32>,
}

impl Order {
    fn new(
        count: usize,
        roots: &[u32],
        children: impl Fn(usize) -> Result<usize>,
        child: impl Fn(usize, usize) -> Result<u32>,
        cx: &mut Cx,
    ) -> Result<Self> {
        reserve(cx, count, 32)?;
        cx.check(Kind::Work, count as u64)?;
        let mut result = Self {
            indices: Vec::with_capacity(count),
            map: vec![u32::MAX; count],
        };
        let mut state = vec![0u8; count];
        let mut stack: Vec<(u32, usize)> = Vec::with_capacity(count);
        let operation = (|| {
            for &root in roots {
                cx.check(Kind::Work, 1)?;
                let Some(s) = state.get(root as usize) else {
                    return Err(GeometryError::Reference);
                };
                if *s == 2 {
                    continue;
                }
                cx.enter()?;
                *result
                    .map
                    .get_mut(root as usize)
                    .ok_or(GeometryError::Reference)? = result.indices.len() as u32;
                result.indices.push(root);
                *state
                    .get_mut(root as usize)
                    .ok_or(GeometryError::Reference)? = 1;
                stack.push((root, 0));
                while let Some((at, next)) = stack.last_mut() {
                    cx.check(Kind::Work, 1)?;
                    if *next == children(*at as usize)? {
                        *state
                            .get_mut(*at as usize)
                            .ok_or(GeometryError::Reference)? = 2;
                        stack.pop();
                        cx.leave();
                        continue;
                    }
                    let to = child(*at as usize, *next)?;
                    *next += 1;
                    match state.get(to as usize) {
                        None => return Err(GeometryError::Reference),
                        Some(1) => return Err(GeometryError::Cycle),
                        Some(2) => continue,
                        _ => {}
                    }
                    cx.enter()?;
                    *result
                        .map
                        .get_mut(to as usize)
                        .ok_or(GeometryError::Reference)? = result.indices.len() as u32;
                    result.indices.push(to);
                    *state.get_mut(to as usize).ok_or(GeometryError::Reference)? = 1;
                    stack.push((to, 0));
                }
            }
            if result.indices.len() != count {
                return Err(GeometryError::Unreachable);
            }
            Ok(())
        })();
        for _ in 0..stack.len() {
            cx.leave();
        }
        operation?;
        Ok(result)
    }

    fn get(&self, index: u32) -> Result<u32> {
        self.map
            .get(index as usize)
            .copied()
            .filter(|v| *v != u32::MAX)
            .ok_or(GeometryError::Reference)
    }
    fn reference(&self, index: u32, out: &mut Canon<'_>) -> Result<()> {
        self.get(index)?.encode(out);
        Ok(())
    }
}

fn shape_order(data: &GeometryData, cx: &mut Cx) -> Result<Order> {
    let roots = data.root.map(|v| v.0);
    Order::new(
        data.nodes.len(),
        roots.as_slice(),
        |i| {
            Ok(
                match &data.nodes.get(i).ok_or(GeometryError::Reference)?.links {
                    GeometryLinks::Definition(v)
                    | GeometryLinks::Set { items: v, .. }
                    | GeometryLinks::Representation { items: v, .. } => v.len(),
                    GeometryLinks::Map(_) | GeometryLinks::Mapped(_) => 1,
                    GeometryLinks::Boolean { .. } => 2,
                    GeometryLinks::Leaf => 0,
                },
            )
        },
        |i, at| {
            Ok(
                match &data.nodes.get(i).ok_or(GeometryError::Reference)?.links {
                    GeometryLinks::Definition(v)
                    | GeometryLinks::Set { items: v, .. }
                    | GeometryLinks::Representation { items: v, .. } => {
                        v.get(at).ok_or(GeometryError::Reference)?.0
                    }
                    GeometryLinks::Map(v) | GeometryLinks::Mapped(v) => v.0,
                    GeometryLinks::Boolean { first, second, .. } => {
                        if at == 0 {
                            first.0
                        } else {
                            second.0
                        }
                    }
                    GeometryLinks::Leaf => return Err(GeometryError::Reference),
                },
            )
        },
        cx,
    )
}

fn curve_order(graph: &CurveGraph, extra: &[CurveRef], cx: &mut Cx) -> Result<Order> {
    reserve(cx, extra.len().saturating_add(1), 4)?;
    cx.check(Kind::Work, extra.len() as u64)?;
    let mut roots = Vec::with_capacity(extra.len() + 1);
    roots.push(graph.root.0);
    roots.extend(extra.iter().map(|r| r.0));
    Order::new(
        graph.items.len(),
        &roots,
        |i| {
            Ok(
                match &graph.items.get(i).ok_or(GeometryError::Reference)?.kind {
                    CurveKind::Composite { segments, .. } => segments.len(),
                    CurveKind::Trimmed { .. } => 1,
                    _ => 0,
                },
            )
        },
        |i, at| match &graph.items.get(i).ok_or(GeometryError::Reference)?.kind {
            CurveKind::Composite { segments, .. } => segments
                .get(at)
                .map(|s| s.curve.0)
                .ok_or(GeometryError::Reference),
            CurveKind::Trimmed { basis, .. } => Ok(basis.0),
            _ => Err(GeometryError::Reference),
        },
        cx,
    )
}

enum ValueOrder {
    None,
    Curve(Order),
    Faces(Order),
    Polygons(Order),
}

fn profile_order(
    profile: &AreaProfile,
    curve: &Option<CurveGraph>,
    cx: &mut Cx,
) -> Result<ValueOrder> {
    let graph = || curve.as_ref().ok_or(GeometryError::MissingDefinition);
    let order = match &profile.kind {
        AreaProfileKind::ArbitraryClosed { boundary } => {
            curve_order(graph()?, core::slice::from_ref(boundary), cx)?
        }
        AreaProfileKind::ArbitraryWithVoids { boundary, holes } => {
            let graph = graph()?;
            if graph.root != *boundary {
                return Err(GeometryError::Reference);
            }
            curve_order(graph, holes, cx)?
        }
        _ => {
            return if curve.is_none() {
                Ok(ValueOrder::None)
            } else {
                Err(GeometryError::MissingDefinition)
            };
        }
    };
    Ok(ValueOrder::Curve(order))
}

fn face_layout(graph: &FaceGraph) -> Result<[usize; 6]> {
    let mut offsets = [0usize; 6];
    let mut total = 0usize;
    for (slot, count) in offsets.iter_mut().skip(1).zip([
        graph.sets.len(),
        graph.faces.len(),
        graph.bounds.len(),
        graph.loops.len(),
        graph.points.len(),
    ]) {
        total = total.checked_add(count).ok_or(GeometryError::Reference)?;
        *slot = total;
    }
    Ok(offsets)
}

fn face_order(graph: &FaceGraph, cx: &mut Cx) -> Result<Order> {
    let o = face_layout(graph)?;
    reserve(cx, graph.roots.len(), 4)?;
    cx.check(Kind::Work, graph.roots.len() as u64)?;
    let roots: Vec<u32> = graph.roots.iter().map(|r| r.0).collect();
    if roots.iter().any(|r| *r as usize >= graph.sets.len()) {
        return Err(GeometryError::Reference);
    }
    Order::new(
        o[5],
        &roots,
        |i| {
            Ok(if i < o[1] {
                entry(&graph.sets, i)?.faces.len()
            } else if i < o[2] {
                entry(&graph.faces, i - o[1])?.bounds.len()
            } else if i < o[3] {
                1
            } else if i < o[4] {
                entry(&graph.loops, i - o[3])?.points.len()
            } else {
                0
            })
        },
        |i, at| {
            let (value, start, end) = if i < o[1] {
                (entry(&entry(&graph.sets, i)?.faces, at)?.0, o[1], o[2])
            } else if i < o[2] {
                (
                    entry(&entry(&graph.faces, i - o[1])?.bounds, at)?.0,
                    o[2],
                    o[3],
                )
            } else if i < o[3] {
                (entry(&graph.bounds, i - o[2])?.polygon.0, o[3], o[4])
            } else if i < o[4] {
                (
                    entry(&entry(&graph.loops, i - o[3])?.points, at)?.0,
                    o[4],
                    o[5],
                )
            } else {
                return Err(GeometryError::Reference);
            };
            if value as usize >= end - start {
                return Err(GeometryError::Reference);
            }
            u32::try_from(start + value as usize).map_err(|_| GeometryError::Reference)
        },
        cx,
    )
}

fn value_order(value: &GeometryDefinition, cx: &mut Cx) -> Result<ValueOrder> {
    Ok(match value {
        GeometryDefinition::Curve(g) => ValueOrder::Curve(curve_order(g, &[], cx)?),
        GeometryDefinition::Extrusion {
            definition,
            boundary,
            ..
        } => profile_order(&definition.profile, boundary, cx)?,
        GeometryDefinition::Revolution {
            definition,
            boundary,
            ..
        } => profile_order(&definition.profile, boundary, cx)?,
        GeometryDefinition::SweptDisk {
            definition,
            directrix,
        } => ValueOrder::Curve(curve_order(
            directrix,
            core::slice::from_ref(&definition.directrix),
            cx,
        )?),
        GeometryDefinition::HalfSpace {
            definition,
            boundary,
            ..
        } => match &definition.limit {
            HalfSpaceLimit::Polygonal { boundary: at, .. } => ValueOrder::Curve(curve_order(
                boundary.as_ref().ok_or(GeometryError::MissingDefinition)?,
                core::slice::from_ref(at),
                cx,
            )?),
            _ if boundary.is_none() => ValueOrder::None,
            _ => return Err(GeometryError::MissingDefinition),
        },
        GeometryDefinition::Faces { definition, .. } => {
            ValueOrder::Faces(face_order(definition, cx)?)
        }
        GeometryDefinition::Tessellation { definition, .. } => match &definition.faces {
            IndexedFaces::Polygons { indices, faces } => {
                reserve(cx, indices.len(), 4)?;
                cx.check(Kind::Work, indices.len() as u64)?;
                let roots: Vec<u32> = indices.iter().map(|r| r.0).collect();
                ValueOrder::Polygons(Order::new(
                    faces.len(),
                    &roots,
                    |_| Ok(0),
                    |_, _| Err(GeometryError::Reference),
                    cx,
                )?)
            }
            _ => ValueOrder::None,
        },
        _ => ValueOrder::None,
    })
}

pub(crate) fn geometry_digest(data: &GeometryData, cx: &mut Cx) -> Result<Hash> {
    cx.check(Kind::Work, 1)?;
    let shape = shape_order(data, cx)?;
    reserve(cx, data.nodes.len(), 128)?;
    let mut values = Vec::with_capacity(data.nodes.len());
    for node in &data.nodes {
        cx.check(Kind::Work, 1)?;
        values.push(value_order(&node.definition, cx)?);
    }
    for style in &data.styles {
        cx.check(Kind::Work, 1)?;
        shape.get(style.item.0)?;
        if let Some(r) = style.representation {
            shape.get(r.0)?;
        }
        if let Acquired::Value(p) = &style.presentation
            && let SurfaceStyleSelection::Layer(at) = p.surface_source
            && at as usize >= p.layers.len()
        {
            return Err(GeometryError::Reference);
        }
    }
    let axes = match &data.grid {
        Some(Acquired::Value(g)) => g.axes.as_slice(),
        _ => &[],
    };
    reserve(cx, axes.len(), 64)?;
    let mut grid = Vec::with_capacity(axes.len());
    for axis in axes {
        grid.push(curve_order(&axis.curve, &[], cx)?);
    }
    let building = Building::default();
    let mut check = || cx.check(Kind::Work, 1);
    let mut out = Canon::new(&building, Some(&mut check));
    "geometry".encode(&mut out);
    data.root.is_some().encode(&mut out);
    shape.indices.len().encode(&mut out);
    for &at in &shape.indices {
        if out.stop.is_some() {
            break;
        }
        let node = entry(&data.nodes, at as usize)?;
        encode_links(&node.links, &shape, &mut out)?;
        encode_definition(&node.definition, entry(&values, at as usize)?, &mut out)?;
    }
    data.styles.len().encode(&mut out);
    for style in &data.styles {
        if out.stop.is_some() {
            break;
        }
        shape.reference(style.item.0, &mut out)?;
        style.representation.is_some().encode(&mut out);
        if let Some(r) = style.representation {
            shape.reference(r.0, &mut out)?;
        }
        style.presentation.encode(&mut out);
    }
    match &data.grid {
        None => 0u8.encode(&mut out),
        Some(Acquired::Unsupported) => 1u8.encode(&mut out),
        Some(Acquired::Value(g)) => {
            2u8.encode(&mut out);
            g.layout.encode(&mut out);
            g.axes.len().encode(&mut out);
            for (axis, order) in g.axes.iter().zip(&grid) {
                if out.stop.is_some() {
                    break;
                }
                axis.definition.encode(&mut out);
                encode_curve(&axis.curve, order, &mut out)?;
            }
        }
    }
    if let Some(stop) = out.stop {
        return Err(stop.into());
    }
    Ok(out.finish())
}

const _: () = assert!(size_of::<ValueOrder>() <= 128);
const _: () = assert!(size_of::<Order>() <= 64);

fn encode_links(links: &GeometryLinks, order: &Order, out: &mut Canon<'_>) -> Result<()> {
    let refs = |values: &[GeometryRef], out: &mut Canon<'_>| -> Result<()> {
        values.len().encode(out);
        for r in values {
            if out.stop.is_some() {
                break;
            }
            order.reference(r.0, out)?;
        }
        Ok(())
    };
    match links {
        GeometryLinks::Definition(v) => {
            0u8.encode(out);
            refs(v, out)?;
        }
        GeometryLinks::Representation {
            identifier,
            kind,
            items,
        } => {
            1u8.encode(out);
            identifier.encode(out);
            kind.encode(out);
            refs(items, out)?;
        }
        GeometryLinks::Mapped(v) => {
            2u8.encode(out);
            order.reference(v.0, out)?;
        }
        GeometryLinks::Map(v) => {
            3u8.encode(out);
            order.reference(v.0, out)?;
        }
        GeometryLinks::Set { curves_only, items } => {
            4u8.encode(out);
            curves_only.encode(out);
            refs(items, out)?;
        }
        GeometryLinks::Boolean {
            operation,
            first,
            second,
        } => {
            5u8.encode(out);
            operation.encode(out);
            order.reference(first.0, out)?;
            order.reference(second.0, out)?;
        }
        GeometryLinks::Leaf => 6u8.encode(out),
    }
    Ok(())
}

impl ValueOrder {
    fn curve(&self) -> Result<&Order> {
        if let Self::Curve(o) = self {
            Ok(o)
        } else {
            Err(GeometryError::MissingDefinition)
        }
    }
}

fn encode_curve(graph: &CurveGraph, order: &Order, out: &mut Canon<'_>) -> Result<()> {
    graph.length_unit.encode(out);
    graph.angle_unit.encode(out);
    order.reference(graph.root.0, out)?;
    order.indices.len().encode(out);
    for &at in &order.indices {
        if out.stop.is_some() {
            break;
        }
        match &entry(&graph.items, at as usize)?.kind {
            CurveKind::Polyline(points) => {
                0u8.encode(out);
                points.encode(out);
            }
            CurveKind::Line {
                point,
                direction,
                magnitude,
            } => {
                1u8.encode(out);
                point.encode(out);
                direction.encode(out);
                magnitude.encode(out);
            }
            CurveKind::Circle { position, radius } => {
                2u8.encode(out);
                position.encode(out);
                radius.encode(out);
            }
            CurveKind::Indexed {
                points,
                segments,
                self_intersect,
            } => {
                3u8.encode(out);
                points.encode(out);
                segments.encode(out);
                self_intersect.encode(out);
            }
            CurveKind::Composite {
                segments,
                self_intersect,
            } => {
                4u8.encode(out);
                self_intersect.encode(out);
                segments.len().encode(out);
                for segment in segments {
                    if out.stop.is_some() {
                        break;
                    }
                    order.reference(segment.curve.0, out)?;
                    segment.same_sense.encode(out);
                    segment.transition.encode(out);
                }
            }
            CurveKind::Trimmed {
                basis,
                trim1,
                trim2,
                sense_agreement,
                preference,
            } => {
                5u8.encode(out);
                order.reference(basis.0, out)?;
                trim1.encode(out);
                trim2.encode(out);
                sense_agreement.encode(out);
                preference.encode(out);
            }
            CurveKind::Unsupported(kind) => {
                6u8.encode(out);
                kind.encode(out);
            }
        }
    }
    Ok(())
}

fn encode_profile(profile: &AreaProfile, order: &ValueOrder, out: &mut Canon<'_>) -> Result<()> {
    profile.name.encode(out);
    profile.position.encode(out);
    match &profile.kind {
        AreaProfileKind::ArbitraryClosed { boundary } => {
            0u8.encode(out);
            order.curve()?.reference(boundary.0, out)?;
        }
        AreaProfileKind::ArbitraryWithVoids { boundary, holes } => {
            1u8.encode(out);
            let curve = order.curve()?;
            curve.reference(boundary.0, out)?;
            holes.len().encode(out);
            for hole in holes {
                if out.stop.is_some() {
                    break;
                }
                curve.reference(hole.0, out)?;
            }
        }
        AreaProfileKind::Rectangle { x_dim, y_dim } => {
            2u8.encode(out);
            x_dim.encode(out);
            y_dim.encode(out);
        }
        AreaProfileKind::Circle { radius } => {
            3u8.encode(out);
            radius.encode(out);
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
            4u8.encode(out);
            overall_width.encode(out);
            overall_depth.encode(out);
            web_thickness.encode(out);
            flange_thickness.encode(out);
            fillet_radius.encode(out);
            flange_edge_radius.encode(out);
            flange_slope.encode(out);
        }
        AreaProfileKind::RectangleHollow {
            x_dim,
            y_dim,
            wall_thickness,
            inner_fillet_radius,
            outer_fillet_radius,
        } => {
            5u8.encode(out);
            x_dim.encode(out);
            y_dim.encode(out);
            wall_thickness.encode(out);
            inner_fillet_radius.encode(out);
            outer_fillet_radius.encode(out);
        }
    }
    Ok(())
}

fn encode_optional_curve(
    graph: &Option<CurveGraph>,
    order: &ValueOrder,
    out: &mut Canon<'_>,
) -> Result<()> {
    graph.is_some().encode(out);
    if let Some(graph) = graph {
        encode_curve(graph, order.curve()?, out)?;
    }
    Ok(())
}

fn encode_faces(graph: &FaceGraph, order: &Order, out: &mut Canon<'_>) -> Result<()> {
    let o = face_layout(graph)?;
    graph.kind.encode(out);
    graph.roots.len().encode(out);
    for root in &graph.roots {
        if out.stop.is_some() {
            break;
        }
        order.reference(root.0, out)?;
    }
    order.indices.len().encode(out);
    for &at in &order.indices {
        if out.stop.is_some() {
            break;
        }
        let i = at as usize;
        if i < o[1] {
            0u8.encode(out);
            let set = entry(&graph.sets, i)?;
            set.closed.encode(out);
            set.faces.len().encode(out);
            for r in &set.faces {
                if out.stop.is_some() {
                    break;
                }
                order.reference((o[1] + r.0 as usize) as u32, out)?;
            }
        } else if i < o[2] {
            1u8.encode(out);
            let face = entry(&graph.faces, i - o[1])?;
            face.bounds.len().encode(out);
            for r in &face.bounds {
                if out.stop.is_some() {
                    break;
                }
                order.reference((o[2] + r.0 as usize) as u32, out)?;
            }
        } else if i < o[3] {
            2u8.encode(out);
            let bound = entry(&graph.bounds, i - o[2])?;
            bound.orientation.encode(out);
            bound.outer.encode(out);
            order.reference((o[3] + bound.polygon.0 as usize) as u32, out)?;
        } else if i < o[4] {
            3u8.encode(out);
            let polygon = entry(&graph.loops, i - o[3])?;
            polygon.points.len().encode(out);
            for r in &polygon.points {
                if out.stop.is_some() {
                    break;
                }
                order.reference((o[4] + r.0 as usize) as u32, out)?;
            }
        } else {
            4u8.encode(out);
            entry(&graph.points, i - o[4])?.encode(out);
        }
    }
    Ok(())
}

fn encode_definition(
    value: &GeometryDefinition,
    order: &ValueOrder,
    out: &mut Canon<'_>,
) -> Result<()> {
    match value {
        GeometryDefinition::Links => 0u8.encode(out),
        GeometryDefinition::Context {
            definition,
            length_unit,
        } => {
            1u8.encode(out);
            definition.encode(out);
            length_unit.encode(out);
        }
        GeometryDefinition::Curve(g) => {
            2u8.encode(out);
            encode_curve(g, order.curve()?, out)?;
        }
        GeometryDefinition::Extrusion {
            definition,
            boundary,
            length_unit,
        } => {
            3u8.encode(out);
            length_unit.encode(out);
            encode_profile(&definition.profile, order, out)?;
            definition.position.encode(out);
            definition.direction.encode(out);
            definition.depth.encode(out);
            encode_optional_curve(boundary, order, out)?;
        }
        GeometryDefinition::Revolution {
            definition,
            boundary,
            length_unit,
        } => {
            4u8.encode(out);
            length_unit.encode(out);
            encode_profile(&definition.profile, order, out)?;
            definition.position.encode(out);
            definition.axis.encode(out);
            definition.angle.encode(out);
            encode_optional_curve(boundary, order, out)?;
        }
        GeometryDefinition::SweptDisk {
            definition,
            directrix,
        } => {
            5u8.encode(out);
            let order = order.curve()?;
            order.reference(definition.directrix.0, out)?;
            definition.radius.encode(out);
            definition.inner_radius.encode(out);
            definition.start_parameter.encode(out);
            definition.end_parameter.encode(out);
            encode_curve(directrix, order, out)?;
        }
        GeometryDefinition::Faces {
            definition,
            length_unit,
        } => {
            6u8.encode(out);
            length_unit.encode(out);
            let ValueOrder::Faces(order) = order else {
                return Err(GeometryError::MissingDefinition);
            };
            encode_faces(definition, order, out)?;
        }
        GeometryDefinition::Tessellation {
            definition,
            length_unit,
        } => {
            7u8.encode(out);
            length_unit.encode(out);
            definition.points.encode(out);
            definition.tags.encode(out);
            definition.point_index.encode(out);
            definition.closed.encode(out);
            match &definition.faces {
                IndexedFaces::Triangles { indices, normals } => {
                    0u8.encode(out);
                    indices.encode(out);
                    normals.encode(out);
                }
                IndexedFaces::Polygons { indices, faces } => {
                    1u8.encode(out);
                    let ValueOrder::Polygons(order) = order else {
                        return Err(GeometryError::MissingDefinition);
                    };
                    indices.len().encode(out);
                    for r in indices {
                        if out.stop.is_some() {
                            break;
                        }
                        order.reference(r.0, out)?;
                    }
                    order.indices.len().encode(out);
                    for &at in &order.indices {
                        if out.stop.is_some() {
                            break;
                        }
                        entry(faces, at as usize)?.encode(out);
                    }
                }
            }
        }
        GeometryDefinition::HalfSpace {
            definition,
            boundary,
            length_unit,
        } => {
            8u8.encode(out);
            length_unit.encode(out);
            definition.plane.encode(out);
            definition.agreement.encode(out);
            match &definition.limit {
                HalfSpaceLimit::Unbounded => 0u8.encode(out),
                HalfSpaceLimit::Box {
                    corner,
                    dimensions,
                    coordinates,
                } => {
                    1u8.encode(out);
                    corner.encode(out);
                    dimensions.encode(out);
                    coordinates.encode(out);
                }
                HalfSpaceLimit::Polygonal { position, boundary } => {
                    2u8.encode(out);
                    position.encode(out);
                    order.curve()?.reference(boundary.0, out)?;
                }
            }
            encode_optional_curve(boundary, order, out)?;
        }
        GeometryDefinition::Mapping {
            definition,
            length_unit,
        } => {
            9u8.encode(out);
            definition.encode(out);
            length_unit.encode(out);
        }
        GeometryDefinition::Unsupported(kind) => {
            10u8.encode(out);
            kind.encode(out);
        }
    }
    Ok(())
}
