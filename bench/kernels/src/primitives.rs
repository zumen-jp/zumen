//! ブーリアンのない形を、自前で三角形にする。どの候補も、ブーリアンの前の形はこれで作る

use zumen_geom3d::{
    Curve, Face, Failure, Mesh, Profile, Shape, Tolerance, Transform, Xy, Xyz, cross, dot,
    normalize, sub,
};

fn signed_area(ring: &[Xy]) -> f64 {
    let next = ring.iter().cycle().skip(1);
    ring.iter()
        .zip(next)
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        / 2.0
}

/// 輪を折れ線にし、向きを揃える（外は反時計回り、穴は時計回り）
fn ring(curve: &Curve, tolerance: Tolerance, outer: bool) -> Result<Vec<Xy>, Failure> {
    let mut points = match curve {
        Curve::Polygon(points) => points.clone(),
        Curve::Circle { center, radius } => {
            if !(radius.is_finite() && *radius > 0.0) {
                return Err(Failure::Invalid("radius"));
            }
            let mut out = Vec::new();
            zumen_geom2d::arc(*center, *radius, 0.0, 360.0, tolerance.chord, &mut out);
            out.pop();
            out
        }
    };
    if points.len() < 3 || !points.iter().flatten().all(|v| v.is_finite()) {
        return Err(Failure::Invalid("profile"));
    }
    let area = signed_area(&points);
    if area == 0.0 {
        return Err(Failure::Invalid("profile"));
    }
    if (area > 0.0) != outer {
        points.reverse();
    }
    Ok(points)
}

/// 断面の輪（外が先）
pub fn rings(profile: &Profile, tolerance: Tolerance) -> Result<Vec<Vec<Xy>>, Failure> {
    let mut out = vec![ring(&profile.outer, tolerance, true)?];
    for hole in &profile.holes {
        out.push(ring(hole, tolerance, false)?);
    }
    Ok(out)
}

/// 平面の輪（外が先）を三角形にする。番号は輪を順に並べた点の番号
fn triangulate(rings: &[Vec<Xy>]) -> Result<Vec<[u32; 3]>, Failure> {
    let mut flat: Vec<f64> = Vec::new();
    let mut holes = Vec::new();
    for (i, r) in rings.iter().enumerate() {
        if i > 0 {
            holes.push(flat.len() / 2);
        }
        flat.extend(r.iter().flatten());
    }
    let index = earcutr::earcut(&flat, &holes, 2).map_err(|_| Failure::Failed("earcut".into()))?;
    Ok(index
        .chunks_exact(3)
        .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
        .collect())
}

/// 断面の輪を、局所の向き `direction` に押し出したメッシュ（局所の座標）
pub fn prism(rings: &[Vec<Xy>], direction: Xyz) -> Result<Mesh, Failure> {
    if !(direction[2].is_finite() && direction[2] != 0.0 && direction.iter().all(|v| v.is_finite()))
    {
        return Err(Failure::Invalid("direction"));
    }
    let caps = triangulate(rings)?;
    let n: u32 = rings.iter().map(|r| r.len() as u32).sum();
    let mut mesh = Mesh::default();
    for r in rings {
        mesh.positions.extend(r.iter().map(|p| [p[0], p[1], 0.0]));
    }
    for r in rings {
        mesh.positions.extend(
            r.iter()
                .map(|p| [p[0] + direction[0], p[1] + direction[1], direction[2]]),
        );
    }
    for t in &caps {
        mesh.triangles.push([t[0], t[2], t[1]]);
        mesh.triangles.push([t[0] + n, t[1] + n, t[2] + n]);
    }
    let mut base = 0u32;
    for r in rings {
        let len = r.len() as u32;
        for i in 0..len {
            let (a, b) = (base + i, base + (i + 1) % len);
            mesh.triangles.push([a, b, b + n]);
            mesh.triangles.push([a, b + n, a + n]);
            mesh.edges.push([a, b]);
            mesh.edges.push([a + n, b + n]);
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

/// 断面を回す（局所の座標）。軸は XY 平面の上
fn revolve(
    rings: &[Vec<Xy>],
    origin: Xyz,
    axis: Xyz,
    angle: f64,
    tolerance: Tolerance,
) -> Result<Mesh, Failure> {
    let axis = normalize(axis).ok_or(Failure::Invalid("axis"))?;
    if !(angle.is_finite() && angle != 0.0 && angle.abs() <= 360.0) || axis[2] != 0.0 {
        return Err(Failure::Invalid("angle"));
    }
    let full = angle.abs() == 360.0;
    let radius = rings
        .iter()
        .flatten()
        .map(|p| zumen_geom3d::length(cross(sub([p[0], p[1], 0.0], origin), axis)))
        .fold(0.0, f64::max);
    // 弦の誤差で分ける数
    let step = if tolerance.chord < radius {
        2.0 * libm::acos(1.0 - tolerance.chord / radius)
    } else {
        core::f64::consts::FRAC_PI_2
    };
    let sweep = angle.to_radians();
    let steps = (libm::ceil(sweep.abs() / step) as u32).clamp(if full { 3 } else { 1 }, 1 << 12);
    let rotate = |p: Xyz, theta: f64| {
        // ロドリゲスの回転
        let v = sub(p, origin);
        let (s, c) = (libm::sin(theta), libm::cos(theta));
        let k = axis;
        let kv = cross(k, v);
        let kd = dot(k, v);
        [
            origin[0] + v[0] * c + kv[0] * s + k[0] * kd * (1.0 - c),
            origin[1] + v[1] * c + kv[1] * s + k[1] * kd * (1.0 - c),
            origin[2] + v[2] * c + kv[2] * s + k[2] * kd * (1.0 - c),
        ]
    };
    let n: u32 = rings.iter().map(|r| r.len() as u32).sum();
    let columns = if full { steps } else { steps + 1 };
    let mut mesh = Mesh::default();
    for j in 0..columns {
        let theta = sweep * f64::from(j) / f64::from(steps);
        for r in rings {
            mesh.positions
                .extend(r.iter().map(|p| rotate([p[0], p[1], 0.0], theta)));
        }
    }
    let mut base = 0u32;
    for r in rings {
        let len = r.len() as u32;
        for j in 0..steps {
            let (c0, c1) = (j * n, ((j + 1) % columns) * n);
            for i in 0..len {
                let (a, b) = (base + i, base + (i + 1) % len);
                // 断面の外の輪を Z の正から見て反時計回りにし、軸の向きに右ねじに回すと、外向きになる
                mesh.triangles.push([c0 + a, c1 + a, c1 + b]);
                mesh.triangles.push([c0 + a, c1 + b, c0 + b]);
            }
        }
        base += len;
    }
    if !full {
        let caps = triangulate(rings)?;
        let last = (columns - 1) * n;
        for t in &caps {
            mesh.triangles.push([t[0], t[1], t[2]]);
            mesh.triangles.push([t[0] + last, t[2] + last, t[1] + last]);
        }
    }
    // 向きは、体積の符号で揃える
    if mesh.volume() < 0.0 {
        for t in &mut mesh.triangles {
            t.swap(1, 2);
        }
    }
    Ok(mesh)
}

/// 面を三角形にする。面の平面に写してから分ける
fn faces(faces: &[Face]) -> Result<Mesh, Failure> {
    let mut mesh = Mesh::default();
    for face in faces {
        let all: Vec<&Vec<Xyz>> = std::iter::once(&face.outer)
            .chain(face.holes.iter())
            .collect();
        // ニューエルの方法で法線を求める
        let mut normal = [0.0; 3];
        for (a, b) in face.outer.iter().zip(face.outer.iter().cycle().skip(1)) {
            normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
            normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
            normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        let frame = Transform::from_axes(
            face.outer.first().copied().unwrap_or_default(),
            Some(normal),
            None,
        )
        .ok_or(Failure::Invalid("face"))?;
        let flat: Vec<Vec<Xy>> = all
            .iter()
            .map(|r| {
                r.iter()
                    .map(|p| zumen_geom3d::Plane::project(&frame, *p))
                    .collect()
            })
            .collect();
        let base = mesh.positions.len() as u32;
        for r in &all {
            mesh.positions.extend(r.iter().copied());
        }
        for t in triangulate(&flat)? {
            mesh.triangles.push(t.map(|i| i + base));
        }
    }
    Ok(mesh)
}

/// ブーリアン・半空間のない形を三角形にする。ほかの形は `None`
pub fn evaluate(shape: &Shape, tolerance: Tolerance) -> Option<Result<Mesh, Failure>> {
    let result = match shape {
        Shape::Extrusion {
            profile,
            position,
            direction,
            depth,
        } => (|| {
            if !(depth.is_finite() && *depth > 0.0) || !position.is_finite() {
                return Err(Failure::Invalid("depth"));
            }
            let d = normalize(*direction).ok_or(Failure::Invalid("direction"))?;
            let mesh = prism(&rings(profile, tolerance)?, zumen_geom3d::scale(d, *depth))?;
            Ok(mesh.transformed(position))
        })(),
        Shape::Revolution {
            profile,
            position,
            axis_origin,
            axis_direction,
            angle,
        } => (|| {
            let mesh = revolve(
                &rings(profile, tolerance)?,
                *axis_origin,
                *axis_direction,
                *angle,
                tolerance,
            )?;
            Ok(mesh.transformed(position))
        })(),
        Shape::SweptDisk {
            directrix,
            radius,
            inner_radius,
        } => (|| {
            let [from, to] = directrix.as_slice() else {
                return Err(Failure::Unsupported("swept_disk"));
            };
            let axis = sub(*to, *from);
            let length = zumen_geom3d::length(axis);
            if !(length.is_finite() && length > 0.0) {
                return Err(Failure::Invalid("directrix"));
            }
            let profile = Profile {
                outer: Curve::Circle {
                    center: [0.0, 0.0],
                    radius: *radius,
                },
                holes: inner_radius
                    .iter()
                    .map(|r| Curve::Circle {
                        center: [0.0, 0.0],
                        radius: *r,
                    })
                    .collect(),
            };
            let frame = Transform::from_axes(*from, Some(axis), None)
                .ok_or(Failure::Invalid("directrix"))?;
            Ok(prism(&rings(&profile, tolerance)?, [0.0, 0.0, length])?.transformed(&frame))
        })(),
        Shape::FaceSet { faces: f, .. } => faces(f),
        Shape::Triangles {
            positions,
            triangles,
            ..
        } => Ok(Mesh {
            positions: positions.clone(),
            triangles: triangles.clone(),
            edges: Vec::new(),
        }),
        _ => return None,
    };
    Some(result.and_then(|mesh| mesh.validate().map(|()| mesh)))
}
