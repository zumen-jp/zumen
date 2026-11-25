//! 4,096 行などの固定長の塊ごとに `Arc` で共有し、書き換える塊だけを写す列（Copy-on-Write 列）。
//!
//! 文書エンジンの「版」を支える部品（`docs/live.md`）。版の写し（`Clone`）を取るときは塊へのポインター配列だけを
//! 複製するため、100 万行の列でも数マイクロ秒で終わる。後ろで動く保存・書き出し・検査が前の版を読んでいる間に
//! 次のトランザクションが 1 行を書き換えても、その行が属する 1 つの塊（既定で 4,096 行）だけが複製され、
//! 残りの塊は共有されたままになる。

use core::fmt;
use std::sync::Arc;

use zumen_base::{Cx, Kind, Stop};

/// 2 のべき乗の大きさ（`1 << BITS`、既定は `1 << 12 = 4,096` 行）の塊ごとに `Arc` で共有する列
pub struct CowColumn<T, const BITS: usize = 12> {
    chunks: Vec<Arc<Vec<T>>>,
    len: usize,
}

impl<T, const BITS: usize> Clone for CowColumn<T, BITS> {
    fn clone(&self) -> Self {
        Self {
            chunks: self.chunks.clone(),
            len: self.len,
        }
    }
}

impl<T, const BITS: usize> Default for CowColumn<T, BITS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug, const BITS: usize> fmt::Debug for CowColumn<T, BITS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: PartialEq, const BITS: usize> PartialEq for CowColumn<T, BITS> {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        self.chunks
            .iter()
            .zip(other.chunks.iter())
            .all(|(a, b)| Arc::ptr_eq(a, b) || a.as_slice() == b.as_slice())
    }
}

impl<T: Eq, const BITS: usize> Eq for CowColumn<T, BITS> {}

impl<T, const BITS: usize> CowColumn<T, BITS> {
    /// 1 つの塊の要素数（既定は 4,096）
    pub const CHUNK: usize = 1 << BITS;
    const MASK: usize = Self::CHUNK - 1;

    /// 空の列を作る
    pub const fn new() -> Self {
        Self {
            chunks: Vec::new(),
            len: 0,
        }
    }

    /// およそ `count` 行分の塊ポインター配列を予約して作る
    pub fn with_capacity(count: usize) -> Self {
        let chunks = count.saturating_add(Self::MASK) >> BITS;
        Self {
            chunks: Vec::with_capacity(chunks),
            len: 0,
        }
    }

    /// 要素の数
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// 空か
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 塊の数
    #[inline]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// `index` 番めの要素を借りる
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.chunks.get(index >> BITS)?.get(index & Self::MASK)
    }

    /// 最後の要素を借りる
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.get(self.len.checked_sub(1)?)
    }

    /// 同じ塊の中にある範囲 `start..end` のスライスをコピーなしで借りる
    #[inline]
    pub fn chunk_slice(&self, start: usize, end: usize) -> Option<&[T]> {
        let end = end.min(self.len);
        if start >= end {
            return Some(&[]);
        }
        let chunk = start >> BITS;
        let last = end.checked_sub(1)?;
        if (last >> BITS) != chunk {
            return None;
        }
        let offset = start & Self::MASK;
        let len = end - start;
        self.chunks.get(chunk)?.get(offset..offset + len)
    }

    /// 要素を順にたどる
    #[inline]
    pub fn iter(&self) -> CowIter<'_, T> {
        CowIter {
            chunks: self.chunks.iter(),
            current: [].iter(),
            remaining: self.len,
        }
    }

    /// 別の [`CowColumn`] とポインターを共有している塊の数
    pub fn shared_chunks_with(&self, other: &Self) -> usize {
        self.chunks
            .iter()
            .zip(other.chunks.iter())
            .filter(|(a, b)| Arc::ptr_eq(a, b))
            .count()
    }

    /// 入っている要素の合計バイト数
    #[inline]
    pub const fn bytes(&self) -> usize {
        self.len.saturating_mul(core::mem::size_of::<T>())
    }
}

impl<T: Clone, const BITS: usize> CowColumn<T, BITS> {
    /// `index` 番めの要素を書き換えるために借りる（ほかの版と共有している塊はこの時点で複製される）
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        let chunk_arc = self.chunks.get_mut(index >> BITS)?;
        Arc::make_mut(chunk_arc).get_mut(index & Self::MASK)
    }

    /// `index` 番めの要素を置き換え、前の値を返す
    #[inline]
    pub fn set(&mut self, index: usize, value: T) -> Option<T> {
        let slot = self.get_mut(index)?;
        Some(core::mem::replace(slot, value))
    }

    /// 終わりに 1 つ足す
    #[inline]
    pub fn push(&mut self, value: T) {
        let chunk_idx = self.len >> BITS;
        if chunk_idx == self.chunks.len() {
            let mut chunk = Vec::with_capacity(Self::CHUNK);
            chunk.push(value);
            self.chunks.push(Arc::new(chunk));
        } else if let Some(chunk_arc) = self.chunks.get_mut(chunk_idx) {
            let vec = Arc::make_mut(chunk_arc);
            if vec.capacity() < Self::CHUNK {
                vec.reserve(Self::CHUNK - vec.len());
            }
            vec.push(value);
        }
        self.len += 1;
    }

    /// 予算を確かめてから終わりに 1 つ足す（塊の新設または共有塊の複製が起きる分を計上する）
    pub fn push_with_cx(&mut self, value: T, cx: &mut Cx<'_>) -> Result<(), Stop> {
        let chunk_idx = self.len >> BITS;
        let chunk_bytes = (Self::CHUNK as u64).saturating_mul(core::mem::size_of::<T>() as u64);
        let needs_alloc = chunk_idx == self.chunks.len()
            || self
                .chunks
                .get(chunk_idx)
                .is_some_and(|arc| Arc::strong_count(arc) > 1);
        if needs_alloc {
            cx.check(Kind::DecodedBytes, chunk_bytes)?;
        }
        self.push(value);
        Ok(())
    }

    /// 終わりの要素を取り出す
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        let next_len = self.len.checked_sub(1)?;
        self.len = next_len;
        let chunk_idx = self.len >> BITS;
        let out = Arc::make_mut(self.chunks.get_mut(chunk_idx)?).pop();
        if self.chunks.get(chunk_idx).is_some_and(|c| c.is_empty()) {
            self.chunks.pop();
        }
        out
    }

    /// 先頭から `new_len` 個だけ残す
    pub fn truncate(&mut self, new_len: usize) {
        if new_len >= self.len {
            return;
        }
        self.len = new_len;
        let keep = new_len.saturating_add(Self::MASK) >> BITS;
        self.chunks.truncate(keep);
        let rem = new_len & Self::MASK;
        if rem != 0
            && let Some(last_arc) = self.chunks.last_mut()
        {
            Arc::make_mut(last_arc).truncate(rem);
        }
    }

    /// 空にする
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// `value` を `count` 個並べた列を作る
    pub fn filled(value: T, count: usize) -> Self {
        let mut out = Self::with_capacity(count);
        let mut rem = count;
        while rem > 0 {
            let take = rem.min(Self::CHUNK);
            let mut chunk = Vec::with_capacity(Self::CHUNK);
            chunk.resize(take, value.clone());
            out.chunks.push(Arc::new(chunk));
            out.len += take;
            rem -= take;
        }
        out
    }

    /// 予算を確かめてから、`value` を `count` 個並べた列を作る
    pub fn filled_with_cx(value: T, count: usize, cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let chunks = count.saturating_add(Self::MASK) >> BITS;
        let bytes = (chunks as u64)
            .saturating_mul((Self::CHUNK as u64).saturating_mul(core::mem::size_of::<T>() as u64));
        cx.check(Kind::DecodedBytes, bytes)?;
        Ok(Self::filled(value, count))
    }

    /// スライスから列を作る
    pub fn from_slice_with_cx(slice: &[T], cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let chunks = slice.len().saturating_add(Self::MASK) >> BITS;
        let bytes = (chunks as u64)
            .saturating_mul((Self::CHUNK as u64).saturating_mul(core::mem::size_of::<T>() as u64));
        cx.check(Kind::DecodedBytes, bytes)?;
        let mut out = Self::with_capacity(slice.len());
        for part in slice.chunks(Self::CHUNK) {
            let mut chunk = Vec::with_capacity(Self::CHUNK);
            chunk.extend_from_slice(part);
            out.chunks.push(Arc::new(chunk));
            out.len += part.len();
        }
        Ok(out)
    }
}

impl<T: Clone, const BITS: usize> FromIterator<T> for CowColumn<T, BITS> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (low, _) = iter.size_hint();
        let mut out = Self::with_capacity(low);
        for item in iter {
            out.push(item);
        }
        out
    }
}

impl<'a, T, const BITS: usize> IntoIterator for &'a CowColumn<T, BITS> {
    type Item = &'a T;
    type IntoIter = CowIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// [`CowColumn`] の要素を順に返す反復子
pub struct CowIter<'a, T> {
    chunks: core::slice::Iter<'a, Arc<Vec<T>>>,
    current: core::slice::Iter<'a, T>,
    remaining: usize,
}

impl<T: fmt::Debug> fmt::Debug for CowIter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CowIter")
            .field("remaining", &self.remaining)
            .finish()
    }
}

impl<'a, T> Iterator for CowIter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.current.next() {
            self.remaining = self.remaining.saturating_sub(1);
            return Some(item);
        }
        let chunk = self.chunks.next()?;
        self.current = chunk.iter();
        let item = self.current.next()?;
        self.remaining = self.remaining.saturating_sub(1);
        Some(item)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for CowIter<'_, T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cow_column_copies_only_mutated_chunk_and_isolates_snapshots() {
        let mut col: CowColumn<u64, 6> = (0..640u64).collect(); // 1 塊 64 行 × 10 塊
        assert_eq!(col.chunk_count(), 10);

        let snap = col.clone();
        assert_eq!(col.shared_chunks_with(&snap), 10);

        // 塊 3（行 200）だけを書き換える
        let prev = col.set(200, 999_999);
        assert_eq!(prev, Some(200));
        assert_eq!(col.shared_chunks_with(&snap), 9);
        assert_eq!(snap.get(200), Some(&200));
        assert_eq!(col.get(200), Some(&999_999));

        // 末尾に足して取り除いても、前の版は影響を受けない
        col.push(640);
        assert_eq!(col.len(), 641);
        assert_eq!(snap.len(), 640);
        assert_eq!(col.pop(), Some(640));
        assert_eq!(col.len(), 640);
    }

    #[test]
    fn out_of_bounds_get_mut_does_not_clone_shared_tail_chunk() {
        let mut col: CowColumn<u64, 2> = (0..5u64).collect(); // 2 塊（[0..4], [4]）
        let snap = col.clone();
        assert_eq!(col.shared_chunks_with(&snap), 2);
        assert_eq!(col.get_mut(5), None);
        assert_eq!(col.set(6, 99), None);
        assert_eq!(col.shared_chunks_with(&snap), 2);
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// `CowColumn<u16, 1>`（1 塊 2 要素）について、`Clone` スナップショットの隔離と
    /// 書き換えていない塊の `Arc` 共有が保たれることを証明する。
    #[kani::proof]
    #[kani::unwind(6)]
    fn verify_cow_column_set_and_snapshot_isolation() {
        let init: [u16; 3] = [kani::any(), kani::any(), kani::any()];
        let mut col: CowColumn<u16, 1> = CowColumn::with_capacity(4);
        col.push(init[0]);
        col.push(init[1]);
        col.push(init[2]);
        let snap = col.clone();
        assert_eq!(col.shared_chunks_with(&snap), 2);

        let idx: usize = kani::any();
        let new_val: u16 = kani::any();
        kani::assume(idx <= 4);

        let mut expected = init;
        let prev = col.set(idx, new_val);
        if idx < 3 {
            assert_eq!(prev, Some(init[idx]));
            expected[idx] = new_val;
            // 2 塊のうち 1 塊だけを複製し、もう 1 塊は共有されたままになる
            assert_eq!(col.shared_chunks_with(&snap), 1);
        } else {
            assert_eq!(prev, None);
            assert_eq!(col.shared_chunks_with(&snap), 2);
        }

        for i in 0..3 {
            assert_eq!(col.get(i), Some(&expected[i]));
            assert_eq!(snap.get(i), Some(&init[i]));
        }
    }

    /// `CowColumn<u16, 1>` の `chunk_slice` が同一塊内のみスライスを返し、
    /// 塊境界をまたぐときは `None` を返すことを全 `(start, end)` で証明する。
    #[kani::proof]
    #[kani::unwind(6)]
    fn verify_cow_column_chunk_slice_boundaries() {
        let init: [u16; 3] = [kani::any(), kani::any(), kani::any()];
        let mut col: CowColumn<u16, 1> = CowColumn::with_capacity(4);
        col.push(init[0]);
        col.push(init[1]);
        col.push(init[2]);

        let start: usize = kani::any();
        let end: usize = kani::any();
        kani::assume(start <= 5 && end <= 5);
        let eff_end = end.min(3);
        let slice = col.chunk_slice(start, end);
        if start >= eff_end {
            assert_eq!(slice, Some(&[][..]));
        } else if (start >> 1) == ((eff_end - 1) >> 1) {
            assert_eq!(slice, Some(&init[start..eff_end]));
        } else {
            assert_eq!(slice, None);
        }
    }

    /// `CowColumn<u16, 1>` の `truncate` が任意の `new_len` で要素・長さ・スナップショット隔離を保つことを証明する。
    #[kani::proof]
    #[kani::unwind(6)]
    fn verify_cow_column_truncate_preserves_snapshot() {
        let init: [u16; 3] = [kani::any(), kani::any(), kani::any()];
        let mut col: CowColumn<u16, 1> = CowColumn::with_capacity(4);
        col.push(init[0]);
        col.push(init[1]);
        col.push(init[2]);
        let snap = col.clone();

        let new_len: usize = kani::any();
        kani::assume(new_len <= 4);
        col.truncate(new_len);
        let expected_len = 3usize.min(new_len);
        assert_eq!(col.len(), expected_len);
        for i in 0..expected_len {
            assert_eq!(col.get(i), Some(&init[i]));
        }
        assert_eq!(col.get(expected_len), None);
        assert_eq!(snap.len(), 3);
        for i in 0..3 {
            assert_eq!(snap.get(i), Some(&init[i]));
        }
    }
}
