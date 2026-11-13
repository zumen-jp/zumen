//! ストリップ・タイルの圧縮展開（無圧縮・PackBits・LZW・Deflate・CCITT Group 4）と `Predictor = 2`。

use zumen_base::{Kind, Stop};
use zumen_sdk::Cx;

use crate::ifd::Endian;

/// 展開失敗の理由
pub(crate) enum Fail {
    /// 圧縮列の破損または長さ不一致
    Invalid,
    /// 予算超過または中断
    Stop(Stop),
}

/// 1 つのストリップまたはタイルを `expected_bytes` バイトに展開する
#[allow(clippy::too_many_arguments)]
pub(crate) fn decompress_chunk(
    raw: &[u8],
    compression: u16,
    fill_order: u16,
    width: u32,
    rows: u32,
    bps: u16,
    expected_bytes: usize,
    cx: &mut Cx,
) -> Result<Vec<u8>, Fail> {
    match compression {
        1 => decompress_uncompressed(raw, fill_order, bps, expected_bytes, cx),
        32773 => decompress_packbits(raw, fill_order, bps, expected_bytes, cx),
        5 => decompress_lzw(raw, fill_order, expected_bytes, cx),
        8 | 32946 => decompress_deflate(raw, compression, fill_order, expected_bytes, cx),
        4 => decompress_group4(raw, fill_order, width, rows, expected_bytes, cx),
        _ => Err(Fail::Invalid),
    }
}

fn decompress_uncompressed(
    raw: &[u8],
    fill_order: u16,
    bps: u16,
    expected_bytes: usize,
    cx: &mut Cx,
) -> Result<Vec<u8>, Fail> {
    let slice = raw.get(..expected_bytes).ok_or(Fail::Invalid)?;
    cx.check(Kind::DecodedBytes, expected_bytes as u64)
        .map_err(Fail::Stop)?;
    cx.check(Kind::Work, (expected_bytes as u64).div_ceil(64).max(1))
        .map_err(Fail::Stop)?;
    let mut out = slice.to_vec();
    if fill_order == 2 && bps == 1 {
        for b in &mut out {
            *b = b.reverse_bits();
        }
    }
    Ok(out)
}

fn decompress_packbits(
    raw: &[u8],
    fill_order: u16,
    bps: u16,
    expected_bytes: usize,
    cx: &mut Cx,
) -> Result<Vec<u8>, Fail> {
    let mut out = Vec::with_capacity(expected_bytes.min(65536));
    let mut pos = 0usize;
    while out.len() < expected_bytes {
        let header = *raw.get(pos).ok_or(Fail::Invalid)?;
        pos += 1;
        cx.check(Kind::Work, 1).map_err(Fail::Stop)?;
        match header {
            0..=127 => {
                let n = usize::from(header) + 1;
                if out.len() + n > expected_bytes {
                    return Err(Fail::Invalid);
                }
                let lit = raw.get(pos..pos + n).ok_or(Fail::Invalid)?;
                pos += n;
                cx.check(Kind::DecodedBytes, n as u64).map_err(Fail::Stop)?;
                out.extend_from_slice(lit);
            }
            128 => {}
            129..=255 => {
                let n = 257 - usize::from(header);
                if out.len() + n > expected_bytes {
                    return Err(Fail::Invalid);
                }
                let val = *raw.get(pos).ok_or(Fail::Invalid)?;
                pos += 1;
                cx.check(Kind::DecodedBytes, n as u64).map_err(Fail::Stop)?;
                out.resize(out.len() + n, val);
            }
        }
    }
    if fill_order == 2 && bps == 1 {
        for b in &mut out {
            *b = b.reverse_bits();
        }
    }
    Ok(out)
}

struct MsbBits<'a> {
    src: &'a [u8],
    at: usize,
    fill_order: u16,
    buf: u64,
    count: u32,
}

impl<'a> MsbBits<'a> {
    fn new(src: &'a [u8], fill_order: u16) -> Self {
        Self {
            src,
            at: 0,
            fill_order,
            buf: 0,
            count: 0,
        }
    }

    fn ensure(&mut self, n: u32) -> bool {
        while self.count < n {
            let Some(&raw_byte) = self.src.get(self.at) else {
                return false;
            };
            self.at += 1;
            let b = if self.fill_order == 2 {
                raw_byte.reverse_bits()
            } else {
                raw_byte
            };
            self.buf = (self.buf << 8) | u64::from(b);
            self.count += 8;
        }
        true
    }

    fn take(&mut self, n: u32) -> Option<u16> {
        if !self.ensure(n) {
            return None;
        }
        self.count -= n;
        let mask = (1u64 << n) - 1;
        let val = (self.buf >> self.count) & mask;
        self.buf &= if self.count == 0 {
            0
        } else {
            (1u64 << self.count) - 1
        };
        u16::try_from(val).ok()
    }
}

/// TIFF 6.0 Section 13 の LZW 展開（MSB 先頭・early-change 9〜12 ビット）
fn decompress_lzw(
    raw: &[u8],
    fill_order: u16,
    expected_bytes: usize,
    cx: &mut Cx,
) -> Result<Vec<u8>, Fail> {
    const CLEAR: u16 = 256;
    const EOI: u16 = 257;
    const MAX_CODES: usize = 4096;

    let mut bits = MsbBits::new(raw, fill_order);
    let mut prefix = [0u16; MAX_CODES];
    let mut suffix = [0u8; MAX_CODES];
    let mut first_byte = [0u8; MAX_CODES];
    let mut seq_len = [0u16; MAX_CODES];
    for i in 0..256usize {
        if let (Some(s), Some(f), Some(l)) =
            (suffix.get_mut(i), first_byte.get_mut(i), seq_len.get_mut(i))
        {
            *s = i as u8;
            *f = i as u8;
            *l = 1;
        }
    }

    let mut scratch = [0u8; MAX_CODES];
    let mut out = Vec::with_capacity(expected_bytes.min(65536));
    let mut code_size = 9u32;
    let mut next_code = 258u16;
    let mut old_code: Option<u16> = None;

    while out.len() < expected_bytes {
        cx.check(Kind::Work, 1).map_err(Fail::Stop)?;
        let code = bits.take(code_size).ok_or(Fail::Invalid)?;
        if code == EOI {
            break;
        }
        if code == CLEAR {
            code_size = 9;
            next_code = 258;
            old_code = None;
            continue;
        }

        let Some(prev) = old_code else {
            if code >= 256 {
                return Err(Fail::Invalid);
            }
            cx.check(Kind::DecodedBytes, 1).map_err(Fail::Stop)?;
            out.push(code as u8);
            old_code = Some(code);
            continue;
        };

        let (decoded_slice, first_char) = if code < next_code {
            let len = usize::from(*seq_len.get(usize::from(code)).ok_or(Fail::Invalid)?);
            let mut cur = code;
            for pos in (0..len).rev() {
                let s = *suffix.get(usize::from(cur)).ok_or(Fail::Invalid)?;
                if let Some(slot) = scratch.get_mut(pos) {
                    *slot = s;
                }
                if cur >= 258 {
                    cur = *prefix.get(usize::from(cur)).ok_or(Fail::Invalid)?;
                }
            }
            let fc = *scratch.first().ok_or(Fail::Invalid)?;
            (scratch.get(..len).ok_or(Fail::Invalid)?, fc)
        } else if code == next_code {
            let prev_len = usize::from(*seq_len.get(usize::from(prev)).ok_or(Fail::Invalid)?);
            let total_len = prev_len + 1;
            if total_len > MAX_CODES {
                return Err(Fail::Invalid);
            }
            let prev_first = *first_byte.get(usize::from(prev)).ok_or(Fail::Invalid)?;
            let mut cur = prev;
            for pos in (0..prev_len).rev() {
                let s = *suffix.get(usize::from(cur)).ok_or(Fail::Invalid)?;
                if let Some(slot) = scratch.get_mut(pos) {
                    *slot = s;
                }
                if cur >= 258 {
                    cur = *prefix.get(usize::from(cur)).ok_or(Fail::Invalid)?;
                }
            }
            if let Some(last_slot) = scratch.get_mut(prev_len) {
                *last_slot = prev_first;
            }
            (scratch.get(..total_len).ok_or(Fail::Invalid)?, prev_first)
        } else {
            return Err(Fail::Invalid);
        };

        let write_len = decoded_slice.len().min(expected_bytes - out.len());
        let to_write = decoded_slice.get(..write_len).ok_or(Fail::Invalid)?;
        if write_len >= 16 {
            cx.check(Kind::Work, (write_len as u64) / 16)
                .map_err(Fail::Stop)?;
        }
        cx.check(Kind::DecodedBytes, write_len as u64)
            .map_err(Fail::Stop)?;
        out.extend_from_slice(to_write);

        if usize::from(next_code) < MAX_CODES {
            let idx = usize::from(next_code);
            let prev_len = *seq_len.get(usize::from(prev)).ok_or(Fail::Invalid)?;
            let prev_first = *first_byte.get(usize::from(prev)).ok_or(Fail::Invalid)?;
            if let (Some(p), Some(s), Some(f), Some(l)) = (
                prefix.get_mut(idx),
                suffix.get_mut(idx),
                first_byte.get_mut(idx),
                seq_len.get_mut(idx),
            ) {
                *p = prev;
                *s = first_char;
                *f = prev_first;
                *l = prev_len.saturating_add(1);
            }
            next_code += 1;
            // TIFF 6.0 Section 13 early-change: 511・1023・2047 に達した時点で符号幅を増やす
            if u32::from(next_code) == (1u32 << code_size) - 1 && code_size < 12 {
                code_size += 1;
            }
        }

        old_code = Some(code);
    }

    if out.len() != expected_bytes {
        return Err(Fail::Invalid);
    }
    Ok(out)
}

/// Deflate（`8` Adobe Deflate・`32946` Deflate）の展開（RFC 1950 + RFC 1951）
fn decompress_deflate(
    raw: &[u8],
    compression: u16,
    fill_order: u16,
    expected_bytes: usize,
    cx: &mut Cx,
) -> Result<Vec<u8>, Fail> {
    let has_zlib = if let (Some(cmf), Some(flg)) = (
        deflate_byte(raw, 0, fill_order),
        deflate_byte(raw, 1, fill_order),
    ) {
        raw.len() >= 6
            && (cmf & 0x0F) == 8
            && (cmf >> 4) <= 7
            && (u16::from(cmf) * 256 + u16::from(flg)).is_multiple_of(31)
            && (flg & 0x20) == 0
    } else {
        false
    };

    if has_zlib {
        let deflate_src = raw.get(2..).ok_or(Fail::Invalid)?;
        let (out, consumed) = inflate_budgeted(deflate_src, fill_order, expected_bytes, cx)?;
        if out.len() != expected_bytes {
            return Err(Fail::Invalid);
        }
        let b0 = deflate_byte(deflate_src, consumed, fill_order).ok_or(Fail::Invalid)?;
        let b1 = deflate_byte(
            deflate_src,
            consumed.checked_add(1).ok_or(Fail::Invalid)?,
            fill_order,
        )
        .ok_or(Fail::Invalid)?;
        let b2 = deflate_byte(
            deflate_src,
            consumed.checked_add(2).ok_or(Fail::Invalid)?,
            fill_order,
        )
        .ok_or(Fail::Invalid)?;
        let b3 = deflate_byte(
            deflate_src,
            consumed.checked_add(3).ok_or(Fail::Invalid)?,
            fill_order,
        )
        .ok_or(Fail::Invalid)?;
        let expected_adler = u32::from_be_bytes([b0, b1, b2, b3]);
        cx.check(Kind::Work, (out.len() as u64).div_ceil(64).max(1))
            .map_err(Fail::Stop)?;
        if adler32(&out) != expected_adler {
            return Err(Fail::Invalid);
        }
        Ok(out)
    } else if compression == 32946 {
        let (out, _) = inflate_budgeted(raw, fill_order, expected_bytes, cx)?;
        if out.len() != expected_bytes {
            return Err(Fail::Invalid);
        }
        Ok(out)
    } else {
        Err(Fail::Invalid)
    }
}

fn deflate_byte(src: &[u8], idx: usize, fill_order: u16) -> Option<u8> {
    src.get(idx)
        .map(|&b| if fill_order == 2 { b.reverse_bits() } else { b })
}

fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for chunk in bytes.chunks(5552) {
        for &b in chunk {
            s1 += u32::from(b);
            s2 += s1;
        }
        s1 %= MOD;
        s2 %= MOD;
    }
    (s2 << 16) | s1
}

struct LsbBits<'a> {
    src: &'a [u8],
    fill_order: u16,
    at: usize,
    buf: u32,
    count: u32,
}

impl LsbBits<'_> {
    fn take(&mut self, n: u32) -> Result<u32, Fail> {
        while self.count < n {
            let byte = deflate_byte(self.src, self.at, self.fill_order).ok_or(Fail::Invalid)?;
            self.at += 1;
            self.buf |= u32::from(byte) << self.count;
            self.count += 8;
        }
        let val = self.buf & ((1u32 << n) - 1);
        self.buf = self.buf.checked_shr(n).unwrap_or(0);
        self.count -= n;
        Ok(val)
    }

    fn align(&mut self) {
        let full_bytes_unread = (self.count / 8) as usize;
        self.at = self.at.saturating_sub(full_bytes_unread);
        self.buf = 0;
        self.count = 0;
    }
}

const DEFLATE_MAX_BITS: usize = 15;
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

#[derive(Clone, Copy)]
enum Gap {
    Never,
    Single,
    Any,
}

struct DeflateHuffman {
    count: [u16; DEFLATE_MAX_BITS + 1],
    symbol: Vec<u16>,
}

impl DeflateHuffman {
    fn new(lengths: &[u8], gap: Gap) -> Option<Self> {
        let mut count = [0u16; DEFLATE_MAX_BITS + 1];
        for &length in lengths {
            *count.get_mut(usize::from(length))? += 1;
        }
        let used = lengths.len() - usize::from(*count.first()?);
        let mut left: i32 = 1;
        for &n in count.iter().skip(1) {
            left = (left << 1) - i32::from(n);
            if left < 0 {
                return None;
            }
        }
        let allowed = match gap {
            Gap::Never => false,
            Gap::Single => used <= 1,
            Gap::Any => true,
        };
        if left > 0 && !allowed {
            return None;
        }
        let mut offsets = [0u16; DEFLATE_MAX_BITS + 2];
        for length in 1..=DEFLATE_MAX_BITS {
            let next = offsets.get(length)? + count.get(length)?;
            *offsets.get_mut(length + 1)? = next;
        }
        let mut symbol = vec![0u16; usize::from(*offsets.get(DEFLATE_MAX_BITS + 1)?)];
        for (index, &length) in lengths.iter().enumerate() {
            if length != 0 {
                let slot = offsets.get_mut(usize::from(length))?;
                *symbol.get_mut(usize::from(*slot))? = u16::try_from(index).ok()?;
                *slot += 1;
            }
        }
        Some(Self { count, symbol })
    }

    fn decode(&self, bits: &mut LsbBits) -> Result<u16, Fail> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;
        for &count in self.count.iter().skip(1) {
            code |= i32::try_from(bits.take(1)?).map_err(|_| Fail::Invalid)?;
            let count = i32::from(count);
            if code - count < first {
                let at = usize::try_from(index + code - first).map_err(|_| Fail::Invalid)?;
                return self.symbol.get(at).copied().ok_or(Fail::Invalid);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err(Fail::Invalid)
    }
}

/// RFC 1951 展開。出力バイトを増やす前に逐次 `Kind::DecodedBytes` を課金して展開爆弾を即停止させる。
fn inflate_budgeted(
    src: &[u8],
    fill_order: u16,
    limit: usize,
    cx: &mut Cx,
) -> Result<(Vec<u8>, usize), Fail> {
    let mut bits = LsbBits {
        src,
        fill_order,
        at: 0,
        buf: 0,
        count: 0,
    };
    let mut out = Vec::with_capacity(limit.min(65536));
    loop {
        cx.check(Kind::Work, 1).map_err(Fail::Stop)?;
        let last = bits.take(1)? == 1;
        match bits.take(2)? {
            0 => {
                bits.align();
                let a = deflate_byte(bits.src, bits.at, fill_order).ok_or(Fail::Invalid)?;
                let b = deflate_byte(bits.src, bits.at + 1, fill_order).ok_or(Fail::Invalid)?;
                let c = deflate_byte(bits.src, bits.at + 2, fill_order).ok_or(Fail::Invalid)?;
                let d = deflate_byte(bits.src, bits.at + 3, fill_order).ok_or(Fail::Invalid)?;
                let len = u16::from_le_bytes([a, b]);
                let nlen = u16::from_le_bytes([c, d]);
                if len != !nlen {
                    return Err(Fail::Invalid);
                }
                let start = bits.at + 4;
                let end = start + usize::from(len);
                let data = bits.src.get(start..end).ok_or(Fail::Invalid)?;
                if out.len() + data.len() > limit {
                    return Err(Fail::Invalid);
                }
                if data.len() >= 64 {
                    cx.check(Kind::Work, (data.len() as u64) / 64)
                        .map_err(Fail::Stop)?;
                }
                cx.check(Kind::DecodedBytes, data.len() as u64)
                    .map_err(Fail::Stop)?;
                if fill_order == 2 {
                    out.extend(data.iter().map(|byte| byte.reverse_bits()));
                } else {
                    out.extend_from_slice(data);
                }
                bits.at = end;
            }
            1 => {
                let (lit, dist) = fixed_tables().ok_or(Fail::Invalid)?;
                decode_deflate_codes(&mut bits, &mut out, limit, &lit, &dist, cx)?;
            }
            2 => {
                let (lit, dist) = dynamic_tables(&mut bits, cx)?;
                decode_deflate_codes(&mut bits, &mut out, limit, &lit, &dist, cx)?;
            }
            _ => return Err(Fail::Invalid),
        }
        if last {
            bits.align();
            return Ok((out, bits.at));
        }
    }
}

fn fixed_tables() -> Option<(DeflateHuffman, DeflateHuffman)> {
    let mut lengths = [0u8; 288];
    for (sym, len) in lengths.iter_mut().enumerate() {
        *len = match sym {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    Some((
        DeflateHuffman::new(&lengths, Gap::Any)?,
        DeflateHuffman::new(&[5u8; 30], Gap::Any)?,
    ))
}

fn dynamic_tables(
    bits: &mut LsbBits,
    cx: &mut Cx,
) -> Result<(DeflateHuffman, DeflateHuffman), Fail> {
    let literals = usize::try_from(bits.take(5)?).map_err(|_| Fail::Invalid)? + 257;
    let distances = usize::try_from(bits.take(5)?).map_err(|_| Fail::Invalid)? + 1;
    let lengths_count = usize::try_from(bits.take(4)?).map_err(|_| Fail::Invalid)? + 4;
    if literals > 286 || distances > 30 {
        return Err(Fail::Invalid);
    }
    let mut code_lengths = [0u8; 19];
    for &at in CODE_LENGTH_ORDER.iter().take(lengths_count) {
        *code_lengths.get_mut(at).ok_or(Fail::Invalid)? =
            u8::try_from(bits.take(3)?).map_err(|_| Fail::Invalid)?;
    }
    let code = DeflateHuffman::new(&code_lengths, Gap::Never).ok_or(Fail::Invalid)?;
    let mut lengths = Vec::with_capacity(literals + distances);
    while lengths.len() < literals + distances {
        cx.check(Kind::Work, 1).map_err(Fail::Stop)?;
        let symbol = code.decode(bits)?;
        let (value, repeat) = match symbol {
            0..=15 => (u8::try_from(symbol).map_err(|_| Fail::Invalid)?, 1),
            16 => (*lengths.last().ok_or(Fail::Invalid)?, 3 + bits.take(2)?),
            17 => (0, 3 + bits.take(3)?),
            _ => (0, 11 + bits.take(7)?),
        };
        for _ in 0..repeat {
            lengths.push(value);
        }
        if lengths.len() > literals + distances {
            return Err(Fail::Invalid);
        }
    }
    if lengths.get(256).copied().unwrap_or(0) == 0 {
        return Err(Fail::Invalid);
    }
    let (lit, dist) = lengths.split_at(literals);
    Ok((
        DeflateHuffman::new(lit, Gap::Single).ok_or(Fail::Invalid)?,
        DeflateHuffman::new(dist, Gap::Single).ok_or(Fail::Invalid)?,
    ))
}

fn decode_deflate_codes(
    bits: &mut LsbBits,
    out: &mut Vec<u8>,
    limit: usize,
    literal: &DeflateHuffman,
    distance: &DeflateHuffman,
    cx: &mut Cx,
) -> Result<(), Fail> {
    loop {
        cx.check(Kind::Work, 1).map_err(Fail::Stop)?;
        let symbol = literal.decode(bits)?;
        match symbol {
            0..=255 => {
                if out.len() >= limit {
                    cx.check(Kind::DecodedBytes, 1).map_err(Fail::Stop)?;
                    return Err(Fail::Invalid);
                }
                cx.check(Kind::DecodedBytes, 1).map_err(Fail::Stop)?;
                out.push(u8::try_from(symbol).map_err(|_| Fail::Invalid)?);
            }
            256 => return Ok(()),
            _ => {
                let at = usize::from(symbol - 257);
                let base = *LENGTH_BASE.get(at).ok_or(Fail::Invalid)?;
                let extra = *LENGTH_EXTRA.get(at).ok_or(Fail::Invalid)?;
                let length = usize::from(base)
                    + usize::try_from(bits.take(u32::from(extra))?).map_err(|_| Fail::Invalid)?;
                let at = usize::from(distance.decode(bits)?);
                let base = *DISTANCE_BASE.get(at).ok_or(Fail::Invalid)?;
                let extra = *DISTANCE_EXTRA.get(at).ok_or(Fail::Invalid)?;
                let back = usize::from(base)
                    + usize::try_from(bits.take(u32::from(extra))?).map_err(|_| Fail::Invalid)?;
                if length >= 16 {
                    cx.check(Kind::Work, (length as u64) / 16)
                        .map_err(Fail::Stop)?;
                }
                cx.check(Kind::DecodedBytes, length as u64)
                    .map_err(Fail::Stop)?;
                if back > out.len() || out.len() + length > limit {
                    return Err(Fail::Invalid);
                }
                let from = out.len() - back;
                for offset in 0..length {
                    let byte = *out.get(from + offset).ok_or(Fail::Invalid)?;
                    out.push(byte);
                }
            }
        }
    }
}

/// ITU-T T.6（CCITT Group 4）2 次元符号化の展開。
///
/// 各行は白（`0`）から反転する変化点座標列 `Vec<u32>`（偶数番目が黒開始、奇数番目が白開始）で表し、
/// 行幅に比例する一時配列確保と線形走査をなくす。
fn decompress_group4(
    raw: &[u8],
    fill_order: u16,
    width: u32,
    rows: u32,
    expected_bytes: usize,
    cx: &mut Cx,
) -> Result<Vec<u8>, Fail> {
    if width == 0 {
        return Err(Fail::Invalid);
    }
    let h = usize::try_from(rows).map_err(|_| Fail::Invalid)?;
    let row_bytes = usize::try_from(u64::from(width).div_ceil(8)).map_err(|_| Fail::Invalid)?;
    if row_bytes.checked_mul(h) != Some(expected_bytes) {
        return Err(Fail::Invalid);
    }

    let mut bits = MsbBits::new(raw, fill_order);
    let mut ref_changes: Vec<u32> = Vec::new();
    let mut cur_changes: Vec<u32> = Vec::new();
    let mut out = Vec::with_capacity(expected_bytes.min(65536));

    for _row in 0..h {
        cx.check(Kind::DecodedBytes, row_bytes as u64)
            .map_err(Fail::Stop)?;
        cx.check(Kind::Work, (row_bytes as u64).div_ceil(64).max(1))
            .map_err(Fail::Stop)?;
        cur_changes.clear();
        let mut ref_cursor = 0usize;
        let mut a0_pos = 0u32;
        let mut at_line_start = true;
        let mut color = 0u8;

        while a0_pos < width {
            cx.check(Kind::Work, 1).map_err(Fail::Stop)?;
            let mode = decode_g4_mode(&mut bits)?;
            let (b1, b2) = find_b1_b2(
                &ref_changes,
                &mut ref_cursor,
                width,
                a0_pos,
                at_line_start,
                color,
            );
            match mode {
                G4Mode::Pass => {
                    if !at_line_start && b2 <= a0_pos {
                        return Err(Fail::Invalid);
                    }
                    if b2 > width {
                        return Err(Fail::Invalid);
                    }
                    append_g4_run(&mut cur_changes, a0_pos, b2, color, cx)?;
                    a0_pos = b2;
                    at_line_start = false;
                }
                G4Mode::Horizontal => {
                    let r1 = u32::try_from(decode_t4_run(&mut bits, color == 0, cx)?)
                        .map_err(|_| Fail::Invalid)?;
                    let r2 = u32::try_from(decode_t4_run(&mut bits, color != 0, cx)?)
                        .map_err(|_| Fail::Invalid)?;
                    if r1 == 0 && r2 == 0 {
                        return Err(Fail::Invalid);
                    }
                    let a1 = a0_pos.checked_add(r1).ok_or(Fail::Invalid)?;
                    let a2 = a1.checked_add(r2).ok_or(Fail::Invalid)?;
                    if a2 > width {
                        return Err(Fail::Invalid);
                    }
                    append_g4_run(&mut cur_changes, a0_pos, a1, color, cx)?;
                    append_g4_run(&mut cur_changes, a1, a2, 1 - color, cx)?;
                    a0_pos = a2;
                    at_line_start = false;
                }
                G4Mode::Vertical(offset) => {
                    let a1_i = i64::from(b1) + i64::from(offset);
                    if a1_i < 0 {
                        return Err(Fail::Invalid);
                    }
                    let a1 = u32::try_from(a1_i).map_err(|_| Fail::Invalid)?;
                    if a1 > width || (!at_line_start && a1 <= a0_pos) {
                        return Err(Fail::Invalid);
                    }
                    append_g4_run(&mut cur_changes, a0_pos, a1, color, cx)?;
                    a0_pos = a1;
                    at_line_start = false;
                    color = 1 - color;
                }
            }
        }

        // 1 行分の黒区間を MSB 先頭で詰め込む
        let base = out.len();
        out.resize(base + row_bytes, 0);
        if let Some(dst_row) = out.get_mut(base..base + row_bytes) {
            pack_g4_row(&cur_changes, width, dst_row, cx)?;
        }
        std::mem::swap(&mut ref_changes, &mut cur_changes);
    }

    Ok(out)
}

fn append_g4_run(
    cur_changes: &mut Vec<u32>,
    start: u32,
    end: u32,
    color: u8,
    cx: &mut Cx,
) -> Result<(), Fail> {
    if start > end {
        return Err(Fail::Invalid);
    }
    if start == end {
        return Ok(());
    }
    let last_color = (cur_changes.len() & 1) as u8;
    if last_color != color {
        cx.check(Kind::DecodedBytes, 4).map_err(Fail::Stop)?;
        cur_changes.push(start);
    }
    Ok(())
}

fn pack_g4_row(
    cur_changes: &[u32],
    width: u32,
    dst_row: &mut [u8],
    cx: &mut Cx,
) -> Result<(), Fail> {
    if !cur_changes.is_empty() {
        cx.check(Kind::Work, (cur_changes.len() as u64).div_ceil(8).max(1))
            .map_err(Fail::Stop)?;
    }
    for pair in cur_changes.chunks(2) {
        let Some(&s32) = pair.first() else {
            continue;
        };
        let e32 = pair.get(1).copied().unwrap_or(width);
        if s32 >= e32 {
            continue;
        }
        let start = usize::try_from(s32).map_err(|_| Fail::Invalid)?;
        let end = usize::try_from(e32).map_err(|_| Fail::Invalid)?;
        let first_byte = start / 8;
        let last_byte = (end - 1) / 8;
        let start_mask = 0xFFu8 >> (start % 8);
        let end_rem = end % 8;
        let end_mask = if end_rem == 0 {
            0xFFu8
        } else {
            0xFFu8 << (8 - end_rem)
        };
        if first_byte == last_byte {
            if let Some(slot) = dst_row.get_mut(first_byte) {
                *slot |= start_mask & end_mask;
            }
        } else {
            if let Some(slot) = dst_row.get_mut(first_byte) {
                *slot |= start_mask;
            }
            if let Some(mid) = dst_row.get_mut((first_byte + 1)..last_byte) {
                mid.fill(0xFF);
            }
            if let Some(slot) = dst_row.get_mut(last_byte) {
                *slot |= end_mask;
            }
        }
    }
    Ok(())
}

fn find_b1_b2(
    ref_changes: &[u32],
    ref_cursor: &mut usize,
    width: u32,
    a0_pos: u32,
    at_line_start: bool,
    color: u8,
) -> (u32, u32) {
    let start_x = if at_line_start {
        0
    } else {
        a0_pos.saturating_add(1)
    };
    while let Some(&pos) = ref_changes.get(*ref_cursor) {
        if pos < start_x {
            *ref_cursor += 1;
        } else {
            break;
        }
    }
    let idx = *ref_cursor;
    if idx >= ref_changes.len() {
        return (width, width);
    }
    // 偶数番目は黒（`1`）への変化点、奇数番目は白（`0`）への変化点
    let idx_color = if idx.is_multiple_of(2) { 1u8 } else { 0u8 };
    let b1_idx = if idx_color != color { idx } else { idx + 1 };
    let Some(&b1) = ref_changes.get(b1_idx) else {
        return (width, width);
    };
    let b2 = ref_changes.get(b1_idx + 1).copied().unwrap_or(width);
    (b1, b2)
}

#[derive(Clone, Copy)]
enum G4Mode {
    Pass,
    Horizontal,
    Vertical(i8),
}

fn decode_g4_mode(bits: &mut MsbBits<'_>) -> Result<G4Mode, Fail> {
    if bits.take(1).ok_or(Fail::Invalid)? == 1 {
        return Ok(G4Mode::Vertical(0));
    }
    if bits.take(1).ok_or(Fail::Invalid)? == 1 {
        let dir = bits.take(1).ok_or(Fail::Invalid)?;
        return Ok(if dir == 1 {
            G4Mode::Vertical(1)
        } else {
            G4Mode::Vertical(-1)
        });
    }
    if bits.take(1).ok_or(Fail::Invalid)? == 1 {
        return Ok(G4Mode::Horizontal);
    }
    if bits.take(1).ok_or(Fail::Invalid)? == 1 {
        return Ok(G4Mode::Pass);
    }
    if bits.take(1).ok_or(Fail::Invalid)? == 1 {
        let dir = bits.take(1).ok_or(Fail::Invalid)?;
        return Ok(if dir == 1 {
            G4Mode::Vertical(2)
        } else {
            G4Mode::Vertical(-2)
        });
    }
    if bits.take(1).ok_or(Fail::Invalid)? == 1 {
        let dir = bits.take(1).ok_or(Fail::Invalid)?;
        return Ok(if dir == 1 {
            G4Mode::Vertical(3)
        } else {
            G4Mode::Vertical(-3)
        });
    }
    // 0000001xxx（拡張）や 000000000001（早期 EOFB）は不正とする
    Err(Fail::Invalid)
}

/// TIFF 6.0 Section 10・ITU-T T.6 の白・黒ランレングス符号表 `(bit_len, code_bits, run_len)`
const WHITE_CODES: [(u8, u16, u16); 104] = [
    // Terminating 0..=63
    (8, 53, 0),
    (6, 7, 1),
    (4, 7, 2),
    (4, 8, 3),
    (4, 11, 4),
    (4, 12, 5),
    (4, 14, 6),
    (4, 15, 7),
    (5, 19, 8),
    (5, 20, 9),
    (5, 7, 10),
    (5, 8, 11),
    (6, 8, 12),
    (6, 3, 13),
    (6, 52, 14),
    (6, 53, 15),
    (6, 42, 16),
    (6, 43, 17),
    (7, 39, 18),
    (7, 12, 19),
    (7, 8, 20),
    (7, 23, 21),
    (7, 3, 22),
    (7, 4, 23),
    (7, 40, 24),
    (7, 43, 25),
    (7, 19, 26),
    (7, 36, 27),
    (7, 24, 28),
    (8, 2, 29),
    (8, 3, 30),
    (8, 26, 31),
    (8, 27, 32),
    (8, 18, 33),
    (8, 19, 34),
    (8, 20, 35),
    (8, 21, 36),
    (8, 22, 37),
    (8, 23, 38),
    (8, 40, 39),
    (8, 41, 40),
    (8, 42, 41),
    (8, 43, 42),
    (8, 44, 43),
    (8, 45, 44),
    (8, 4, 45),
    (8, 5, 46),
    (8, 10, 47),
    (8, 11, 48),
    (8, 82, 49),
    (8, 83, 50),
    (8, 84, 51),
    (8, 85, 52),
    (8, 36, 53),
    (8, 37, 54),
    (8, 88, 55),
    (8, 89, 56),
    (8, 90, 57),
    (8, 91, 58),
    (8, 74, 59),
    (8, 75, 60),
    (8, 50, 61),
    (8, 51, 62),
    (8, 52, 63),
    // Makeup 64..=2560
    (5, 27, 64),
    (5, 18, 128),
    (6, 23, 192),
    (7, 55, 256),
    (8, 54, 320),
    (8, 55, 384),
    (8, 100, 448),
    (8, 101, 512),
    (8, 104, 576),
    (8, 103, 640),
    (9, 204, 704),
    (9, 205, 768),
    (9, 210, 832),
    (9, 211, 896),
    (9, 212, 960),
    (9, 213, 1024),
    (9, 214, 1088),
    (9, 215, 1152),
    (9, 216, 1216),
    (9, 217, 1280),
    (9, 218, 1344),
    (9, 219, 1408),
    (9, 152, 1472),
    (9, 153, 1536),
    (9, 154, 1600),
    (6, 24, 1664),
    (9, 155, 1728),
    (11, 8, 1792),
    (11, 12, 1856),
    (11, 13, 1920),
    (12, 18, 1984),
    (12, 19, 2048),
    (12, 20, 2112),
    (12, 21, 2176),
    (12, 22, 2240),
    (12, 23, 2304),
    (12, 28, 2368),
    (12, 29, 2432),
    (12, 30, 2496),
    (12, 31, 2560),
];

const BLACK_CODES: [(u8, u16, u16); 104] = [
    // Terminating 0..=63
    (10, 55, 0),
    (3, 2, 1),
    (2, 3, 2),
    (2, 2, 3),
    (3, 3, 4),
    (4, 3, 5),
    (4, 2, 6),
    (5, 3, 7),
    (6, 5, 8),
    (6, 4, 9),
    (7, 4, 10),
    (7, 5, 11),
    (7, 7, 12),
    (8, 4, 13),
    (8, 7, 14),
    (9, 24, 15),
    (10, 23, 16),
    (10, 24, 17),
    (10, 8, 18),
    (11, 103, 19),
    (11, 104, 20),
    (11, 108, 21),
    (11, 55, 22),
    (11, 40, 23),
    (11, 23, 24),
    (11, 24, 25),
    (12, 202, 26),
    (12, 203, 27),
    (12, 204, 28),
    (12, 205, 29),
    (12, 104, 30),
    (12, 105, 31),
    (12, 106, 32),
    (12, 107, 33),
    (12, 210, 34),
    (12, 211, 35),
    (12, 212, 36),
    (12, 213, 37),
    (12, 214, 38),
    (12, 215, 39),
    (12, 108, 40),
    (12, 109, 41),
    (12, 218, 42),
    (12, 219, 43),
    (12, 84, 44),
    (12, 85, 45),
    (12, 86, 46),
    (12, 87, 47),
    (12, 100, 48),
    (12, 101, 49),
    (12, 82, 50),
    (12, 83, 51),
    (12, 36, 52),
    (12, 55, 53),
    (12, 56, 54),
    (12, 39, 55),
    (12, 40, 56),
    (12, 88, 57),
    (12, 89, 58),
    (12, 43, 59),
    (12, 44, 60),
    (12, 90, 61),
    (12, 102, 62),
    (12, 103, 63),
    // Makeup 64..=2560
    (10, 15, 64),
    (12, 200, 128),
    (12, 201, 192),
    (12, 91, 256),
    (12, 51, 320),
    (12, 52, 384),
    (12, 53, 448),
    (13, 108, 512),
    (13, 109, 576),
    (13, 74, 640),
    (13, 75, 704),
    (13, 76, 768),
    (13, 77, 832),
    (13, 114, 896),
    (13, 115, 960),
    (13, 116, 1024),
    (13, 117, 1088),
    (13, 118, 1152),
    (13, 119, 1216),
    (13, 82, 1280),
    (13, 83, 1344),
    (13, 84, 1408),
    (13, 85, 1472),
    (13, 90, 1536),
    (13, 91, 1600),
    (13, 100, 1664),
    (13, 101, 1728),
    (11, 8, 1792),
    (11, 12, 1856),
    (11, 13, 1920),
    (12, 18, 1984),
    (12, 19, 2048),
    (12, 20, 2112),
    (12, 21, 2176),
    (12, 22, 2240),
    (12, 23, 2304),
    (12, 28, 2368),
    (12, 29, 2432),
    (12, 30, 2496),
    (12, 31, 2560),
];

fn decode_t4_run(bits: &mut MsbBits<'_>, is_white: bool, cx: &mut Cx) -> Result<usize, Fail> {
    let table = if is_white { &WHITE_CODES } else { &BLACK_CODES };
    let min_len = if is_white { 4u8 } else { 2u8 };
    let max_len = if is_white { 12u8 } else { 13u8 };

    let mut total = 0usize;
    loop {
        cx.check(Kind::Work, 1).map_err(Fail::Stop)?;
        let mut acc: u16 = 0;
        let mut cur_len: u8 = 0;
        let mut matched: Option<u16> = None;
        while cur_len < max_len {
            let bit = bits.take(1).ok_or(Fail::Invalid)?;
            acc = (acc << 1) | bit;
            cur_len += 1;
            if cur_len >= min_len
                && let Some(&(_, _, run)) =
                    table.iter().find(|&&(l, c, _)| l == cur_len && c == acc)
            {
                matched = Some(run);
                break;
            }
        }
        let run = usize::from(matched.ok_or(Fail::Invalid)?);
        total = total.checked_add(run).ok_or(Fail::Invalid)?;
        if run < 64 {
            return Ok(total);
        }
    }
}

/// TIFF 6.0 Section 14 の水平差分（`Predictor = 2`）を元に戻す
pub(crate) fn apply_predictor_2(
    buf: &mut [u8],
    width: u32,
    rows: u32,
    bps: u16,
    spp: u16,
    endian: Endian,
    cx: &mut Cx,
) -> Result<(), Stop> {
    let w = width as usize;
    let h = rows as usize;
    let stride = usize::from(spp);
    match bps {
        8 => {
            let row_bytes = w * stride;
            cx.check(Kind::Work, (row_bytes * h) as u64)?;
            for r in 0..h {
                if let Some(row) = buf.get_mut(r * row_bytes..(r + 1) * row_bytes) {
                    for i in stride..row.len() {
                        let prev = row.get(i - stride).copied().unwrap_or(0);
                        if let Some(cur) = row.get_mut(i) {
                            *cur = cur.wrapping_add(prev);
                        }
                    }
                }
            }
        }
        16 => {
            let samples_per_row = w * stride;
            let row_bytes = samples_per_row * 2;
            cx.check(Kind::Work, (samples_per_row * h) as u64)?;
            for r in 0..h {
                if let Some(row) = buf.get_mut(r * row_bytes..(r + 1) * row_bytes) {
                    for s in stride..samples_per_row {
                        let prev_off = (s - stride) * 2;
                        let cur_off = s * 2;
                        let prev = endian.read_u16(row, prev_off).unwrap_or(0);
                        let cur = endian.read_u16(row, cur_off).unwrap_or(0);
                        let sum = cur.wrapping_add(prev);
                        let bytes = match endian {
                            Endian::Little => sum.to_le_bytes(),
                            Endian::Big => sum.to_be_bytes(),
                        };
                        if let Some(dst) = row.get_mut(cur_off..cur_off + 2) {
                            dst.copy_from_slice(&bytes);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
