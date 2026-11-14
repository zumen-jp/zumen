//! 固定長のページに分けた列と、64 KiB の塊に分けたバイト列の置き場。
//!
//! 100 万要素の図面を開いた直後に要素を 1 つ足しても、既存の列・表示バッファー・空間索引を写し直さないように、
//! 伸びる配列を一定の大きさ（既定で 1,024 要素）のページに分けて持つ。

use core::fmt;

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

    /// 終わりの要素を取り出す
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
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
        let last_idx = self.len - 1;
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
        self.len * core::mem::size_of::<T>()
    }

    /// ページ表を含めて確保している合計バイト数
    pub fn allocated_bytes(&self) -> usize {
        self.pages.capacity() * core::mem::size_of::<Vec<T>>()
            + self
                .pages
                .iter()
                .map(|page| page.capacity() * core::mem::size_of::<T>())
                .sum::<usize>()
    }
}

impl<T: Copy, const BITS: usize> Paged<T, BITS> {
    /// `value` を `count` 個並べた配列を作る
    pub fn filled(value: T, count: usize) -> Self {
        let mut out = Self::with_capacity(count);
        out.resize(count, value);
        out
    }

    /// 長さを `new_len` に変え、伸ばす分は `value` で埋める
    pub fn resize(&mut self, new_len: usize, value: T) {
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

    /// 別の [`Paged`] の中身を後ろに繋ぐ。ページ境界が揃っていればページごと移す
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

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.current.next() {
            self.remaining -= 1;
            return Some(item);
        }
        let page = self.pages.next()?;
        self.current = page.iter();
        let item = self.current.next()?;
        self.remaining -= 1;
        Some(item)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

/// [`BytePool`] の中の文字列の位置（8 バイト）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    /// 既定の画層名 `0`（ヒープを使わない）
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

    /// 別の [`BytePool`] を後ろに繋いだときの、塊の番号をずらした位置
    #[inline]
    pub const fn shifted(self, chunk_base: u32) -> Self {
        if self.chunk >= Self::SENTINEL_MIN {
            self
        } else {
            Self {
                chunk: self.chunk + chunk_base,
                pos: self.pos,
            }
        }
    }
}

/// 64 KiB の塊に分けた UTF-8 文字列の置き場。追加しても既存の塊を再確保せず、分割走査の結果もコピーなしで束ねる
#[derive(Clone, Debug)]
pub struct BytePool {
    chunks: Vec<Vec<u8>>,
    recent: [TextSpan; 4],
    recent_next: u8,
}

impl Default for BytePool {
    fn default() -> Self {
        Self {
            chunks: Vec::new(),
            recent: [TextSpan::NONE; 4],
            recent_next: 0,
        }
    }
}

impl PartialEq for BytePool {
    fn eq(&self, other: &Self) -> bool {
        self.chunks == other.chunks
    }
}

impl Eq for BytePool {}

impl BytePool {
    const CHUNK_CAP: usize = 65_536;
    const MAX_INLINE_LEN: usize = 0x7FFF;

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
            self.chunks.push(bytes.to_vec());
            return Some(TextSpan {
                chunk,
                pos: TextSpan::WHOLE_CHUNK,
            });
        }
        let len = bytes.len();
        let reuse = self.chunks.last().is_some_and(|tail| {
            tail.capacity() == Self::CHUNK_CAP && tail.len() + len <= Self::CHUNK_CAP
        });
        if !reuse {
            u32::try_from(self.chunks.len()).ok()?;
            self.chunks.push(Vec::with_capacity(Self::CHUNK_CAP));
        }
        let chunk = (self.chunks.len() - 1) as u32;
        let tail = self.chunks.last_mut()?;
        let offset = tail.len() as u32;
        tail.extend_from_slice(bytes);
        let span = TextSpan {
            chunk,
            pos: (offset << 16) | (len as u32),
        };
        if len <= 64 {
            if let Some(slot) = self.recent.get_mut(usize::from(self.recent_next & 3)) {
                *slot = span;
            }
            self.recent_next = self.recent_next.wrapping_add(1);
        }
        Some(span)
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
        let Some(chunk) = self.chunks.get(span.chunk as usize) else {
            return b"";
        };
        if span.pos == TextSpan::WHOLE_CHUNK {
            return chunk.as_slice();
        }
        let offset = (span.pos >> 16) as usize;
        let len = (span.pos & 0x7FFF) as usize;
        chunk.get(offset..offset + len).unwrap_or_default()
    }

    /// 位置 `span` の UTF-8 文字列を借りる
    #[inline]
    pub fn str_of(&self, span: TextSpan) -> &str {
        core::str::from_utf8(self.get(span)).unwrap_or_default()
    }

    /// 別の置き場の塊をそのまま後ろに繋ぎ、足した最初の塊の番号を返す（バイト列のコピーなし）
    pub fn append_pool(&mut self, mut other: Self) -> u32 {
        let base = self.chunks.len() as u32;
        self.chunks.append(&mut other.chunks);
        base
    }

    /// 巻き戻し用の印
    pub fn marks(&self) -> (usize, usize) {
        (self.chunks.len(), self.chunks.last().map_or(0, Vec::len))
    }

    /// `marks` の状態に戻す
    pub fn truncate(&mut self, (chunks, last_len): (usize, usize)) {
        self.chunks.truncate(chunks);
        if let Some(last) = self.chunks.last_mut() {
            last.truncate(last_len);
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
        self.chunks.iter().map(Vec::len).sum()
    }

    /// 空か
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// 元のバイト列の 1 区間を展開した一時ビュー
#[derive(Clone, Copy)]
pub struct RecordSlice<'a> {
    /// 元の先頭からの開始バイト位置
    pub base: usize,
    /// `base .. base + bytes.len()` の展開済みバイト列
    pub bytes: &'a [u8],
}

impl<'a> RecordSlice<'a> {
    /// 元の絶対バイト範囲 `start..end` を切り出す
    #[inline]
    pub fn slice(&self, start: u32, end: u32) -> &'a [u8] {
        let s = (start as usize).saturating_sub(self.base);
        let e = (end as usize).saturating_sub(self.base);
        self.bytes.get(s..e).unwrap_or_default()
    }
}

/// 16 KiB のページごとに LZ4 形式で圧縮して保持する元のファイルバイト列。
///
/// 468 MiB の ASCII DXF を数十 MiB でメモリに持ち続け、変更時は触るレコードの 1 ページだけを展開し、
/// 保存時は変えていないページをそのまま出力先へ展開する。
#[derive(Clone, Debug)]
pub struct SourceBytes {
    id: u64,
    data: Vec<u8>,
    offsets: Vec<usize>,
    page_newlines: Vec<u16>,
    len: usize,
    newline: &'static [u8],
}

impl Default for SourceBytes {
    fn default() -> Self {
        Self {
            id: 0,
            data: Vec::new(),
            offsets: vec![0],
            page_newlines: Vec::new(),
            len: 0,
            newline: b"\n",
        }
    }
}

#[inline]
fn read_u32_le(src: &[u8], pos: usize) -> Option<u32> {
    let sub = src.get(pos..pos + 4)?;
    Some(u32::from_le_bytes([sub[0], sub[1], sub[2], sub[3]]))
}

#[inline]
fn read_u64_le(src: &[u8], pos: usize) -> Option<u64> {
    let sub = src.get(pos..pos + 8)?;
    Some(u64::from_le_bytes([
        sub[0], sub[1], sub[2], sub[3], sub[4], sub[5], sub[6], sub[7],
    ]))
}

#[inline]
fn hash4_val(v: u32) -> usize {
    ((v.wrapping_mul(2_654_435_761)) >> 21) as usize
}

fn push_lz4_len(out: &mut Vec<u8>, mut rem: usize) {
    while rem >= 255 {
        out.push(255);
        rem -= 255;
    }
    out.push(rem as u8);
}

fn lz4_compress_page(src: &[u8], out: &mut Vec<u8>) -> bool {
    out.clear();
    if src.len() < 16 {
        return false;
    }
    let limit = src.len() - 5;
    let mut table = [0u16; 2048];
    let mut anchor = 0usize;
    let mut ip = 1usize;
    if let Some(v0) = read_u32_le(src, 0) {
        table[hash4_val(v0)] = 0;
    }

    while ip < limit {
        let Some(cur4) = read_u32_le(src, ip) else {
            break;
        };
        let h = hash4_val(cur4);
        let mut cand = usize::from(table[h]);
        table[h] = ip as u16;
        if read_u32_le(src, cand) != Some(cur4) {
            ip += 1 + ((ip - anchor) >> 5);
            continue;
        }
        while ip > anchor && cand > 0 && src.get(ip - 1) == src.get(cand - 1) {
            ip -= 1;
            cand -= 1;
        }
        let lit_len = ip - anchor;
        let mut mlen = 4usize;
        while let (Some(w_ip), Some(w_cand)) =
            (read_u64_le(src, ip + mlen), read_u64_le(src, cand + mlen))
        {
            let diff = w_ip ^ w_cand;
            if diff == 0 {
                mlen += 8;
            } else {
                mlen += (diff.trailing_zeros() >> 3) as usize;
                break;
            }
        }
        if ip + mlen + 8 > src.len() {
            while ip + mlen < src.len() && src.get(ip + mlen) == src.get(cand + mlen) {
                mlen += 1;
            }
        }
        let lit_tok = lit_len.min(15) as u8;
        let mat_tok = (mlen - 4).min(15) as u8;
        out.push((lit_tok << 4) | mat_tok);
        if lit_len >= 15 {
            push_lz4_len(out, lit_len - 15);
        }
        if let Some(lits) = src.get(anchor..ip) {
            out.extend_from_slice(lits);
        }
        let offset = (ip - cand) as u16;
        out.extend_from_slice(&offset.to_le_bytes());
        if mlen - 4 >= 15 {
            push_lz4_len(out, mlen - 4 - 15);
        }
        if out.len() >= src.len() {
            return false;
        }
        ip += mlen;
        anchor = ip;
        if ip < limit
            && let Some(sv) = read_u32_le(src, ip.saturating_sub(2))
        {
            let seed = ip.saturating_sub(2);
            table[hash4_val(sv)] = seed as u16;
        }
    }

    let lit_len = src.len() - anchor;
    let lit_tok = lit_len.min(15) as u8;
    out.push(lit_tok << 4);
    if lit_len >= 15 {
        push_lz4_len(out, lit_len - 15);
    }
    if let Some(lits) = src.get(anchor..) {
        out.extend_from_slice(lits);
    }
    out.len() < src.len()
}

/// [`lz4_decompress_prefix`] が書き込んだ先頭バイト数を返す
fn lz4_decompress_prefix(
    comp: &[u8],
    orig_len: usize,
    need_len: usize,
    out: &mut [u8; SourceBytes::PAGE_SIZE],
) -> usize {
    let need_len = need_len.min(orig_len);
    if comp.len() == orig_len {
        if let (Some(dst), Some(src)) = (out.get_mut(..need_len), comp.get(..need_len)) {
            dst.copy_from_slice(src);
        }
        return need_len;
    }
    let mut cp = 0usize;
    let mut op = 0usize;
    while cp < comp.len() && op < need_len {
        let Some(&tok) = comp.get(cp) else {
            break;
        };
        cp += 1;
        let mut lit_len = usize::from(tok >> 4);
        if lit_len == 15 {
            while let Some(&b) = comp.get(cp) {
                cp += 1;
                lit_len += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        if let (Some(dst), Some(src)) = (out.get_mut(op..op + lit_len), comp.get(cp..cp + lit_len))
        {
            dst.copy_from_slice(src);
        }
        cp += lit_len;
        op += lit_len;
        if cp >= comp.len() || op >= need_len {
            break;
        }
        let (Some(&lo), Some(&hi)) = (comp.get(cp), comp.get(cp + 1)) else {
            break;
        };
        cp += 2;
        let offset = usize::from(u16::from_le_bytes([lo, hi]));
        let mut mlen = usize::from(tok & 0x0F) + 4;
        if (tok & 0x0F) == 15 {
            while let Some(&b) = comp.get(cp) {
                cp += 1;
                mlen += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        let Some(src_start) = op.checked_sub(offset) else {
            break;
        };
        if offset >= mlen {
            out.copy_within(src_start..src_start + mlen, op);
        } else {
            for i in 0..mlen {
                if let Some(&b) = out.get(src_start + i)
                    && let Some(slot) = out.get_mut(op + i)
                {
                    *slot = b;
                }
            }
        }
        op += mlen;
    }
    op.min(orig_len)
}

fn lz4_append_full(comp: &[u8], orig_len: usize, out: &mut Vec<u8>) {
    if comp.len() == orig_len {
        out.extend_from_slice(comp);
        return;
    }
    let mut cp = 0usize;
    while cp < comp.len() {
        let Some(&tok) = comp.get(cp) else {
            break;
        };
        cp += 1;
        let mut lit_len = usize::from(tok >> 4);
        if lit_len == 15 {
            while let Some(&b) = comp.get(cp) {
                cp += 1;
                lit_len += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        if let Some(src) = comp.get(cp..cp + lit_len) {
            out.extend_from_slice(src);
        }
        cp += lit_len;
        if cp >= comp.len() {
            break;
        }
        let (Some(&lo), Some(&hi)) = (comp.get(cp), comp.get(cp + 1)) else {
            break;
        };
        cp += 2;
        let offset = usize::from(u16::from_le_bytes([lo, hi]));
        let mut mlen = usize::from(tok & 0x0F) + 4;
        if (tok & 0x0F) == 15 {
            while let Some(&b) = comp.get(cp) {
                cp += 1;
                mlen += usize::from(b);
                if b != 255 {
                    break;
                }
            }
        }
        let Some(src_start) = out.len().checked_sub(offset) else {
            break;
        };
        if offset >= mlen {
            out.extend_from_within(src_start..src_start + mlen);
        } else {
            for _ in 0..mlen {
                if let Some(&b) = out.get(out.len().saturating_sub(offset)) {
                    out.push(b);
                }
            }
        }
    }
}

struct CompressedPart {
    data: Vec<u8>,
    lens: Vec<u16>,
    newlines: Vec<u16>,
}

fn compress_pages_slice(bytes: &[u8]) -> CompressedPart {
    let pages = bytes.len().div_ceil(SourceBytes::PAGE_SIZE);
    let mut data = Vec::with_capacity(bytes.len() / 4 + 64);
    let mut lens = Vec::with_capacity(pages);
    let mut newlines = Vec::with_capacity(pages);
    let mut scratch = Vec::with_capacity(SourceBytes::PAGE_SIZE);
    for page in bytes.chunks(SourceBytes::PAGE_SIZE) {
        let nl = page.iter().filter(|&&b| b == b'\n').count() as u16;
        newlines.push(nl);
        if lz4_compress_page(page, &mut scratch) {
            lens.push(scratch.len() as u16);
            data.extend_from_slice(&scratch);
        } else {
            lens.push(page.len() as u16);
            data.extend_from_slice(page);
        }
    }
    CompressedPart {
        data,
        lens,
        newlines,
    }
}

fn alloc_page_buf() -> Box<[u8; SourceBytes::PAGE_SIZE]> {
    vec![0u8; SourceBytes::PAGE_SIZE]
        .into_boxed_slice()
        .try_into()
        .unwrap_or_else(|_| Box::new([0u8; SourceBytes::PAGE_SIZE]))
}

struct TlsPageCache {
    src_id: u64,
    page: usize,
    valid: usize,
    buf: Box<[u8; SourceBytes::PAGE_SIZE]>,
}

std::thread_local! {
    static TLS_PAGE_CACHE: core::cell::RefCell<TlsPageCache> = core::cell::RefCell::new(TlsPageCache {
        src_id: 0,
        page: usize::MAX,
        valid: 0,
        buf: alloc_page_buf(),
    });
}

fn next_source_id() -> u64 {
    use core::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl SourceBytes {
    /// 1 ページのシフト幅（`1 << 14 = 16,384` バイト）
    pub const PAGE_BITS: usize = 14;
    /// 1 ページのバイト数（16 KiB）
    pub const PAGE_SIZE: usize = 1 << Self::PAGE_BITS;
    const PAGE_MASK: usize = Self::PAGE_SIZE - 1;

    fn assemble_parts(len: usize, parts: Vec<CompressedPart>) -> Self {
        let total_data: usize = parts.iter().map(|p| p.data.len()).sum();
        let total_pages: usize = parts.iter().map(|p| p.lens.len()).sum();
        let mut data = Vec::with_capacity(total_data);
        let mut offsets = Vec::with_capacity(total_pages + 1);
        let mut page_newlines = Vec::with_capacity(total_pages);
        offsets.push(0);
        for part in parts {
            data.extend_from_slice(&part.data);
            for c_len in part.lens {
                let next = offsets.last().copied().unwrap_or(0) + usize::from(c_len);
                offsets.push(next);
            }
            page_newlines.extend_from_slice(&part.newlines);
        }
        let mut out = Self {
            id: next_source_id(),
            data,
            offsets,
            page_newlines,
            len,
            newline: b"\n",
        };
        out.newline = out.detect_newline();
        out
    }

    fn detect_newline(&self) -> &'static [u8] {
        let mut buf = alloc_page_buf();
        for (p, &nl) in self.page_newlines.iter().enumerate() {
            if nl == 0 {
                continue;
            }
            let orig_len = self.page_orig_len(p);
            lz4_decompress_prefix(self.page_comp(p), orig_len, orig_len, &mut buf);
            let Some(pos) = buf
                .get(..orig_len)
                .and_then(|s| s.iter().position(|&b| b == b'\n'))
            else {
                return b"\n";
            };
            if pos > 0 {
                return if buf.get(pos - 1) == Some(&b'\r') {
                    b"\r\n"
                } else {
                    b"\n"
                };
            }
            if p > 0 {
                let prev_len = self.page_orig_len(p - 1);
                lz4_decompress_prefix(self.page_comp(p - 1), prev_len, prev_len, &mut buf);
                if prev_len > 0 && buf.get(prev_len - 1) == Some(&b'\r') {
                    return b"\r\n";
                }
            }
            return b"\n";
        }
        b"\n"
    }

    /// メモリ上のスライスを `threads` 本のスレッドでページ圧縮する
    pub fn compress_slice(src: &[u8], threads: usize) -> Self {
        if src.is_empty() {
            return Self::default();
        }
        let total_pages = src.len().div_ceil(Self::PAGE_SIZE);
        let workers = threads.max(1).min(total_pages.div_ceil(4).max(1));
        if workers <= 1 {
            return Self::assemble_parts(src.len(), vec![compress_pages_slice(src)]);
        }
        let pages_per = total_pages.div_ceil(workers);
        let parts: Vec<CompressedPart> = std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(workers);
            for w in 0..workers {
                let start = (w * pages_per) * Self::PAGE_SIZE;
                if start >= src.len() {
                    break;
                }
                let end = ((w + 1) * pages_per * Self::PAGE_SIZE).min(src.len());
                let sub = src.get(start..end).unwrap_or_default();
                handles.push(scope.spawn(move || compress_pages_slice(sub)));
            }
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        Self::assemble_parts(src.len(), parts)
    }

    /// ファイルを丸ごと `Vec<u8>` に確保せず、`threads` 本のスレッドで 64 KiB ずつ読みながらページ圧縮する
    pub fn compress_file(path: &std::path::Path, threads: usize) -> std::io::Result<Self> {
        use std::io::{Read, Seek, SeekFrom};

        let meta = std::fs::metadata(path)?;
        let len = usize::try_from(meta.len()).unwrap_or(usize::MAX);
        if len == 0 {
            return Ok(Self::default());
        }
        let total_pages = len.div_ceil(Self::PAGE_SIZE);
        let workers = threads.max(1).min(total_pages.div_ceil(4).max(1));
        let pages_per = total_pages.div_ceil(workers);

        let stream_range = |start: usize, end: usize| -> std::io::Result<CompressedPart> {
            let mut file = std::fs::File::open(path)?;
            file.seek(SeekFrom::Start(start as u64))?;
            let range_len = end - start;
            let pages = range_len.div_ceil(Self::PAGE_SIZE);
            let mut data = Vec::with_capacity(range_len / 4 + 64);
            let mut lens = Vec::with_capacity(pages);
            let mut newlines = Vec::with_capacity(pages);
            let mut scratch = Vec::with_capacity(Self::PAGE_SIZE);
            let mut read_buf = vec![0u8; Self::PAGE_SIZE * 4];
            let mut rem = range_len;
            while rem > 0 {
                let want = rem.min(read_buf.len());
                let mut filled = 0usize;
                while filled < want {
                    let n = file.read(read_buf.get_mut(filled..want).unwrap_or_default())?;
                    if n == 0 {
                        return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof));
                    }
                    filled += n;
                }
                for page in read_buf
                    .get(..want)
                    .unwrap_or_default()
                    .chunks(Self::PAGE_SIZE)
                {
                    let nl = page.iter().filter(|&&b| b == b'\n').count() as u16;
                    newlines.push(nl);
                    if lz4_compress_page(page, &mut scratch) {
                        lens.push(scratch.len() as u16);
                        data.extend_from_slice(&scratch);
                    } else {
                        lens.push(page.len() as u16);
                        data.extend_from_slice(page);
                    }
                }
                rem -= want;
            }
            Ok(CompressedPart {
                data,
                lens,
                newlines,
            })
        };

        if workers <= 1 {
            let part = stream_range(0, len)?;
            return Ok(Self::assemble_parts(len, vec![part]));
        }

        let results: Vec<std::io::Result<CompressedPart>> = std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(workers);
            for w in 0..workers {
                let start = (w * pages_per) * Self::PAGE_SIZE;
                if start >= len {
                    break;
                }
                let end = ((w + 1) * pages_per * Self::PAGE_SIZE).min(len);
                handles.push(scope.spawn(move || stream_range(start, end)));
            }
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err(std::io::Error::other("圧縮スレッドが中断した")))
                })
                .collect()
        });
        let mut parts = Vec::with_capacity(results.len());
        for res in results {
            parts.push(res?);
        }
        Ok(Self::assemble_parts(len, parts))
    }

    /// 元のファイルの全体バイト数
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// 空か
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 足したタグに使う改行コード
    #[inline]
    pub const fn newline(&self) -> &'static [u8] {
        self.newline
    }

    /// ページ数
    #[inline]
    pub fn page_count(&self) -> usize {
        self.page_newlines.len()
    }

    /// ページごとの `\n` の個数
    #[inline]
    pub fn page_newlines(&self) -> &[u16] {
        &self.page_newlines
    }

    /// メモリ上で保持に使っている合計バイト数
    pub fn stored_bytes(&self) -> usize {
        self.data.capacity()
            + self.offsets.capacity() * core::mem::size_of::<usize>()
            + self.page_newlines.capacity() * core::mem::size_of::<u16>()
    }

    #[inline]
    fn page_orig_len(&self, page: usize) -> usize {
        let start = page << Self::PAGE_BITS;
        self.len.saturating_sub(start).min(Self::PAGE_SIZE)
    }

    #[inline]
    fn page_comp(&self, page: usize) -> &[u8] {
        let s = self.offsets.get(page).copied().unwrap_or(0);
        let e = self.offsets.get(page + 1).copied().unwrap_or(s);
        self.data.get(s..e).unwrap_or_default()
    }

    /// 先頭が `prefix` で始まるか
    pub fn starts_with(&self, prefix: &[u8]) -> bool {
        if prefix.len() > self.len {
            return false;
        }
        self.with_range(0..prefix.len(), |s| s.bytes == prefix)
    }

    /// `page` 番めのページを `out` の末尾に展開する
    pub fn append_page(&self, out: &mut Vec<u8>, page: usize) {
        let orig_len = self.page_orig_len(page);
        if orig_len == 0 {
            return;
        }
        lz4_append_full(self.page_comp(page), orig_len, out);
    }

    /// 元のバイト範囲 `range` を `out` の末尾に展開する
    pub fn append_range(&self, out: &mut Vec<u8>, range: core::ops::Range<usize>) {
        let end = range.end.min(self.len);
        let mut cur = range.start.min(end);
        if cur >= end {
            return;
        }
        let mut scratch = alloc_page_buf();
        while cur < end {
            let page = cur >> Self::PAGE_BITS;
            let off = cur & Self::PAGE_MASK;
            let orig_len = self.page_orig_len(page);
            let take_end = (end - (page << Self::PAGE_BITS)).min(orig_len);
            if off == 0 && take_end == orig_len {
                lz4_append_full(self.page_comp(page), orig_len, out);
            } else {
                lz4_decompress_prefix(self.page_comp(page), orig_len, take_end, &mut scratch);
                if let Some(sub) = scratch.get(off..take_end) {
                    out.extend_from_slice(sub);
                }
            }
            cur = (page << Self::PAGE_BITS) + take_end;
        }
    }

    /// 元のバイト範囲 `range` を展開して `f` に渡す（1 ページ内ならスレッド局所の 1 ページ領域を再利用）
    pub fn with_range<R>(
        &self,
        range: core::ops::Range<usize>,
        f: impl FnOnce(RecordSlice<'_>) -> R,
    ) -> R {
        let end = range.end.min(self.len);
        let start = range.start.min(end);
        if start >= end {
            return f(RecordSlice {
                base: start,
                bytes: &[],
            });
        }
        let first_page = start >> Self::PAGE_BITS;
        let last_page = (end - 1) >> Self::PAGE_BITS;
        if first_page == last_page {
            let page_base = first_page << Self::PAGE_BITS;
            let off_s = start - page_base;
            let off_e = end - page_base;
            let orig_len = self.page_orig_len(first_page);
            let comp = self.page_comp(first_page);
            if comp.len() == orig_len {
                return f(RecordSlice {
                    base: start,
                    bytes: comp.get(off_s..off_e).unwrap_or_default(),
                });
            }
            return TLS_PAGE_CACHE.with(|cell| {
                if let Ok(mut cache) = cell.try_borrow_mut() {
                    if self.id == 0
                        || cache.src_id != self.id
                        || cache.page != first_page
                        || cache.valid < off_e
                    {
                        let need = off_e.saturating_add(2048).min(orig_len);
                        let valid = lz4_decompress_prefix(comp, orig_len, need, &mut cache.buf);
                        cache.src_id = self.id;
                        cache.page = first_page;
                        cache.valid = valid;
                    }
                    f(RecordSlice {
                        base: start,
                        bytes: cache.buf.get(off_s..off_e).unwrap_or_default(),
                    })
                } else {
                    let mut fallback = alloc_page_buf();
                    lz4_decompress_prefix(comp, orig_len, off_e, &mut fallback);
                    f(RecordSlice {
                        base: start,
                        bytes: fallback.get(off_s..off_e).unwrap_or_default(),
                    })
                }
            });
        }
        let mut owned = Vec::with_capacity(end - start);
        self.append_range(&mut owned, start..end);
        f(RecordSlice {
            base: start,
            bytes: &owned,
        })
    }

    /// 同じページの再展開を避ける 1 ページ分のキャッシュ付きリーダーを作る
    pub fn page_reader(&self) -> PageReader<'_> {
        PageReader {
            src: self,
            cached_page: usize::MAX,
            cached_valid: 0,
            buf: alloc_page_buf(),
        }
    }
}

/// [`SourceBytes`] の直近 1 ページを保持して連続・近接レコードの重複展開を防ぐリーダー
pub struct PageReader<'a> {
    src: &'a SourceBytes,
    cached_page: usize,
    cached_valid: usize,
    buf: Box<[u8; SourceBytes::PAGE_SIZE]>,
}

impl PageReader<'_> {
    #[inline]
    fn ensure_page(&mut self, page: usize, need_end: usize) {
        if self.cached_page == page && self.cached_valid >= need_end {
            return;
        }
        let orig_len = self.src.page_orig_len(page);
        let comp = self.src.page_comp(page);
        let valid = lz4_decompress_prefix(comp, orig_len, need_end, &mut self.buf);
        self.cached_page = page;
        self.cached_valid = valid;
    }

    /// 元のバイト範囲 `range` を `out` の末尾に展開する
    pub fn append_range(&mut self, out: &mut Vec<u8>, range: core::ops::Range<usize>) {
        let end = range.end.min(self.src.len);
        let mut cur = range.start.min(end);
        if cur >= end {
            return;
        }
        while cur < end {
            let page = cur >> SourceBytes::PAGE_BITS;
            let off = cur & SourceBytes::PAGE_MASK;
            let orig_len = self.src.page_orig_len(page);
            let take_end = (end - (page << SourceBytes::PAGE_BITS)).min(orig_len);
            if off == 0 && take_end == orig_len && self.cached_page != page {
                lz4_append_full(self.src.page_comp(page), orig_len, out);
            } else {
                self.ensure_page(page, orig_len);
                if let Some(sub) = self.buf.get(off..take_end) {
                    out.extend_from_slice(sub);
                }
            }
            cur = (page << SourceBytes::PAGE_BITS) + take_end;
        }
    }

    /// 元のバイト範囲 `range` を展開して `f` に渡す（同じページ内の連続参照はキャッシュを再利用）
    pub fn with_range<R>(
        &mut self,
        range: core::ops::Range<usize>,
        f: impl FnOnce(RecordSlice<'_>) -> R,
    ) -> R {
        let end = range.end.min(self.src.len);
        let start = range.start.min(end);
        if start >= end {
            return f(RecordSlice {
                base: start,
                bytes: &[],
            });
        }
        let first_page = start >> SourceBytes::PAGE_BITS;
        let last_page = (end - 1) >> SourceBytes::PAGE_BITS;
        if first_page == last_page {
            let page_base = first_page << SourceBytes::PAGE_BITS;
            let off_s = start - page_base;
            let off_e = end - page_base;
            let orig_len = self.src.page_orig_len(first_page);
            let comp = self.src.page_comp(first_page);
            if comp.len() == orig_len {
                return f(RecordSlice {
                    base: start,
                    bytes: comp.get(off_s..off_e).unwrap_or_default(),
                });
            }
            self.ensure_page(first_page, off_e);
            return f(RecordSlice {
                base: start,
                bytes: self.buf.get(off_s..off_e).unwrap_or_default(),
            });
        }
        let mut owned = Vec::with_capacity(end - start);
        self.append_range(&mut owned, start..end);
        f(RecordSlice {
            base: start,
            bytes: &owned,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_bytes_roundtrips_pages_and_ranges() {
        let mut raw = Vec::new();
        for i in 0..3500usize {
            raw.extend_from_slice(
                format!("  0\r\nINSERT\r\n  5\r\n{:X}\r\n  8\r\n0\r\n  2\r\nSYM_A\r\n 10\r\n{i}.25\r\n 20\r\n0.0\r\n", 0x100 + i).as_bytes(),
            );
        }
        assert!(raw.len() > SourceBytes::PAGE_SIZE * 5);
        let s1 = SourceBytes::compress_slice(&raw, 1);
        let s4 = SourceBytes::compress_slice(&raw, 4);
        assert_eq!(s1.len(), raw.len());
        assert_eq!(s4.len(), raw.len());
        assert_eq!(s1.newline(), b"\r\n");
        assert!(s1.stored_bytes() * 2 < raw.len());

        let mut full = Vec::new();
        s4.append_range(&mut full, 0..raw.len());
        assert_eq!(full, raw);

        let mut reader = s4.page_reader();
        for &(a, b) in &[
            (0usize, 50usize),
            (4_090, 4_110),
            (4_096, 8_192),
            (16_370, 16_410),
            (100, 40_000),
            (raw.len() - 77, raw.len()),
        ] {
            let got = s4.with_range(a..b, |rs| rs.slice(a as u32, b as u32).to_vec());
            assert_eq!(got.as_slice(), &raw[a..b]);
            let got_r = reader.with_range(a..b, |rs| rs.slice(a as u32, b as u32).to_vec());
            assert_eq!(got_r.as_slice(), &raw[a..b]);
        }
    }

    #[test]
    fn source_bytes_handles_incompressible_and_rle() {
        let mut weird = vec![b' '; 20_000];
        let mut x = 0x1234_5678_9ABC_DEF0u64;
        for _ in 0..20_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            weird.push((x & 0xFF) as u8);
        }
        let sb = SourceBytes::compress_slice(&weird, 2);
        let mut out = Vec::new();
        sb.append_range(&mut out, 0..weird.len());
        assert_eq!(out, weird);
    }
}
