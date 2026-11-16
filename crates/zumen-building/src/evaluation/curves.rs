//! 所有する曲線を評価用の折れ線へ写す。媒介変数の単位を推測しない。

use super::values::{coordinates, reserve, unit};
use crate::{
    CurveGraph, CurveKind, CurvePosition, CurveRef, CurveTrim, IndexedSegment, TrimmingPreference,
};
use zumen_base::{Cx, Kind};
use zumen_geom3d::{
    Failure, MeshChanges, Plane, Transform, Xyz, add, cross, dot, length, normalize, scale, sub,
};

pub(super) fn frame(position: &CurvePosition, factor: f64) -> Result<Transform, Failure> {
    Transform::from_axes(
        scale(coordinates(position.location), factor),
        position.axis,
        position.reference_direction.map(coordinates),
    )
    .ok_or(Failure::Invalid("curve_axis"))
}

pub(super) fn arc(
    frame: Transform,
    radius: f64,
    start: f64,
    sweep: f64,
    chord: f64,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Vec<Xyz>, Failure> {
    if !(radius.is_finite()
        && radius > 0.0
        && start.is_finite()
        && sweep.is_finite()
        && sweep.abs() <= core::f64::consts::TAU
        && chord.is_finite()
        && chord > 0.0)
    {
        return Err(Failure::Invalid("arc"));
    }
    let step = 4.0 * libm::asin(libm::sqrt((chord / (2.0 * radius)).min(0.5)));
    let n = libm::ceil(sweep.abs() / step).max(1.0);
    if !n.is_finite() || n > 4096.0 {
        return Err(Failure::Invalid("curve_segments"));
    }
    let n = n as u32;
    changes.approximated_curves += 1;
    cx.check(Kind::Work, u64::from(n) + 1)?;
    cx.check(Kind::Segments, u64::from(n) + 1)?;
    let mut points = Vec::new();
    reserve(&mut points, n as usize + 1, 24, cx)?;
    for i in 0..=n {
        let a = start + sweep * f64::from(i) / f64::from(n);
        points.push(frame.point([radius * libm::cos(a), radius * libm::sin(a), 0.0]));
    }
    if sweep.abs() == core::f64::consts::TAU {
        let first = points.first().copied();
        if let (Some(first), Some(last)) = (first, points.last_mut()) {
            *last = first;
        }
    }
    Ok(points)
}

fn through(
    a: Xyz,
    b: Xyz,
    c: Xyz,
    chord: f64,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Vec<Xyz>, Failure> {
    let u = sub(b, a);
    let v = sub(c, a);
    let normal = cross(u, v);
    let denom = 2.0 * dot(normal, normal);
    if !denom.is_finite() || denom <= 0.0 {
        return Err(Failure::Invalid("arc_points"));
    }
    let offset = scale(
        add(
            scale(cross(v, normal), dot(u, u)),
            scale(cross(normal, u), dot(v, v)),
        ),
        1.0 / denom,
    );
    let center = add(a, offset);
    let radius = length(offset);
    let frame = Transform::from_axes(center, Some(normal), Some(sub(a, center)))
        .ok_or(Failure::Invalid("arc_axis"))?;
    let p = Plane::project(&frame, c);
    let sweep = libm::atan2(p[1], p[0]).rem_euclid(core::f64::consts::TAU);
    let mut points = arc(frame, radius, 0.0, sweep, chord, changes, cx)?;
    if let Some(p) = points.first_mut() {
        *p = a;
    }
    if let Some(p) = points.last_mut() {
        *p = c;
    }
    Ok(points)
}

fn append(out: &mut Vec<Xyz>, part: &[Xyz], cx: &mut Cx) -> Result<(), Failure> {
    cx.check(Kind::Work, part.len() as u64)?;
    reserve(out, part.len(), 24, cx)?;
    for &p in part {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    Ok(())
}

fn trim_value(
    values: &[CurveTrim],
    preference: TrimmingPreference,
    factor: f64,
    cx: &mut Cx,
) -> Result<(Option<Xyz>, Option<f64>), Failure> {
    cx.check(Kind::Work, values.len() as u64)?;
    let mut point = None;
    let mut parameter = None;
    for value in values {
        match *value {
            CurveTrim::Point(p) => point = Some(scale(coordinates(p), factor)),
            CurveTrim::Parameter(t) => parameter = Some(t),
        }
    }
    if preference == TrimmingPreference::Parameter && parameter.is_some() {
        point = None;
    }
    Ok((point, parameter))
}

fn trimmed(
    graph: &CurveGraph,
    basis: CurveRef,
    trims: [&[CurveTrim]; 2],
    selection: (bool, TrimmingPreference),
    chord: f64,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Vec<Xyz>, Failure> {
    let (sense, preference) = selection;
    let factor = unit(graph.length_unit.as_ref(), "LENGTHUNIT")?;
    let ((p1, t1), (p2, t2)) = (
        trim_value(trims[0], preference, factor, cx)?,
        trim_value(trims[1], preference, factor, cx)?,
    );
    let kind = &graph
        .item(basis)
        .ok_or(Failure::Invalid("curve_reference"))?
        .kind;
    match kind {
        CurveKind::Line {
            point,
            direction,
            magnitude,
        } => {
            let origin = scale(coordinates(*point), factor);
            let d = normalize(coordinates(*direction)).ok_or(Failure::Invalid("line_direction"))?;
            let at = |point: Option<Xyz>, parameter: Option<f64>| -> Result<Xyz, Failure> {
                if let Some(p) = point {
                    return Ok(p);
                }
                let t = parameter.ok_or(Failure::Invalid("trim"))?;
                Ok(add(origin, scale(d, t * magnitude * factor)))
            };
            let (a, b) = (at(p1, t1)?, at(p2, t2)?);
            let mut out = Vec::new();
            reserve(&mut out, 2, 24, cx)?;
            out.extend(if sense { [a, b] } else { [b, a] });
            Ok(out)
        }
        CurveKind::Circle { position, radius } => {
            let frame = frame(position, factor)?;
            let angle = |point: Option<Xyz>, parameter: Option<f64>| -> Result<f64, Failure> {
                if let Some(p) = point {
                    let p = Plane::project(&frame, p);
                    return Ok(libm::atan2(p[1], p[0]));
                }
                Ok(parameter.ok_or(Failure::Invalid("trim"))?
                    * unit(graph.angle_unit.as_ref(), "PLANEANGLEUNIT")?)
            };
            let (a, b) = (angle(p1, t1)?, angle(p2, t2)?);
            let mut sweep = if sense {
                (b - a).rem_euclid(core::f64::consts::TAU)
            } else {
                -(a - b).rem_euclid(core::f64::consts::TAU)
            };
            if sweep == 0.0 {
                sweep = if sense {
                    core::f64::consts::TAU
                } else {
                    -core::f64::consts::TAU
                };
            }
            let mut out = arc(frame, radius * factor, a, sweep, chord, changes, cx)?;
            if let (Some(p), Some(first)) = (p1, out.first_mut()) {
                *first = p;
            }
            if let (Some(p), Some(last)) = (p2, out.last_mut()) {
                *last = p;
            }
            Ok(out)
        }
        _ => Err(Failure::Unsupported("trimmed_curve")),
    }
}

pub(super) fn evaluate(
    graph: &CurveGraph,
    reference: CurveRef,
    chord: f64,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Vec<Xyz>, Failure> {
    cx.enter()?;
    let result = (|| {
        cx.check(Kind::Work, 1)?;
        let factor = unit(graph.length_unit.as_ref(), "LENGTHUNIT")?;
        let kind = &graph
            .item(reference)
            .ok_or(Failure::Invalid("curve_reference"))?
            .kind;
        let mut out = Vec::new();
        match kind {
            CurveKind::Polyline(points)
            | CurveKind::Indexed {
                points,
                segments: None,
                ..
            } => {
                cx.check(Kind::Work, points.len() as u64)?;
                reserve(&mut out, points.len(), 24, cx)?;
                out.extend(points.iter().map(|p| scale(coordinates(*p), factor)));
            }
            CurveKind::Circle { position, radius } => {
                out = arc(
                    frame(position, factor)?,
                    radius * factor,
                    0.0,
                    core::f64::consts::TAU,
                    chord,
                    changes,
                    cx,
                )?
            }
            CurveKind::Indexed {
                points,
                segments: Some(segments),
                ..
            } => {
                let at = |i: u32| {
                    points
                        .get(i.checked_sub(1).ok_or(Failure::Invalid("curve_index"))? as usize)
                        .map(|p| scale(coordinates(*p), factor))
                        .ok_or(Failure::Invalid("curve_index"))
                };
                for segment in segments {
                    cx.check(Kind::Work, 1)?;
                    match segment {
                        IndexedSegment::Line(indices) => {
                            reserve(&mut out, indices.len(), 24, cx)?;
                            for &i in indices {
                                cx.check(Kind::Work, 1)?;
                                let p = at(i)?;
                                if out.last() != Some(&p) {
                                    out.push(p);
                                }
                            }
                        }
                        IndexedSegment::Arc([a, b, c]) => append(
                            &mut out,
                            &through(at(*a)?, at(*b)?, at(*c)?, chord, changes, cx)?,
                            cx,
                        )?,
                    }
                }
            }
            CurveKind::Composite { segments, .. } => {
                for segment in segments {
                    let mut points = evaluate(graph, segment.curve, chord, changes, cx)?;
                    if !segment.same_sense {
                        points.reverse();
                    }
                    append(&mut out, &points, cx)?;
                }
            }
            CurveKind::Trimmed {
                basis,
                trim1,
                trim2,
                sense_agreement,
                preference,
            } => {
                out = trimmed(
                    graph,
                    *basis,
                    [trim1, trim2],
                    (*sense_agreement, *preference),
                    chord,
                    changes,
                    cx,
                )?
            }
            _ => return Err(Failure::Unsupported("curve")),
        }
        if !out.iter().flatten().all(|v| v.is_finite()) {
            return Err(Failure::Invalid("curve_position"));
        }
        Ok(out)
    })();
    cx.leave();
    result
}

/// buildingSMART 公開資料の IfcPolyline の媒介変数は頂点ごとに 0, 1, ...。
/// https://standards.buildingsmart.org/IFC/RELEASE/IFC2x/ADD1/HTML/ifcgeometryresource/lexical/ifcpolyline.html
pub(super) fn interval(
    graph: &CurveGraph,
    reference: CurveRef,
    ends: [Option<f64>; 2],
    chord: f64,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Vec<Xyz>, Failure> {
    if ends == [None, None] {
        return evaluate(graph, reference, chord, changes, cx);
    }
    let kind = &graph
        .item(reference)
        .ok_or(Failure::Invalid("curve_reference"))?
        .kind;
    let CurveKind::Polyline(points) = kind else {
        return Err(Failure::Unsupported("sweep_interval"));
    };
    if points.len() < 2 {
        return Err(Failure::Invalid("directrix"));
    }
    let last = (points.len() - 1) as f64;
    let (start, end) = (ends[0].unwrap_or(0.0), ends[1].unwrap_or(last));
    if !(start.is_finite() && end.is_finite() && 0.0 <= start && start < end && end <= last) {
        return Err(Failure::Invalid("sweep_interval"));
    }
    let factor = unit(graph.length_unit.as_ref(), "LENGTHUNIT")?;
    let at = |t: f64| -> Result<Xyz, Failure> {
        let i = (libm::floor(t) as usize).min(points.len() - 2);
        let a = coordinates(*points.get(i).ok_or(Failure::Invalid("curve_index"))?);
        let b = coordinates(*points.get(i + 1).ok_or(Failure::Invalid("curve_index"))?);
        let p = scale(add(a, scale(sub(b, a), t - i as f64)), factor);
        if !p.iter().all(|v| v.is_finite()) {
            return Err(Failure::Invalid("directrix"));
        }
        Ok(p)
    };
    let first_inside = libm::floor(start) as usize + 1;
    let after_inside = libm::ceil(end) as usize;
    let count = after_inside - first_inside + 2;
    cx.check(Kind::Work, count as u64)?;
    let mut out = Vec::new();
    reserve(&mut out, count, 24, cx)?;
    out.push(at(start)?);
    for i in first_inside..after_inside {
        out.push(at(i as f64)?);
    }
    out.push(at(end)?);
    Ok(out)
}
