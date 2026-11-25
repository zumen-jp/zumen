//! 生存スロットの順序番号（`rank`）と逆引き（`select`）を高速に求めるビット集合。
//!
//! 固定キーを持たずレコード番号で要素の ID が決まる形式（JWW やハンドルなしの DXF など）で、
//! 途中のレコードの削除・末尾への追加があっても全行を走査せずに現在の直列レコード番号と元のスロット番号を相互に引く。

/// 1 ブロックあたりの 64 ビット語数（64 語 = 4,096 ビット）
const WORDS_PER_BLOCK: usize = 64;

/// `rank`（先頭からの 1 の個数）と `select`（`k` 番目の 1 の位置）を持つビット集合
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RankBitset {
    words: Vec<u64>,
    /// ブロック `b` の開始時点（語 `0 .. b * 64`）での 1 の累積個数。長さは `words.div_ceil(64) + 1`
    block_prefix: Vec<u32>,
    len: usize,
}

impl RankBitset {
    /// 空のビット集合を作る
    pub fn new() -> Self {
        Self {
            words: Vec::new(),
            block_prefix: vec![0],
            len: 0,
        }
    }

    /// 長さ `len` で、すべてのビットが `1` のビット集合を作る
    pub fn all_ones(len: usize) -> Self {
        if len == 0 {
            return Self::new();
        }
        let word_len = len.div_ceil(64);
        let mut words = vec![u64::MAX; word_len];
        let rem = len % 64;
        if rem != 0
            && let Some(last) = words.last_mut()
        {
            *last = (1u64 << rem) - 1;
        }
        let mut set = Self {
            words,
            block_prefix: Vec::new(),
            len,
        };
        set.rebuild_prefix();
        set
    }

    /// 長さ `len` で、すべてのビットが `0` のビット集合を作る
    pub fn all_zeros(len: usize) -> Self {
        if len == 0 {
            return Self::new();
        }
        let word_len = len.div_ceil(64);
        let block_len = word_len.div_ceil(WORDS_PER_BLOCK);
        Self {
            words: vec![0; word_len],
            block_prefix: vec![0; block_len + 1],
            len,
        }
    }

    /// ビット列の長さ
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// 空か
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 全体に含まれる `1` の総数
    #[inline]
    pub fn count_ones(&self) -> u32 {
        self.block_prefix.last().copied().unwrap_or(0)
    }

    /// 保持しているヒープの概算バイト数
    pub fn resident_bytes(&self) -> usize {
        self.words.capacity() * core::mem::size_of::<u64>()
            + self.block_prefix.capacity() * core::mem::size_of::<u32>()
    }

    /// 位置 `pos` のビットを返す。範囲外は `false`
    #[inline]
    pub fn get(&self, pos: usize) -> bool {
        if pos >= self.len {
            return false;
        }
        let word = pos / 64;
        let bit = pos % 64;
        self.words.get(word).is_some_and(|w| (w >> bit) & 1 != 0)
    }

    /// 位置 `pos` のビットを `value` に更新し、変わったかどうかを返す
    pub fn set(&mut self, pos: usize, value: bool) -> bool {
        if pos >= self.len {
            return false;
        }
        let word_idx = pos / 64;
        let bit = pos % 64;
        let mask = 1u64 << bit;
        let Some(word) = self.words.get_mut(word_idx) else {
            return false;
        };
        let was = (*word & mask) != 0;
        if was == value {
            return false;
        }
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        let block = word_idx / WORDS_PER_BLOCK;
        if let Some(tail) = self.block_prefix.get_mut(block + 1..) {
            for sum in tail {
                if value {
                    *sum = sum.saturating_add(1);
                } else {
                    *sum = sum.saturating_sub(1);
                }
            }
        }
        true
    }

    /// 末尾に 1 ビット追加する
    pub fn push(&mut self, value: bool) {
        let pos = self.len;
        let word_idx = pos / 64;
        let bit = pos % 64;
        if word_idx == self.words.len() {
            self.words.push(0);
            let needed_blocks = self.words.len().div_ceil(WORDS_PER_BLOCK);
            while self.block_prefix.len() < needed_blocks + 1 {
                let cur_total = self.count_ones();
                self.block_prefix.push(cur_total);
            }
        }
        self.len += 1;
        if value {
            if let Some(word) = self.words.get_mut(word_idx) {
                *word |= 1u64 << bit;
            }
            let block = word_idx / WORDS_PER_BLOCK;
            if let Some(tail) = self.block_prefix.get_mut(block + 1..) {
                for sum in tail {
                    *sum = sum.saturating_add(1);
                }
            }
        }
    }

    /// 末尾の 1 ビットを取り除く
    pub fn pop(&mut self) -> Option<bool> {
        if self.len == 0 {
            return None;
        }
        let last = self.len - 1;
        let was = self.get(last);
        if was {
            let _ = self.set(last, false);
        }
        self.len = last;
        let needed_words = self.len.div_ceil(64);
        self.words.truncate(needed_words);
        let needed_blocks = needed_words.div_ceil(WORDS_PER_BLOCK);
        self.block_prefix.truncate(needed_blocks + 1);
        if self.block_prefix.is_empty() {
            self.block_prefix.push(0);
        }
        Some(was)
    }

    /// 区間 `0 .. pos` に含まれる `1` の個数を返す（`pos >= len` のときは全体の `1` の総数）
    pub fn rank(&self, pos: usize) -> u32 {
        if pos >= self.len {
            return self.count_ones();
        }
        let word_idx = pos / 64;
        let bit = pos % 64;
        let block = word_idx / WORDS_PER_BLOCK;
        let mut sum = self.block_prefix.get(block).copied().unwrap_or(0);
        let block_start = block * WORDS_PER_BLOCK;
        if let Some(slice) = self.words.get(block_start..word_idx) {
            for &w in slice {
                sum += w.count_ones();
            }
        }
        if bit > 0
            && let Some(&w) = self.words.get(word_idx)
        {
            let mask = (1u64 << bit) - 1;
            sum += (w & mask).count_ones();
        }
        sum
    }

    /// `0` 始まりで `k` 番目の `1` がある位置を返す（存在しなければ `None`）
    pub fn select(&self, k: u32) -> Option<usize> {
        if k >= self.count_ones() {
            return None;
        }
        let block_count = self.block_prefix.len().saturating_sub(1);
        if block_count == 0 {
            return None;
        }
        // `block_prefix[b] <= k < block_prefix[b + 1]` となるブロック `b` を二分探索する
        let mut lo = 0usize;
        let mut hi = block_count;
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if self.block_prefix.get(mid).copied().unwrap_or(0) <= k {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let mut rem = k - self.block_prefix.get(lo).copied().unwrap_or(0);
        let word_start = lo * WORDS_PER_BLOCK;
        let word_end = (word_start + WORDS_PER_BLOCK).min(self.words.len());
        let slice = self.words.get(word_start..word_end)?;
        for (offset, &w) in slice.iter().enumerate() {
            let ones = w.count_ones();
            if rem < ones {
                let mut cur = w;
                for _ in 0..rem {
                    cur &= cur - 1;
                }
                let bit = cur.trailing_zeros() as usize;
                let pos = (word_start + offset) * 64 + bit;
                return (pos < self.len).then_some(pos);
            }
            rem -= ones;
        }
        None
    }

    fn rebuild_prefix(&mut self) {
        let block_count = self.words.len().div_ceil(WORDS_PER_BLOCK);
        self.block_prefix.clear();
        self.block_prefix.reserve(block_count + 1);
        let mut acc = 0u32;
        self.block_prefix.push(0);
        for chunk in self.words.chunks(WORDS_PER_BLOCK) {
            for &w in chunk {
                acc += w.count_ones();
            }
            self.block_prefix.push(acc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RankBitset;

    #[test]
    fn rank_and_select_round_trip_across_block_boundaries() {
        let n = 10_000usize;
        let mut bits = RankBitset::all_ones(n);
        assert_eq!(bits.count_ones(), n as u32);
        for i in (0..n).step_by(3) {
            assert!(bits.set(i, false));
        }
        for _ in 0..137 {
            bits.push(true);
            bits.push(false);
        }
        let mut alive = Vec::new();
        for pos in 0..bits.len() {
            assert_eq!(bits.rank(pos), alive.len() as u32);
            if bits.get(pos) {
                alive.push(pos);
            }
        }
        assert_eq!(bits.count_ones(), alive.len() as u32);
        for (k, &expected_pos) in alive.iter().enumerate() {
            assert_eq!(bits.select(k as u32), Some(expected_pos));
        }
        assert_eq!(bits.select(alive.len() as u32), None);

        for _ in 0..274 {
            let _ = bits.pop();
        }
        assert_eq!(bits.len(), n);
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// `RankBitset` の任意ビット列（6 bit）と 1 回の `set`・`push`・`pop` について、
    /// `get`・`rank`・`select`・`count_ones` が素朴な bool 配列と完全一致することを証明する。
    #[kani::proof]
    #[kani::unwind(10)]
    fn verify_rank_and_select_match_naive_model() {
        let mut bs = RankBitset::all_zeros(6);
        let mut model = [false; 7];
        let mut len = 6usize;
        for i in 0..6usize {
            let v: bool = kani::any();
            bs.set(i, v);
            model[i] = v;
        }

        // 任意の 1 操作（set / push / pop）
        let op: u8 = kani::any();
        kani::assume(op < 3);
        match op {
            0 => {
                let pos: usize = kani::any();
                let val: bool = kani::any();
                kani::assume(pos <= 7);
                let changed = bs.set(pos, val);
                if pos < len {
                    assert_eq!(changed, model[pos] != val);
                    model[pos] = val;
                } else {
                    assert!(!changed);
                }
            }
            1 => {
                let val: bool = kani::any();
                bs.push(val);
                model[len] = val;
                len += 1;
            }
            _ => {
                let popped = bs.pop();
                len -= 1;
                assert_eq!(popped, Some(model[len]));
                model[len] = false;
            }
        }

        assert_eq!(bs.len(), len);
        let mut total_ones = 0u32;
        for i in 0..len {
            if model[i] {
                total_ones += 1;
            }
        }
        assert_eq!(bs.count_ones(), total_ones);

        // 全称プローブ: 任意の位置 probe と任意の順位 k について rank / select の完全一致を証明
        let probe: usize = kani::any();
        kani::assume(probe <= len + 1);
        let mut expected_rank = 0u32;
        for i in 0..probe.min(len) {
            if model[i] {
                expected_rank += 1;
            }
        }
        assert_eq!(bs.rank(probe), expected_rank);
        if probe < len {
            assert_eq!(bs.get(probe), model[probe]);
        } else {
            assert!(!bs.get(probe));
        }

        let k: u32 = kani::any();
        kani::assume(k <= total_ones + 1);
        let mut seen = 0u32;
        let mut expected_select = None;
        for i in 0..len {
            if model[i] {
                if seen == k {
                    expected_select = Some(i);
                    break;
                }
                seen += 1;
            }
        }
        assert_eq!(bs.select(k), expected_select);
    }
}
