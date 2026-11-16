//! 切り出しで使う固定の論理量。環境のポインター幅で停止位置を変えない。

use zumen_base::{Cx, Kind, Stop};
use zumen_geom2d::topology::{Charge, Failure as TopologyFailure};
use zumen_geom3d::Failure;

pub(super) fn work(cx: &mut Cx<'_>, amount: u64) -> Result<(), Failure> {
    cx.check(Kind::Work, amount)?;
    Ok(())
}

pub(super) fn reserve<T>(
    out: &mut Vec<T>,
    extra: usize,
    slot: u64,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    let wanted = out
        .len()
        .checked_add(extra)
        .ok_or(Failure::Invalid("drawing_size"))?;
    if wanted <= out.capacity() {
        return Ok(());
    }
    let capacity = wanted.max(out.capacity().saturating_mul(2)).max(4);
    let total = (capacity as u64)
        .checked_mul(slot)
        .ok_or(Failure::Invalid("drawing_size"))?;
    if total > i32::MAX as u64 {
        return Err(Failure::Invalid("drawing_size"));
    }
    cx.check(
        Kind::DecodedBytes,
        (capacity - out.capacity()) as u64 * slot,
    )?;
    out.try_reserve_exact(capacity - out.len())
        .map_err(|_| Failure::Failed("allocation".into()))
}

pub(super) fn push<T>(
    out: &mut Vec<T>,
    value: T,
    slot: u64,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    reserve(out, 1, slot, cx)?;
    out.push(value);
    Ok(())
}

pub(super) fn charge(cx: &mut Cx<'_>, value: Charge) -> Result<(), Stop> {
    let (kind, amount) = match value {
        Charge::Work(n) => (Kind::Work, n),
        Charge::Bytes(n) => (Kind::DecodedBytes, n),
        Charge::Points(n) => (Kind::Segments, n),
        _ => (Kind::Work, 0),
    };
    cx.check(kind, amount)
}

pub(super) fn topology(error: TopologyFailure<Stop>) -> Failure {
    match error {
        TopologyFailure::Stopped(stop) => Failure::Stopped(stop),
        TopologyFailure::Geometry(error) => Failure::Failed(format!("drawing_topology:{error:?}")),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn logical_slots_cover_the_native_layouts() {
        use super::super::{Line, Source, output::Stroke, project::Edge, visibility::Surface};
        assert!(size_of::<Source>() <= 112);
        assert!(size_of::<Line>() <= 96);
        assert!(size_of::<Stroke>() <= 48);
        assert!(size_of::<Edge>() <= 64);
        assert!(size_of::<Surface>() <= 80);
        assert!(size_of::<zumen_drawing::Element<zumen_drawing::ModelPoint>>() <= 512);
        assert!(size_of::<zumen_drawing::Extension>() <= 256);
        assert!(size_of::<zumen_drawing::Field>() <= 80);
    }
}
