//! 16 KiB のページごとに LZ4 形式で圧縮して保持する元のファイルバイト列。
//!
//! 数百 MiB のテキスト図面（DXF・STEP・JWW など）を小さいメモリで持ち続け、変更時は触るレコードの
//! 1 ページだけを展開し、保存時は変えていないページをそのまま出力先へ展開する。
//! 中身は `Arc` で包んでいるため、版の写し（`Clone`）はポインターの複製だけで済む。

use core::ops::Range;
use std::sync::Arc;

use zumen_base::{Cx, Kind, Stop};

/// 元のバイト列の 1 区間を展開した一時ビュー
#[derive(Clone, Copy, Debug)]
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

/// 分割してページ圧縮した断片。並列ワーカーごとに作って入力順に束ねるために公開する
#[derive(Clone, Debug)]
pub struct CompressedPart {
    data: Vec<u8>,
    lens: Vec<u16>,
    newlines: Vec<u16>,
}

impl CompressedPart {
    /// 圧縮後のバイト数
    #[inline]
    pub fn compressed_len(&self) -> usize {
        self.data.len()
    }

    /// 含まれるページ数
    #[inline]
    pub fn page_count(&self) -> usize {
        self.lens.len()
    }
}

#[derive(Debug)]
struct SourceInner {
    id: u64,
    data: Vec<u8>,
    offsets: Vec<usize>,
    page_newlines: Vec<u16>,
    len: usize,
    newline: &'static [u8],
}

/// 16 KiB のページごとに LZ4 形式で圧縮して保持する元のファイルバイト列
#[derive(Clone, Debug)]
pub struct SourceBytes {
    inner: Arc<SourceInner>,
}

impl Default for SourceBytes {
    fn default() -> Self {
        Self {
            inner: Arc::new(SourceInner {
                id: 0,
                data: Vec::new(),
                offsets: vec![0],
                page_newlines: Vec::new(),
                len: 0,
                newline: b"\n",
            }),
        }
    }
}

#[inline]
fn read_u32_le(src: &[u8], pos: usize) -> Option<u32> {
    let end = pos.checked_add(4)?;
    let bytes: &[u8; 4] = src.get(pos..end)?.first_chunk::<4>()?;
    Some(u32::from_le_bytes(*bytes))
}

#[inline]
fn read_u64_le(src: &[u8], pos: usize) -> Option<u64> {
    let end = pos.checked_add(8)?;
    let bytes: &[u8; 8] = src.get(pos..end)?.first_chunk::<8>()?;
    Some(u64::from_le_bytes(*bytes))
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
    out.push((rem & 0xFF) as u8);
}

fn lz4_compress_page(src: &[u8], out: &mut Vec<u8>) -> bool {
    out.clear();
    if src.len() < 16 || src.len() > SourceBytes::PAGE_SIZE {
        return false;
    }
    let limit = src.len() - 5;
    let mut table = [0u16; 2048];
    let mut anchor = 0usize;
    let mut ip = 1usize;
    if let Some(v0) = read_u32_le(src, 0)
        && let Some(slot) = table.get_mut(hash4_val(v0))
    {
        *slot = 0;
    }

    while ip < limit {
        let Some(cur4) = read_u32_le(src, ip) else {
            break;
        };
        let h = hash4_val(cur4);
        let cand_raw = table.get(h).copied().unwrap_or(0);
        let mut cand = usize::from(cand_raw);
        if let Some(slot) = table.get_mut(h) {
            *slot = u16::try_from(ip).unwrap_or(0);
        }
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
        let lit_tok = (lit_len.min(15) & 0x0F) as u8;
        let mat_tok = ((mlen - 4).min(15) & 0x0F) as u8;
        out.push((lit_tok << 4) | mat_tok);
        if lit_len >= 15 {
            push_lz4_len(out, lit_len - 15);
        }
        if let Some(lits) = src.get(anchor..ip) {
            out.extend_from_slice(lits);
        }
        let offset = u16::try_from(ip.saturating_sub(cand)).unwrap_or(0);
        out.extend_from_slice(&offset.to_le_bytes());
        if mlen - 4 >= 15 {
            push_lz4_len(out, mlen - 4 - 15);
        }
        if out.len() >= src.len() {
            return false;
        }
        ip += mlen;
        anchor = ip;
        if ip < limit {
            let seed = ip.saturating_sub(2);
            if let Some(sv) = read_u32_le(src, seed)
                && let Some(slot) = table.get_mut(hash4_val(sv))
            {
                *slot = u16::try_from(seed).unwrap_or(0);
            }
        }
    }

    let lit_len = src.len() - anchor;
    let lit_tok = (lit_len.min(15) & 0x0F) as u8;
    out.push(lit_tok << 4);
    if lit_len >= 15 {
        push_lz4_len(out, lit_len - 15);
    }
    if let Some(lits) = src.get(anchor..) {
        out.extend_from_slice(lits);
    }
    out.len() < src.len()
}

/// 壊れた圧縮列に対しても決してパニックせず、`out` に書き込んだ先頭バイト数を返す
fn lz4_decompress_prefix(
    comp: &[u8],
    orig_len: usize,
    need_len: usize,
    out: &mut [u8; SourceBytes::PAGE_SIZE],
) -> usize {
    lz4_decompress_prefix_into(comp, orig_len, need_len, out.as_mut_slice())
}

fn lz4_decompress_prefix_into(
    comp: &[u8],
    orig_len: usize,
    need_len: usize,
    out: &mut [u8],
) -> usize {
    let orig_len = orig_len.min(SourceBytes::PAGE_SIZE).min(out.len());
    let need_len = need_len.min(orig_len);
    if comp.len() == orig_len {
        if let (Some(dst), Some(src)) = (out.get_mut(..need_len), comp.get(..need_len)) {
            dst.copy_from_slice(src);
            return need_len;
        }
        return 0;
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
                let Some(next) = lit_len.checked_add(usize::from(b)) else {
                    return op.min(orig_len);
                };
                lit_len = next;
                if lit_len > orig_len {
                    return op.min(orig_len);
                }
                if b != 255 {
                    break;
                }
            }
        }
        let (Some(cp_end), Some(op_end)) = (cp.checked_add(lit_len), op.checked_add(lit_len))
        else {
            break;
        };
        if op_end > orig_len {
            break;
        }
        let (Some(dst), Some(src)) = (out.get_mut(op..op_end), comp.get(cp..cp_end)) else {
            break;
        };
        dst.copy_from_slice(src);
        cp = cp_end;
        op = op_end;
        if cp >= comp.len() || op >= need_len {
            break;
        }
        let (Some(&lo), Some(&hi)) = (comp.get(cp), comp.get(cp + 1)) else {
            break;
        };
        cp += 2;
        let offset = usize::from(u16::from_le_bytes([lo, hi]));
        if offset == 0 {
            break;
        }
        let mut mlen = usize::from(tok & 0x0F) + 4;
        if (tok & 0x0F) == 15 {
            while let Some(&b) = comp.get(cp) {
                cp += 1;
                let Some(next) = mlen.checked_add(usize::from(b)) else {
                    return op.min(orig_len);
                };
                mlen = next;
                if mlen > orig_len {
                    return op.min(orig_len);
                }
                if b != 255 {
                    break;
                }
            }
        }
        let Some(src_start) = op.checked_sub(offset) else {
            break;
        };
        let Some(match_end) = op.checked_add(mlen) else {
            break;
        };
        if match_end > orig_len {
            break;
        }
        if offset >= mlen {
            if let Some((before, after)) = out.split_at_mut_checked(op)
                && let (Some(src_slice), Some(dst_slice)) = (
                    before.get(src_start..src_start + mlen),
                    after.get_mut(..mlen),
                )
            {
                dst_slice.copy_from_slice(src_slice);
            } else {
                break;
            }
        } else {
            for i in 0..mlen {
                if let Some(&b) = out.get(src_start + i)
                    && let Some(slot) = out.get_mut(op + i)
                {
                    *slot = b;
                } else {
                    return op.min(orig_len);
                }
            }
        }
        op = match_end;
    }
    op.min(orig_len)
}

fn lz4_append_full(comp: &[u8], orig_len: usize, out: &mut Vec<u8>) {
    let orig_len = orig_len.min(SourceBytes::PAGE_SIZE);
    if comp.len() == orig_len {
        out.extend_from_slice(comp);
        return;
    }
    let base_out = out.len();
    out.reserve(orig_len);
    let mut cp = 0usize;
    while cp < comp.len() && out.len().saturating_sub(base_out) < orig_len {
        let Some(&tok) = comp.get(cp) else {
            break;
        };
        cp += 1;
        let mut lit_len = usize::from(tok >> 4);
        if lit_len == 15 {
            while let Some(&b) = comp.get(cp) {
                cp += 1;
                let Some(next) = lit_len.checked_add(usize::from(b)) else {
                    return;
                };
                lit_len = next;
                if lit_len > orig_len {
                    return;
                }
                if b != 255 {
                    break;
                }
            }
        }
        let Some(cp_end) = cp.checked_add(lit_len) else {
            break;
        };
        if out.len().saturating_sub(base_out).saturating_add(lit_len) > orig_len {
            break;
        }
        let Some(src) = comp.get(cp..cp_end) else {
            break;
        };
        out.extend_from_slice(src);
        cp = cp_end;
        if cp >= comp.len() {
            break;
        }
        let (Some(&lo), Some(&hi)) = (comp.get(cp), comp.get(cp + 1)) else {
            break;
        };
        cp += 2;
        let offset = usize::from(u16::from_le_bytes([lo, hi]));
        if offset == 0 {
            break;
        }
        let mut mlen = usize::from(tok & 0x0F) + 4;
        if (tok & 0x0F) == 15 {
            while let Some(&b) = comp.get(cp) {
                cp += 1;
                let Some(next) = mlen.checked_add(usize::from(b)) else {
                    return;
                };
                mlen = next;
                if mlen > orig_len {
                    return;
                }
                if b != 255 {
                    break;
                }
            }
        }
        if out.len().saturating_sub(base_out).saturating_add(mlen) > orig_len {
            break;
        }
        let Some(src_start) = out.len().checked_sub(offset) else {
            break;
        };
        if src_start < base_out {
            break;
        }
        if offset >= mlen {
            let cur_len = out.len();
            out.resize(cur_len + mlen, 0);
            if let Some((before, after)) = out.split_at_mut_checked(cur_len)
                && let Some(src_slice) = before.get(src_start..src_start + mlen)
            {
                after.copy_from_slice(src_slice);
            }
        } else {
            for _ in 0..mlen {
                if let Some(&b) = out.get(out.len().saturating_sub(offset)) {
                    out.push(b);
                } else {
                    break;
                }
            }
        }
    }
}

/// 16 KiB 境界にそろったスライスをページごとに LZ4 圧縮する
pub fn compress_pages_slice(bytes: &[u8]) -> CompressedPart {
    let pages = bytes.len().div_ceil(SourceBytes::PAGE_SIZE);
    let mut data = Vec::with_capacity(bytes.len() / 4 + 64);
    let mut lens = Vec::with_capacity(pages);
    let mut newlines = Vec::with_capacity(pages);
    let mut scratch = Vec::with_capacity(SourceBytes::PAGE_SIZE);
    for page in bytes.chunks(SourceBytes::PAGE_SIZE) {
        let nl = u16::try_from(page.iter().filter(|&&b| b == b'\n').count()).unwrap_or(u16::MAX);
        newlines.push(nl);
        if lz4_compress_page(page, &mut scratch) {
            lens.push(u16::try_from(scratch.len()).unwrap_or(u16::MAX));
            data.extend_from_slice(&scratch);
        } else {
            lens.push(u16::try_from(page.len()).unwrap_or(u16::MAX));
            data.extend_from_slice(page);
        }
    }
    CompressedPart {
        data,
        lens,
        newlines,
    }
}

/// 予算と中断を確かめながらスライスをページごとに LZ4 圧縮する
pub fn compress_pages_slice_with_cx(bytes: &[u8], cx: &mut Cx<'_>) -> Result<CompressedPart, Stop> {
    cx.check(Kind::InputBytes, bytes.len() as u64)?;
    let pages = bytes.len().div_ceil(SourceBytes::PAGE_SIZE);
    // 大きな確保の前にページ数分の作業量と展開保持量の上限を確かめる
    cx.check(Kind::Work, (pages as u64).max(1))?;
    cx.check(Kind::DecodedBytes, bytes.len() as u64)?;
    let mut data = Vec::with_capacity(bytes.len() / 4 + 64);
    let mut lens = Vec::with_capacity(pages);
    let mut newlines = Vec::with_capacity(pages);
    let mut scratch = Vec::with_capacity(SourceBytes::PAGE_SIZE);
    for page in bytes.chunks(SourceBytes::PAGE_SIZE) {
        cx.check(Kind::Work, 0)?;
        let nl = u16::try_from(page.iter().filter(|&&b| b == b'\n').count()).unwrap_or(u16::MAX);
        newlines.push(nl);
        if lz4_compress_page(page, &mut scratch) {
            lens.push(u16::try_from(scratch.len()).unwrap_or(u16::MAX));
            data.extend_from_slice(&scratch);
        } else {
            lens.push(u16::try_from(page.len()).unwrap_or(u16::MAX));
            data.extend_from_slice(page);
        }
    }
    Ok(CompressedPart {
        data,
        lens,
        newlines,
    })
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

    /// 入力順に並んだ [`CompressedPart`] を束ねて [`SourceBytes`] を組み立てる
    pub fn assemble_parts(len: usize, parts: Vec<CompressedPart>) -> Self {
        let total_data: usize = parts
            .iter()
            .fold(0usize, |acc, p| acc.saturating_add(p.data.len()));
        let total_pages: usize = parts
            .iter()
            .fold(0usize, |acc, p| acc.saturating_add(p.lens.len()));
        let max_len = total_pages.saturating_mul(Self::PAGE_SIZE);
        let clamped_len = len.min(max_len);
        let mut data = Vec::with_capacity(total_data);
        let mut offsets = Vec::with_capacity(total_pages.saturating_add(1));
        let mut page_newlines = Vec::with_capacity(total_pages);
        offsets.push(0);
        for part in parts {
            data.extend_from_slice(&part.data);
            for (idx, c_len) in part.lens.into_iter().enumerate() {
                let next = offsets
                    .last()
                    .copied()
                    .unwrap_or(0usize)
                    .saturating_add(usize::from(c_len));
                offsets.push(next);
                let nl = part.newlines.get(idx).copied().unwrap_or(0);
                page_newlines.push(nl);
            }
        }
        let mut inner = SourceInner {
            id: next_source_id(),
            data,
            offsets,
            page_newlines,
            len: clamped_len,
            newline: b"\n",
        };
        inner.newline = Self::detect_newline_inner(&inner);
        Self {
            inner: Arc::new(inner),
        }
    }

    fn detect_newline_inner(inner: &SourceInner) -> &'static [u8] {
        let mut buf = alloc_page_buf();
        for (p, &nl) in inner.page_newlines.iter().enumerate() {
            if nl == 0 {
                continue;
            }
            let orig_len = Self::page_orig_len_inner(inner, p);
            let valid = lz4_decompress_prefix(
                Self::page_comp_inner(inner, p),
                orig_len,
                orig_len,
                &mut buf,
            );
            let Some(pos) = buf
                .get(..valid)
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
                let prev_len = Self::page_orig_len_inner(inner, p - 1);
                let prev_valid = lz4_decompress_prefix(
                    Self::page_comp_inner(inner, p - 1),
                    prev_len,
                    prev_len,
                    &mut buf,
                );
                if prev_valid == prev_len
                    && prev_valid > 0
                    && buf.get(prev_valid - 1) == Some(&b'\r')
                {
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
        if cfg!(target_arch = "wasm32") || workers <= 1 {
            return Self::assemble_parts(src.len(), vec![compress_pages_slice(src)]);
        }
        let pages_per = total_pages.div_ceil(workers);
        let parts: Vec<CompressedPart> = std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(workers);
            for w in 0..workers {
                let Some(start) = w
                    .checked_mul(pages_per)
                    .and_then(|pages| pages.checked_mul(Self::PAGE_SIZE))
                else {
                    break;
                };
                if start >= src.len() {
                    break;
                }
                let end = w
                    .saturating_add(1)
                    .saturating_mul(pages_per)
                    .saturating_mul(Self::PAGE_SIZE)
                    .min(src.len());
                let sub = src.get(start..end).unwrap_or_default();
                handles.push(scope.spawn(move || compress_pages_slice(sub)));
            }
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        Self::assemble_parts(src.len(), parts)
    }

    /// 予算と中断を確かめながら、スライスを入力順の予算計上でページ圧縮する
    pub fn compress_slice_with_cx(
        src: &[u8],
        threads: usize,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Stop> {
        if src.is_empty() {
            return Ok(Self::default());
        }
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let total_pages = src.len().div_ceil(Self::PAGE_SIZE);
        // 入力の順にページごとの作業量と最大保持量を先に引き当てることで、スレッド数によらず停止位置を一致させる
        cx.check(Kind::Work, total_pages as u64)?;
        cx.check(Kind::DecodedBytes, src.len() as u64)?;
        Ok(Self::compress_slice(src, threads))
    }

    /// ファイルを丸ごと `Vec<u8>` に確保せず、`threads` 本のスレッドで 64 KiB ずつ読みながらページ圧縮する
    #[cfg(not(target_arch = "wasm32"))]
    pub fn compress_file(path: &std::path::Path, threads: usize) -> std::io::Result<Self> {
        use std::io::{Read, Seek, SeekFrom};

        let meta = std::fs::metadata(path)?;
        let len = usize::try_from(meta.len()).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "ファイルの大きさがアドレス空間の上限を超えている",
            )
        })?;
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
                    let nl = u16::try_from(page.iter().filter(|&&b| b == b'\n').count())
                        .unwrap_or(u16::MAX);
                    newlines.push(nl);
                    if lz4_compress_page(page, &mut scratch) {
                        lens.push(u16::try_from(scratch.len()).unwrap_or(u16::MAX));
                        data.extend_from_slice(&scratch);
                    } else {
                        lens.push(u16::try_from(page.len()).unwrap_or(u16::MAX));
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
                let Some(start) = w
                    .checked_mul(pages_per)
                    .and_then(|pages| pages.checked_mul(Self::PAGE_SIZE))
                else {
                    break;
                };
                if start >= len {
                    break;
                }
                let end = w
                    .saturating_add(1)
                    .saturating_mul(pages_per)
                    .saturating_mul(Self::PAGE_SIZE)
                    .min(len);
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
    pub fn len(&self) -> usize {
        self.inner.len
    }

    /// 空か
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.len == 0
    }

    /// 足したタグに使う改行コード
    #[inline]
    pub fn newline(&self) -> &'static [u8] {
        self.inner.newline
    }

    /// ページ数
    #[inline]
    pub fn page_count(&self) -> usize {
        self.inner.page_newlines.len()
    }

    /// ページごとの `\n` の個数
    #[inline]
    pub fn page_newlines(&self) -> &[u16] {
        &self.inner.page_newlines
    }

    /// メモリ上で保持に使っている合計バイト数
    pub fn stored_bytes(&self) -> usize {
        self.inner.data.capacity()
            + self.inner.offsets.capacity() * core::mem::size_of::<usize>()
            + self.inner.page_newlines.capacity() * core::mem::size_of::<u16>()
    }

    #[inline]
    fn page_orig_len_inner(inner: &SourceInner, page: usize) -> usize {
        let Some(start) = page.checked_mul(Self::PAGE_SIZE) else {
            return 0;
        };
        inner.len.saturating_sub(start).min(Self::PAGE_SIZE)
    }

    #[inline]
    fn page_comp_inner(inner: &SourceInner, page: usize) -> &[u8] {
        let s = inner.offsets.get(page).copied().unwrap_or(0);
        let e = page
            .checked_add(1)
            .and_then(|next| inner.offsets.get(next).copied())
            .unwrap_or(s);
        inner.data.get(s..e).unwrap_or_default()
    }

    #[inline]
    fn page_orig_len(&self, page: usize) -> usize {
        Self::page_orig_len_inner(&self.inner, page)
    }

    #[inline]
    fn page_comp(&self, page: usize) -> &[u8] {
        Self::page_comp_inner(&self.inner, page)
    }

    /// 先頭が `prefix` で始まるか
    pub fn starts_with(&self, prefix: &[u8]) -> bool {
        if prefix.len() > self.len() {
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
    pub fn append_range(&self, out: &mut Vec<u8>, range: Range<usize>) {
        let end = range.end.min(self.len());
        let mut cur = range.start.min(end);
        if cur >= end {
            return;
        }
        let mut scratch = alloc_page_buf();
        while cur < end {
            let page = cur >> Self::PAGE_BITS;
            let off = cur & Self::PAGE_MASK;
            let orig_len = self.page_orig_len(page);
            if orig_len == 0 {
                break;
            }
            let Some(page_base) = page.checked_mul(Self::PAGE_SIZE) else {
                break;
            };
            let take_end = (end - page_base).min(orig_len);
            if take_end == 0 {
                break;
            }
            if off == 0 && take_end == orig_len {
                lz4_append_full(self.page_comp(page), orig_len, out);
            } else {
                let valid =
                    lz4_decompress_prefix(self.page_comp(page), orig_len, take_end, &mut scratch);
                let actual_end = take_end.min(valid);
                if let Some(sub) = scratch.get(off.min(actual_end)..actual_end) {
                    out.extend_from_slice(sub);
                }
            }
            cur = page_base.saturating_add(take_end);
        }
    }

    /// 元のバイト範囲 `range` を展開して `f` に渡す（1 ページ内ならスレッド局所の 1 ページ領域を再利用）
    pub fn with_range<R>(&self, range: Range<usize>, f: impl FnOnce(RecordSlice<'_>) -> R) -> R {
        let end = range.end.min(self.len());
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
            let src_id = self.inner.id;
            return TLS_PAGE_CACHE.with(|cell| {
                if let Ok(mut cache) = cell.try_borrow_mut() {
                    if src_id == 0
                        || cache.src_id != src_id
                        || cache.page != first_page
                        || cache.valid < off_e
                    {
                        let need = off_e.saturating_add(2048).min(orig_len);
                        let valid = lz4_decompress_prefix(comp, orig_len, need, &mut cache.buf);
                        cache.src_id = src_id;
                        cache.page = first_page;
                        cache.valid = valid;
                    }
                    let actual_end = off_e.min(cache.valid);
                    let actual_start = off_s.min(actual_end);
                    f(RecordSlice {
                        base: start,
                        bytes: cache.buf.get(actual_start..actual_end).unwrap_or_default(),
                    })
                } else {
                    let mut fallback = alloc_page_buf();
                    let valid = lz4_decompress_prefix(comp, orig_len, off_e, &mut fallback);
                    let actual_end = off_e.min(valid);
                    let actual_start = off_s.min(actual_end);
                    f(RecordSlice {
                        base: start,
                        bytes: fallback.get(actual_start..actual_end).unwrap_or_default(),
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
#[derive(Debug)]
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
    pub fn append_range(&mut self, out: &mut Vec<u8>, range: Range<usize>) {
        let end = range.end.min(self.src.len());
        let mut cur = range.start.min(end);
        if cur >= end {
            return;
        }
        while cur < end {
            let page = cur >> SourceBytes::PAGE_BITS;
            let off = cur & SourceBytes::PAGE_MASK;
            let orig_len = self.src.page_orig_len(page);
            if orig_len == 0 {
                break;
            }
            let Some(page_base) = page.checked_mul(SourceBytes::PAGE_SIZE) else {
                break;
            };
            let take_end = (end - page_base).min(orig_len);
            if take_end == 0 {
                break;
            }
            if off == 0 && take_end == orig_len && self.cached_page != page {
                lz4_append_full(self.src.page_comp(page), orig_len, out);
            } else {
                self.ensure_page(page, orig_len);
                let actual_end = take_end.min(self.cached_valid);
                if let Some(sub) = self.buf.get(off.min(actual_end)..actual_end) {
                    out.extend_from_slice(sub);
                }
            }
            cur = page_base.saturating_add(take_end);
        }
    }

    /// 元のバイト範囲 `range` を展開して `f` に渡す（同じページ内の連続参照はキャッシュを再利用）
    pub fn with_range<R>(
        &mut self,
        range: Range<usize>,
        f: impl FnOnce(RecordSlice<'_>) -> R,
    ) -> R {
        let end = range.end.min(self.src.len());
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
            let actual_end = off_e.min(self.cached_valid);
            let actual_start = off_s.min(actual_end);
            return f(RecordSlice {
                base: start,
                bytes: self.buf.get(actual_start..actual_end).unwrap_or_default(),
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
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

    #[test]
    fn source_bytes_roundtrips_pages_and_ranges() {
        let mut raw = Vec::new();
        for i in 0..3500usize {
            raw.extend_from_slice(
                format!(
                    "  0\r\nINSERT\r\n  5\r\n{:X}\r\n  8\r\n0\r\n  2\r\nSYM_A\r\n 10\r\n{i}.25\r\n 20\r\n0.0\r\n",
                    0x100 + i
                )
                .as_bytes(),
            );
        }
        assert!(raw.len() > SourceBytes::PAGE_SIZE * 5);
        let s1 = SourceBytes::compress_slice(&raw, 1);
        let s4 = SourceBytes::compress_slice(&raw, 4);
        assert_eq!(s1.len(), raw.len());
        assert_eq!(s4.len(), raw.len());
        assert_eq!(s1.newline(), b"\r\n");
        assert!(s1.stored_bytes() * 2 < raw.len());

        let cloned = s4.clone();
        assert!(Arc::ptr_eq(&s4.inner, &cloned.inner));

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
    fn source_bytes_handles_incompressible_rle_and_budget_limits() {
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

        let mut budget = Budget::new(Limits::default().with(Kind::InputBytes, Limit::Finite(1024)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let err = SourceBytes::compress_slice_with_cx(&weird, 4, &mut cx).unwrap_err();
        assert!(matches!(err, Stop::Exceeded(e) if e.kind == Kind::InputBytes));
    }

    #[test]
    fn corrupt_lz4_streams_never_panic() {
        let mut out_buf = alloc_page_buf();
        let mut vec_buf = Vec::new();
        let adversarial: &[&[u8]] = &[
            &[],
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            &[0x1F, b'A', 0x00, 0x00],
            &[0x1F, b'A', 0x05, 0x00],
            &[0x4F, b'A', b'B', b'C', b'D', 0x04, 0x00, 0xFF, 0xFF, 0xFF],
            &[0x4F, b'A', b'B', b'C', b'D', 0x01, 0x00, 0xFF, 0xFF, 0xFF],
        ];
        for sample in adversarial {
            let n = lz4_decompress_prefix(sample, SourceBytes::PAGE_SIZE, 8192, &mut out_buf);
            assert!(n <= SourceBytes::PAGE_SIZE);
            vec_buf.clear();
            lz4_append_full(sample, SourceBytes::PAGE_SIZE, &mut vec_buf);
            assert!(vec_buf.len() <= SourceBytes::PAGE_SIZE);
        }
    }

    #[test]
    fn short_or_empty_parts_never_leak_tls_or_reader_cache_and_check_budgets_upfront() {
        // 先に圧縮が効くページを展開して TLS と PageReader の領域を既知のバイトで埋める
        let secret = vec![b'S'; 8_192];
        let src_a = SourceBytes::compress_slice(&secret, 1);
        let read_a = src_a.with_range(0..64, |rs| rs.bytes.to_vec());
        assert_eq!(read_a, vec![b'S'; 64]);

        // 1. 空の parts に対して大きな len を渡しても、clamped_len = 0 になり残存バイトは一切出ない
        let empty_parts = SourceBytes::assemble_parts(100, Vec::new());
        assert_eq!(empty_parts.len(), 0);
        let got_empty = empty_parts.with_range(0..100, |rs| rs.bytes.to_vec());
        assert!(got_empty.is_empty());

        // 2. 展開長が 1 バイト（b'X'）しかない短尺 LZ4 ページに対し、100 バイト要求しても 1 バイトしか返さない
        let short_part = CompressedPart {
            data: vec![0x10, b'X'],
            lens: vec![2],
            newlines: vec![0],
        };
        let short_src = SourceBytes::assemble_parts(100, vec![short_part]);
        let got_tls = short_src.with_range(0..64, |rs| {
            // 再入経路も同じ結果を返すことを確かめる
            let got_reentrant = short_src.with_range(0..64, |inner| inner.bytes.to_vec());
            assert_eq!(got_reentrant, b"X");
            rs.bytes.to_vec()
        });
        assert_eq!(got_tls, b"X");

        let mut appended = Vec::new();
        short_src.append_range(&mut appended, 0..64);
        assert_eq!(appended, b"X");

        let mut reader = short_src.page_reader();
        let got_reader = reader.with_range(0..64, |rs| rs.bytes.to_vec());
        assert_eq!(got_reader, b"X");
        let mut reader_appended = Vec::new();
        reader.append_range(&mut reader_appended, 0..64);
        assert_eq!(reader_appended, b"X");

        // 3. 巨大なページ番号を渡してもシフトあふれで 0 ページ目を誤読しない
        let mut page_out = Vec::new();
        src_a.append_page(&mut page_out, usize::MAX);
        assert!(page_out.is_empty());
        let high_page = 1usize << (usize::BITS as usize - SourceBytes::PAGE_BITS);
        src_a.append_page(&mut page_out, high_page);
        assert!(page_out.is_empty());

        // 4. InputBytes が無制限でも DecodedBytes が足りなければ確保前に止まる
        let mut budget = Budget::new(
            Limits::default()
                .with(Kind::InputBytes, Limit::Unlimited)
                .with(Kind::DecodedBytes, Limit::Finite(16)),
        );
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let err = compress_pages_slice_with_cx(&secret, &mut cx).unwrap_err();
        assert!(matches!(err, Stop::Exceeded(e) if e.kind == Kind::DecodedBytes));
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn verify_record_slice_never_panics_for_any_bounds() {
        let data: [u8; 8] = kani::any();
        let base: usize = kani::any();
        let start: u32 = kani::any();
        let end: u32 = kani::any();
        let view = RecordSlice { base, bytes: &data };
        let sliced = view.slice(start, end);
        assert!(sliced.len() <= data.len());
    }

    #[kani::proof]
    #[kani::unwind(5)]
    #[kani::solver(kissat)]
    fn verify_lz4_decompress_corrupt_input_never_panics() {
        let comp: [u8; 4] = kani::any();
        let orig_len: usize = kani::any();
        let need_len: usize = kani::any();
        kani::assume(orig_len <= 4 && need_len <= 4);
        let mut out = [0u8; 4];
        let written = lz4_decompress_prefix_into(&comp, orig_len, need_len, &mut out);
        assert!(written <= orig_len);
    }

    #[kani::proof]
    #[kani::unwind(6)]
    #[kani::solver(kissat)]
    fn verify_lz4_decompress_match_copy_never_panics() {
        // リテラル 4 バイト＋マッチ長 4 バイト（任意の 16 bit オフセット）で非重複・重複コピーの両経路を網羅
        let mut comp = [0u8; 7];
        comp[0] = 0x40;
        let payload: [u8; 6] = kani::any();
        comp[1..7].copy_from_slice(&payload);
        let orig_len: usize = kani::any();
        let need_len: usize = kani::any();
        kani::assume(orig_len <= 8 && need_len <= 8);
        let mut out = [0u8; 8];
        let written = lz4_decompress_prefix_into(&comp, orig_len, need_len, &mut out);
        assert!(written <= orig_len);
    }
}
