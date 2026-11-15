//! 列の変更範囲を 64 要素の塊ごとのビット集合で追跡する部品。
//!
//! 表示のバッファーの GPU 転送や、別プロセス・Worker への差分送信で、変更された塊だけを
//! ページの境界をまたがない連続範囲として取り出す。

use core::ops::Range;

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
        let first_word = first >> 6;
        if first_word == last_word {
            let lo = first & 63;
            let hi = last & 63;
            let mask = (u64::MAX << lo) & (u64::MAX >> (63 - hi));
            if let Some(slot) = self.words.get_mut(first_word) {
                *slot |= mask;
            }
            return;
        }
        if let Some(slot) = self.words.get_mut(first_word) {
            *slot |= u64::MAX << (first & 63);
        }
        if let Some(mid) = self.words.get_mut(first_word + 1..last_word) {
            mid.fill(u64::MAX);
        }
        if let Some(slot) = self.words.get_mut(last_word) {
            *slot |= u64::MAX >> (63 - (last & 63));
        }
    }

    /// 要素数が `new_len` に縮んだときに、末尾の外のフラグを落とす
    pub fn truncate(&mut self, new_len: usize) {
        let keep_blocks = new_len.saturating_add(DIRTY_BLOCK - 1) >> DIRTY_SHIFT;
        let keep_words = keep_blocks.div_ceil(64);
        if self.words.len() > keep_words {
            self.words.truncate(keep_words);
        }
        let rem = keep_blocks & 63;
        if rem != 0
            && let Some(last) = self.words.last_mut()
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

    /// 同じページ（`1 << PAGE_BITS`）内で連続する変更範囲をまとめて返す
    pub fn spans(&self, current_len: usize) -> Vec<Range<usize>> {
        let () = Self::VALID_PAGE_BITS;
        let mut out: Vec<Range<usize>> = Vec::new();
        for (wi, &word_val) in self.words.iter().enumerate() {
            let mut w = word_val;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                w &= w - 1;
                let block = (wi << 6) | bit;
                let start = block << DIRTY_SHIFT;
                if start >= current_len {
                    return out;
                }
                let end = (start + DIRTY_BLOCK).min(current_len);
                if let Some(prev) = out.last_mut()
                    && prev.end == start
                    && (prev.start >> PAGE_BITS) == (start >> PAGE_BITS)
                {
                    prev.end = end;
                } else {
                    out.push(start..end);
                }
            }
        }
        out
    }

    /// 変更された要素の合計数
    pub fn dirty_elements(&self, current_len: usize) -> usize {
        self.spans(current_len).into_iter().map(|r| r.len()).sum()
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
}
