//! R2004 の入れ物（ODS 5.4.1 の 4「R2004 DWG FILE FORMAT ORGANIZATION」）。
//!
//! ファイルの見出しの後ろに、暗号化した見出しがある。そこからページの地図と区切りの地図を読み、
//! 区切りの名前ごとに、ページを展開してつなげたバイト列を作る（決定 0026）。
//! 書くときは、ページを並べ直して番号を振り直し、2 つの地図と見出しを作り直す（決定 0070）。

use core::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::Error;

use super::edit::put;
use super::{le32, stopped};

const R2004: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "4 R2004 DWG FILE FORMAT ORGANIZATION",
};

/// 暗号化した見出しの位置と大きさ
const HEADER: Range<usize> = 0x80..0x80 + 0x6C;

/// 暗号化した見出しの初めの印
const HEADER_MAGIC: &[u8; 12] = b"AcFssFcAJMB\0";

/// ページの位置を数え始める所
const PAGES_START: u64 = 0x100;

/// ページの地図のページの型
const PAGE_MAP_TYPE: u32 = 0x4163_0E3B;

/// 区切りの地図のページの型
const SECTION_MAP_TYPE: u32 = 0x4163_003B;

/// 区切りのデータのページの型
const DATA_PAGE_TYPE: u32 = 0x4163_043B;

/// データのページの見出しを隠す値。ページの位置と排他的論理和をとって使う
const DATA_PAGE_MASK: u32 = 0x4164_536B;

/// 地図のページの見出しの大きさ
const SYSTEM_PAGE_HEADER: usize = 0x14;

/// データのページの見出しの大きさ
const DATA_PAGE_HEADER: usize = 0x20;

/// 区切りの地図の 1 つの区切りの、ページの並びの前の大きさ
const DESCRIPTION_SIZE: usize = 0x60;

/// 区切りの地図の 1 つのページの大きさ
const DESCRIPTION_PAGE_SIZE: usize = 0x10;

/// 区切りの地図の、区切りの説明の前の大きさ
const SECTION_MAP_HEAD: usize = 20;

/// 地図のページの終わりに置く、空のページの見出し
const EMPTY_PAGE: [u8; SYSTEM_PAGE_HEADER] = [
    0x3B, 0x0E, 0x63, 0x41, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0,
];

/// 読んだ R2004 の入れ物
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Container {
    /// 戻した暗号化した見出し
    header: [u8; 0x6C],
    /// 区切りの地図の、区切りの説明の前のバイト列
    map_head: [u8; SECTION_MAP_HEAD],
    sections: Vec<Section>,
}

/// 区切りの説明と、ページを展開してつなげたバイト列
#[derive(Clone, Debug, PartialEq, Eq)]
struct Section {
    /// 区切りの地図の説明（大きさ・ページの数・名前など）
    description: [u8; DESCRIPTION_SIZE],
    name: Vec<u8>,
    /// 暗号化した区切りは読まないので `None`
    data: Option<Vec<u8>>,
    /// ページの位置と、区切りの中の位置
    pages: Vec<(u64, u64)>,
    /// `data` を置き換えたか
    replaced: bool,
}

impl Container {
    /// 名前（`AcDb:Header`）の区切り
    pub(super) fn section(&self, name: &[u8]) -> Option<&[u8]> {
        self.sections
            .iter()
            .find(|section| section.name == name)
            .and_then(|section| section.data.as_deref())
    }

    /// 名前の区切りを置き換える。読めた区切りがなければ `None`
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "R2004 のオブジェクトの書き換えで使う")
    )]
    pub(super) fn replace(&mut self, name: &[u8], data: Vec<u8>) -> Option<()> {
        let section = self
            .sections
            .iter_mut()
            .find(|section| section.name == name && section.data.is_some())?;
        section.data = Some(data);
        section.replaced = true;
        Some(())
    }

    /// 置き換えた区切りがあるか
    pub(super) fn replaced(&self) -> bool {
        self.sections.iter().any(|section| section.replaced)
    }
}

/// ページの地図の 1 つのページ
#[derive(Clone, Copy, Debug)]
struct Page {
    number: i32,
    address: u64,
}

/// 入れ物を読む。見出しとページの CRC が合わないときは `dwg.crc.invalid` を返して読み続ける
pub(super) fn read(src: &[u8], cx: &mut Cx) -> Result<Container, Error> {
    let header = header(src, cx)?;
    let page_map_at = u64::from_le_bytes(header[0x54..0x5C].try_into().unwrap_or_default());
    let section_map_id = field(&header, 0x5C);
    let pages = page_map(src, page_map_at.saturating_add(PAGES_START), cx)?;
    let section_map_at = pages
        .iter()
        .find(|page| i64::from(page.number) == i64::from(section_map_id))
        .map(|page| page.address)
        .ok_or_else(|| invalid(cx, "section map", HEADER))?;
    let map = system_page(src, section_map_at, SECTION_MAP_TYPE, "section map", cx)?;
    let sections = section_map(src, &map, &pages, cx)?;
    let mut map_head = [0; SECTION_MAP_HEAD];
    map_head.copy_from_slice(
        map.get(..SECTION_MAP_HEAD)
            .unwrap_or(&[0; SECTION_MAP_HEAD]),
    );
    Ok(Container {
        header,
        map_head,
        sections,
    })
}

/// 暗号化した見出しを戻し、初めの印と CRC を確かめる
fn header(src: &[u8], cx: &mut Cx) -> Result<[u8; 0x6C], Error> {
    let bytes = src
        .get(HEADER)
        .ok_or_else(|| invalid(cx, "r2004 header", 0..src.len()))?;
    let mut header = [0; 0x6C];
    header.copy_from_slice(bytes);
    let header = crypt(&header);
    if !header.starts_with(HEADER_MAGIC) {
        return Err(invalid(cx, "r2004 header", HEADER));
    }
    let mut zeroed = header;
    zeroed[0x68..].fill(0);
    if crc32(&zeroed) != field(&header, 0x68) {
        cx.report(crc_invalid("r2004 header", HEADER));
    }
    Ok(header)
}

/// 見出しを暗号化する、または戻す。乱数の列と排他的論理和をとる
fn crypt(bytes: &[u8; 0x6C]) -> [u8; 0x6C] {
    let mut out = [0; 0x6C];
    let mut seed: u32 = 1;
    for (out, byte) in out.iter_mut().zip(bytes) {
        seed = seed.wrapping_mul(0x343FD).wrapping_add(0x0026_9EC3);
        *out = byte ^ (seed >> 16) as u8;
    }
    out
}

/// ページの地図を読む。ページの位置は、`0x100` からページの大きさを足していく
fn page_map(src: &[u8], at: u64, cx: &mut Cx) -> Result<Vec<Page>, Error> {
    let map = system_page(src, at, PAGE_MAP_TYPE, "page map", cx)?;
    let count = map.len() / 8;
    cx.check(Kind::Records, count as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut pages = Vec::with_capacity(count);
    let mut address = PAGES_START;
    let mut pos = 0;
    while pos < map.len() {
        let (Some(number), Some(size)) = (le32(&map, pos), le32(&map, pos + 4)) else {
            return Err(invalid(cx, "page map", page_range(at)));
        };
        let number = number as i32;
        pos += 8;
        // 負の番号は空いたページで、親と左右のページと 0 の 4 つの値が続く
        if number < 0 {
            pos += 16;
        }
        pages.push(Page { number, address });
        address = address.saturating_add(u64::from(size));
    }
    Ok(pages)
}

/// 地図のページの見出しを確かめ、データを展開する
fn system_page(src: &[u8], at: u64, kind: u32, part: &str, cx: &mut Cx) -> Result<Vec<u8>, Error> {
    let start = usize::try_from(at).unwrap_or(usize::MAX);
    let head = start
        .checked_add(SYSTEM_PAGE_HEADER)
        .and_then(|end| src.get(start..end))
        .ok_or_else(|| invalid(cx, part, page_range(at)))?;
    let [page_type, decompressed, compressed, compression, checksum] =
        core::array::from_fn(|i| le32(head, i * 4).unwrap_or_default());
    if page_type != kind || compression != 2 {
        return Err(invalid(cx, part, page_range(at)));
    }
    let data_start = start + SYSTEM_PAGE_HEADER;
    let data = usize::try_from(compressed)
        .ok()
        .and_then(|size| data_start.checked_add(size))
        .and_then(|end| src.get(data_start..end))
        .ok_or_else(|| invalid(cx, part, page_range(at)))?;
    let mut zeroed = [0; SYSTEM_PAGE_HEADER];
    zeroed.copy_from_slice(head);
    zeroed[16..].fill(0);
    if adler(adler(0, &zeroed), data) != checksum {
        cx.report(crc_invalid(part, start..data_start + data.len()));
    }
    let size = usize::try_from(decompressed).unwrap_or(usize::MAX);
    cx.check(Kind::DecodedBytes, size as u64)
        .map_err(|stop| stopped(cx, stop))?;
    decompress(data, size)
        .filter(|out| out.len() == size)
        .ok_or_else(|| invalid(cx, part, start..data_start + data.len()))
}

/// 区切りの地図を読み、区切りごとにページを展開してつなげる。暗号化した区切りは読まない
fn section_map(src: &[u8], map: &[u8], pages: &[Page], cx: &mut Cx) -> Result<Vec<Section>, Error> {
    let count = le32(map, 0).ok_or_else(|| invalid(cx, "section map", HEADER))?;
    cx.check(Kind::Records, u64::from(count))
        .map_err(|stop| stopped(cx, stop))?;
    let mut sections = Vec::new();
    let mut pos: usize = 20;
    for _ in 0..count {
        let description: [u8; DESCRIPTION_SIZE] = pos
            .checked_add(DESCRIPTION_SIZE)
            .and_then(|end| map.get(pos..end))
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| invalid(cx, "section map", HEADER))?;
        let size = le64(&description, 0).unwrap_or_default();
        let [page_count, max_size, _, compressed, _, encrypted] =
            core::array::from_fn(|i| le32(&description, 8 + i * 4).unwrap_or_default());
        let name = description.get(0x20..).unwrap_or_default();
        let name: Vec<u8> = name.iter().copied().take_while(|&b| b != 0).collect();
        pos += DESCRIPTION_SIZE;
        let list = usize::try_from(page_count)
            .ok()
            .and_then(|n| n.checked_mul(DESCRIPTION_PAGE_SIZE))
            .and_then(|len| pos.checked_add(len))
            .and_then(|end| map.get(pos..end))
            .ok_or_else(|| invalid(cx, "section map", HEADER))?;
        pos += list.len();
        let mut places = Vec::with_capacity(list.len() / DESCRIPTION_PAGE_SIZE);
        for entry in list.as_chunks::<DESCRIPTION_PAGE_SIZE>().0 {
            let number = le32(entry, 0).unwrap_or_default() as i32;
            let offset = le64(entry, 8).unwrap_or_default();
            let address = pages
                .iter()
                .find(|page| page.number == number)
                .map(|page| page.address)
                .ok_or_else(|| invalid(cx, "section map", HEADER))?;
            places.push((address, offset));
        }
        let mut section = Section {
            description,
            name,
            data: None,
            pages: places,
            replaced: false,
        };
        if encrypted != 0 {
            sections.push(section);
            continue;
        }
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        cx.check(Kind::DecodedBytes, size as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let mut data = vec![0; size];
        for &(address, offset) in &section.pages {
            let page = data_page(src, address, compressed == 2, max_size, cx)?;
            let offset = usize::try_from(offset)
                .ok()
                .filter(|&offset| offset <= size)
                .ok_or_else(|| invalid(cx, "data page", page_range(address)))?;
            let len = page.len().min(size - offset);
            if let (Some(to), Some(from)) = (data.get_mut(offset..offset + len), page.get(..len)) {
                to.copy_from_slice(from);
            }
        }
        section.data = Some(data);
        sections.push(section);
    }
    Ok(sections)
}

/// データのページの見出しを戻して型を確かめ、データを展開する
fn data_page(
    src: &[u8],
    at: u64,
    compressed: bool,
    max_size: u32,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    let start = usize::try_from(at).unwrap_or(usize::MAX);
    let head = start
        .checked_add(DATA_PAGE_HEADER)
        .and_then(|end| src.get(start..end))
        .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
    let mask = DATA_PAGE_MASK ^ at as u32;
    let field = |i: usize| le32(head, i * 4).unwrap_or_default() ^ mask;
    let data_start = start + DATA_PAGE_HEADER;
    let data = usize::try_from(field(2))
        .ok()
        .filter(|_| field(0) == DATA_PAGE_TYPE)
        .and_then(|size| data_start.checked_add(size))
        .and_then(|end| src.get(data_start..end))
        .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
    // データの検査の値は 0 を種に、見出しの検査の値はデータの検査の値を種に、見出しの検査の値を 0 にした
    // 戻した見出しから求める
    let mut plain = [0; DATA_PAGE_HEADER];
    for (i, out) in plain.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        *out = field(i).to_le_bytes();
    }
    plain[24..28].fill(0);
    let checksum = adler(0, data);
    if checksum != field(7) || adler(checksum, &plain) != field(6) {
        cx.report(crc_invalid("data page", start..data_start + data.len()));
    }
    if !compressed {
        return Ok(data.to_vec());
    }
    let size = usize::try_from(max_size).unwrap_or(usize::MAX);
    decompress(data, size).ok_or_else(|| invalid(cx, "data page", start..data_start + data.len()))
}

/// ODS の 4 の圧縮（LZ77 の変形）を展開する。展開した大きさが `limit` を超えるか、データが壊れていれば `None`
fn decompress(src: &[u8], limit: usize) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut pos = 0;
    let literal = literal_length(src, &mut pos)?;
    copy_literal(src, &mut pos, literal, &mut out, limit)?;
    while let Some(&op) = src.get(pos) {
        pos += 1;
        let (count, offset, literal) = match op {
            0x11 => break,
            0x10 | 0x12..=0x1F => {
                let count = if op == 0x10 {
                    long_count(src, &mut pos)?.checked_add(9)?
                } else {
                    usize::from(op & 0x0F) + 2
                };
                let (offset, literal) = two_byte_offset(src, &mut pos)?;
                (count, offset + 0x3FFF, literal)
            }
            0x20..=0x3F => {
                let count = if op == 0x20 {
                    long_count(src, &mut pos)?.checked_add(0x21)?
                } else {
                    usize::from(op) - 0x1E
                };
                let (offset, literal) = two_byte_offset(src, &mut pos)?;
                (count, offset, literal)
            }
            0x40..=0xFF => {
                let next = *src.get(pos)?;
                pos += 1;
                let offset = (usize::from(next) << 2) | usize::from((op & 0x0C) >> 2);
                (usize::from(op >> 4) - 1, offset, usize::from(op & 0x03))
            }
            _ => return None,
        };
        let from = out.len().checked_sub(offset + 1)?;
        if out.len().checked_add(count)? > limit {
            return None;
        }
        for i in 0..count {
            let byte = *out.get(from + i)?;
            out.push(byte);
        }
        let literal = if literal == 0 {
            literal_length(src, &mut pos)?
        } else {
            literal
        };
        copy_literal(src, &mut pos, literal, &mut out, limit)?;
    }
    Some(out)
}

/// 続くリテラルの長さ。次のバイトの上の 4 ビットが 0 でなければ、長さは 0 で、バイトは読まない
fn literal_length(src: &[u8], pos: &mut usize) -> Option<usize> {
    let first = *src.get(*pos)?;
    if first & 0xF0 != 0 {
        return Some(0);
    }
    *pos += 1;
    if first != 0 {
        return Some(usize::from(first) + 3);
    }
    let mut total: usize = 0x0F;
    loop {
        let byte = *src.get(*pos)?;
        *pos += 1;
        if byte != 0 {
            return total.checked_add(usize::from(byte) + 3);
        }
        total = total.checked_add(0xFF)?;
    }
}

/// 長い数（`0x10`・`0x20` の後）。0 のバイトごとに `0xFF` を足す
fn long_count(src: &[u8], pos: &mut usize) -> Option<usize> {
    let first = *src.get(*pos)?;
    *pos += 1;
    if first != 0 {
        return Some(usize::from(first));
    }
    let mut total: usize = 0xFF;
    loop {
        let byte = *src.get(*pos)?;
        *pos += 1;
        if byte != 0 {
            return total.checked_add(usize::from(byte));
        }
        total = total.checked_add(0xFF)?;
    }
}

/// 2 バイトの距離と、続くリテラルの長さ（下の 2 ビット）
fn two_byte_offset(src: &[u8], pos: &mut usize) -> Option<(usize, usize)> {
    let first = *src.get(*pos)?;
    let second = *src.get(*pos + 1)?;
    *pos += 2;
    let offset = usize::from(first >> 2) | (usize::from(second) << 6);
    Some((offset, usize::from(first & 0x03)))
}

fn copy_literal(
    src: &[u8],
    pos: &mut usize,
    len: usize,
    out: &mut Vec<u8>,
    limit: usize,
) -> Option<()> {
    let bytes = src.get(*pos..pos.checked_add(len)?)?;
    if out.len().checked_add(len)? > limit {
        return None;
    }
    out.extend_from_slice(bytes);
    *pos += len;
    Some(())
}

/// 一致を探す距離の上限。2 バイトの距離（`0x20`〜`0x3F`）で書ける所まで
const WINDOW: usize = 0x4000;

/// 一致を探すときにたどる鎖の長さ
const CHAIN: usize = 32;

/// ODS の 4 の圧縮で縮める。初めの 4 バイトはリテラルにして、その後は距離 `WINDOW` までの一致を、
/// 3 バイトのハッシュの鎖で探す。`src` は 4 バイト以上とする
fn compress(src: &[u8]) -> Vec<u8> {
    let mut chains = Chains::new(src.len());
    // 初めのリテラルの長さと、一致（長さ・距離）とその後ろのリテラルの長さの並び
    let mut first = 0;
    let mut matches: Vec<(usize, usize, usize)> = Vec::new();
    let mut pos = 0;
    while pos < src.len() {
        let best = if pos >= 4 {
            chains.longest(src, pos)
        } else {
            (0, 0)
        };
        chains.insert(src, pos);
        if best.0 < 3 {
            match matches.last_mut() {
                Some(last) => last.2 += 1,
                None => first += 1,
            }
            pos += 1;
            continue;
        }
        for at in pos + 1..pos + best.0 {
            chains.insert(src, at);
        }
        matches.push((best.0, best.1, 0));
        pos += best.0;
    }
    let mut out = Vec::with_capacity(src.len() / 2);
    push_literal_length(&mut out, first);
    let mut rest = src;
    let (literal, tail) = rest.split_at(first);
    out.extend_from_slice(literal);
    rest = tail;
    for (count, distance, literal) in matches {
        let offset = distance - 1;
        let bits = if (1..=3).contains(&literal) {
            literal as u8
        } else {
            0
        };
        if count <= 14 && offset <= 0x3FF {
            out.push(((count as u8 + 1) << 4) | ((offset as u8 & 0x03) << 2) | bits);
            out.push((offset >> 2) as u8);
        } else {
            if count <= 33 {
                out.push(0x1E + count as u8);
            } else {
                out.push(0x20);
                push_long_count(&mut out, count - 0x21);
            }
            out.push(((offset as u8 & 0x3F) << 2) | bits);
            out.push((offset >> 6) as u8);
        }
        if literal > 3 {
            push_literal_length(&mut out, literal);
        }
        let (bytes, tail) = rest.get(count..).unwrap_or_default().split_at(literal);
        out.extend_from_slice(bytes);
        rest = tail;
    }
    out.push(0x11);
    out
}

/// 3 バイトのハッシュごとの、前に出た位置の鎖
struct Chains {
    head: Vec<usize>,
    prev: Vec<usize>,
}

impl Chains {
    const BITS: u32 = 13;

    fn new(len: usize) -> Self {
        Self {
            head: vec![usize::MAX; 1 << Self::BITS],
            prev: vec![usize::MAX; len],
        }
    }

    fn hash(src: &[u8], at: usize) -> Option<usize> {
        let &[a, b, c] = src.get(at..at.checked_add(3)?)? else {
            return None;
        };
        let key = u32::from_le_bytes([a, b, c, 0]);
        Some((key.wrapping_mul(0x9E37_79B1) >> (32 - Self::BITS)) as usize)
    }

    fn insert(&mut self, src: &[u8], at: usize) {
        let Some(key) = Self::hash(src, at) else {
            return;
        };
        if let (Some(head), Some(prev)) = (self.head.get_mut(key), self.prev.get_mut(at)) {
            *prev = *head;
            *head = at;
        }
    }

    /// `at` から始まる一番長い一致の長さと距離
    fn longest(&self, src: &[u8], at: usize) -> (usize, usize) {
        let mut best = (0, 0);
        let Some(key) = Self::hash(src, at) else {
            return best;
        };
        let here = src.get(at..).unwrap_or_default();
        let mut candidate = self.head.get(key).copied().unwrap_or(usize::MAX);
        for _ in 0..CHAIN {
            if candidate == usize::MAX || at - candidate > WINDOW {
                break;
            }
            let there = src.get(candidate..).unwrap_or_default();
            let len = there.iter().zip(here).take_while(|(a, b)| a == b).count();
            if len > best.0 {
                best = (len, at - candidate);
            }
            candidate = self.prev.get(candidate).copied().unwrap_or(usize::MAX);
        }
        best
    }
}

/// リテラルの長さ（4 以上）を書く。`literal_length` の逆
fn push_literal_length(out: &mut Vec<u8>, len: usize) {
    if len - 3 <= 0x0F {
        out.push((len - 3) as u8);
        return;
    }
    out.push(0);
    push_rest(out, len - 3 - 0x0F);
}

/// 長い数（1 以上）を書く。`long_count` の逆
fn push_long_count(out: &mut Vec<u8>, count: usize) {
    if count <= 0xFF {
        out.push(count as u8);
        return;
    }
    out.push(0);
    push_rest(out, count - 0xFF);
}

/// 0 のバイトごとに `0xFF` を足し、最後に残り（1 以上）を書く
fn push_rest(out: &mut Vec<u8>, mut rest: usize) {
    while rest > 0xFF {
        out.push(0);
        rest -= 0xFF;
    }
    out.push(rest as u8);
}

/// 入れ物を組み直して書く。ページは元の位置の順に並べ、番号を 1 から振り直す。
/// 置き換えていない区切りのページは `src` から写して、見出しの隠しだけを新しい位置に合わせる。
/// 置き換えた区切りは、元の初めのページの所で、新しいページに分けて圧縮する。
/// 区切りの地図・ページの地図・空のページの見出し・2 つめの見出しを、この順に終わりに置く
pub(super) fn write(src: &[u8], container: &Container) -> Option<Vec<u8>> {
    let mut out = src.get(..PAGES_START as usize)?.to_vec();
    let mut order = Vec::new();
    for (index, section) in container.sections.iter().enumerate() {
        if section.replaced {
            let first = section.pages.iter().map(|&(address, _)| address).min();
            order.push((first.unwrap_or(u64::MAX), index, None));
        } else {
            for (page, &(address, _)) in section.pages.iter().enumerate() {
                order.push((address, index, Some(page)));
            }
        }
    }
    order.sort_unstable();
    // ページの地図の並び（番号と大きさ）と、区切りごとの新しいページ（番号・データの大きさ・区切りの中の位置）
    let mut map: Vec<(u32, u32)> = Vec::new();
    let mut lists: Vec<Vec<(u32, u32, u64)>> = vec![Vec::new(); container.sections.len()];
    let mut moved = Vec::new();
    for (old, index, page) in order {
        let section = container.sections.get(index)?;
        let list = lists.get_mut(index)?;
        if let Some(page) = page {
            let (address, offset) = *section.pages.get(page)?;
            let new = out.len() as u64;
            let (size, data) = copy_data_page(src, address, &mut out)?;
            map.push((u32::try_from(map.len() + 1).ok()?, size));
            list.push((map.last()?.0, data, offset));
            moved.push((address, new));
            continue;
        }
        if old != u64::MAX {
            moved.push((old, out.len() as u64));
        }
        let [_, max_size, _, compressed, id] =
            core::array::from_fn(|i| le32(&section.description, 8 + i * 4).unwrap_or_default());
        let max_size = usize::try_from(max_size).ok().filter(|&size| size > 0)?;
        for (part, chunk) in section.data.as_deref()?.chunks(max_size).enumerate() {
            let mut plain = chunk.to_vec();
            plain.resize(max_size, 0);
            let data = if compressed == 2 {
                compress(&plain)
            } else {
                plain
            };
            let offset = (part * max_size) as u64;
            let size = push_data_page(&mut out, id, offset, &data)?;
            map.push((u32::try_from(map.len() + 1).ok()?, size));
            list.push((map.last()?.0, u32::try_from(data.len()).ok()?, offset));
        }
    }
    let mut section_map = container.map_head.to_vec();
    for (section, list) in container.sections.iter().zip(&lists) {
        let mut description = section.description;
        let size = match (&section.data, section.replaced) {
            (Some(data), true) => data.len() as u64,
            _ => le64(&description, 0)?,
        };
        description[..8].copy_from_slice(&size.to_le_bytes());
        description[8..12].copy_from_slice(&u32::try_from(list.len()).ok()?.to_le_bytes());
        section_map.extend_from_slice(&description);
        for &(number, size, offset) in list {
            section_map.extend_from_slice(&number.to_le_bytes());
            section_map.extend_from_slice(&size.to_le_bytes());
            section_map.extend_from_slice(&offset.to_le_bytes());
        }
    }
    let section_map_id = u32::try_from(map.len() + 1).ok()?;
    let page = system_page_bytes(SECTION_MAP_TYPE, &section_map);
    let size = round_up(page.len());
    out.extend_from_slice(&page);
    out.resize(out.len() + size - page.len(), 0);
    map.push((section_map_id, u32::try_from(size).ok()?));
    // ページの地図の大きさは、自分の見出しと続く空のページの見出しを含めて 0x20 に切り上げる。
    // 地図は自分の大きさを含むので、大きさが変わらなくなるまで作り直す
    let page_map_id = section_map_id + 1;
    let page_map_at = out.len();
    let mut page_size = 0x20;
    let mut page = Vec::new();
    for _ in 0..8 {
        let mut entries = Vec::with_capacity((map.len() + 1) * 8);
        for &(number, size) in map.iter().chain([&(page_map_id, page_size)]) {
            entries.extend_from_slice(&number.to_le_bytes());
            entries.extend_from_slice(&size.to_le_bytes());
        }
        page = system_page_bytes(PAGE_MAP_TYPE, &entries);
        let size = u32::try_from(round_up(page.len() + SYSTEM_PAGE_HEADER)).ok()?;
        if size == page_size {
            break;
        }
        page_size = size;
    }
    out.extend_from_slice(&page);
    out.extend_from_slice(&EMPTY_PAGE);
    let second = out.len() as u64;
    let mut header = container.header;
    let pages_start = PAGES_START;
    let page_map_at = page_map_at as u64;
    for (at, value) in [
        (0x28, page_map_id),
        (0x40, page_map_id),
        (0x50, page_map_id),
        (0x5C, section_map_id),
        (0x60, page_map_id),
        (0x68, 0),
    ] {
        put(&mut header, at, &value.to_le_bytes())?;
    }
    for (at, value) in [
        (0x2C, page_map_at + u64::from(page_size) - pages_start),
        (0x34, second),
        (0x54, page_map_at - pages_start),
    ] {
        put(&mut header, at, &value.to_le_bytes())?;
    }
    let crc = crc32(&header);
    put(&mut header, 0x68, &crc.to_le_bytes())?;
    let encrypted = crypt(&header);
    put(&mut out, HEADER.start, &encrypted)?;
    out.extend_from_slice(&encrypted);
    // ファイルの見出しの、要約とプレビューのデータの位置（ページの見出しの後ろ）を、動いたページに合わせる
    for at in [0x0D, 0x20] {
        let value = u64::from(le32(&out, at)?);
        let found = moved
            .iter()
            .find(|&&(old, _)| old + DATA_PAGE_HEADER as u64 == value);
        if let Some(&(_, new)) = found {
            let new = u32::try_from(new + DATA_PAGE_HEADER as u64).ok()?;
            put(&mut out, at, &new.to_le_bytes())?;
        }
    }
    Some(out)
}

/// データのページを `src` の `at` から `out` の終わりに写し、見出しを新しい位置で隠し直す。
/// ページの大きさとデータの大きさを返す
fn copy_data_page(src: &[u8], at: u64, out: &mut Vec<u8>) -> Option<(u32, u32)> {
    let start = usize::try_from(at).ok()?;
    let head = src.get(start..start.checked_add(DATA_PAGE_HEADER)?)?;
    let old = DATA_PAGE_MASK ^ at as u32;
    let new = DATA_PAGE_MASK ^ out.len() as u32;
    let size = le32(head, 8)? ^ old;
    let data_start = start + DATA_PAGE_HEADER;
    let data = src.get(data_start..data_start.checked_add(usize::try_from(size).ok()?)?)?;
    let begin = out.len();
    for chunk in head.as_chunks::<4>().0 {
        out.extend_from_slice(&(u32::from_le_bytes(*chunk) ^ old ^ new).to_le_bytes());
    }
    out.extend_from_slice(data);
    let page = round_up(DATA_PAGE_HEADER + data.len());
    out.resize(begin + page, 0);
    Some((u32::try_from(page).ok()?, size))
}

/// データのページを `out` の終わりに書き、ページの大きさを返す。見出しは書く位置で隠す
fn push_data_page(out: &mut Vec<u8>, id: u32, offset: u64, data: &[u8]) -> Option<u32> {
    let size = u32::try_from(data.len()).ok()?;
    let page = round_up(DATA_PAGE_HEADER + data.len());
    let checksum = adler(0, data);
    let mut fields = [
        DATA_PAGE_TYPE,
        id,
        size,
        u32::try_from(page).ok()?,
        u32::try_from(offset).ok()?,
        0,
        0,
        checksum,
    ];
    let mut plain = [0; DATA_PAGE_HEADER];
    for (out, field) in plain.as_chunks_mut::<4>().0.iter_mut().zip(fields) {
        *out = field.to_le_bytes();
    }
    fields[6] = adler(checksum, &plain);
    let mask = DATA_PAGE_MASK ^ out.len() as u32;
    let begin = out.len();
    for field in fields {
        out.extend_from_slice(&(field ^ mask).to_le_bytes());
    }
    out.extend_from_slice(data);
    out.resize(begin + page, 0);
    u32::try_from(page).ok()
}

/// 地図のページ（見出しと圧縮したデータ）を作る。後ろは埋めない
fn system_page_bytes(kind: u32, data: &[u8]) -> Vec<u8> {
    let compressed = compress(data);
    let mut fields = [kind, data.len() as u32, compressed.len() as u32, 2, 0];
    let head: Vec<u8> = fields
        .iter()
        .flat_map(|field| field.to_le_bytes())
        .collect();
    fields[4] = adler(adler(0, &head), &compressed);
    let mut page: Vec<u8> = fields
        .iter()
        .flat_map(|field| field.to_le_bytes())
        .collect();
    page.extend_from_slice(&compressed);
    page
}

/// ページの大きさは 0x20 の倍数に切り上げる
fn round_up(size: usize) -> usize {
    size.next_multiple_of(0x20)
}

/// 地図とデータのページの検査の値。2 つの和を `0xFFF1` で割った余りを、`0x15B0` バイトごとにとる
fn adler(seed: u32, data: &[u8]) -> u32 {
    let mut low = seed & 0xFFFF;
    let mut high = seed >> 16;
    for chunk in data.chunks(0x15B0) {
        for &byte in chunk {
            low += u32::from(byte);
            high += low;
        }
        low %= 0xFFF1;
        high %= 0xFFF1;
    }
    (high << 16) | low
}

/// 見出しの CRC（多項式 0x04C11DB7 を反転した CRC-32）
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn le64(src: &[u8], at: usize) -> Option<u64> {
    let bytes = src.get(at..at.checked_add(8)?)?;
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

fn field(header: &[u8], at: usize) -> u32 {
    le32(header, at).unwrap_or_default()
}

fn page_range(at: u64) -> Range<usize> {
    let start = usize::try_from(at).unwrap_or(usize::MAX);
    start..start.saturating_add(SYSTEM_PAGE_HEADER)
}

fn crc_invalid(part: &str, at: Range<usize>) -> Diagnostic {
    Diagnostic::new(codes::DWG_CRC_INVALID)
        .with("part", part)
        .at(Ref::at_bytes(at.start as u64, at.end as u64))
        .cite(R2004)
}

pub(super) fn invalid(cx: &mut Cx, part: &str, at: Range<usize>) -> Error {
    cx.report(
        Diagnostic::new(codes::DWG_FILE_INVALID)
            .with("part", part)
            .at(Ref::at_bytes(at.start as u64, at.end as u64))
            .cite(R2004),
    );
    Error::Invalid
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Diagnostics};

    use super::*;

    fn read_all(src: &[u8]) -> (Container, Diagnostics) {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let container = read(
            src,
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        )
        .unwrap();
        (container, diag)
    }

    fn sections(container: &Container) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
        let list = container.sections.iter();
        list.map(|s| (s.name.clone(), s.data.clone())).collect()
    }

    fn description(name: &[u8], max_size: u32, compressed: u32, id: u32) -> [u8; DESCRIPTION_SIZE] {
        let mut description = [0; DESCRIPTION_SIZE];
        for (i, value) in [0, max_size, 1, compressed, id, 0].into_iter().enumerate() {
            description[8 + i * 4..12 + i * 4].copy_from_slice(&value.to_le_bytes());
        }
        description[0x20..0x20 + name.len()].copy_from_slice(name);
        description
    }

    fn section(name: &[u8], max_size: u32, compressed: u32, id: u32, data: Vec<u8>) -> Section {
        Section {
            description: description(name, max_size, compressed, id),
            name: name.to_vec(),
            data: Some(data),
            pages: Vec::new(),
            replaced: true,
        }
    }

    #[test]
    fn compressed_data_comes_back() {
        let mut text = Vec::new();
        let mut seed: u32 = 7;
        for i in 0..0x9000u32 {
            seed = seed.wrapping_mul(0x343FD).wrapping_add(0x0026_9EC3);
            // 乱れたバイト・繰り返し・0 の並びを混ぜる
            text.push(match i / 0x1000 {
                0 | 4 => (seed >> 16) as u8,
                1 | 5 => b"AcDb:Objects"[i as usize % 12],
                2 => 0,
                _ => (i % 251) as u8,
            });
        }
        for len in [4, 5, 19, 20, 300, 0x1000, 0x7400, text.len()] {
            let src = &text[..len];
            let packed = compress(src);
            assert_eq!(
                decompress(&packed, src.len()).as_deref(),
                Some(src),
                "{len}"
            );
        }
        let zeros = compress(&[0; 0x7400]);
        assert!(zeros.len() < 0x100, "{}", zeros.len());
    }

    #[test]
    fn rebuilt_containers_read_back() {
        let mut header = [0; 0x6C];
        header[..HEADER_MAGIC.len()].copy_from_slice(HEADER_MAGIC);
        let mut map_head = [0; SECTION_MAP_HEAD];
        for (i, value) in [3, 2, 0x7400, 0, 3].into_iter().enumerate() {
            map_head[i * 4..i * 4 + 4].copy_from_slice(&u32::to_le_bytes(value));
        }
        let objects: Vec<u8> = (0..0x9000u32).map(|i| (i % 253) as u8).collect();
        let mut container = Container {
            header,
            map_head,
            sections: vec![
                section(b"", 0x7400, 2, 0, Vec::new()),
                section(b"AcDb:SummaryInfo", 0x80, 1, 1, b"summary".to_vec()),
                section(b"AcDb:AcDbObjects", 0x7400, 2, 2, objects),
            ],
        };
        let mut src = vec![0; PAGES_START as usize];
        src[..6].copy_from_slice(b"AC1018");
        let first = write(&src, &container).unwrap();
        let (back, diag) = read_all(&first);
        assert!(diag.is_empty(), "{diag:?}");
        assert_eq!(sections(&back), sections(&container));
        assert_eq!(back.sections[2].pages.len(), 2);

        // 置き換えていない区切りは写し、置き換えた区切りだけを作り直す
        let mut edited = back;
        edited
            .replace(b"AcDb:SummaryInfo", b"changed summary".to_vec())
            .unwrap();
        assert!(edited.replace(b"AcDb:Missing", Vec::new()).is_none());
        let second = write(&first, &edited).unwrap();
        let (again, diag) = read_all(&second);
        assert!(diag.is_empty(), "{diag:?}");
        container.sections[1].data = Some(b"changed summary".to_vec());
        assert_eq!(sections(&again), sections(&container));
        assert_eq!(second.len(), first.len());
    }

    /// `ZUMEN_SAMPLES` の R2004 からの DWG を、そのままと、読めた区切りをすべて置き換えて組み直す
    #[test]
    #[allow(
        clippy::disallowed_methods,
        reason = "環境変数を読むのは試験だけで、核は読まない（AGENTS.md の図面と秘密）"
    )]
    fn sample_containers_are_rebuilt() {
        let Some(dir) = std::env::var_os("ZUMEN_SAMPLES").filter(|d| !d.is_empty()) else {
            return;
        };
        let mut seen = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let Ok(src) = std::fs::read(&path) else {
                continue;
            };
            if !matches!(
                src.get(..6),
                Some(b"AC1018" | b"AC1024" | b"AC1027" | b"AC1032")
            ) {
                continue;
            }
            seen += 1;
            let (container, diag) = read_all(&src);
            assert!(diag.is_empty(), "{path:?} {diag:?}");
            let copied = write(&src, &container).unwrap();
            let (back, diag) = read_all(&copied);
            assert!(diag.is_empty(), "{path:?} {diag:?}");
            assert_eq!(sections(&back), sections(&container), "{path:?}");
            assert_eq!(copied[..0x80], src[..0x80], "{path:?}");
            let mut all = container.clone();
            for (name, data) in sections(&container) {
                if let Some(data) = data {
                    all.replace(&name, data).unwrap();
                }
            }
            let rebuilt = write(&src, &all).unwrap();
            let (again, diag) = read_all(&rebuilt);
            assert!(diag.is_empty(), "{path:?} {diag:?}");
            assert_eq!(sections(&again), sections(&container), "{path:?}");
        }
        assert!(seen > 0);
    }
}
