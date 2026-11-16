//! 幾何用の確保。型の大きさではなく固定の予約量で 32 bit と 64 bit をそろえる。

use zumen_base::{Cx, Kind};

use crate::{Failure, Mesh};

pub(super) const MAX_POINTS: usize = 65_536;
pub(super) const MAX_TRIANGLES: usize = 65_536;
pub(super) const MAX_FACE_POINTS: usize = 4_096;

pub(super) fn work(cx: &mut Cx, amount: u64) -> Result<(), Failure> {
    cx.check(Kind::Work, amount)?;
    Ok(())
}

pub(super) fn bytes(cx: &mut Cx, amount: u64) -> Result<(), Failure> {
    // usize の上限より小さい共通の上限。呼び出し単位の総量は Cx が制限する。
    if amount > i32::MAX as u64 {
        return Err(Failure::Invalid("allocation_size"));
    }
    cx.check(Kind::DecodedBytes, amount)?;
    Ok(())
}

pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    slot_bytes: u64,
    cx: &mut Cx,
) -> Result<(), Failure> {
    let wanted = values
        .len()
        .checked_add(additional)
        .ok_or(Failure::Invalid("size"))?;
    if wanted <= values.capacity() {
        return Ok(());
    }
    let capacity = wanted.max(values.capacity().saturating_mul(2)).max(4);
    let charge = (capacity - values.capacity()) as u64;
    bytes(
        cx,
        charge
            .checked_mul(slot_bytes)
            .ok_or(Failure::Invalid("size"))?,
    )?;
    values
        .try_reserve_exact(capacity - values.len())
        .map_err(|_| Failure::Failed("allocation".into()))
}

pub(super) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    slot: u64,
    cx: &mut Cx,
) -> Result<(), Failure> {
    reserve(values, 1, slot, cx)?;
    values.push(value);
    Ok(())
}

pub(super) fn triangle(mesh: &mut Mesh, triangle: [u32; 3], cx: &mut Cx) -> Result<(), Failure> {
    if mesh.triangles.len() >= MAX_TRIANGLES {
        return Err(Failure::Invalid("triangle_count"));
    }
    cx.check(Kind::Segments, 1)?;
    push(&mut mesh.triangles, triangle, 12, cx)
}

pub(super) fn mesh_copy(mesh: &Mesh, cx: &mut Cx) -> Result<Mesh, Failure> {
    let mut result = Mesh::default();
    append(&mut result, mesh, cx)?;
    Ok(result)
}

pub(super) fn append(mesh: &mut Mesh, other: &Mesh, cx: &mut Cx) -> Result<(), Failure> {
    let n = mesh
        .positions
        .len()
        .checked_add(other.positions.len())
        .ok_or(Failure::Invalid("size"))?;
    let t = mesh
        .triangles
        .len()
        .checked_add(other.triangles.len())
        .ok_or(Failure::Invalid("size"))?;
    if n > MAX_POINTS || t > MAX_TRIANGLES {
        return Err(Failure::Invalid("mesh_count"));
    }
    work(
        cx,
        (other.positions.len() + other.triangles.len() + other.edges.len()) as u64,
    )?;
    reserve(&mut mesh.positions, other.positions.len(), 24, cx)?;
    reserve(&mut mesh.triangles, other.triangles.len(), 12, cx)?;
    reserve(&mut mesh.edges, other.edges.len(), 8, cx)?;
    // 点の数を MAX_POINTS 以下に検査済み。各添字は呼び出し側で validate する。
    let base = mesh.positions.len() as u32;
    mesh.positions.extend_from_slice(&other.positions);
    mesh.triangles
        .extend(other.triangles.iter().map(|t| t.map(|i| i + base)));
    mesh.edges
        .extend(other.edges.iter().map(|e| e.map(|i| i + base)));
    Ok(())
}
