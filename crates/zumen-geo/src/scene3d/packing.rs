//! 地物ごとの f64 原点と f32 の局所座標。同じ地物を複数の塊へ分けない。
use super::*;
use zumen_scene3d::{Chunk, Color, Instance};

pub(super) fn append(
    part: Pending,
    tolerance: f64,
    data: &mut Data,
    cx: &mut Cx,
) -> Result<(Bounds, f64), Failure> {
    let [lo, hi] = part.bounds;
    let origin = [
        lo[0] / 2.0 + hi[0] / 2.0,
        lo[1] / 2.0 + hi[1] / 2.0,
        lo[2] / 2.0 + hi[2] / 2.0,
    ];
    let mut meshes = Vec::new();
    visual::reserve(&mut meshes, part.meshes.len(), 96, cx)?;
    let mut bounds: Option<[[f32; 3]; 2]> = None;
    let mut error = 0.0_f64;
    for mesh in part.meshes {
        cx.check(Kind::Work, 1)?;
        let mut vertices = Vec::new();
        visual::reserve(&mut vertices, mesh.positions.len(), 12, cx)?;
        for point in mesh.positions {
            cx.check(Kind::Work, 1)?;
            let local = zumen_geom3d::sub(point, origin).map(|n| n as f32);
            if !local.iter().all(|n| n.is_finite()) {
                return Err(Failure::Invalid("coordinate"));
            }
            let restored = zumen_geom3d::add(local.map(f64::from), origin);
            error = error.max(zumen_geom3d::length(zumen_geom3d::sub(restored, point)));
            if !error.is_finite() || error > tolerance / 4.0 {
                return Err(Failure::Invalid("coordinate_precision"));
            }
            if let Some([lo, hi]) = &mut bounds {
                for ((lo, hi), value) in lo.iter_mut().zip(hi).zip(local) {
                    *lo = lo.min(value);
                    *hi = hi.max(value);
                }
            } else {
                bounds = Some([local, local]);
            }
            vertices.push(local);
        }
        meshes.push(zumen_scene3d::Mesh {
            vertices,
            triangles: mesh.triangles,
            edges: mesh.edges,
        });
    }
    let [lo, hi] = bounds.ok_or(Failure::Invalid("empty"))?;
    let first_mesh = u32::try_from(data.meshes.len()).map_err(|_| Failure::Invalid("size"))?;
    let instance = u32::try_from(data.instances.len()).map_err(|_| Failure::Invalid("size"))?;
    let count = u32::try_from(meshes.len()).map_err(|_| Failure::Invalid("size"))?;
    first_mesh
        .checked_add(count)
        .ok_or(Failure::Invalid("size"))?;
    instance
        .checked_add(count)
        .ok_or(Failure::Invalid("size"))?;
    visual::reserve(&mut data.meshes, meshes.len(), 96, cx)?;
    visual::reserve(&mut data.instances, meshes.len(), 64, cx)?;
    visual::reserve(&mut data.chunks, 1, 64, cx)?;
    if data.colors.is_empty() {
        visual::reserve(&mut data.colors, 1, 32, cx)?;
        data.colors.push(Color {
            rgba: [0.0, 0.0, 0.0, 1.0],
            unresolved: true,
        });
    }
    for (index, mesh) in meshes.into_iter().enumerate() {
        data.meshes.push(mesh);
        data.instances.push(Instance {
            mesh: first_mesh + index as u32,
            element: part.element,
            color: 0,
            transform: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        });
    }
    data.chunks.push(Chunk {
        origin,
        bounds: [lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]],
        instances: instance,
    });
    Ok((
        [
            zumen_geom3d::add(lo.map(f64::from), origin),
            zumen_geom3d::add(hi.map(f64::from), origin),
        ],
        error,
    ))
}
