//! R2007（AC1021）のコンテナー（ODS 5.4.1 の 5「R2007 DWG FILE FORMAT ORGANIZATION」）。
//!
//! `0x80..0x480` の圧縮ヘッダーを Reed-Solomon（255, 239）と R2007 方式の LZ77 で戻し、
//! `0x480` から始まるページマップとセクションマップを読んで、必要なデータセクションを展開する。

use core::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::Error;

use super::stopped;

const R2007: SpecRef = SpecRef {
    spec: "ods_dwg_5_4_1",
    section: "5 R2007 DWG FILE FORMAT ORGANIZATION",
};

/// 圧縮したファイルヘッダーの位置と大きさ
const HEADER: Range<usize> = 0x80..0x480;

/// ページの位置を数え始める先頭からの位置
const PAGES_START: u64 = 0x480;

/// 展開したファイルヘッダーのメタデータの大きさ
const METADATA_SIZE: usize = 0x110;

/// システムページの Reed-Solomon のデータ長（RS(255, 239)、原始多項式 `0x169`）
const SYSTEM_BLOCK_SIZE: usize = 239;
const SYSTEM_POLY: u16 = 0x169;

/// データページの Reed-Solomon のデータ長（RS(255, 251)、原始多項式 `0x11D`）
const DATA_BLOCK_SIZE: usize = 251;
const DATA_POLY: u16 = 0x11D;

/// Reed-Solomon の符号語長
const CODEWORD_LEN: usize = 255;

/// セクションマップのセクション記述の固定部の大きさ
const SECTION_HEAD_SIZE: usize = 0x40;

/// セクションマップの 1 ページの記録の大きさ
const LOCAL_PAGE_SIZE: usize = 56;

/// `wasm32` と 64 ビット環境で同じ上限にする確保バイト数の上限
const MAX_ALLOC: u64 = i32::MAX as u64;

/// 段階的に伸ばす展開バッファーの初回の確保単位
const GROW_STEP: usize = 4096;

/// 読んだ R2007 のコンテナー。ヘッダーの変数とオブジェクトの本体を保つ
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Container {
    header: Vec<u8>,
    objects: Vec<u8>,
}

/// 読んだ直後に使う 4 つの必須セクション
pub(super) struct Sections {
    pub(super) container: Container,
    pub(super) classes: Vec<u8>,
    pub(super) handles: Vec<u8>,
}

/// ページマップの 1 つのページ
#[derive(Clone, Copy, Debug)]
struct Page {
    id: i64,
    offset: u64,
    size: u64,
}

impl Container {
    pub(super) fn snapshot(&self, cx: &mut Cx<'_>) -> Result<Self, Error> {
        use crate::snapshot::bytes;
        Ok(Self {
            header: bytes(&self.header, cx)?,
            objects: bytes(&self.objects, cx)?,
        })
    }

    pub(super) fn header(&self) -> &[u8] {
        &self.header
    }

    pub(super) fn objects(&self) -> &[u8] {
        &self.objects
    }
}

/// R2007 のコンテナーを読み、必須の 4 セクションを展開して返す
pub(super) fn read(src: &[u8], cx: &mut Cx) -> Result<Sections, Error> {
    let meta = read_metadata(src, cx)?;
    let pm_corr = le64(&meta, 0x18).unwrap_or_default();
    let pm_offset = le64(&meta, 0x38).unwrap_or_default();
    let pm_comp = le64(&meta, 0x50).unwrap_or_default();
    let pm_uncomp = le64(&meta, 0x58).unwrap_or_default();
    let sm_comp = le64(&meta, 0xB0).unwrap_or_default();
    let sm_id = le64(&meta, 0xC0).unwrap_or_default() as i64;
    let sm_uncomp = le64(&meta, 0xC8).unwrap_or_default();
    let sm_corr = le64(&meta, 0xD8).unwrap_or_default();

    let pm_bytes = system_page(src, pm_offset, pm_comp, pm_uncomp, pm_corr, "page map", cx)?;
    let pages = page_map(&pm_bytes, pm_offset, cx)?;

    let sm_page = find_page(&pages, sm_id).ok_or_else(|| invalid(cx, "section map", HEADER))?;
    let sm_bytes = system_page(
        src,
        sm_page.offset,
        sm_comp,
        sm_uncomp,
        sm_corr,
        "section map",
        cx,
    )?;

    section_map(src, &sm_bytes, &pages, cx)
}

enum DecodeError {
    Invalid,
    Stopped(zumen_base::Stop),
}

impl From<zumen_base::Stop> for DecodeError {
    fn from(stop: zumen_base::Stop) -> Self {
        Self::Stopped(stop)
    }
}

fn map_decode<T>(
    res: Result<T, DecodeError>,
    cx: &mut Cx,
    part: &str,
    at: Range<usize>,
) -> Result<T, Error> {
    match res {
        Ok(value) => Ok(value),
        Err(DecodeError::Invalid) => Err(invalid(cx, part, at)),
        Err(DecodeError::Stopped(stop)) => Err(stopped(cx, stop)),
    }
}

/// `0x80..0x480` の圧縮ヘッダーを戻して `0x110` バイトのメタデータを得る
fn read_metadata(src: &[u8], cx: &mut Cx) -> Result<[u8; METADATA_SIZE], Error> {
    let raw = src
        .get(HEADER)
        .ok_or_else(|| invalid(cx, "r2007 header", 0..src.len()))?;
    let (decoded, rs_corrupted) = map_decode(
        rs_decode(
            raw,
            3 * SYSTEM_BLOCK_SIZE,
            3,
            SYSTEM_BLOCK_SIZE,
            SYSTEM_POLY,
            cx,
        ),
        cx,
        "r2007 header",
        HEADER,
    )?;
    if rs_corrupted {
        crc_invalid(cx, "r2007 header", HEADER);
    }
    let compr_len =
        le32_signed(&decoded, 0x18).ok_or_else(|| invalid(cx, "r2007 header", HEADER))?;
    let mut meta = [0u8; METADATA_SIZE];
    if compr_len < 0 {
        let len = usize::try_from(compr_len.unsigned_abs())
            .ok()
            .filter(|&len| len > 0 && len <= METADATA_SIZE)
            .ok_or_else(|| invalid(cx, "r2007 header", HEADER))?;
        let plain = decoded
            .get(0x20..0x20 + len)
            .ok_or_else(|| invalid(cx, "r2007 header", HEADER))?;
        if let Some(dst) = meta.get_mut(..len) {
            dst.copy_from_slice(plain);
        }
    } else {
        let len = usize::try_from(compr_len)
            .ok()
            .filter(|&len| len > 0)
            .ok_or_else(|| invalid(cx, "r2007 header", HEADER))?;
        let comp = decoded
            .get(0x20..)
            .and_then(|rest| rest.get(..len))
            .ok_or_else(|| invalid(cx, "r2007 header", HEADER))?;
        let plain = map_decode(
            decompress_to_vec(comp, METADATA_SIZE, cx),
            cx,
            "r2007 header",
            HEADER,
        )?;
        meta.copy_from_slice(&plain);
    }
    Ok(meta)
}

/// システムページ（ページマップ・セクションマップ）を Reed-Solomon と LZ77 で展開する
fn system_page(
    src: &[u8],
    page_offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
    correction_factor: u64,
    part: &str,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    let at = PAGES_START
        .checked_add(page_offset)
        .ok_or_else(|| invalid(cx, part, HEADER))?;
    let aligned = compressed_size
        .checked_add(7)
        .map(|v| v & !7)
        .and_then(|v| v.checked_mul(correction_factor))
        .filter(|&v| v > 0)
        .ok_or_else(|| invalid(cx, part, page_range(at)))?;
    let factor_u64 = aligned.div_ceil(SYSTEM_BLOCK_SIZE as u64);
    let encoded_u64 = factor_u64
        .checked_mul(CODEWORD_LEN as u64)
        .ok_or_else(|| invalid(cx, part, page_range(at)))?;
    let end_u64 = at
        .checked_add(encoded_u64)
        .filter(|&end| end <= src.len() as u64)
        .ok_or_else(|| invalid(cx, part, page_range(at)))?;
    let (Ok(start), Ok(end), Ok(aligned_len), Ok(factor)) = (
        usize::try_from(at),
        usize::try_from(end_u64),
        usize::try_from(aligned),
        usize::try_from(factor_u64),
    ) else {
        return Err(invalid(cx, part, page_range(at)));
    };
    let encoded = src
        .get(start..end)
        .ok_or_else(|| invalid(cx, part, page_range(at)))?;
    let span = start..end;
    let (decoded, rs_corrupted) = map_decode(
        rs_decode(
            encoded,
            aligned_len,
            factor,
            SYSTEM_BLOCK_SIZE,
            SYSTEM_POLY,
            cx,
        ),
        cx,
        part,
        span.clone(),
    )?;
    if rs_corrupted {
        crc_invalid(cx, part, span.clone());
    }
    let comp_len = usize::try_from(compressed_size)
        .ok()
        .ok_or_else(|| invalid(cx, part, span.clone()))?;
    let comp = decoded
        .get(..comp_len)
        .ok_or_else(|| invalid(cx, part, span.clone()))?;
    cx.check(Kind::DecodedBytes, uncompressed_size)
        .map_err(|stop| stopped(cx, stop))?;
    let uncomp_len =
        checked_alloc_len(uncompressed_size).ok_or_else(|| invalid(cx, part, span.clone()))?;
    map_decode(decompress_to_vec(comp, uncomp_len, cx), cx, part, span)
}

/// ページマップの `(size: i64, id: i64)` の並びを読む
fn page_map(map: &[u8], page_offset: u64, cx: &mut Cx) -> Result<Vec<Page>, Error> {
    let at = PAGES_START.saturating_add(page_offset);
    let count = (map.len() / 16) as u64;
    cx.check(Kind::Records, count)
        .map_err(|stop| stopped(cx, stop))?;
    let mut pages = Vec::new();
    let mut offset = 0u64;
    let mut pos = 0usize;
    while pos + 16 <= map.len() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let (Some(size_raw), Some(id_raw)) = (le64(map, pos), le64(map, pos + 8)) else {
            return Err(invalid(cx, "page map", page_range(at)));
        };
        pos += 16;
        let size_i64 = size_raw as i64;
        let id = id_raw as i64;
        if size_i64 == 0 && id == 0 {
            break;
        }
        let Ok(size) = u64::try_from(size_i64) else {
            return Err(invalid(cx, "page map", page_range(at)));
        };
        pages.push(Page { id, offset, size });
        offset = offset
            .checked_add(size)
            .ok_or_else(|| invalid(cx, "page map", page_range(at)))?;
    }
    Ok(pages)
}

/// 正のページ ID だけを検索する（負の ID は空き領域を表すため一致させない）
fn find_page(pages: &[Page], id: i64) -> Option<Page> {
    if id <= 0 {
        return None;
    }
    pages.iter().copied().find(|p| p.id == id)
}

/// セクションマップを読み、必須の 4 セクション（Header・Classes・Handles・AcDbObjects）を展開する
fn section_map(src: &[u8], map: &[u8], pages: &[Page], cx: &mut Cx) -> Result<Sections, Error> {
    let mut header = None;
    let mut classes = None;
    let mut handles = None;
    let mut objects = None;
    let mut pos = 0usize;
    while pos + SECTION_HEAD_SIZE <= map.len() {
        cx.check(Kind::Records, 1)
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let head = map
            .get(pos..pos + SECTION_HEAD_SIZE)
            .ok_or_else(|| invalid(cx, "section map", HEADER))?;
        let data_size = le64(head, 0x00).unwrap_or_default();
        let encryption = le64(head, 0x10).unwrap_or_default();
        let name_len_u64 = le64(head, 0x20).unwrap_or_default();
        let encoding = le64(head, 0x30).unwrap_or_default();
        let page_count_u64 = le64(head, 0x38).unwrap_or_default();
        pos += SECTION_HEAD_SIZE;

        let rem = (map.len() - pos) as u64;
        if name_len_u64 > rem {
            return Err(invalid(cx, "section map", HEADER));
        }
        let name_len =
            usize::try_from(name_len_u64).map_err(|_| invalid(cx, "section map", HEADER))?;
        let name_bytes = map
            .get(pos..pos + name_len)
            .ok_or_else(|| invalid(cx, "section map", HEADER))?;
        pos += name_len;

        let rem_after_name = (map.len() - pos) as u64;
        let pages_bytes_u64 = page_count_u64
            .checked_mul(LOCAL_PAGE_SIZE as u64)
            .filter(|&len| len <= rem_after_name)
            .ok_or_else(|| invalid(cx, "section map", HEADER))?;
        let pages_bytes_len =
            usize::try_from(pages_bytes_u64).map_err(|_| invalid(cx, "section map", HEADER))?;
        let local_pages = map
            .get(pos..pos + pages_bytes_len)
            .ok_or_else(|| invalid(cx, "section map", HEADER))?;
        pos += pages_bytes_len;

        if name_len == 0 && page_count_u64 == 0 {
            continue;
        }
        let slot = match match_required_section(name_bytes) {
            Some(RequiredSection::Header) => &mut header,
            Some(RequiredSection::Classes) => &mut classes,
            Some(RequiredSection::Handles) => &mut handles,
            Some(RequiredSection::Objects) => &mut objects,
            None => continue,
        };
        // 暗号化された必須セクション（encryption == 1）は読めない
        if encryption == 1 {
            continue;
        }
        *slot = Some(read_data_section(
            src,
            data_size,
            encoding,
            local_pages,
            pages,
            cx,
        )?);
    }

    let (Some(header), Some(classes), Some(handles), Some(objects)) =
        (header, classes, handles, objects)
    else {
        return Err(invalid(cx, "section", 0..6));
    };
    if header.is_empty() || classes.is_empty() || handles.is_empty() || objects.is_empty() {
        return Err(invalid(cx, "section", 0..6));
    }
    Ok(Sections {
        container: Container { header, objects },
        classes,
        handles,
    })
}

enum RequiredSection {
    Header,
    Classes,
    Handles,
    Objects,
}

/// 必須の 4 セクション名をヒープ確保なしで照合する
fn match_required_section(bytes: &[u8]) -> Option<RequiredSection> {
    if bytes.is_empty() || bytes.len() > 64 || !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut ascii = [0u8; 32];
    let mut len = 0usize;
    for pair in bytes.as_chunks::<2>().0 {
        let code = u16::from_le_bytes(*pair);
        if code == 0 {
            break;
        }
        let byte = u8::try_from(code).ok()?;
        *ascii.get_mut(len)? = byte;
        len += 1;
    }
    match ascii.get(..len)? {
        b"AcDb:Header" => Some(RequiredSection::Header),
        b"AcDb:Classes" => Some(RequiredSection::Classes),
        b"AcDb:Handles" => Some(RequiredSection::Handles),
        b"AcDb:AcDbObjects" => Some(RequiredSection::Objects),
        _ => None,
    }
}

/// データセクションのページを順に展開してつなげる
fn read_data_section(
    src: &[u8],
    data_size: u64,
    encoding: u64,
    local_pages: &[u8],
    pages: &[Page],
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    if encoding != 1 && encoding != 4 {
        return Err(invalid(cx, "data page", HEADER));
    }
    cx.check(Kind::DecodedBytes, data_size)
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::Records, (local_pages.len() / LOCAL_PAGE_SIZE) as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let total_size =
        checked_alloc_len(data_size).ok_or_else(|| invalid(cx, "data page", HEADER))?;
    if local_pages.is_empty() && total_size > 0 {
        return Err(invalid(cx, "data page", HEADER));
    }

    let mut out = Vec::new();
    for entry in local_pages.as_chunks::<LOCAL_PAGE_SIZE>().0 {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let page_offset = le64(entry, 0x00).unwrap_or_default();
        let page_id = le64(entry, 0x10).unwrap_or_default() as i64;
        let uncomp_size = le64(entry, 0x18).unwrap_or_default();
        let comp_size = le64(entry, 0x20).unwrap_or_default();

        let rec = find_page(pages, page_id).ok_or_else(|| invalid(cx, "data page", HEADER))?;
        let at = PAGES_START
            .checked_add(rec.offset)
            .ok_or_else(|| invalid(cx, "data page", HEADER))?;
        let rec_end = at
            .checked_add(rec.size)
            .filter(|&end| end <= src.len() as u64)
            .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
        let (Ok(start), Ok(end)) = (usize::try_from(at), usize::try_from(rec_end)) else {
            return Err(invalid(cx, "data page", page_range(at)));
        };
        let raw = src
            .get(start..end)
            .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
        let span = start..end;

        let decoded_storage: Vec<u8>;
        let page_input: &[u8] = if encoding == 4 {
            let aligned = comp_size
                .checked_add(7)
                .map(|v| v & !7)
                .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
            let factor_u64 = aligned.div_ceil(DATA_BLOCK_SIZE as u64);
            let encoded_needed = factor_u64
                .checked_mul(CODEWORD_LEN as u64)
                .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
            if encoded_needed > raw.len() as u64 {
                return Err(invalid(cx, "data page", span));
            }
            let rs_u64 = factor_u64
                .checked_mul(DATA_BLOCK_SIZE as u64)
                .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
            let (Ok(rs_len), Ok(factor)) = (usize::try_from(rs_u64), usize::try_from(factor_u64))
            else {
                return Err(invalid(cx, "data page", span));
            };
            let (decoded, rs_corrupted) = map_decode(
                rs_decode(raw, rs_len, factor, DATA_BLOCK_SIZE, DATA_POLY, cx),
                cx,
                "data page",
                span.clone(),
            )?;
            if rs_corrupted {
                crc_invalid(cx, "data page", span.clone());
            }
            decoded_storage = decoded;
            &decoded_storage
        } else {
            if comp_size != uncomp_size {
                return Err(invalid(cx, "data page", span));
            }
            raw
        };

        let decompressed_storage: Vec<u8>;
        let page_plain: &[u8] = if encoding == 4 && comp_size != uncomp_size {
            cx.check(Kind::DecodedBytes, uncomp_size)
                .map_err(|stop| stopped(cx, stop))?;
            let uncomp_len = checked_alloc_len(uncomp_size)
                .ok_or_else(|| invalid(cx, "data page", span.clone()))?;
            let comp_len = usize::try_from(comp_size)
                .ok()
                .ok_or_else(|| invalid(cx, "data page", span.clone()))?;
            let comp = page_input
                .get(..comp_len)
                .ok_or_else(|| invalid(cx, "data page", span.clone()))?;
            decompressed_storage = map_decode(
                decompress_to_vec(comp, uncomp_len, cx),
                cx,
                "data page",
                span,
            )?;
            &decompressed_storage
        } else {
            let uncomp_len = usize::try_from(uncomp_size)
                .ok()
                .ok_or_else(|| invalid(cx, "data page", span.clone()))?;
            page_input
                .get(..uncomp_len)
                .ok_or_else(|| invalid(cx, "data page", span))?
        };

        let dst_off = usize::try_from(page_offset)
            .ok()
            .filter(|&off| off <= total_size)
            .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
        let copy_len = page_plain.len().min(total_size - dst_off);
        let needed_len = dst_off
            .checked_add(copy_len)
            .filter(|&len| len <= total_size)
            .ok_or_else(|| invalid(cx, "data page", page_range(at)))?;
        if out.len() < needed_len {
            out.try_reserve(needed_len - out.len())
                .map_err(|_| invalid(cx, "data page", page_range(at)))?;
            out.resize(needed_len, 0);
        }
        if let (Some(dst), Some(from)) = (
            out.get_mut(dst_off..dst_off + copy_len),
            page_plain.get(..copy_len),
        ) {
            dst.copy_from_slice(from);
        }
    }
    if out.len() < total_size {
        out.try_reserve(total_size - out.len())
            .map_err(|_| invalid(cx, "data page", HEADER))?;
        out.resize(total_size, 0);
    }
    Ok(out)
}

/// GF(2^8) の指数・対数表
struct Gf256 {
    exp: [u8; 512],
    log: [u8; 256],
}

impl Gf256 {
    fn new(poly: u16) -> Self {
        let mut exp = [0u8; 512];
        let mut log = [0u8; 256];
        let mut x = 1u16;
        for i in 0..255usize {
            let b = x as u8;
            if let Some(slot) = exp.get_mut(i) {
                *slot = b;
            }
            if let Some(slot) = exp.get_mut(i + 255) {
                *slot = b;
            }
            if let Some(slot) = log.get_mut(usize::from(b)) {
                *slot = i as u8;
            }
            x <<= 1;
            if (x & 0x100) != 0 {
                x ^= poly;
            }
        }
        Self { exp, log }
    }

    fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        let la = usize::from(*self.log.get(usize::from(a)).unwrap_or(&0));
        let lb = usize::from(*self.log.get(usize::from(b)).unwrap_or(&0));
        *self.exp.get(la + lb).unwrap_or(&0)
    }

    fn div(&self, a: u8, b: u8) -> Option<u8> {
        if b == 0 {
            return None;
        }
        if a == 0 {
            return Some(0);
        }
        let la = usize::from(*self.log.get(usize::from(a))?);
        let lb = usize::from(*self.log.get(usize::from(b))?);
        self.exp.get(la + 255 - lb).copied()
    }
}

/// 1 ブロック（255 バイト）の Reed-Solomon 符号を検査・誤り訂正する。誤りがあったかを返す
fn rs_correct_codeword(cw: &mut [u8; CODEWORD_LEN], nroots: usize, gf: &Gf256) -> bool {
    let mut syn = [0u8; 16];
    let mut any_error = false;
    for r in 0..nroots {
        let mut acc = 0u8;
        let alpha_r = *gf.exp.get(r + 1).unwrap_or(&1);
        for &byte in cw.iter().rev() {
            acc = gf.mul(acc, alpha_r) ^ byte;
        }
        if let Some(slot) = syn.get_mut(r) {
            *slot = acc;
        }
        if acc != 0 {
            any_error = true;
        }
    }
    if !any_error {
        return false;
    }

    let original = *cw;
    // Berlekamp-Massey で誤り位置多項式を求める
    let mut c_poly = [0u8; 17];
    let mut b_poly = [0u8; 17];
    c_poly[0] = 1;
    b_poly[0] = 1;
    let mut l = 0usize;
    let mut m = 1usize;
    let mut b = 1u8;
    for n in 0..nroots {
        let mut d = *syn.get(n).unwrap_or(&0);
        for i in 1..=l {
            let ci = *c_poly.get(i).unwrap_or(&0);
            let sn = *syn.get(n - i).unwrap_or(&0);
            d ^= gf.mul(ci, sn);
        }
        if d == 0 {
            m += 1;
        } else {
            let t_poly = c_poly;
            let Some(factor) = gf.div(d, b) else {
                return true;
            };
            for i in 0..=(nroots - m) {
                let bi = *b_poly.get(i).unwrap_or(&0);
                if let Some(slot) = c_poly.get_mut(i + m) {
                    *slot ^= gf.mul(factor, bi);
                }
            }
            if 2 * l <= n {
                l = n + 1 - l;
                b_poly = t_poly;
                b = d;
                m = 1;
            } else {
                m += 1;
            }
        }
    }
    if l == 0 || l > nroots / 2 {
        return true;
    }

    // Chien 探索で誤り位置を求める
    let mut err_pos = [0usize; 8];
    let mut found = 0usize;
    for j in 0..CODEWORD_LEN {
        let inv_x = *gf.exp.get((255 - j) % 255).unwrap_or(&1);
        let mut val = 1u8;
        let mut p = 1u8;
        for u in 1..=l {
            p = gf.mul(p, inv_x);
            val ^= gf.mul(*c_poly.get(u).unwrap_or(&0), p);
        }
        if val == 0 {
            let Some(slot) = err_pos.get_mut(found) else {
                return true;
            };
            *slot = j;
            found += 1;
        }
    }
    if found != l {
        return true;
    }

    // Forney アルゴリズムで誤り値を求めて訂正する
    let mut omega = [0u8; 16];
    for i in 0..nroots {
        let mut acc = 0u8;
        for j in 0..=i.min(l) {
            acc ^= gf.mul(*syn.get(i - j).unwrap_or(&0), *c_poly.get(j).unwrap_or(&0));
        }
        if let Some(slot) = omega.get_mut(i) {
            *slot = acc;
        }
    }
    for &pos in err_pos.get(..found).unwrap_or_default() {
        let inv_x = *gf.exp.get((255 - pos) % 255).unwrap_or(&1);
        let mut num = 0u8;
        let mut p = 1u8;
        for &om in omega.get(..nroots).unwrap_or_default() {
            num ^= gf.mul(om, p);
            p = gf.mul(p, inv_x);
        }
        let mut den = 0u8;
        let inv_x2 = gf.mul(inv_x, inv_x);
        let mut p2 = 1u8;
        let mut u = 1usize;
        while u <= l {
            den ^= gf.mul(*c_poly.get(u).unwrap_or(&0), p2);
            p2 = gf.mul(p2, inv_x2);
            u += 2;
        }
        let Some(err_val) = gf.div(num, den) else {
            *cw = original;
            return true;
        };
        if let Some(slot) = cw.get_mut(pos) {
            *slot ^= err_val;
        }
    }

    // 訂正後にすべてのシンドロームが 0 になったことを確かめ、合わなければ元に戻す
    for r in 0..nroots {
        let mut acc = 0u8;
        let alpha_r = *gf.exp.get(r + 1).unwrap_or(&1);
        for &byte in cw.iter().rev() {
            acc = gf.mul(acc, alpha_r) ^ byte;
        }
        if acc != 0 {
            *cw = original;
            return true;
        }
    }
    true
}

/// インターリーブされた組織符号の Reed-Solomon を検査・誤り訂正して情報記号を取り出す
fn rs_decode(
    encoded: &[u8],
    out_len: usize,
    factor: usize,
    block_size: usize,
    poly: u16,
    cx: &mut Cx,
) -> Result<(Vec<u8>, bool), DecodeError> {
    if out_len == 0 || factor == 0 || block_size == 0 || block_size >= CODEWORD_LEN {
        return Ok((Vec::new(), false));
    }
    let needed = factor
        .checked_mul(CODEWORD_LEN)
        .ok_or(DecodeError::Invalid)?;
    if needed > encoded.len() {
        return Err(DecodeError::Invalid);
    }
    cx.check(Kind::DecodedBytes, out_len as u64)?;
    cx.check(Kind::Work, (factor as u64).saturating_mul(4))?;
    let mut out = Vec::new();
    out.try_reserve_exact(out_len)
        .map_err(|_| DecodeError::Invalid)?;
    out.resize(out_len, 0);

    let gf = Gf256::new(poly);
    let nroots = CODEWORD_LEN - block_size;
    let mut corrupted = false;
    let mut index = 0usize;
    let mut remaining = out_len;
    for i in 0..factor {
        cx.check(Kind::Work, 1)?;
        let mut cw = [0u8; CODEWORD_LEN];
        let mut cindex = i;
        for slot in &mut cw {
            *slot = *encoded.get(cindex).ok_or(DecodeError::Invalid)?;
            cindex = cindex.checked_add(factor).ok_or(DecodeError::Invalid)?;
        }
        if rs_correct_codeword(&mut cw, nroots, &gf) {
            corrupted = true;
        }
        let size = remaining.min(block_size);
        remaining -= size;
        let dst = out
            .get_mut(index..index + size)
            .ok_or(DecodeError::Invalid)?;
        let src_cw = cw.get(..size).ok_or(DecodeError::Invalid)?;
        dst.copy_from_slice(src_cw);
        index += size;
    }
    if remaining == 0 {
        Ok((out, corrupted))
    } else {
        Err(DecodeError::Invalid)
    }
}

/// 段階的に容量を伸ばしながら R2007 方式の LZ77 を展開し、期待長ちょうどの出力を返す
fn decompress_to_vec(src: &[u8], target_len: usize, cx: &mut Cx) -> Result<Vec<u8>, DecodeError> {
    if target_len == 0 {
        return Ok(Vec::new());
    }
    if src.len() < 2 {
        return Err(DecodeError::Invalid);
    }
    let mut dst = Vec::new();
    let end = src.len();
    let mut pos = 0usize;
    let mut op = usize::from(*src.get(pos).ok_or(DecodeError::Invalid)?);
    pos += 1;
    let mut dst_pos = 0usize;
    let mut length = 0usize;
    let mut src_offset = 0usize;
    if (op & 0xF0) == 0x20 {
        pos = pos.checked_add(3).ok_or(DecodeError::Invalid)?;
        if pos > end {
            return Err(DecodeError::Invalid);
        }
        length = usize::from(*src.get(pos - 1).ok_or(DecodeError::Invalid)?) & 0x07;
    }
    while pos < end {
        cx.check(Kind::Work, 1)?;
        if length == 0 {
            let lit_u64 = read_literal_length(src, &mut pos, op, cx)?;
            cx.check(Kind::Work, (lit_u64 / 32).max(1))?;
            if lit_u64 > (end - pos) as u64 {
                return Err(DecodeError::Invalid);
            }
            length = usize::try_from(lit_u64).map_err(|_| DecodeError::Invalid)?;
        } else {
            cx.check(Kind::Work, (length / 32).max(1) as u64)?;
        }
        let next_dst = dst_pos.checked_add(length).ok_or(DecodeError::Invalid)?;
        if next_dst > target_len {
            return Err(DecodeError::Invalid);
        }
        grow_to(&mut dst, next_dst, target_len)?;
        copy_permuted(src, pos, &mut dst, dst_pos, length).ok_or(DecodeError::Invalid)?;
        pos = pos.checked_add(length).ok_or(DecodeError::Invalid)?;
        dst_pos = next_dst;
        if pos >= end {
            break;
        }
        length = 0;
        op = usize::from(*src.get(pos).ok_or(DecodeError::Invalid)?);
        pos += 1;
        read_instructions(src, &mut pos, &mut op, &mut length, &mut src_offset)
            .ok_or(DecodeError::Invalid)?;
        loop {
            cx.check(Kind::Work, (length / 32).max(1) as u64)?;
            let next_back = dst_pos.checked_add(length).ok_or(DecodeError::Invalid)?;
            if next_back > target_len {
                return Err(DecodeError::Invalid);
            }
            grow_to(&mut dst, next_back, target_len)?;
            copy_backref(&mut dst, dst_pos, length, src_offset).ok_or(DecodeError::Invalid)?;
            dst_pos = next_back;
            length = op & 0x07;
            if length != 0 || pos >= end {
                break;
            }
            op = usize::from(*src.get(pos).ok_or(DecodeError::Invalid)?);
            pos += 1;
            if (op >> 4) == 0 {
                break;
            }
            if (op >> 4) == 15 {
                op &= 0x0F;
            }
            read_instructions(src, &mut pos, &mut op, &mut length, &mut src_offset)
                .ok_or(DecodeError::Invalid)?;
        }
    }
    if dst_pos != target_len {
        return Err(DecodeError::Invalid);
    }
    Ok(dst)
}

fn grow_to(dst: &mut Vec<u8>, needed: usize, target_len: usize) -> Result<(), DecodeError> {
    if dst.len() >= needed {
        return Ok(());
    }
    if dst.capacity() < needed {
        let next_cap = dst
            .capacity()
            .saturating_mul(2)
            .max(GROW_STEP)
            .max(needed)
            .min(target_len);
        dst.try_reserve_exact(next_cap - dst.len())
            .map_err(|_| DecodeError::Invalid)?;
    }
    dst.resize(needed, 0);
    Ok(())
}

fn checked_alloc_len(size: u64) -> Option<usize> {
    if size > MAX_ALLOC {
        return None;
    }
    usize::try_from(size).ok()
}

fn read_literal_length(
    src: &[u8],
    pos: &mut usize,
    op: usize,
    cx: &mut Cx,
) -> Result<u64, DecodeError> {
    let mut length = (op as u64).checked_add(8).ok_or(DecodeError::Invalid)?;
    if length == 0x17 {
        let n_byte = u64::from(*src.get(*pos).ok_or(DecodeError::Invalid)?);
        *pos += 1;
        length = length.checked_add(n_byte).ok_or(DecodeError::Invalid)?;
        if n_byte == 0xFF {
            loop {
                cx.check(Kind::Work, 1)?;
                let lo = u64::from(*src.get(*pos).ok_or(DecodeError::Invalid)?);
                let hi = u64::from(*src.get(*pos + 1).ok_or(DecodeError::Invalid)?);
                *pos += 2;
                let n16 = lo | (hi << 8);
                length = length.checked_add(n16).ok_or(DecodeError::Invalid)?;
                if n16 != 0xFFFF {
                    break;
                }
            }
        }
    }
    Ok(length)
}

fn read_instructions(
    src: &[u8],
    pos: &mut usize,
    op: &mut usize,
    length: &mut usize,
    src_offset: &mut usize,
) -> Option<()> {
    match *op >> 4 {
        0 => {
            *length = (*op & 0x0F) + 0x13;
            let b0 = usize::from(*src.get(*pos)?);
            let b1 = usize::from(*src.get(*pos + 1)?);
            *pos += 2;
            *op = b1;
            *length += (b1 >> 3) & 0x10;
            *src_offset = ((b1 & 0x78) << 5) + 1 + b0;
        }
        1 => {
            *length = (*op & 0x0F) + 3;
            let b0 = usize::from(*src.get(*pos)?);
            let b1 = usize::from(*src.get(*pos + 1)?);
            *pos += 2;
            *op = b1;
            *src_offset = ((b1 & 0xF8) << 5) + 1 + b0;
        }
        2 => {
            let b0 = usize::from(*src.get(*pos)?);
            let b1 = usize::from(*src.get(*pos + 1)?);
            *pos += 2;
            *src_offset = ((b1 << 8) & 0xFF00) | b0;
            *length = *op & 0x07;
            if (*op & 0x08) == 0 {
                let b2 = usize::from(*src.get(*pos)?);
                *pos += 1;
                *op = b2;
                *length += b2 & 0xF8;
            } else {
                *src_offset += 1;
                let b2 = usize::from(*src.get(*pos)?);
                let b3 = usize::from(*src.get(*pos + 1)?);
                *pos += 2;
                *length += b2 << 3;
                *op = b3;
                *length += ((b3 & 0xF8) << 8) + 0x100;
            }
        }
        _ => {
            *length = *op >> 4;
            *src_offset = *op & 0x0F;
            let b0 = usize::from(*src.get(*pos)?);
            *pos += 1;
            *op = b0;
            *src_offset = ((b0 & 0xF8) << 1) + *src_offset + 1;
        }
    }
    Some(())
}

fn copy_backref(dst: &mut [u8], dst_pos: usize, length: usize, src_offset: usize) -> Option<()> {
    if src_offset == 0 {
        return None;
    }
    let init = dst_pos.checked_sub(src_offset)?;
    let end = dst_pos.checked_add(length)?;
    if end > dst.len() {
        return None;
    }
    for i in 0..length {
        let byte = *dst.get(init + i)?;
        *dst.get_mut(dst_pos + i)? = byte;
    }
    Some(())
}

/// R2007 の 32 バイト単位・端数（1〜31 バイト）の並び替えコピー
fn copy_permuted(
    src: &[u8],
    mut si: usize,
    dst: &mut [u8],
    mut di: usize,
    mut length: usize,
) -> Option<()> {
    if si.checked_add(length)? > src.len() || di.checked_add(length)? > dst.len() {
        return None;
    }
    while length >= 32 {
        c4(src, si + 24, dst, di)?;
        c4(src, si + 28, dst, di + 4)?;
        c4(src, si + 16, dst, di + 8)?;
        c4(src, si + 20, dst, di + 12)?;
        c4(src, si + 8, dst, di + 16)?;
        c4(src, si + 12, dst, di + 20)?;
        c4(src, si, dst, di + 24)?;
        c4(src, si + 4, dst, di + 28)?;
        si += 32;
        di += 32;
        length -= 32;
    }
    match length {
        0 => {}
        1 => c1(src, si, dst, di)?,
        2 => c2(src, si, dst, di)?,
        3 => c3(src, si, dst, di)?,
        4 => c4(src, si, dst, di)?,
        5 => {
            c1(src, si + 4, dst, di)?;
            c4(src, si, dst, di + 1)?;
        }
        6 => {
            c1(src, si + 5, dst, di)?;
            c4(src, si + 1, dst, di + 1)?;
            c1(src, si, dst, di + 5)?;
        }
        7 => {
            c2(src, si + 5, dst, di)?;
            c4(src, si + 1, dst, di + 2)?;
            c1(src, si, dst, di + 6)?;
        }
        8 => c8(src, si, dst, di)?,
        9 => {
            c1(src, si + 8, dst, di)?;
            c8(src, si, dst, di + 1)?;
        }
        10 => {
            c1(src, si + 9, dst, di)?;
            c8(src, si + 1, dst, di + 1)?;
            c1(src, si, dst, di + 9)?;
        }
        11 => {
            c2(src, si + 9, dst, di)?;
            c8(src, si + 1, dst, di + 2)?;
            c1(src, si, dst, di + 10)?;
        }
        12 => {
            c4(src, si + 8, dst, di)?;
            c8(src, si, dst, di + 4)?;
        }
        13 => {
            c1(src, si + 12, dst, di)?;
            c4(src, si + 8, dst, di + 1)?;
            c8(src, si, dst, di + 5)?;
        }
        14 => {
            c1(src, si + 13, dst, di)?;
            c4(src, si + 9, dst, di + 1)?;
            c8(src, si + 1, dst, di + 5)?;
            c1(src, si, dst, di + 13)?;
        }
        15 => {
            c2(src, si + 13, dst, di)?;
            c4(src, si + 9, dst, di + 2)?;
            c8(src, si + 1, dst, di + 6)?;
            c1(src, si, dst, di + 14)?;
        }
        16 => c16(src, si, dst, di)?,
        17 => {
            c8(src, si + 9, dst, di)?;
            c1(src, si + 8, dst, di + 8)?;
            c8(src, si, dst, di + 9)?;
        }
        18 => {
            c1(src, si + 17, dst, di)?;
            c16(src, si + 1, dst, di + 1)?;
            c1(src, si, dst, di + 17)?;
        }
        19 => {
            c3(src, si + 16, dst, di)?;
            c16(src, si, dst, di + 3)?;
        }
        20 => {
            c4(src, si + 16, dst, di)?;
            c8(src, si + 8, dst, di + 4)?;
            c8(src, si, dst, di + 12)?;
        }
        21 => {
            c1(src, si + 20, dst, di)?;
            c4(src, si + 16, dst, di + 1)?;
            c8(src, si + 8, dst, di + 5)?;
            c8(src, si, dst, di + 13)?;
        }
        22 => {
            c2(src, si + 20, dst, di)?;
            c4(src, si + 16, dst, di + 2)?;
            c8(src, si + 8, dst, di + 6)?;
            c8(src, si, dst, di + 14)?;
        }
        23 => {
            c3(src, si + 20, dst, di)?;
            c4(src, si + 16, dst, di + 3)?;
            c8(src, si + 8, dst, di + 7)?;
            c8(src, si, dst, di + 15)?;
        }
        24 => {
            c8(src, si + 16, dst, di)?;
            c16(src, si, dst, di + 8)?;
        }
        25 => {
            c8(src, si + 17, dst, di)?;
            c1(src, si + 16, dst, di + 8)?;
            c16(src, si, dst, di + 9)?;
        }
        26 => {
            c1(src, si + 25, dst, di)?;
            c8(src, si + 17, dst, di + 1)?;
            c1(src, si + 16, dst, di + 9)?;
            c16(src, si, dst, di + 10)?;
        }
        27 => {
            c2(src, si + 25, dst, di)?;
            c8(src, si + 17, dst, di + 2)?;
            c1(src, si + 16, dst, di + 10)?;
            c16(src, si, dst, di + 11)?;
        }
        28 => {
            c4(src, si + 24, dst, di)?;
            c8(src, si + 16, dst, di + 4)?;
            c8(src, si + 8, dst, di + 12)?;
            c8(src, si, dst, di + 20)?;
        }
        29 => {
            c1(src, si + 28, dst, di)?;
            c4(src, si + 24, dst, di + 1)?;
            c8(src, si + 16, dst, di + 5)?;
            c8(src, si + 8, dst, di + 13)?;
            c8(src, si, dst, di + 21)?;
        }
        30 => {
            c2(src, si + 28, dst, di)?;
            c4(src, si + 24, dst, di + 2)?;
            c8(src, si + 16, dst, di + 6)?;
            c8(src, si + 8, dst, di + 14)?;
            c8(src, si, dst, di + 22)?;
        }
        _ => {
            c1(src, si + 30, dst, di)?;
            c4(src, si + 26, dst, di + 1)?;
            c8(src, si + 18, dst, di + 5)?;
            c8(src, si + 10, dst, di + 13)?;
            c8(src, si + 2, dst, di + 21)?;
            c2(src, si, dst, di + 29)?;
        }
    }
    Some(())
}

fn c1(s: &[u8], si: usize, d: &mut [u8], di: usize) -> Option<()> {
    *d.get_mut(di)? = *s.get(si)?;
    Some(())
}

fn c2(s: &[u8], si: usize, d: &mut [u8], di: usize) -> Option<()> {
    *d.get_mut(di)? = *s.get(si + 1)?;
    *d.get_mut(di + 1)? = *s.get(si)?;
    Some(())
}

fn c3(s: &[u8], si: usize, d: &mut [u8], di: usize) -> Option<()> {
    *d.get_mut(di)? = *s.get(si + 2)?;
    *d.get_mut(di + 1)? = *s.get(si + 1)?;
    *d.get_mut(di + 2)? = *s.get(si)?;
    Some(())
}

fn c4(s: &[u8], si: usize, d: &mut [u8], di: usize) -> Option<()> {
    d.get_mut(di..di + 4)?.copy_from_slice(s.get(si..si + 4)?);
    Some(())
}

fn c8(s: &[u8], si: usize, d: &mut [u8], di: usize) -> Option<()> {
    d.get_mut(di..di + 8)?.copy_from_slice(s.get(si..si + 8)?);
    Some(())
}

fn c16(s: &[u8], si: usize, d: &mut [u8], di: usize) -> Option<()> {
    c8(s, si + 8, d, di)?;
    c8(s, si, d, di + 8)?;
    Some(())
}

fn le32_signed(src: &[u8], at: usize) -> Option<i32> {
    let bytes = src.get(at..at + 4)?;
    Some(i32::from_le_bytes(bytes.try_into().ok()?))
}

fn le64(src: &[u8], at: usize) -> Option<u64> {
    let bytes = src.get(at..at + 8)?;
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

fn page_range(at: u64) -> Range<usize> {
    let start = usize::try_from(at).unwrap_or(usize::MAX);
    start..start.saturating_add(1)
}

fn crc_invalid(cx: &mut Cx, part: &str, at: Range<usize>) {
    cx.report(
        Diagnostic::new(codes::DWG_CRC_INVALID)
            .with("part", part)
            .at(Ref::at_bytes(at.start as u64, at.end as u64))
            .cite(R2007),
    );
}

fn invalid(cx: &mut Cx, part: &str, at: Range<usize>) -> Error {
    cx.report(
        Diagnostic::new(codes::DWG_FILE_INVALID)
            .with("part", part)
            .at(Ref::at_bytes(at.start as u64, at.end as u64))
            .cite(R2007),
    );
    Error::Invalid
}
