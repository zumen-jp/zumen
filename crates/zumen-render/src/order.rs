//! 列や ID を変えず、塊ごとに描画順の索引を作る（決定 0272）。

use zumen_base::{Cx, Kind};

use crate::{
    Error,
    scene::{STREAMS, Scene},
};

/// unit、種類、元の行の順。同じ unit・種類では列の順を保つ。
type Key = [u32; 3];

pub(crate) fn build(scene: &Scene<'_>, chunk: usize, cx: &mut Cx<'_>) -> Result<Vec<Key>, Error> {
    let mut ranges = [(0, 0); 5];
    let mut count = 0_u64;
    for ((start, end), (starts, units)) in ranges.iter_mut().zip(STREAMS) {
        let range = scene.range(starts, units, chunk)?;
        count += range.len() as u64;
        (*start, *end) = (range.start, range.end);
    }
    // 5 列の行数は u32 の範囲。索引の確保前に塊全体の要素数と容量を数える。
    cx.check(Kind::Elements, count)?;
    let bytes = count * 12;
    cx.check(Kind::DecodedBytes, bytes)?;
    if bytes > i32::MAX as u64 {
        return Err(Error::Allocation);
    }
    let mut keys = Vec::new();
    keys.try_reserve_exact(count as usize)
        .map_err(|_| Error::Allocation)?;
    for (kind, (&(start, end), (_, units))) in ranges.iter().zip(STREAMS).enumerate() {
        for row in start..end {
            cx.check(Kind::Work, 1)?;
            keys.push([scene.uint(units, row, 0)?, kind as u32, row as u32]);
        }
    }
    // 再帰と追加の確保を使わない heap sort。比較する順と work を環境間でそろえる。
    for root in (0..keys.len() / 2).rev() {
        let end = keys.len();
        sift(&mut keys, root, end, cx)?;
    }
    for end in (1..keys.len()).rev() {
        cx.check(Kind::Work, 1)?;
        keys.swap(0, end);
        sift(&mut keys, 0, end, cx)?;
    }
    Ok(keys)
}

fn sift(keys: &mut [Key], mut root: usize, end: usize, cx: &mut Cx<'_>) -> Result<(), Error> {
    loop {
        cx.check(Kind::Work, 1)?;
        let mut child = root
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::Scene)?;
        if child >= end {
            return Ok(());
        }
        if child + 1 < end && keys.get(child) < keys.get(child + 1) {
            child += 1;
        }
        if keys.get(root).ok_or(Error::Scene)? >= keys.get(child).ok_or(Error::Scene)? {
            return Ok(());
        }
        keys.swap(root, child);
        root = child;
    }
}
