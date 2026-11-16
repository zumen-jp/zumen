//! 折れ線の区間を同じ輪郭でつなぐ円盤の掃引。折れ点は接線の二等分面で継ぐ。

use super::*;

pub(super) fn evaluate(
    path: &[Xyz],
    radius: f64,
    inner: Option<f64>,
    tol: Tolerance,
    changes: &mut MeshChanges,
    cx: &mut Cx,
) -> Result<Mesh, Failure> {
    if path.len() < 2
        || path.len() > MAX_FACE_POINTS
        || !radius.is_finite()
        || radius <= 0.0
        || inner.is_some_and(|r| !r.is_finite() || r <= 0.0 || r >= radius)
    {
        return Err(Failure::Invalid("swept_disk"));
    }
    let mut points = Vec::new();
    reserve(&mut points, path.len(), 24, cx)?;
    work(cx, path.len() as u64)?;
    for &p in path {
        if !is_finite(p) {
            return Err(Failure::Invalid("directrix"));
        }
        if points.last() != Some(&p) {
            points.push(p);
        }
    }
    if points.len() < 2 {
        return Err(Failure::Invalid("directrix"));
    }
    if points.first() == points.last() {
        return Err(Failure::Unsupported("closed_sweep"));
    }
    let mut tangents = Vec::new();
    reserve(&mut tangents, points.len() - 1, 24, cx)?;
    for pair in points.windows(2) {
        work(cx, 1)?;
        let [a, b] = pair else {
            return Err(Failure::Invalid("directrix"));
        };
        tangents.push(normalize(sub(*b, *a)).ok_or(Failure::Invalid("directrix"))?);
    }
    let mut rings = Vec::new();
    reserve(&mut rings, 2, 24, cx)?;
    rings.push(ring(
        &Curve::Circle {
            center: [0.0; 2],
            radius,
        },
        tol,
        true,
        changes,
        cx,
    )?);
    if let Some(radius) = inner {
        rings.push(ring(
            &Curve::Circle {
                center: [0.0; 2],
                radius,
            },
            tol,
            false,
            changes,
            cx,
        )?);
    }
    let n = rings.iter().map(Vec::len).sum::<usize>();
    if n.saturating_mul(points.len()) > MAX_POINTS {
        return Err(Failure::Invalid("point_count"));
    }
    let mut mesh = Mesh::default();
    reserve(&mut mesh.positions, n * points.len(), 24, cx)?;
    let tangent = *tangents.first().ok_or(Failure::Invalid("directrix"))?;
    let mut x = Transform::from_axes([0.0; 3], Some(tangent), None)
        .ok_or(Failure::Invalid("directrix"))?
        .column(0);
    for (i, &p) in points.iter().enumerate() {
        work(cx, n as u64)?;
        let incoming = tangents
            .get(i.saturating_sub(1))
            .copied()
            .ok_or(Failure::Invalid("directrix"))?;
        let outgoing = tangents.get(i).copied().unwrap_or(incoming);
        let normal =
            normalize(crate::add(incoming, outgoing)).ok_or(Failure::Invalid("sweep_corner"))?;
        let denom = dot(normal, incoming);
        if denom <= 1e-6 {
            return Err(Failure::Invalid("sweep_corner"));
        }
        let y = cross(incoming, x);
        for q in rings.iter().flatten() {
            let v = crate::add(scale(x, q[0]), scale(y, q[1]));
            let v = sub(v, scale(incoming, dot(v, normal) / denom));
            mesh.positions.push(crate::add(p, v));
        }
        // 最短の回転で基準方向を運び、区間ごとの軸の選び直しによるねじれを避ける。
        let v = cross(incoming, outgoing);
        let c = dot(incoming, outgoing);
        x = normalize(crate::add(
            x,
            crate::add(cross(v, x), scale(cross(v, cross(v, x)), 1.0 / (1.0 + c))),
        ))
        .ok_or(Failure::Invalid("sweep_frame"))?;
    }
    let n = n as u32;
    let last = (points.len() as u32 - 1) * n;
    for t in triangulate(&rings, changes, cx)? {
        triangle(&mut mesh, [t[0], t[2], t[1]], cx)?;
        triangle(&mut mesh, t.map(|i| i + last), cx)?;
    }
    let mut base = 0;
    for ring in rings {
        let len = ring.len() as u32;
        for j in 0..points.len() as u32 - 1 {
            for i in 0..len {
                work(cx, 1)?;
                let (a, b) = (j * n + base + i, j * n + base + (i + 1) % len);
                triangle(&mut mesh, [a, b, b + n], cx)?;
                triangle(&mut mesh, [a, b + n, a + n], cx)?;
            }
        }
        base += len;
    }
    Ok(mesh)
}
