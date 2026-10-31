//! deflate の展開（RFC 1951）。表を引かず、符号を 1 ビットずつたどる（zlib の puff と同じ方法）。

use zumen_base::{Cx, Kind, Stop};

/// 展開を止めた理由
pub(crate) enum Fail {
    /// データが壊れているか、展開した大きさが上限を超える
    Invalid,
    /// 中断の合図
    Stop(Stop),
}

/// 符号の長さの上限
const MAX_BITS: usize = 15;
/// 中断の合図を確かめる、出力のバイト数の間隔
const CHECK_EVERY: usize = 1 << 16;

/// 長さの符号（257〜285）の基の長さと、足すビットの数
const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
/// 距離の符号（0〜29）の基の距離と、足すビットの数
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
/// 符号の長さの符号を書く順
const ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

struct Bits<'a> {
    src: &'a [u8],
    at: usize,
    buf: u32,
    count: u32,
}

impl Bits<'_> {
    /// 下位のビットから `n`（16 以下）ビットを読む
    fn take(&mut self, n: u32) -> Result<u32, Fail> {
        while self.count < n {
            let byte = *self.src.get(self.at).ok_or(Fail::Invalid)?;
            self.at += 1;
            self.buf |= u32::from(byte) << self.count;
            self.count += 8;
        }
        let value = self.buf & ((1u32 << n) - 1);
        self.buf = self.buf.checked_shr(n).unwrap_or(0);
        self.count -= n;
        Ok(value)
    }

    /// バイトの境目まで残りのビットを捨てる
    fn align(&mut self) {
        self.buf = 0;
        self.count = 0;
    }
}

/// 符号の足りない組を許すか
#[derive(Clone, Copy)]
enum Gap {
    Never,
    Single,
    /// 固定の符号。距離の符号は 32 の枠に 30 だけを使う
    Any,
}

/// 標準の形のハフマン符号。長さごとの符号の数と、符号の順の記号
struct Huffman {
    count: [u16; MAX_BITS + 1],
    symbol: Vec<u16>,
}

impl Huffman {
    /// 記号ごとの符号の長さから作る。符号が多すぎるときは `None`。符号の足りない組を、`gap` が
    /// `Gap::Never` なら許さず、`Gap::Single` なら符号が 1 つ以下のときにだけ許す（zlib と同じ）
    fn new(lengths: &[u8], gap: Gap) -> Option<Self> {
        let mut count = [0u16; MAX_BITS + 1];
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
        let mut offsets = [0u16; MAX_BITS + 2];
        for length in 1..=MAX_BITS {
            let next = offsets.get(length)? + count.get(length)?;
            *offsets.get_mut(length + 1)? = next;
        }
        let mut symbol = vec![0u16; usize::from(*offsets.get(MAX_BITS + 1)?)];
        for (index, &length) in lengths.iter().enumerate() {
            if length != 0 {
                let slot = offsets.get_mut(usize::from(length))?;
                *symbol.get_mut(usize::from(*slot))? = u16::try_from(index).ok()?;
                *slot += 1;
            }
        }
        Some(Self { count, symbol })
    }

    fn decode(&self, bits: &mut Bits) -> Result<u16, Fail> {
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

/// `src` の deflate のデータを展開する。出力が `limit` バイトを超えるなら壊れているとする
pub(crate) fn inflate(src: &[u8], limit: usize, cx: &mut Cx) -> Result<Vec<u8>, Fail> {
    let mut bits = Bits {
        src,
        at: 0,
        buf: 0,
        count: 0,
    };
    let mut out = Vec::with_capacity(limit.min(1 << 20));
    let mut checked = 0;
    loop {
        cx.check(Kind::Work, 0).map_err(Fail::Stop)?;
        let last = bits.take(1)? == 1;
        match bits.take(2)? {
            0 => stored(&mut bits, &mut out, limit)?,
            1 => {
                let (literal, distance) = fixed().ok_or(Fail::Invalid)?;
                codes(
                    &mut bits,
                    &mut out,
                    limit,
                    &literal,
                    &distance,
                    &mut checked,
                    cx,
                )?;
            }
            2 => {
                let (literal, distance) = dynamic(&mut bits)?;
                codes(
                    &mut bits,
                    &mut out,
                    limit,
                    &literal,
                    &distance,
                    &mut checked,
                    cx,
                )?;
            }
            _ => return Err(Fail::Invalid),
        }
        if last {
            return Ok(out);
        }
    }
}

fn stored(bits: &mut Bits, out: &mut Vec<u8>, limit: usize) -> Result<(), Fail> {
    bits.align();
    let header = bits.src.get(bits.at..bits.at + 4).ok_or(Fail::Invalid)?;
    let [a, b, c, d] = <[u8; 4]>::try_from(header).map_err(|_| Fail::Invalid)?;
    let length = u16::from_le_bytes([a, b]);
    let check = u16::from_le_bytes([c, d]);
    if length != !check {
        return Err(Fail::Invalid);
    }
    let start = bits.at + 4;
    let end = start + usize::from(length);
    let data = bits.src.get(start..end).ok_or(Fail::Invalid)?;
    if out.len() + data.len() > limit {
        return Err(Fail::Invalid);
    }
    out.extend_from_slice(data);
    bits.at = end;
    Ok(())
}

fn fixed() -> Option<(Huffman, Huffman)> {
    let mut lengths = [0u8; 288];
    for (symbol, length) in lengths.iter_mut().enumerate() {
        *length = match symbol {
            0..=143 => 8,
            144..=255 => 9,
            256..=279 => 7,
            _ => 8,
        };
    }
    Some((
        Huffman::new(&lengths, Gap::Any)?,
        Huffman::new(&[5u8; 30], Gap::Any)?,
    ))
}

fn dynamic(bits: &mut Bits) -> Result<(Huffman, Huffman), Fail> {
    let literals = usize::try_from(bits.take(5)?).map_err(|_| Fail::Invalid)? + 257;
    let distances = usize::try_from(bits.take(5)?).map_err(|_| Fail::Invalid)? + 1;
    let lengths_count = usize::try_from(bits.take(4)?).map_err(|_| Fail::Invalid)? + 4;
    if literals > 286 || distances > 30 {
        return Err(Fail::Invalid);
    }
    let mut code_lengths = [0u8; 19];
    for &at in ORDER.iter().take(lengths_count) {
        *code_lengths.get_mut(at).ok_or(Fail::Invalid)? =
            u8::try_from(bits.take(3)?).map_err(|_| Fail::Invalid)?;
    }
    let code = Huffman::new(&code_lengths, Gap::Never).ok_or(Fail::Invalid)?;
    let mut lengths = Vec::with_capacity(literals + distances);
    while lengths.len() < literals + distances {
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
    // 終わりの符号（256）がなければ、ブロックを終えられない
    if lengths.get(256).copied().unwrap_or(0) == 0 {
        return Err(Fail::Invalid);
    }
    let (literal, distance) = lengths.split_at(literals);
    Ok((
        Huffman::new(literal, Gap::Single).ok_or(Fail::Invalid)?,
        Huffman::new(distance, Gap::Single).ok_or(Fail::Invalid)?,
    ))
}

fn codes(
    bits: &mut Bits,
    out: &mut Vec<u8>,
    limit: usize,
    literal: &Huffman,
    distance: &Huffman,
    checked: &mut usize,
    cx: &mut Cx,
) -> Result<(), Fail> {
    loop {
        if out.len() >= *checked + CHECK_EVERY {
            cx.check(Kind::Work, 0).map_err(Fail::Stop)?;
            *checked = out.len();
        }
        let symbol = literal.decode(bits)?;
        match symbol {
            0..=255 => {
                if out.len() >= limit {
                    return Err(Fail::Invalid);
                }
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
