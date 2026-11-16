//! 評価済みの部品を、移動を除いた共通座標で計測する。

use crate::evaluation::{ElementMesh, MeshPart};
use zumen_base::{Cx, Kind};
use zumen_geom3d::{BooleanOp, Failure, ManifoldKernel, Mesh, Tolerance, Xyz};

/// 大きなプロジェクト座標を先に引き、局所の小さい点を後から足す。
pub(super) fn local_mesh(part: &MeshPart, origin: Xyz, cx: &mut Cx<'_>) -> Result<Mesh, Failure> {
    let n = part.mesh.positions.len() as u64;
    let t = part.mesh.triangles.len() as u64;
    let e = part.mesh.edges.len() as u64;
    let bytes = n
        .saturating_mul(24)
        .saturating_add(t.saturating_mul(12))
        .saturating_add(e.saturating_mul(8));
    if bytes > i32::MAX as u64 {
        return Err(Failure::Invalid("measure_mesh_size"));
    }
    cx.check(Kind::Work, n.saturating_add(t).saturating_add(e))?;
    cx.check(Kind::DecodedBytes, bytes)?;
    let mut transform = part.placement;
    for (row, coordinate) in transform.0.iter_mut().zip(origin) {
        row[3] -= coordinate;
    }
    let determinant = transform.determinant();
    if !transform.is_finite() || !determinant.is_finite() || determinant == 0.0 {
        return Err(Failure::Invalid("measure_placement"));
    }
    let mut mesh = part.mesh.transformed(&transform);
    // Mesh::transformed は鏡映時の向きも直す。
    mesh.validate()?;
    // 描画用の辺を体積の演算へ持ち込まない。
    mesh.edges.clear();
    Ok(mesh)
}

pub(super) fn volume(
    element: &ElementMesh,
    tolerance: Tolerance,
    place: &zumen_base::Ref,
    cx: &mut Cx<'_>,
) -> Result<f64, Failure> {
    if !element.complete {
        return Err(Failure::Invalid("measure_incomplete_shape"));
    }
    let first = element
        .parts
        .first()
        .ok_or(Failure::Invalid("measure_shape_missing"))?;
    let origin = first.placement.column(3);
    let mut combined = None;
    // 複数の項目が重なる場合も、体積を二重に足さない。失敗した和を単純和へ読み替えない。
    for part in &element.parts {
        cx.check(Kind::Work, 1)?;
        let next = local_mesh(part, origin, cx)?;
        // 開口が全部を引いた場合の空の結果は、体積 0 として保つ。
        if next.positions.is_empty() && next.triangles.is_empty() {
            continue;
        }
        if !ManifoldKernel.is_solid(&next, cx)? {
            return Err(Failure::Invalid("measure_open_shell"));
        }
        combined = Some(if let Some(mesh) = combined {
            let evaluated =
                ManifoldKernel.boolean(BooleanOp::Union, &mesh, &next, tolerance, cx)?;
            evaluated.changes.report(Some(place), tolerance, cx)?;
            evaluated.mesh
        } else {
            next
        });
    }
    let Some(mesh) = combined else { return Ok(0.0) };
    cx.check(Kind::Work, mesh.triangles.len() as u64)?;
    let value = mesh.volume();
    if !value.is_finite() || value < 0.0 {
        return Err(Failure::Invalid("measure_volume"));
    }
    Ok(value)
}
