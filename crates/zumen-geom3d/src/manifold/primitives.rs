//! 押し出し・回転・面集合の三角形分割。試作の手順に確保と繰り返しの予算を加えた。

use std::collections::BTreeSet;

mod sweep;

use zumen_base::{Cx, Kind};

use super::{
    MeshChanges,
    budget::{self, MAX_FACE_POINTS, MAX_POINTS, push, reserve, triangle, work},
};
use crate::{
    Curve, Face, Failure, Mesh, Plane, Profile, Shape, Tolerance, Transform, Xy, Xyz, cross, dot,
    is_finite, length, normalize, scale, sub,
};

fn area(points: &[Xy]) -> f64 {
    let origin = points.first().copied().unwrap_or_default();
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| {
            (a[0] - origin[0]) * (b[1] - origin[1]) - (b[0] - origin[0]) * (a[1] - origin[1])
        })
        .sum::<f64>()
        / 2.0
}

fn steps(radius: f64, angle: f64, chord: f64, cx: &mut Cx) -> Result<u32, Failure> {
    if !(radius.is_finite() && radius > 0.0 && angle.is_finite()) {
        return Err(Failure::Invalid("radius"));
    }
    // asin の形は chord / radius が小さい場合も 1 からの減算で桁を失わない。
    let step = 4.0 * libm::asin(libm::sqrt((chord / (2.0 * radius)).min(0.5)));
    let n = libm::ceil(angle.abs() / step).max(3.0);
    if !n.is_finite() || n > MAX_FACE_POINTS as f64 {
        return Err(Failure::Invalid("curve_segments"));
    }
    cx.check(Kind::Segments, n as u64)?;
    Ok(n as u32)
}

fn ring(
    curve: &Curve,
    tol: Tolerance,
    outer: bool,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Vec<Xy>, Failure> {
    let mut points = Vec::new();
    match curve {
        Curve::Polygon(input) => {
            if input.len() > MAX_FACE_POINTS {
                return Err(Failure::Invalid("face_points"));
            }
            work(cx, input.len() as u64)?;
            reserve(&mut points, input.len(), 16, cx)?;
            for &p in input {
                if !p.iter().all(|v| v.is_finite()) {
                    return Err(Failure::Invalid("profile"));
                }
                if points.last() != Some(&p) {
                    points.push(p);
                }
            }
            if points.first() == points.last() {
                points.pop();
            }
        }
        Curve::Circle { center, radius } => {
            if !center.iter().all(|v| v.is_finite()) {
                return Err(Failure::Invalid("center"));
            }
            let count = steps(*radius, core::f64::consts::TAU, tol.chord, cx)?;
            changes.approximated_curves += 1;
            reserve(&mut points, count as usize, 16, cx)?;
            work(cx, u64::from(count))?;
            for i in 0..count {
                let angle = core::f64::consts::TAU * f64::from(i) / f64::from(count);
                points.push([
                    center[0] + radius * libm::cos(angle),
                    center[1] + radius * libm::sin(angle),
                ]);
            }
        }
    }
    let area = area(&points);
    if points.len() < 3 || !area.is_finite() || area == 0.0 {
        return Err(Failure::Invalid("profile"));
    }
    if (area > 0.0) != outer {
        points.reverse();
    }
    Ok(points)
}

fn rings(
    profile: &Profile,
    tol: Tolerance,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Vec<Vec<Xy>>, Failure> {
    if profile.holes.len() > MAX_FACE_POINTS {
        return Err(Failure::Invalid("holes"));
    }
    let mut rings = Vec::new();
    reserve(&mut rings, profile.holes.len() + 1, 24, cx)?;
    rings.push(ring(&profile.outer, tol, true, changes, cx)?);
    for hole in &profile.holes {
        rings.push(ring(hole, tol, false, changes, cx)?);
    }
    Ok(rings)
}

fn inside(p: Xy, a: Xy, b: Xy) -> bool {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let length2 = ab[0] * ab[0] + ab[1] * ab[1];
    if !length2.is_finite() || length2 <= 0.0 {
        return false;
    }
    let t = (ab[0] * ap[0] + ab[1] * ap[1]) / length2;
    let cross = ab[0] * ap[1] - ab[1] * ap[0];
    t > 0.0 && t < 1.0 && cross * cross <= 1e-18 * length2 * length2
}

/// earcut が落とす直線上の頂点を、片側だけの辺へ戻して T 字の継ぎ目を防ぐ。
fn triangulate(
    rings: &[Vec<Xy>],
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Vec<[u32; 3]>, Failure> {
    let n = rings
        .iter()
        .try_fold(0_usize, |n, r| n.checked_add(r.len()))
        .ok_or(Failure::Invalid("size"))?;
    if n > MAX_FACE_POINTS {
        return Err(Failure::Invalid("face_points"));
    }
    // 依存内では予算を数えられないため、入力を制限し、二乗の走査と作業領域を先に予約する。
    work(cx, (n as u64).saturating_mul(n as u64).saturating_mul(8))?;
    budget::bytes(cx, n as u64 * 512)?;
    let mut flat: Vec<f64> = Vec::new();
    let mut points = Vec::new();
    let mut holes = Vec::new();
    reserve(&mut flat, 2 * n, 8, cx)?;
    reserve(&mut points, n, 16, cx)?;
    reserve(&mut holes, rings.len(), 8, cx)?;
    for (i, ring) in rings.iter().enumerate() {
        if i > 0 {
            holes.push(points.len());
        }
        flat.extend(ring.iter().flatten());
        points.extend_from_slice(ring);
    }
    let indices = earcutr::earcut(&flat, &holes, 2).map_err(|_| Failure::Invalid("polygon"))?;
    work(cx, 0)?;
    let mut triangles = Vec::new();
    reserve(&mut triangles, indices.len() / 3, 12, cx)?;
    let mut edges = BTreeSet::new();
    budget::bytes(cx, indices.len() as u64 * 64)?;
    for t in indices.as_chunks::<3>().0 {
        if t.iter().any(|&i| i >= n) {
            return Err(Failure::Invalid("triangulation"));
        }
        let t = t.map(|i| i as u32);
        triangles.push(t);
        for edge in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            edges.insert(edge);
        }
    }
    for (k, &p) in points.iter().enumerate() {
        let k = k as u32;
        let mut at = 0;
        while let Some(&t) = triangles.get(at) {
            work(cx, 3)?;
            let mut split = None;
            if !t.contains(&k) {
                for (a, b, c) in [(t[0], t[1], t[2]), (t[1], t[2], t[0]), (t[2], t[0], t[1])] {
                    let pa = points
                        .get(a as usize)
                        .copied()
                        .ok_or(Failure::Invalid("index"))?;
                    let pb = points
                        .get(b as usize)
                        .copied()
                        .ok_or(Failure::Invalid("index"))?;
                    if !edges.contains(&(b, a)) && inside(p, pa, pb) {
                        split = Some((a, b, c));
                        break;
                    }
                }
            }
            if let Some((a, b, c)) = split {
                if triangles.len() >= 4 * MAX_FACE_POINTS {
                    return Err(Failure::Invalid("triangulation_size"));
                }
                for edge in [(a, b), (b, c), (c, a)] {
                    edges.remove(&edge);
                }
                if let Some(t) = triangles.get_mut(at) {
                    *t = [a, k, c];
                }
                push(&mut triangles, [k, b, c], 12, cx)?;
                budget::bytes(cx, 6 * 64)?;
                for edge in [(a, k), (k, c), (c, a), (k, b), (b, c), (c, k)] {
                    edges.insert(edge);
                }
                changes.split_edges += 1;
            }
            at += 1;
        }
    }
    Ok(triangles)
}

pub(super) fn prism(
    rings: &[Vec<Xy>],
    direction: Xyz,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Mesh, Failure> {
    if !is_finite(direction) || direction[2] == 0.0 {
        return Err(Failure::Invalid("direction"));
    }
    let caps = triangulate(rings, changes, cx)?;
    let n = rings.iter().map(Vec::len).sum::<usize>() as u32;
    let mut mesh = Mesh::default();
    reserve(&mut mesh.positions, 2 * n as usize, 24, cx)?;
    work(cx, u64::from(n) * 2)?;
    for ring in rings {
        mesh.positions
            .extend(ring.iter().map(|p| [p[0], p[1], 0.0]));
    }
    for ring in rings {
        mesh.positions.extend(
            ring.iter()
                .map(|p| [p[0] + direction[0], p[1] + direction[1], direction[2]]),
        );
    }
    for t in caps {
        triangle(&mut mesh, [t[0], t[2], t[1]], cx)?;
        triangle(&mut mesh, t.map(|i| i + n), cx)?;
    }
    let mut base = 0;
    for ring in rings {
        let len = ring.len() as u32;
        for i in 0..len {
            work(cx, 1)?;
            let (a, b) = (base + i, base + (i + 1) % len);
            triangle(&mut mesh, [a, b, b + n], cx)?;
            triangle(&mut mesh, [a, b + n, a + n], cx)?;
        }
        base += len;
    }
    if direction[2] < 0.0 {
        for t in &mut mesh.triangles {
            t.swap(1, 2);
        }
    }
    Ok(mesh)
}

fn revolve(
    rings: &[Vec<Xy>],
    origin: Xyz,
    axis: Xyz,
    angle: f64,
    tol: Tolerance,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Mesh, Failure> {
    let axis = normalize(axis).ok_or(Failure::Invalid("axis"))?;
    if !is_finite(origin)
        || !(angle.is_finite() && angle != 0.0 && angle.abs() <= 360.0)
        || axis[2] != 0.0
    {
        return Err(Failure::Invalid("angle"));
    }
    let full = angle.abs() == 360.0;
    let n = rings.iter().map(Vec::len).sum::<usize>();
    work(cx, n as u64)?;
    let radius = rings
        .iter()
        .flatten()
        .map(|p| length(cross(sub([p[0], p[1], 0.0], origin), axis)))
        .fold(0.0, f64::max);
    let sweep = angle.to_radians();
    let steps = steps(radius, sweep, tol.chord, cx)?;
    changes.approximated_curves += 1;
    let columns = if full { steps } else { steps + 1 };
    if n.saturating_mul(columns as usize) > MAX_POINTS {
        return Err(Failure::Invalid("point_count"));
    }
    let n = n as u32;
    let mut mesh = Mesh::default();
    reserve(&mut mesh.positions, (n * columns) as usize, 24, cx)?;
    for j in 0..columns {
        work(cx, u64::from(n))?;
        let theta = sweep * f64::from(j) / f64::from(steps);
        let (s, c) = (libm::sin(theta), libm::cos(theta));
        for p in rings.iter().flatten() {
            let v = sub([p[0], p[1], 0.0], origin);
            let kv = cross(axis, v);
            let kd = dot(axis, v);
            mesh.positions.push(crate::add(
                origin,
                crate::add(
                    scale(v, c),
                    crate::add(scale(kv, s), scale(axis, kd * (1.0 - c))),
                ),
            ));
        }
    }
    let mut base = 0;
    for ring in rings {
        let len = ring.len() as u32;
        for j in 0..steps {
            let (c0, c1) = (j * n, ((j + 1) % columns) * n);
            for i in 0..len {
                work(cx, 1)?;
                let (a, b) = (base + i, base + (i + 1) % len);
                triangle(&mut mesh, [c0 + a, c1 + a, c1 + b], cx)?;
                triangle(&mut mesh, [c0 + a, c1 + b, c0 + b], cx)?;
            }
        }
        base += len;
    }
    if !full {
        let last = (columns - 1) * n;
        for t in triangulate(rings, changes, cx)? {
            triangle(&mut mesh, t, cx)?;
            triangle(&mut mesh, [t[0] + last, t[2] + last, t[1] + last], cx)?;
        }
    }
    if mesh.volume() < 0.0 {
        for t in &mut mesh.triangles {
            t.swap(1, 2);
        }
    }
    Ok(mesh)
}

fn faces(faces: &[Face], changes: &mut MeshChanges, cx: &mut Cx) -> Result<Mesh, Failure> {
    let mut mesh = Mesh::default();
    for face in faces {
        work(cx, face.outer.len() as u64 + 1)?;
        if face.outer.len() > MAX_FACE_POINTS || face.holes.len() > MAX_FACE_POINTS {
            return Err(Failure::Invalid("face_points"));
        }
        let origin = face.outer.first().copied().unwrap_or_default();
        let mut normal = [0.0; 3];
        for (a, b) in face.outer.iter().zip(face.outer.iter().cycle().skip(1)) {
            if !is_finite(*a) {
                return Err(Failure::Invalid("position"));
            }
            let c = cross(sub(*a, origin), sub(*b, origin));
            normal = crate::add(normal, c);
        }
        let Some(frame) = Transform::from_axes(origin, Some(normal), None) else {
            changes.dropped_faces += 1;
            continue;
        };
        let mut flat = Vec::new();
        let base = mesh.positions.len() as u32;
        reserve(&mut flat, face.holes.len() + 1, 24, cx)?;
        for ring in std::iter::once(&face.outer).chain(&face.holes) {
            if ring.len() > MAX_FACE_POINTS
                || mesh.positions.len().saturating_add(ring.len()) > MAX_POINTS
            {
                return Err(Failure::Invalid("point_count"));
            }
            work(cx, ring.len() as u64)?;
            let mut points = Vec::new();
            reserve(&mut points, ring.len(), 16, cx)?;
            reserve(&mut mesh.positions, ring.len(), 24, cx)?;
            for &p in ring {
                if !is_finite(p) {
                    return Err(Failure::Invalid("position"));
                }
                points.push(Plane::project(&frame, p));
                mesh.positions.push(p);
            }
            flat.push(points);
        }
        for t in triangulate(&flat, changes, cx)? {
            triangle(&mut mesh, t.map(|i| i + base), cx)?;
        }
    }
    Ok(mesh)
}

pub(super) fn transform(
    mesh: &mut Mesh,
    transform: &Transform,
    cx: &mut Cx,
) -> Result<(), Failure> {
    if !transform.is_finite()
        || !transform.determinant().is_finite()
        || transform.determinant() == 0.0
    {
        return Err(Failure::Invalid("transform"));
    }
    work(cx, (mesh.positions.len() + mesh.triangles.len()) as u64)?;
    for p in &mut mesh.positions {
        *p = transform.point(*p);
    }
    if transform.determinant() < 0.0 {
        for t in &mut mesh.triangles {
            t.swap(1, 2);
        }
    }
    Ok(())
}

pub(super) fn evaluate(
    shape: &Shape,
    tol: Tolerance,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Mesh, Failure> {
    match shape {
        Shape::Extrusion {
            profile,
            position,
            direction,
            depth,
        } => {
            if !(depth.is_finite() && *depth > 0.0) {
                return Err(Failure::Invalid("depth"));
            }
            let d = normalize(*direction).ok_or(Failure::Invalid("direction"))?;
            let mut mesh = prism(
                &rings(profile, tol, changes, cx)?,
                scale(d, *depth),
                changes,
                cx,
            )?;
            transform(&mut mesh, position, cx)?;
            Ok(mesh)
        }
        Shape::Revolution {
            profile,
            position,
            axis_origin,
            axis_direction,
            angle,
        } => {
            let mut mesh = revolve(
                &rings(profile, tol, changes, cx)?,
                *axis_origin,
                *axis_direction,
                *angle,
                tol,
                changes,
                cx,
            )?;
            transform(&mut mesh, position, cx)?;
            Ok(mesh)
        }
        Shape::SweptDisk {
            directrix,
            radius,
            inner_radius,
        } => sweep::evaluate(directrix, *radius, *inner_radius, tol, changes, cx),
        Shape::FaceSet { faces: input, .. } => faces(input, changes, cx),
        Shape::Triangles {
            positions,
            triangles,
            ..
        } => {
            if positions.len() > MAX_POINTS || triangles.len() > budget::MAX_TRIANGLES {
                return Err(Failure::Invalid("mesh_count"));
            }
            work(cx, (positions.len() + triangles.len()) as u64)?;
            cx.check(Kind::Segments, triangles.len() as u64)?;
            let mut mesh = Mesh::default();
            reserve(&mut mesh.positions, positions.len(), 24, cx)?;
            reserve(&mut mesh.triangles, triangles.len(), 12, cx)?;
            mesh.positions.extend_from_slice(positions);
            mesh.triangles.extend_from_slice(triangles);
            Ok(mesh)
        }
        other => Err(Failure::Unsupported(other.kind())),
    }
}
