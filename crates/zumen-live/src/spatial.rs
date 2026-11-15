//! 範囲で選ぶための階層格子の空間索引。
//!
//! 段ごとにセルの大きさが 2 倍になる格子で、要素は、囲む箱より大きいセルを持ついちばん細かい段の、
//! 箱の中心があるセルに入る。セルの中の要素は両方向のリストでつなぐので、足す・除く・動かすのに再確保が要らず、
//! 手間は図面の要素数によらない。要素ごとの表（`next`・`prev`・`cell`）は固定長のページに分けて持つ。
//! 浮動小数の平方根・切り上げ・切り捨ては `libm` を通し、極端な座標でもあふれない。

use zumen_base::{Cx, Kind, Stop};

use crate::cow::CowColumn;
use crate::handle::RowId;
use crate::paged::Paged;

/// つながりの終わり
const NONE: u32 = u32::MAX;
/// 1 段あたりの最大分割数（異常な極端値で巨大な配列を確保しないための上限）
const MAX_SIDE: u32 = 32_768;
/// 全段合わせたセル数の上限
const MAX_TOTAL_CELLS: u32 = 16_777_216;
/// 段数の上限
const MAX_LEVELS: usize = 64;

/// 行の番号から囲む箱 `[min_x, min_y, max_x, max_y]` を引くための共通インターフェース
pub trait BoundsSource {
    /// 行の数
    fn bounds_len(&self) -> usize;
    /// `index` 行めの箱を返す
    fn bounds_at(&self, index: usize) -> Option<[f64; 4]>;
}

impl<const BITS: usize> BoundsSource for Paged<[f64; 4], BITS> {
    #[inline]
    fn bounds_len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn bounds_at(&self, index: usize) -> Option<[f64; 4]> {
        self.get(index).copied()
    }
}

impl<const BITS: usize> BoundsSource for CowColumn<[f64; 4], BITS> {
    #[inline]
    fn bounds_len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn bounds_at(&self, index: usize) -> Option<[f64; 4]> {
        self.get(index).copied()
    }
}

impl BoundsSource for [[f64; 4]] {
    #[inline]
    fn bounds_len(&self) -> usize {
        self.len()
    }

    #[inline]
    fn bounds_at(&self, index: usize) -> Option<[f64; 4]> {
        self.get(index).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Level {
    /// セルの辺の長さの半分（`±f64::MAX` 付近でも引き算があふれないよう半分の尺度で持つ）
    half_size: f64,
    /// 横のセルの数
    columns: u32,
    /// 縦のセルの数
    rows: u32,
    /// `head` の中の、この段の最初のセル
    offset: u32,
}

/// 階層格子の空間索引
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpatialGrid {
    /// 原点の座標の半分 `[min_x * 0.5, min_y * 0.5]`
    half_origin: [f64; 2],
    levels: Vec<Level>,
    /// セルごとの最初の要素
    head: Vec<u32>,
    /// 要素ごとの、同じセルの次の要素
    next: Paged<u32>,
    /// 要素ごとの、同じセルの前の要素
    prev: Paged<u32>,
    /// 要素が入っているセル（`head` の番号）
    cell: Paged<u32>,
}

/// `bench/live` などからの短い別名
pub type Grid = SpatialGrid;

/// 箱が有効な有限の範囲を持つか。数でない値や無限大、逆転した箱は空とみなす
#[inline]
fn is_valid_box([x0, y0, x1, y1]: [f64; 4]) -> bool {
    x0.is_finite() && y0.is_finite() && x1.is_finite() && y1.is_finite() && x0 <= x1 && y0 <= y1
}

/// 検索矩形が空か（無限大を含む全域検索は許し、逆転と NaN だけを空とする）
#[inline]
fn is_empty_rect([x0, y0, x1, y1]: [f64; 4]) -> bool {
    !(x0 <= x1 && y0 <= y1)
}

fn compute_levels<B: BoundsSource + ?Sized>(boxes: &B) -> ([f64; 2], Vec<Level>, u32) {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let len = boxes.bounds_len();
    for i in 0..len {
        if let Some(item) = boxes.bounds_at(i)
            && is_valid_box(item)
        {
            bounds = [
                bounds[0].min(item[0]),
                bounds[1].min(item[1]),
                bounds[2].max(item[2]),
                bounds[3].max(item[3]),
            ];
        }
    }
    if !is_valid_box(bounds) {
        bounds = [0.0, 0.0, 1.0, 1.0];
    }
    let half_origin = [bounds[0] * 0.5, bounds[1] * 0.5];
    let raw_hw = bounds[2] * 0.5 - half_origin[0];
    let raw_hh = bounds[3] * 0.5 - half_origin[1];
    let longer_half = raw_hw.max(raw_hh);
    let longer_half = if longer_half > 0.0 && longer_half.is_finite() {
        longer_half
    } else {
        0.5
    };
    // 細長い範囲でも、セルの数が要素の数ほどになるようにする
    let half_w = raw_hw.max(longer_half * 1e-6);
    let half_h = raw_hh.max(longer_half * 1e-6);
    let count = (len.max(1)) as f64;
    let sqrt_count = libm::sqrt(count);
    let finest_half = ((libm::sqrt(half_w) / sqrt_count) * libm::sqrt(half_h))
        .max(longer_half / f64::from(MAX_SIDE));
    let finest_half = if finest_half > 0.0 && finest_half.is_finite() {
        finest_half
    } else {
        0.5
    };

    let mut levels = Vec::new();
    let mut half_size = finest_half;
    let mut offset = 0u32;
    for _ in 0..MAX_LEVELS {
        let mut columns;
        let mut rows;
        let mut level_cells;
        loop {
            let col_f = libm::ceil(half_w / half_size);
            let row_f = libm::ceil(half_h / half_size);
            columns = if col_f.is_finite() && col_f >= 1.0 {
                (col_f.min(f64::from(MAX_SIDE))) as u32
            } else {
                1
            };
            rows = if row_f.is_finite() && row_f >= 1.0 {
                (row_f.min(f64::from(MAX_SIDE))) as u32
            } else {
                1
            };
            level_cells = columns.saturating_mul(rows);
            // 0 段目でも MAX_TOTAL_CELLS を超えないようセル幅を倍化する
            if !levels.is_empty() || level_cells <= MAX_TOTAL_CELLS || (columns <= 1 && rows <= 1) {
                break;
            }
            let coarser = half_size * 2.0;
            if !coarser.is_finite() {
                columns = 1;
                rows = 1;
                level_cells = 1;
                break;
            }
            half_size = coarser;
        }
        let Some(next_offset) = offset.checked_add(level_cells) else {
            break;
        };
        if next_offset > MAX_TOTAL_CELLS && !levels.is_empty() {
            // 最後に全体を覆う 1×1 の段を足して終える
            let cover = half_w.max(half_h).max(half_size);
            if let Some(final_offset) = offset.checked_add(1) {
                levels.push(Level {
                    half_size: cover,
                    columns: 1,
                    rows: 1,
                    offset,
                });
                offset = final_offset;
            }
            break;
        }
        levels.push(Level {
            half_size,
            columns,
            rows,
            offset,
        });
        offset = next_offset;
        if columns == 1 && rows == 1 {
            break;
        }
        let next_half = half_size * 2.0;
        if !next_half.is_finite() {
            if let Some(final_offset) = offset.checked_add(1) {
                levels.push(Level {
                    half_size: f64::MAX * 0.5,
                    columns: 1,
                    rows: 1,
                    offset,
                });
                offset = final_offset;
            }
            break;
        }
        half_size = next_half;
    }
    if levels.is_empty() {
        levels.push(Level {
            half_size: 0.5,
            columns: 1,
            rows: 1,
            offset: 0,
        });
        offset = 1;
    }
    (half_origin, levels, offset)
}

impl SpatialGrid {
    /// 要素の箱（`[min_x, min_y, max_x, max_y]`）から作る
    pub fn build<B: BoundsSource + ?Sized>(boxes: &B) -> Self {
        let len = boxes.bounds_len();
        let (half_origin, levels, total_cells) = compute_levels(boxes);
        let mut grid = Self {
            half_origin,
            levels,
            head: vec![NONE; total_cells as usize],
            next: Paged::filled(NONE, len),
            prev: Paged::filled(NONE, len),
            cell: Paged::filled(NONE, len),
        };
        for row in 0..len {
            if let (Ok(row_u32), Some(item)) = (u32::try_from(row), boxes.bounds_at(row))
                && row_u32 != NONE
            {
                grid.insert(row_u32, item);
            }
        }
        grid
    }

    /// 予算と中断を確かめながら、要素の箱から作る
    pub fn build_with_cx<B: BoundsSource + ?Sized>(
        boxes: &B,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Stop> {
        let len = boxes.bounds_len();
        cx.check(Kind::Work, len as u64)?;
        let (half_origin, levels, total_cells) = compute_levels(boxes);
        let bytes = ((total_cells as u64).saturating_add((len as u64).saturating_mul(3)))
            .saturating_mul(core::mem::size_of::<u32>() as u64);
        cx.check(Kind::DecodedBytes, bytes)?;
        let mut grid = Self {
            half_origin,
            levels,
            head: vec![NONE; total_cells as usize],
            next: Paged::filled(NONE, len),
            prev: Paged::filled(NONE, len),
            cell: Paged::filled(NONE, len),
        };
        for row in 0..len {
            if (row & 4095) == 0 {
                cx.check(Kind::Work, 1)?;
            }
            if let (Ok(row_u32), Some(item)) = (u32::try_from(row), boxes.bounds_at(row))
                && row_u32 != NONE
            {
                grid.insert(row_u32, item);
            }
        }
        Ok(grid)
    }

    /// 段 `level` の、半分の座標 `half_value` が入る列か行。範囲の外は端のセルに寄せる
    #[inline]
    fn index_half(&self, level: &Level, axis: usize, half_value: f64) -> u32 {
        let count = if axis == 0 { level.columns } else { level.rows };
        if count <= 1 {
            return 0;
        }
        if half_value == f64::NEG_INFINITY {
            return 0;
        }
        if half_value == f64::INFINITY {
            return count - 1;
        }
        let origin = self.half_origin.get(axis).copied().unwrap_or(0.0);
        let at = libm::floor((half_value - origin) / level.half_size);
        if at.is_nan() || at <= 0.0 {
            return 0;
        }
        let max_idx = count - 1;
        if at >= f64::from(max_idx) {
            max_idx
        } else {
            at as u32
        }
    }

    /// 箱が入るセル
    fn place(&self, item: [f64; 4]) -> Option<u32> {
        if !is_valid_box(item) {
            return None;
        }
        let half_extent = (item[2] * 0.5 - item[0] * 0.5).max(item[3] * 0.5 - item[1] * 0.5);
        let last = self.levels.len().checked_sub(1)?;
        let at = self
            .levels
            .iter()
            .position(|level| level.half_size >= half_extent)
            .unwrap_or(last);
        let level = self.levels.get(at)?;
        let column = self.index_half(level, 0, item[0] * 0.25 + item[2] * 0.25);
        let row = self.index_half(level, 1, item[1] * 0.25 + item[3] * 0.25);
        Some(level.offset + row * level.columns + column)
    }

    fn insert(&mut self, row: u32, item: [f64; 4]) {
        let Some(cell) = self.place(item) else {
            return;
        };
        let Some(&first) = self.head.get(cell as usize) else {
            return;
        };
        if let Some(slot) = self.next.get_mut(row as usize) {
            *slot = first;
        }
        if let Some(slot) = self.prev.get_mut(row as usize) {
            *slot = NONE;
        }
        if first != NONE
            && let Some(slot) = self.prev.get_mut(first as usize)
        {
            *slot = row;
        }
        if let Some(slot) = self.head.get_mut(cell as usize) {
            *slot = row;
        }
        if let Some(slot) = self.cell.get_mut(row as usize) {
            *slot = cell;
        }
    }

    fn remove(&mut self, row: u32) {
        let Some(&cell) = self.cell.get(row as usize) else {
            return;
        };
        if cell == NONE {
            return;
        }
        let before = self.prev.get(row as usize).copied().unwrap_or(NONE);
        let after = self.next.get(row as usize).copied().unwrap_or(NONE);
        if before == NONE {
            if let Some(slot) = self.head.get_mut(cell as usize) {
                *slot = after;
            }
        } else if let Some(slot) = self.next.get_mut(before as usize) {
            *slot = after;
        }
        if after != NONE
            && let Some(slot) = self.prev.get_mut(after as usize)
        {
            *slot = before;
        }
        if let Some(slot) = self.cell.get_mut(row as usize) {
            *slot = NONE;
        }
    }

    /// 終わりに要素を足す。段の作りは開いたときのまま保つ
    pub fn push(&mut self, item: [f64; 4]) {
        let Ok(row) = u32::try_from(self.cell.len()) else {
            return;
        };
        if row == NONE {
            return;
        }
        self.next.push(NONE);
        self.prev.push(NONE);
        self.cell.push(NONE);
        self.insert(row, item);
    }

    /// 終わりの要素を除く
    pub fn pop(&mut self) {
        let Some(row) = self.cell.len().checked_sub(1) else {
            return;
        };
        if let Ok(row_u32) = u32::try_from(row) {
            self.remove(row_u32);
        }
        self.next.pop();
        self.prev.pop();
        self.cell.pop();
    }

    /// 要素の箱が変わったときに索引を更新する。空の箱（除いた要素）は索引から外す
    pub fn update(&mut self, row: u32, item: [f64; 4]) {
        let Some(&current) = self.cell.get(row as usize) else {
            return;
        };
        if self.place(item) == Some(current) {
            return;
        }
        self.remove(row);
        self.insert(row, item);
    }

    /// [`RowId`] で指す要素の箱を更新する
    #[inline]
    pub fn update_row(&mut self, row: RowId, item: [f64; 4]) {
        self.update(row.0, item);
    }

    /// 箱が `rect` と重なる要素の行番号を、昇順で `out` へ足す
    pub fn query<B: BoundsSource + ?Sized>(&self, boxes: &B, rect: [f64; 4], out: &mut Vec<u32>) {
        let start = out.len();
        if is_empty_rect(rect) {
            return;
        }
        let hr0 = rect[0] * 0.5;
        let hr1 = rect[1] * 0.5;
        let hr2 = rect[2] * 0.5;
        let hr3 = rect[3] * 0.5;
        for level in &self.levels {
            // この段の要素は、中心からセルの半分までしか広がらない。丸めの分だけ余分に見る
            let half_reach = level.half_size * 0.5625;
            let columns = self.index_half(level, 0, hr0 - half_reach)
                ..=self.index_half(level, 0, hr2 + half_reach);
            let rows = self.index_half(level, 1, hr1 - half_reach)
                ..=self.index_half(level, 1, hr3 + half_reach);
            for row in rows {
                for column in columns.clone() {
                    let cell = (level.offset + row * level.columns + column) as usize;
                    let mut at = self.head.get(cell).copied().unwrap_or(NONE);
                    while at != NONE {
                        if let Some(item) = boxes.bounds_at(at as usize)
                            && is_valid_box(item)
                            && item[0] <= rect[2]
                            && item[2] >= rect[0]
                            && item[1] <= rect[3]
                            && item[3] >= rect[1]
                        {
                            out.push(at);
                        }
                        at = self.next.get(at as usize).copied().unwrap_or(NONE);
                    }
                }
            }
        }
        out.get_mut(start..).unwrap_or_default().sort_unstable();
    }

    /// 予算と中断を確かめながら、箱が `rect` と重なる [`RowId`] を昇順で `out` へ足す。
    /// 予算超過または中断で止まったときは `out` を呼び出し前の長さに戻す。
    pub fn query_with_cx<B: BoundsSource + ?Sized>(
        &self,
        boxes: &B,
        rect: [f64; 4],
        out: &mut Vec<RowId>,
        cx: &mut Cx<'_>,
    ) -> Result<(), Stop> {
        let start = out.len();
        cx.check(Kind::Work, 1)?;
        if is_empty_rect(rect) {
            return Ok(());
        }
        let hr0 = rect[0] * 0.5;
        let hr1 = rect[1] * 0.5;
        let hr2 = rect[2] * 0.5;
        let hr3 = rect[3] * 0.5;
        for level in &self.levels {
            let half_reach = level.half_size * 0.5625;
            let columns = self.index_half(level, 0, hr0 - half_reach)
                ..=self.index_half(level, 0, hr2 + half_reach);
            let rows = self.index_half(level, 1, hr1 - half_reach)
                ..=self.index_half(level, 1, hr3 + half_reach);
            for row in rows {
                for column in columns.clone() {
                    if let Err(stop) = cx.check(Kind::Work, 1) {
                        out.truncate(start);
                        return Err(stop);
                    }
                    let cell = (level.offset + row * level.columns + column) as usize;
                    let mut at = self.head.get(cell).copied().unwrap_or(NONE);
                    while at != NONE {
                        if let Err(stop) = cx.check(Kind::Work, 1) {
                            out.truncate(start);
                            return Err(stop);
                        }
                        if let Some(item) = boxes.bounds_at(at as usize)
                            && is_valid_box(item)
                            && item[0] <= rect[2]
                            && item[2] >= rect[0]
                            && item[1] <= rect[3]
                            && item[3] >= rect[1]
                        {
                            out.push(RowId(at));
                        }
                        at = self.next.get(at as usize).copied().unwrap_or(NONE);
                    }
                }
            }
        }
        out.get_mut(start..).unwrap_or_default().sort_unstable();
        Ok(())
    }

    /// 索引が持つ行の数
    #[inline]
    pub const fn len(&self) -> usize {
        self.cell.len()
    }

    /// 索引が空か
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.cell.is_empty()
    }

    /// 索引のバイト数
    pub fn bytes(&self) -> usize {
        self.head
            .len()
            .saturating_add(self.next.len().saturating_mul(3))
            .saturating_mul(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Random(u64);

    impl Random {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
    }

    fn brute(boxes: &Paged<[f64; 4]>, rect: [f64; 4]) -> Vec<u32> {
        (0..boxes.len() as u32)
            .filter(|&row| {
                let Some(&item) = boxes.get(row as usize) else {
                    return false;
                };
                is_valid_box(item)
                    && item[0] <= rect[2]
                    && item[2] >= rect[0]
                    && item[1] <= rect[3]
                    && item[3] >= rect[1]
            })
            .collect()
    }

    #[test]
    fn queries_match_full_scan_before_and_after_moves() {
        let mut random = Random(7);
        let mut boxes: Paged<[f64; 4]> = (0..3000)
            .map(|index| {
                let (x, y) = (random.next() * 200.0, random.next() * 100.0);
                let size = match index % 7 {
                    0 => 0.0,
                    1 => random.next() * 80.0,
                    _ => random.next() * 0.5,
                };
                [x, y, x + size, y + size * random.next()]
            })
            .collect();
        let mut grid = SpatialGrid::build(&boxes);
        let check = |grid: &SpatialGrid, boxes: &Paged<[f64; 4]>, random: &mut Random| {
            for _ in 0..200 {
                let (x, y) = (random.next() * 260.0 - 30.0, random.next() * 160.0 - 30.0);
                let rect = [x, y, x + random.next() * 20.0, y + random.next() * 20.0];
                let mut found = Vec::new();
                grid.query(boxes, rect, &mut found);
                assert_eq!(found, brute(boxes, rect));
            }
        };
        check(&grid, &boxes, &mut random);
        for _ in 0..2000 {
            let row = (random.next() * boxes.len() as f64) as usize;
            let (dx, dy) = (random.next() * 400.0 - 200.0, random.next() * 400.0 - 200.0);
            let item = *boxes.get(row).unwrap();
            let moved = [item[0] + dx, item[1] + dy, item[2] + dx, item[3] + dy];
            *boxes.get_mut(row).unwrap() = moved;
            grid.update(row as u32, moved);
        }
        check(&grid, &boxes, &mut random);
        let mut all = Vec::new();
        grid.query(&boxes, [-1e9, -1e9, 1e9, 1e9], &mut all);
        assert_eq!(all.len(), boxes.len());
    }

    #[test]
    fn extreme_f64_and_nan_boxes_never_overflow_or_panic() {
        let boxes: [[f64; 4]; 5] = [
            [-1e308, -1e308, -1e308 + 1.0, -1e308 + 1.0],
            [1e308 - 1.0, 1e308 - 1.0, 1e308, 1e308],
            [-1e308, -1e308, 1e308, 1e308],
            [f64::NAN, 0.0, 1.0, 1.0],
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
        ];
        let grid = SpatialGrid::build(&boxes[..]);
        let mut found = Vec::new();
        grid.query(&boxes[..], [-1e308, -1e308, 1e308, 1e308], &mut found);
        assert_eq!(found, vec![0, 1, 2]);

        // query_with_cx が途中で予算上限に達したときは out を呼び出し前の長さに巻き戻す
        let mut budget = zumen_base::Budget::new(
            zumen_base::Limits::default().with(Kind::Work, zumen_base::Limit::Finite(3)),
        );
        let mut diag = zumen_base::Diagnostics::new();
        let cancel = zumen_base::CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut out_rows = vec![RowId(999)];
        let err = grid
            .query_with_cx(
                &boxes[..],
                [-1e308, -1e308, 1e308, 1e308],
                &mut out_rows,
                &mut cx,
            )
            .unwrap_err();
        assert!(matches!(err, Stop::Exceeded(e) if e.kind == Kind::Work));
        assert_eq!(out_rows, vec![RowId(999)]);
    }
}
