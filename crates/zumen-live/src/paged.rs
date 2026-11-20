//! 固定長のページに分けた伸びる配列と、64 KiB の塊に分けた文字列の置き場。
//!
//! 100 万要素の図面を開いた直後に要素を 1 つ足しても、既存の列・表示バッファー・空間索引を写し直さないように、
//! 伸びる配列を 2 のべき乗の大きさ（既定で 1,024 要素）のページに分けて持つ。
//! [`BytePool`] の各塊は `Arc<Vec<u8>>` で持ち、版の写しを取るときも確定済みの 64 KiB 塊を共有する。

use core::fmt;
use std::sync::Arc;

use zumen_base::{Cx, Kind, Stop};

/// 2 のべき乗の大きさ（`1 << BITS`）のページに分けた配列
#[derive(Clone)]
pub struct Paged<T, const BITS: usize = 10> {
    pages: Vec<Vec<T>>,
    len: usize,
}

impl<T, const BITS: usize> Default for Paged<T, BITS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: fmt::Debug, const BITS: usize> fmt::Debug for Paged<T, BITS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: PartialEq, const BITS: usize> PartialEq for Paged<T, BITS> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl<T: Eq, const BITS: usize> Eq for Paged<T, BITS> {}

impl<T, const BITS: usize> Paged<T, BITS> {
    /// 1 ページの要素数
    pub const PAGE: usize = 1 << BITS;
    const MASK: usize = Self::PAGE - 1;

    /// 空の配列を作る
    pub const fn new() -> Self {
        Self {
            pages: Vec::new(),
            len: 0,
        }
    }

    /// およそ `count` 要素分の外側のページ表だけ予約して作る。各ページは、入れるときに 1 ページ分だけ確保する
    pub fn with_capacity(count: usize) -> Self {
        let pages = count.saturating_add(Self::MASK) >> BITS;
        Self {
            pages: Vec::with_capacity(pages),
            len: 0,
        }
    }

    /// 予算を確かめてから、およそ `count` 要素分のページ表を予約して作る
    pub fn with_capacity_in(count: usize, cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let pages = count.saturating_add(Self::MASK) >> BITS;
        let table_bytes = (pages as u64).saturating_mul(core::mem::size_of::<Vec<T>>() as u64);
        cx.check(Kind::DecodedBytes, table_bytes)?;
        Ok(Self {
            pages: Vec::with_capacity(pages),
            len: 0,
        })
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

    /// `index` 番めの要素を借りる
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        self.pages.get(index >> BITS)?.get(index & Self::MASK)
    }

    /// `index` 番めの要素を書き換えるために借りる
    #[inline]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.pages
            .get_mut(index >> BITS)?
            .get_mut(index & Self::MASK)
    }

    /// 最後の要素
    #[inline]
    pub fn last(&self) -> Option<&T> {
        self.get(self.len.checked_sub(1)?)
    }

    /// 最後の要素を書き換えるために借りる
    #[inline]
    pub fn last_mut(&mut self) -> Option<&mut T> {
        let at = self.len.checked_sub(1)?;
        self.get_mut(at)
    }

    /// 終わりに 1 つ足す。既存のページは再確保しない
    #[inline]
    pub fn push(&mut self, value: T) {
        let page = self.len >> BITS;
        if page == self.pages.len() {
            let mut chunk = Vec::with_capacity(Self::PAGE);
            chunk.push(value);
            self.pages.push(chunk);
        } else if let Some(chunk) = self.pages.get_mut(page) {
            chunk.push(value);
        }
        self.len += 1;
    }

    /// 新しいページを確保するときだけ予算を確かめて、終わりに 1 つ足す
    #[inline]
    pub fn push_with_cx(&mut self, value: T, cx: &mut Cx<'_>) -> Result<(), Stop> {
        let page = self.len >> BITS;
        if page == self.pages.len() {
            let page_bytes = (Self::PAGE as u64).saturating_mul(core::mem::size_of::<T>() as u64);
            cx.check(Kind::DecodedBytes, page_bytes)?;
        }
        self.push(value);
        Ok(())
    }

    /// 終わりの要素を取り出す
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        let next_len = self.len.checked_sub(1)?;
        self.len = next_len;
        let page = self.len >> BITS;
        let out = self.pages.get_mut(page)?.pop();
        if self.pages.get(page).is_some_and(Vec::is_empty) {
            self.pages.pop();
        }
        out
    }

    /// `index` の要素を取り出し、最後の要素をそこへ移す（`O(1)`）
    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }
        let last_idx = self.len.checked_sub(1)?;
        let last = self.pop()?;
        if index == last_idx {
            return Some(last);
        }
        let slot = self.get_mut(index)?;
        Some(core::mem::replace(slot, last))
    }

    /// 先頭から `new_len` 個だけ残す
    pub fn truncate(&mut self, new_len: usize) {
        if new_len >= self.len {
            return;
        }
        self.len = new_len;
        let keep = new_len.saturating_add(Self::MASK) >> BITS;
        self.pages.truncate(keep);
        let rem = new_len & Self::MASK;
        if rem != 0
            && let Some(last) = self.pages.last_mut()
        {
            last.truncate(rem);
        }
    }

    /// 空にする
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// ページのスライスの列。GPU の頂点バッファーへページ単位で転送するときにも使う
    #[inline]
    pub fn pages(&self) -> &[Vec<T>] {
        &self.pages
    }

    /// 同じページの中にある範囲 `start..end` のスライスをコピーなしで借りる
    #[inline]
    pub fn page_slice(&self, start: usize, end: usize) -> Option<&[T]> {
        let end = end.min(self.len);
        if start >= end {
            return Some(&[]);
        }
        let page = start >> BITS;
        let last = end.checked_sub(1)?;
        if (last >> BITS) != page {
            return None;
        }
        let offset = start & Self::MASK;
        let len = end - start;
        self.pages.get(page)?.get(offset..offset + len)
    }

    /// 要素を順にたどる
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            pages: self.pages.iter(),
            current: [].iter(),
            remaining: self.len,
        }
    }

    /// 入っている要素の合計バイト数
    #[inline]
    pub const fn bytes(&self) -> usize {
        self.len.saturating_mul(core::mem::size_of::<T>())
    }

    /// ページ表を含めて確保している合計バイト数
    pub fn allocated_bytes(&self) -> usize {
        self.pages
            .capacity()
            .saturating_mul(core::mem::size_of::<Vec<T>>())
            .saturating_add(
                self.pages
                    .iter()
                    .map(|page| page.capacity().saturating_mul(core::mem::size_of::<T>()))
                    .sum::<usize>(),
            )
    }
}

impl<T: Copy, const BITS: usize> Paged<T, BITS> {
    /// `value` を `count` 個並べた配列を作る
    pub fn filled(value: T, count: usize) -> Self {
        let mut out = Self::with_capacity(count);
        out.resize(count, value);
        out
    }

    /// 予算を確かめてから、`value` を `count` 個並べた配列を作る
    pub fn filled_with_cx(value: T, count: usize, cx: &mut Cx<'_>) -> Result<Self, Stop> {
        let pages = count.saturating_add(Self::MASK) >> BITS;
        let alloc_bytes = (pages as u64)
            .saturating_mul((Self::PAGE as u64).saturating_mul(core::mem::size_of::<T>() as u64))
            .saturating_add((pages as u64).saturating_mul(core::mem::size_of::<Vec<T>>() as u64));
        cx.check(Kind::DecodedBytes, alloc_bytes)?;
        Ok(Self::filled(value, count))
    }

    /// 長さを `new_len` に変え、伸ばす分は `value` で埋める
    #[inline]
    pub fn resize(&mut self, new_len: usize, value: T) {
        if new_len == self.len.wrapping_add(1) && self.len < usize::MAX {
            self.push(value);
            return;
        }
        if new_len <= self.len {
            self.truncate(new_len);
            return;
        }
        let mut add = new_len - self.len;
        while add > 0 {
            let page = self.len >> BITS;
            let used = self.len & Self::MASK;
            let room = Self::PAGE - used;
            let take = add.min(room);
            if page == self.pages.len() {
                let mut chunk = Vec::with_capacity(Self::PAGE);
                chunk.resize(take, value);
                self.pages.push(chunk);
            } else if let Some(chunk) = self.pages.get_mut(page) {
                chunk.resize(used + take, value);
            }
            self.len += take;
            add -= take;
        }
    }

    /// スライスの中身を続けて足す
    pub fn extend_from_slice(&mut self, mut slice: &[T]) {
        while !slice.is_empty() {
            let page = self.len >> BITS;
            let used = self.len & Self::MASK;
            let room = Self::PAGE - used;
            let take = slice.len().min(room);
            let (head, rest) = slice.split_at(take);
            if page == self.pages.len() {
                let mut chunk = Vec::with_capacity(Self::PAGE);
                chunk.extend_from_slice(head);
                self.pages.push(chunk);
            } else if let Some(chunk) = self.pages.get_mut(page) {
                chunk.extend_from_slice(head);
            }
            self.len += take;
            slice = rest;
        }
    }

    /// 別の [`Paged`] の中身を後ろにつなぐ。ページ境界がそろっていればページごと移す
    pub fn extend_paged(&mut self, mut other: Self) {
        if other.is_empty() {
            return;
        }
        if (self.len & Self::MASK) == 0 {
            self.len += other.len;
            self.pages.append(&mut other.pages);
            return;
        }
        for page in other.pages {
            self.extend_from_slice(&page);
        }
    }

    /// `start..end` の範囲を `value` で埋める
    pub fn fill_range(&mut self, mut start: usize, end: usize, value: T) {
        let end = end.min(self.len);
        while start < end {
            let page = start >> BITS;
            let offset = start & Self::MASK;
            let take = (end - start).min(Self::PAGE - offset);
            if let Some(chunk) = self.pages.get_mut(page)
                && let Some(sub) = chunk.get_mut(offset..offset + take)
            {
                sub.fill(value);
            }
            start += take;
        }
    }

    /// `start` から `src.len()` 個を `src` で上書きする
    pub fn copy_range_from_slice(&mut self, mut start: usize, mut src: &[T]) {
        while !src.is_empty() && start < self.len {
            let page = start >> BITS;
            let offset = start & Self::MASK;
            let take = src.len().min(Self::PAGE - offset).min(self.len - start);
            let (head, rest) = src.split_at(take);
            if let Some(chunk) = self.pages.get_mut(page)
                && let Some(sub) = chunk.get_mut(offset..offset + take)
            {
                sub.copy_from_slice(head);
            }
            start += take;
            src = rest;
        }
    }

    /// `start..end` の範囲を新しい `Vec` に写す
    pub fn slice_to_vec(&self, mut start: usize, end: usize) -> Vec<T> {
        let end = end.min(self.len);
        if start >= end {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(end - start);
        while start < end {
            let page = start >> BITS;
            let offset = start & Self::MASK;
            let take = (end - start).min(Self::PAGE - offset);
            if let Some(chunk) = self.pages.get(page)
                && let Some(sub) = chunk.get(offset..offset + take)
            {
                out.extend_from_slice(sub);
            } else {
                break;
            }
            start += take;
        }
        out
    }
}

impl<T, const BITS: usize> Extend<T> for Paged<T, BITS> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.push(item);
        }
    }
}

impl<T, const BITS: usize> FromIterator<T> for Paged<T, BITS> {
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

impl<'a, T, const BITS: usize> IntoIterator for &'a Paged<T, BITS> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// [`Paged`] の要素を順に返す反復子
pub struct Iter<'a, T> {
    pages: core::slice::Iter<'a, Vec<T>>,
    current: core::slice::Iter<'a, T>,
    remaining: usize,
}

impl<T: fmt::Debug> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Iter")
            .field("remaining", &self.remaining)
            .finish()
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.current.next() {
            self.remaining = self.remaining.saturating_sub(1);
            return Some(item);
        }
        let page = self.pages.next()?;
        self.current = page.iter();
        let item = self.current.next()?;
        self.remaining = self.remaining.saturating_sub(1);
        Some(item)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

/// [`BytePool`] の中の文字列の位置（8 バイト）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextSpan {
    chunk: u32,
    pos: u32,
}

impl Default for TextSpan {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl TextSpan {
    /// 空文字列（ヒープを使わない）
    pub const EMPTY: Self = Self {
        chunk: u32::MAX,
        pos: 0,
    };

    /// 既定の文字スタイル `STANDARD`（ヒープを使わない）
    pub const STANDARD: Self = Self {
        chunk: u32::MAX - 1,
        pos: 0,
    };

    /// 値がないこと（`Option<TextSpan>` の代わりに 8 バイトで持つ）
    pub const NONE: Self = Self {
        chunk: u32::MAX - 2,
        pos: 0,
    };

    /// 既定のレイヤ名 `0`（ヒープを使わない）
    pub const ZERO: Self = Self {
        chunk: u32::MAX - 3,
        pos: 0,
    };

    const SENTINEL_MIN: u32 = u32::MAX - 3;
    const WHOLE_CHUNK: u32 = 0x0000_8000;

    /// [`Self::NONE`] か
    #[inline]
    pub const fn is_none(self) -> bool {
        self.chunk == Self::NONE.chunk
    }

    /// 別の [`BytePool`] を後ろにつないだときの、塊の番号をずらした位置
    #[inline]
    pub const fn shifted(self, chunk_base: u32) -> Self {
        if self.chunk >= Self::SENTINEL_MIN {
            self
        } else {
            Self {
                chunk: self.chunk.saturating_add(chunk_base),
                pos: self.pos,
            }
        }
    }
}

#[derive(Clone, Debug)]
struct PoolChunk {
    bytes: Arc<Vec<u8>>,
    /// `MAX_INLINE_LEN` を超える 1 文字列専用の塊か（後ろに別の文字列を詰めない）
    whole: bool,
}

/// 64 KiB の塊に分けたバイト列・UTF-8 文字列の置き場。
///
/// 追加しても既存の塊を再確保せず、分割走査の結果もコピーなしで束ねる。
/// 各塊は `Arc<Vec<u8>>` で持つため、版の写し（`Clone`）を取っても確定済みの 64 KiB 塊は共有されたままになる。
#[derive(Clone, Debug)]
pub struct BytePool {
    chunks: Vec<PoolChunk>,
    recent: [TextSpan; 4],
    recent_next: u8,
}

impl Default for BytePool {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for BytePool {
    fn eq(&self, other: &Self) -> bool {
        self.chunks.len() == other.chunks.len()
            && self.chunks.iter().zip(other.chunks.iter()).all(|(a, b)| {
                a.whole == b.whole
                    && (Arc::ptr_eq(&a.bytes, &b.bytes) || a.bytes.as_slice() == b.bytes.as_slice())
            })
    }
}

impl Eq for BytePool {}

impl BytePool {
    const CHUNK_CAP: usize = 65_536;
    const MAX_INLINE_LEN: usize = 0x7FFF;

    /// 空の置き場を作る
    pub const fn new() -> Self {
        Self {
            chunks: Vec::new(),
            recent: [TextSpan::NONE; 4],
            recent_next: 0,
        }
    }

    /// バイト列を置いて位置を返す
    pub fn push(&mut self, bytes: &[u8]) -> Option<TextSpan> {
        if bytes.is_empty() {
            return Some(TextSpan::EMPTY);
        }
        if bytes == b"0" {
            return Some(TextSpan::ZERO);
        }
        if bytes == b"STANDARD" {
            return Some(TextSpan::STANDARD);
        }
        if bytes.len() <= 64 {
            for &cand in &self.recent {
                if !cand.is_none() && self.get(cand) == bytes {
                    return Some(cand);
                }
            }
        }
        if bytes.len() > Self::MAX_INLINE_LEN {
            let chunk = u32::try_from(self.chunks.len()).ok()?;
            if chunk >= TextSpan::SENTINEL_MIN {
                return None;
            }
            self.chunks.push(PoolChunk {
                bytes: Arc::new(bytes.to_vec()),
                whole: true,
            });
            return Some(TextSpan {
                chunk,
                pos: TextSpan::WHOLE_CHUNK,
            });
        }
        let len = bytes.len();
        let reuse = self
            .chunks
            .last()
            .is_some_and(|tail| !tail.whole && tail.bytes.len() + len <= Self::CHUNK_CAP);
        if !reuse {
            let next_chunk = u32::try_from(self.chunks.len()).ok()?;
            if next_chunk >= TextSpan::SENTINEL_MIN {
                return None;
            }
            self.chunks.push(PoolChunk {
                bytes: Arc::new(Vec::with_capacity(Self::CHUNK_CAP)),
                whole: false,
            });
        }
        let last_index = self.chunks.len().checked_sub(1)?;
        let chunk = u32::try_from(last_index).ok()?;
        let tail_chunk = self.chunks.last_mut()?;
        let tail = Arc::make_mut(&mut tail_chunk.bytes);
        if tail.capacity() < Self::CHUNK_CAP && tail.len() + len <= Self::CHUNK_CAP {
            tail.reserve(Self::CHUNK_CAP - tail.len());
        }
        let offset = u32::try_from(tail.len()).ok()?;
        let inline_len = u32::try_from(len).ok()?;
        tail.extend_from_slice(bytes);
        let span = TextSpan {
            chunk,
            pos: (offset << 16) | inline_len,
        };
        if len <= 64 {
            if let Some(slot) = self.recent.get_mut(usize::from(self.recent_next & 3)) {
                *slot = span;
            }
            self.recent_next = self.recent_next.wrapping_add(1);
        }
        Some(span)
    }

    /// 予算を確かめてからバイト列を置く
    pub fn push_with_cx(
        &mut self,
        bytes: &[u8],
        cx: &mut Cx<'_>,
    ) -> Result<Option<TextSpan>, Stop> {
        if !bytes.is_empty() && bytes != b"0" && bytes != b"STANDARD" {
            cx.check(Kind::DecodedBytes, bytes.len() as u64)?;
        }
        Ok(self.push(bytes))
    }

    /// 位置 `span` のバイト列を借りる
    #[inline]
    pub fn get(&self, span: TextSpan) -> &[u8] {
        match span.chunk {
            u32::MAX => return b"",
            x if x == u32::MAX - 1 => return b"STANDARD",
            x if x == u32::MAX - 2 => return b"",
            x if x == u32::MAX - 3 => return b"0",
            _ => {}
        }
        let Ok(index) = usize::try_from(span.chunk) else {
            return b"";
        };
        let Some(chunk) = self.chunks.get(index) else {
            return b"";
        };
        if span.pos == TextSpan::WHOLE_CHUNK {
            return chunk.bytes.as_slice();
        }
        let offset = (span.pos >> 16) as usize;
        let len = (span.pos & 0x7FFF) as usize;
        let Some(end) = offset.checked_add(len) else {
            return b"";
        };
        chunk.bytes.get(offset..end).unwrap_or_default()
    }

    /// 位置 `span` の UTF-8 文字列を借りる
    #[inline]
    pub fn str_of(&self, span: TextSpan) -> &str {
        core::str::from_utf8(self.get(span)).unwrap_or_default()
    }

    /// 別の置き場の塊をそのまま後ろにつなぎ、足した最初の塊の番号を返す（バイト列のコピーなし）
    pub fn append_pool(&mut self, mut other: Self) -> u32 {
        let base = u32::try_from(self.chunks.len()).unwrap_or(u32::MAX);
        self.chunks.append(&mut other.chunks);
        base
    }

    /// 巻き戻し用の位置（塊の数と最後の塊の長さ）
    pub fn marks(&self) -> (usize, usize) {
        (
            self.chunks.len(),
            self.chunks.last().map_or(0, |c| c.bytes.len()),
        )
    }

    /// [`Self::marks`] で記録した状態に戻す
    pub fn truncate(&mut self, (chunks, last_len): (usize, usize)) {
        self.chunks.truncate(chunks);
        if let Some(last_chunk) = self.chunks.last_mut()
            && last_chunk.bytes.len() > last_len
        {
            Arc::make_mut(&mut last_chunk.bytes).truncate(last_len);
        }
        self.recent = [TextSpan::NONE; 4];
    }

    /// 空にする
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.recent = [TextSpan::NONE; 4];
    }

    /// 入っているバイト数の合計
    pub fn len(&self) -> usize {
        self.chunks.iter().map(|c| c.bytes.len()).sum()
    }

    /// 空か
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 別の [`BytePool`] とポインターを共有している塊の数
    pub fn shared_chunks_with(&self, other: &Self) -> usize {
        self.chunks
            .iter()
            .zip(other.chunks.iter())
            .filter(|(a, b)| Arc::ptr_eq(&a.bytes, &b.bytes))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    #[test]
    fn paged_push_pop_swap_remove_and_slices_work_across_pages() {
        let mut p: Paged<u32, 4> = Paged::new(); // 1 ページ 16 要素
        for i in 0..70u32 {
            p.push(i);
        }
        assert_eq!(p.len(), 70);
        assert_eq!(p.pages().len(), 5);
        assert_eq!(
            p.page_slice(16, 32),
            Some(&(16..32).collect::<Vec<_>>()[..])
        );
        assert_eq!(p.page_slice(15, 17), None);
        assert_eq!(p.swap_remove(3), Some(3));
        assert_eq!(p.get(3), Some(&69));
        assert_eq!(p.len(), 69);

        let copied = p.slice_to_vec(10, 40);
        assert_eq!(copied.len(), 30);
        p.fill_range(16, 35, 999);
        assert_eq!(p.get(16), Some(&999));
        assert_eq!(p.get(34), Some(&999));
        p.truncate(18);
        assert_eq!(p.len(), 18);
        assert_eq!(p.pages().len(), 2);
    }

    #[test]
    fn paged_respects_decoded_bytes_budget() {
        let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(64)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut p: Paged<u64, 3> = Paged::new(); // 1 ページ 8 * 8 = 64 B
        for i in 0..8 {
            assert!(p.push_with_cx(i, &mut cx).is_ok());
        }
        // 2 ページめで上限 64 B を超える
        let err = p.push_with_cx(8, &mut cx).unwrap_err();
        assert!(matches!(err, Stop::Exceeded(e) if e.kind == Kind::DecodedBytes));
        assert_eq!(p.len(), 8);
    }

    #[test]
    fn byte_pool_shares_sealed_chunks_across_clones_and_supports_marks() {
        let mut pool = BytePool::new();
        let s_empty = pool.push(b"").unwrap();
        let s_zero = pool.push(b"0").unwrap();
        let s_std = pool.push(b"STANDARD").unwrap();
        assert_eq!(pool.str_of(s_empty), "");
        assert_eq!(pool.str_of(s_zero), "0");
        assert_eq!(pool.str_of(s_std), "STANDARD");

        let big = vec![b'A'; 40_000];
        let s_big = pool.push(&big).unwrap();
        let s_small = pool.push(b"layer-1").unwrap();
        let mark = pool.marks();

        let snap = pool.clone();
        assert_eq!(pool.shared_chunks_with(&snap), 2);

        // 末尾の塊に追記すると末尾の塊だけが写され、確定済みの最初の塊は共有されたままになる
        let s_added = pool.push(b"layer-2").unwrap();
        assert_eq!(pool.shared_chunks_with(&snap), 1);
        assert_eq!(snap.get(s_added), b"");
        assert_eq!(pool.str_of(s_added), "layer-2");

        pool.truncate(mark);
        assert_eq!(pool.get(s_big).len(), 40_000);
        assert_eq!(pool.str_of(s_small), "layer-1");
        assert_eq!(pool.get(s_added), b"");
    }
}
