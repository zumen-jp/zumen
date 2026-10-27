//! R2004 の入れ物（ODS 5.4.1 の 4「R2004 DWG FILE FORMAT ORGANIZATION」）。
//!
//! ファイルの見出しの後ろに、暗号化した見出しがある。そこからページの地図と区切りの地図を読み、
//! 区切りの名前ごとに、ページを展開してつなげたバイト列を作る（決定 0026）。

use core::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::Error;

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

/// 読んだ R2004 の入れ物
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Container {
    sections: Vec<Section>,
}

/// 区切りの名前と、ページを展開してつなげたバイト列
#[derive(Clone, Debug, PartialEq, Eq)]
struct Section {
    name: Vec<u8>,
    data: Vec<u8>,
}

impl Container {
    /// 名前（`AcDb:Header`）の区切り
    pub(super) fn section(&self, name: &[u8]) -> Option<&[u8]> {
        self.sections
            .iter()
            .find(|section| section.name == name)
            .map(|section| section.data.as_slice())
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
    Ok(Container { sections })
}

/// 暗号化した見出しを戻し、初めの印と CRC を確かめる
fn header(src: &[u8], cx: &mut Cx) -> Result<[u8; 0x6C], Error> {
    let bytes = src
        .get(HEADER)
        .ok_or_else(|| invalid(cx, "r2004 header", 0..src.len()))?;
    let mut header = [0; 0x6C];
    let mut seed: u32 = 1;
    for (out, byte) in header.iter_mut().zip(bytes) {
        seed = seed.wrapping_mul(0x343FD).wrapping_add(0x0026_9EC3);
        *out = byte ^ (seed >> 16) as u8;
    }
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
        let description = pos
            .checked_add(DESCRIPTION_SIZE)
            .and_then(|end| map.get(pos..end))
            .ok_or_else(|| invalid(cx, "section map", HEADER))?;
        let size = le64(description, 0).unwrap_or_default();
        let [page_count, max_size, _, compressed, _, encrypted] =
            core::array::from_fn(|i| le32(description, 8 + i * 4).unwrap_or_default());
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
        if encrypted != 0 {
            continue;
        }
        let size = usize::try_from(size).unwrap_or(usize::MAX);
        cx.check(Kind::DecodedBytes, size as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let mut data = vec![0; size];
        for entry in list.as_chunks::<DESCRIPTION_PAGE_SIZE>().0 {
            let number = le32(entry, 0).unwrap_or_default() as i32;
            let offset = le64(entry, 8).unwrap_or_default();
            let address = pages
                .iter()
                .find(|page| page.number == number)
                .map(|page| page.address)
                .ok_or_else(|| invalid(cx, "section map", HEADER))?;
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
        sections.push(Section { name, data });
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
