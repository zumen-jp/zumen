//! 列の変更範囲を 64 要素の塊ごとのビット集合で追跡する部品。
//!
//! 表示のバッファーの GPU 転送や、別プロセス・Worker への差分送信で、変更された塊だけを
//! ページの境界をまたがない連続範囲として取り出す。

use core::ops::Range;

use zumen_base::{Cx, Kind, Stop};

/// 差分追跡の 1 塊の要素数（64 要素）
pub const DIRTY_BLOCK: usize = 64;
const DIRTY_SHIFT: usize = 6;

/// 1 本の配列の変更範囲を 64 要素単位のビット集合で追跡する（既定のページ境界は `1 << 10 = 1,024` 要素）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyColumn<const PAGE_BITS: usize = 10> {
    words: Vec<u64>,
    synced_len: usize,
}

impl<const PAGE_BITS: usize> Default for DirtyColumn<PAGE_BITS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PAGE_BITS: usize> DirtyColumn<PAGE_BITS> {
    const VALID_PAGE_BITS: () =
        assert!(PAGE_BITS >= DIRTY_SHIFT && PAGE_BITS < usize::BITS as usize);

    /// 空の追跡器を作る
    pub const fn new() -> Self {
        let () = Self::VALID_PAGE_BITS;
        Self {
            words: Vec::new(),
            synced_len: 0,
        }
    }

    /// 前回同期したときの要素数
    #[inline]
    pub const fn synced_len(&self) -> usize {
        self.synced_len
    }

    /// 1 要素を変更済みとして記録する
    #[inline]
    pub fn mark_one(&mut self, index: usize) {
        let block = index >> DIRTY_SHIFT;
        let word = block >> 6;
        if self.words.len() <= word {
            self.words.resize(word + 1, 0);
        }
        if let Some(slot) = self.words.get_mut(word) {
            *slot |= 1u64 << (block & 63);
        }
    }

    /// `start..end` の範囲を変更済みとして記録する
    #[inline]
    pub fn mark_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let first = start >> DIRTY_SHIFT;
        let last = (end - 1) >> DIRTY_SHIFT;
        let last_word = last >> 6;
        if self.words.len() <= last_word {
            self.words.resize(last_word + 1, 0);
        }
        Self::mark_range_blocks(&mut self.words, first, last);
    }

    #[inline]
    fn mark_range_blocks(words: &mut [u64], first: usize, last: usize) {
        let first_word = first >> 6;
        let last_word = last >> 6;
        if first_word == last_word {
            let lo = first & 63;
            let hi = last & 63;
            let mask = (u64::MAX << lo) & (u64::MAX >> (63 - hi));
            if let Some(slot) = words.get_mut(first_word) {
                *slot |= mask;
            }
            return;
        }
        if let Some(slot) = words.get_mut(first_word) {
            *slot |= u64::MAX << (first & 63);
        }
        if let Some(mid) = words.get_mut(first_word + 1..last_word) {
            for slot in mid {
                *slot = u64::MAX;
            }
        }
        if let Some(slot) = words.get_mut(last_word) {
            *slot |= u64::MAX >> (63 - (last & 63));
        }
    }

    /// 要素数が `new_len` に縮んだときに、末尾の外のフラグを落とす
    pub fn truncate(&mut self, new_len: usize) {
        let keep_blocks = new_len.div_ceil(DIRTY_BLOCK);
        let keep_words = keep_blocks.div_ceil(64);
        if self.words.len() > keep_words {
            self.words.truncate(keep_words);
        }
        let rem = keep_blocks & 63;
        if rem != 0
            && let Some(last) = self.words.get_mut(keep_words - 1)
        {
            *last &= (1u64 << rem) - 1;
        }
    }

    /// すべての変更フラグを消し、現在の長さ `current_len` を同期済みとして覚える
    pub fn clear(&mut self, current_len: usize) {
        self.words.fill(0);
        self.synced_len = current_len;
    }

    /// 変更があるか（要素数の増減を含む）
    pub fn is_dirty(&self, current_len: usize) -> bool {
        current_len != self.synced_len || self.words.iter().any(|&w| w != 0)
    }

    /// 同じページ（`1 << PAGE_BITS`）内で連続する変更範囲をまとめて順に渡す
    fn visit_spans(&self, current_len: usize, mut visit: impl FnMut(Range<usize>)) {
        let () = Self::VALID_PAGE_BITS;
        let mut pending: Option<Range<usize>> = None;
        for (wi, &word_val) in self.words.iter().enumerate() {
            let mut w = word_val;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                w &= w - 1;
                let block = (wi << 6) | bit;
                let start = block << DIRTY_SHIFT;
                if start >= current_len {
                    if let Some(prev) = pending {
                        visit(prev);
                    }
                    return;
                }
                let end = (start + DIRTY_BLOCK).min(current_len);
                if let Some(prev) = pending.as_mut()
                    && prev.end == start
                    && (prev.start >> PAGE_BITS) == (start >> PAGE_BITS)
                {
                    prev.end = end;
                } else {
                    if let Some(prev) = pending.replace(start..end) {
                        visit(prev);
                    }
                }
            }
        }
        if let Some(prev) = pending {
            visit(prev);
        }
    }

    /// 同じページ（`1 << PAGE_BITS`）内で連続する変更範囲をまとめて返す
    pub fn spans(&self, current_len: usize) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        self.visit_spans(current_len, |span| out.push(span));
        out
    }

    /// 変更された要素の合計数
    pub fn dirty_elements(&self, current_len: usize) -> usize {
        self.spans(current_len).into_iter().map(|r| r.len()).sum()
    }

    /// IPC の書き出しで、一時配列を作らず 64 要素の塊を昇順にたどる。
    pub(crate) fn visit_blocks_with_cx(
        &self,
        current_len: usize,
        cx: &mut Cx<'_>,
        mut visit: impl FnMut(Range<usize>, &mut Cx<'_>) -> Result<(), Stop>,
    ) -> Result<(), Stop> {
        for (index, &word) in self.words.iter().enumerate() {
            cx.check(Kind::Work, 1)?;
            let mut remaining = word;
            while remaining != 0 {
                cx.check(Kind::Work, 1)?;
                let bit = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                let start = ((index << 6) | bit) << DIRTY_SHIFT;
                if start >= current_len {
                    return Ok(());
                }
                visit(
                    start..start.saturating_add(DIRTY_BLOCK).min(current_len),
                    cx,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesces_within_page_and_splits_at_page_boundary() {
        let mut d: DirtyColumn<10> = DirtyColumn::new();
        d.clear(2048);
        assert!(!d.is_dirty(2048));

        // 0..128 は同じ 1,024 ページ内なので 1 区間にまとまる
        d.mark_one(5);
        d.mark_one(70);
        // 960..1088 は 1,024 の境界をまたぐので 960..1024 と 1024..1088 に分かれる
        d.mark_range(1020, 1030);

        assert_eq!(d.spans(2048), vec![0..128, 960..1024, 1024..1088]);
        assert_eq!(d.dirty_elements(2048), 256);

        d.truncate(1000);
        assert_eq!(d.spans(1000), vec![0..128, 960..1000]);
        assert!(d.is_dirty(1000));
        d.clear(1000);
        assert!(!d.is_dirty(1000));
    }

    #[test]
    fn truncate_above_allocated_words_keeps_earlier_dirty_blocks() {
        let mut d: DirtyColumn<10> = DirtyColumn::new();
        d.mark_one(1000); // block 15 (word 0)
        // keep_blocks = 65 -> keep_words = 2, rem = 1 だが words.len() == 1 のため word 0 は削らない
        d.truncate(4160);
        assert_eq!(d.spans(4160), vec![960..1024]);
        assert!(d.is_dirty(4160));
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// 3 ワード（192 ブロック = 12,288 要素）にわたり `mark_range_blocks` が
    /// 同一ワード・隣接ワード・中間ワード全埋めのすべてで素朴な範囲判定と一致することを証明する。
    #[kani::proof]
    #[kani::unwind(4)]
    fn verify_dirty_column_mark_range_matches_model() {
        let mut words = [0u64; 3];
        let start: usize = kani::any();
        let end: usize = kani::any();
        kani::assume(start < end && end <= 3 * 64 * DIRTY_BLOCK);
        let first = start >> DIRTY_SHIFT;
        let last = (end - 1) >> DIRTY_SHIFT;
        DirtyColumn::<10>::mark_range_blocks(&mut words, first, last);

        let probe: usize = kani::any();
        kani::assume(probe < 3 * 64);
        let bit_set = (words[probe >> 6] & (1u64 << (probe & 63))) != 0;
        assert_eq!(bit_set, probe >= first && probe <= last);
    }

    /// `DirtyColumn<7>` の任意のビット状態（2 ページ × 2 ブロックの全 16 通り）と任意の長さについて、
    /// ページ境界での分割・同一ページ内での極大結合・全変更ブロックの被覆一致を証明する。
    #[kani::proof]
    #[kani::unwind(6)]
    #[kani::solver(kissat)]
    fn verify_dirty_column_spans_coalescing_and_page_split() {
        const BLOCKS: usize = 4;
        const CAP: usize = BLOCKS * DIRTY_BLOCK;

        let bits: u8 = kani::any::<u8>() & 0x0F;
        let current_len: usize = kani::any();
        kani::assume(current_len <= CAP);

        let col: DirtyColumn<7> = DirtyColumn {
            words: vec![u64::from(bits)],
            synced_len: 0,
        };

        let mut count = 0usize;
        let mut prev_start = 0usize;
        let mut prev_end = 0usize;
        let mut covered = 0u8;
        col.visit_spans(current_len, |r| {
            assert!(r.start < r.end);
            assert!(r.end <= current_len);
            assert!(r.start >= prev_end);
            assert_eq!(r.start >> 7, (r.end - 1) >> 7);
            if count > 0 && r.start == prev_end {
                // 連続しているのに分かれた区間は必ずページ境界をまたいでいる
                assert_ne!(prev_start >> 7, r.start >> 7);
            }
            let first_b = r.start >> DIRTY_SHIFT;
            let last_b = (r.end - 1) >> DIRTY_SHIFT;
            assert!(last_b - first_b <= 1);
            covered |= (1u8 << first_b) | (1u8 << last_b);
            prev_start = r.start;
            prev_end = r.end;
            count += 1;
        });

        let mut expected = 0u8;
        for b in 0..BLOCKS {
            if (bits & (1u8 << b)) != 0 && (b << DIRTY_SHIFT) < current_len {
                expected |= 1u8 << b;
            }
        }
        assert_eq!(covered, expected);
    }

    #[kani::proof]
    #[kani::unwind(4)]
    fn verify_dirty_column_truncate_across_words_and_extreme_usize() {
        let mut col: DirtyColumn<10> = DirtyColumn::new();
        let idx: usize = kani::any();
        kani::assume(idx < 64 * DIRTY_BLOCK); // word 0 内（0..4096）
        col.mark_one(idx);

        // word 0 より大きい new_len（word 1 や usize::MAX 付近を含む）への truncate は word 0 のビットを消さない
        let new_len: usize = kani::any();
        kani::assume(new_len >= 64 * DIRTY_BLOCK);
        col.truncate(new_len);
        assert!(col.is_dirty(new_len));
        let block = idx >> DIRTY_SHIFT;
        assert_ne!(col.words[0] & (1u64 << block), 0);
    }
}
