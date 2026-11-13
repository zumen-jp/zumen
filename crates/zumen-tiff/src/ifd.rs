//! TIFF 6.0 のヘッダー・IFD 鎖・タグの読み取りと、ストリップ・タイル画素の組み立て。

use std::collections::BTreeSet;

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Cx, Error};

use crate::decompress::{Fail, apply_predictor_2, decompress_chunk};
use crate::geotiff::{GeoInfo, RawGeoTags};
use crate::{MAX_ALLOC, Record, TiffFile, invalid, stopped};

/// TIFF のバイト順
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endian {
    /// リトルエンディアン（`II*\0`）
    Little,
    /// ビッグエンディアン（`MM\0*`）
    Big,
}

impl Endian {
    pub(crate) fn read_u16(self, src: &[u8], offset: usize) -> Option<u16> {
        let s = src.get(offset..offset.checked_add(2)?)?;
        let bytes = <[u8; 2]>::try_from(s).ok()?;
        Some(match self {
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        })
    }

    pub(crate) fn read_i16(self, src: &[u8], offset: usize) -> Option<i16> {
        let s = src.get(offset..offset.checked_add(2)?)?;
        let bytes = <[u8; 2]>::try_from(s).ok()?;
        Some(match self {
            Self::Little => i16::from_le_bytes(bytes),
            Self::Big => i16::from_be_bytes(bytes),
        })
    }

    pub(crate) fn read_u32(self, src: &[u8], offset: usize) -> Option<u32> {
        let s = src.get(offset..offset.checked_add(4)?)?;
        let bytes = <[u8; 4]>::try_from(s).ok()?;
        Some(match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        })
    }

    pub(crate) fn read_i32(self, src: &[u8], offset: usize) -> Option<i32> {
        let s = src.get(offset..offset.checked_add(4)?)?;
        let bytes = <[u8; 4]>::try_from(s).ok()?;
        Some(match self {
            Self::Little => i32::from_le_bytes(bytes),
            Self::Big => i32::from_be_bytes(bytes),
        })
    }

    pub(crate) fn read_f32(self, src: &[u8], offset: usize) -> Option<f32> {
        let s = src.get(offset..offset.checked_add(4)?)?;
        let bytes = <[u8; 4]>::try_from(s).ok()?;
        Some(match self {
            Self::Little => f32::from_le_bytes(bytes),
            Self::Big => f32::from_be_bytes(bytes),
        })
    }

    pub(crate) fn read_f64(self, src: &[u8], offset: usize) -> Option<f64> {
        let s = src.get(offset..offset.checked_add(8)?)?;
        let bytes = <[u8; 8]>::try_from(s).ok()?;
        Some(match self {
            Self::Little => f64::from_le_bytes(bytes),
            Self::Big => f64::from_be_bytes(bytes),
        })
    }
}

/// 色空間（`PhotometricInterpretation` = 262）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Photometric {
    /// 0 が白（`0`）
    WhiteIsZero,
    /// 0 が黒（`1`）
    BlackIsZero,
    /// RGB（`2`）
    Rgb,
    /// パレットカラー（`3`）
    Palette,
}

/// 標本値の数値型（`SampleFormat` = 339）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    /// 符号なし整数（`1`、既定）
    UInt,
    /// 2 の補数の符号付き整数（`2`）
    Int,
    /// IEEE 754 浮動小数点（`3`）
    Float,
}

/// チャネルの並び（`PlanarConfiguration` = 284）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanarConfig {
    /// 画素ごとにチャネルを並べる（`1`、既定）
    Chunky,
    /// チャネルごとに面を分ける（`2`）
    Planar,
}

/// 解像度の単位（`ResolutionUnit` = 296）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionUnit {
    /// 単位なし（`1`）
    None,
    /// インチ（`2`、既定）
    Inch,
    /// センチメートル（`3`）
    Centimeter,
}

/// アルファチャネルの種別（`ExtraSamples` = 338）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AlphaKind {
    None,
    Associated,
    Unassociated,
}

/// 1 ページ（IFD）の公開情報
#[derive(Clone, Debug, PartialEq)]
pub struct PageInfo {
    /// 表示・向き反映後の幅（ピクセル）
    pub width: u32,
    /// 表示・向き反映後の高さ（ピクセル）
    pub height: u32,
    /// サンプル（バンド）数
    pub samples_per_pixel: u16,
    /// チャネルごとのビット深さ（先頭チャネル）
    pub bits_per_sample: u16,
    /// 圧縮方式コード
    pub compression: u16,
    /// 色空間
    pub photometric: Option<Photometric>,
    /// 標本値の型
    pub sample_format: SampleFormat,
    /// 並び
    pub planar_config: PlanarConfig,
    /// X 解像度
    pub x_resolution: Option<f64>,
    /// Y 解像度
    pub y_resolution: Option<f64>,
    /// 解像度の単位
    pub resolution_unit: ResolutionUnit,
    /// GeoTIFF メタデータ
    pub geo: Option<GeoInfo>,
    /// L1 の出所参照
    pub source: Ref,
}

/// 復号済みページの内部表現
#[derive(Clone, Debug)]
pub(crate) struct DecodedIfd {
    pub info: PageInfo,
    /// 向き（`Orientation` 1〜8）。1 が標準（左上原点・行優先）
    pub orientation: u16,
    /// 向き回転前の格納幅
    pub raw_width: u32,
    /// 向き回転前の格納高さ
    pub raw_height: u32,
    pub alpha: AlphaKind,
    /// パレット（`ColorMap` 320: 長さ `3 * (1 << bits_per_sample)` の `u16`）
    pub color_map: Option<Vec<u16>>,
    pub data: PagePixels,
}

#[derive(Clone, Debug)]
pub(crate) enum PagePixels {
    /// 行ごとに詰め直した生サンプル列（Chunky 正規化済み・ネイティブバイト順ではなく TIFF の `endian`）
    Native(Vec<u8>),
    /// 未対応の圧縮方式または形式条件のためスキップしたページ
    Unsupported,
}

impl DecodedIfd {
    /// 単バンドの数値標高格子（平面直角または経緯度 GeoTIFF 変換付き）の候補か
    pub(crate) fn is_dem_candidate(&self) -> bool {
        if !matches!(self.data, PagePixels::Native(_)) {
            return false;
        }
        if self.info.samples_per_pixel != 1 || self.color_map.is_some() {
            return false;
        }
        let bits = self.info.bits_per_sample;
        let valid_numeric = match self.info.sample_format {
            SampleFormat::Float => matches!(bits, 32 | 64),
            SampleFormat::Int => matches!(bits, 8 | 16 | 32),
            SampleFormat::UInt => matches!(bits, 16 | 32),
        };
        if !valid_numeric {
            return false;
        }
        self.info
            .geo
            .as_ref()
            .is_some_and(|g| g.transform.is_some())
    }
}

#[derive(Clone, Copy)]
struct RawEntry {
    tag: u16,
    field_type: u16,
    count: u32,
    value_offset_pos: usize,
    entry_offset: usize,
}

fn type_byte_width(field_type: u16) -> Option<usize> {
    match field_type {
        1 | 2 | 6 | 7 => Some(1),
        3 | 8 => Some(2),
        4 | 9 | 11 => Some(4),
        5 | 10 | 12 => Some(8),
        _ => None,
    }
}

pub(crate) fn read_tiff(src: &[u8], cx: &mut Cx) -> Result<TiffFile, Error> {
    if src.len() < 8 {
        return Err(invalid(cx, 0));
    }
    let endian = match src.get(0..2) {
        Some(b"II") => Endian::Little,
        Some(b"MM") => Endian::Big,
        _ => return Err(invalid(cx, 0)),
    };
    let magic = endian.read_u16(src, 2).ok_or_else(|| invalid(cx, 2))?;
    if magic == 43 {
        cx.report(
            Diagnostic::new(codes::TIFF_FORMAT_UNSUPPORTED)
                .with("reason", "bigtiff")
                .at(Ref::at_bytes(0, 8)),
        );
        return Err(invalid(cx, 2));
    }
    if magic != 42 {
        return Err(invalid(cx, 2));
    }

    cx.check(Kind::Records, 1)
        .map_err(|s| stopped(cx, s, None))?;
    let first_ifd = endian.read_u32(src, 4).ok_or_else(|| invalid(cx, 4))?;
    if first_ifd == 0 {
        return Err(invalid(cx, 4));
    }

    let mut raw_ifds: Vec<(ByteRange, Vec<RawEntry>)> = Vec::new();
    let mut visited = BTreeSet::new();
    let mut cur_offset = usize::try_from(first_ifd).map_err(|_| invalid(cx, 4))?;

    while cur_offset != 0 {
        if !visited.insert(cur_offset as u64) {
            return Err(invalid(cx, cur_offset));
        }
        cx.check(Kind::Records, 1)
            .map_err(|s| stopped(cx, s, None))?;
        cx.check(Kind::Work, 1).map_err(|s| stopped(cx, s, None))?;

        let count = endian
            .read_u16(src, cur_offset)
            .ok_or_else(|| invalid(cx, cur_offset))?;
        let entries_bytes = usize::from(count)
            .checked_mul(12)
            .ok_or_else(|| invalid(cx, cur_offset))?;
        let next_ptr_pos = cur_offset
            .checked_add(2)
            .and_then(|p| p.checked_add(entries_bytes))
            .ok_or_else(|| invalid(cx, cur_offset))?;
        let ifd_end = next_ptr_pos
            .checked_add(4)
            .ok_or_else(|| invalid(cx, cur_offset))?;
        if ifd_end > src.len() || cur_offset < 8 {
            return Err(invalid(cx, cur_offset));
        }

        let mut entries = Vec::with_capacity(usize::from(count));
        for idx in 0..usize::from(count) {
            cx.check(Kind::Work, 1).map_err(|s| stopped(cx, s, None))?;
            let entry_offset = cur_offset + 2 + idx * 12;
            let tag = endian
                .read_u16(src, entry_offset)
                .ok_or_else(|| invalid(cx, entry_offset))?;
            let field_type = endian
                .read_u16(src, entry_offset + 2)
                .ok_or_else(|| invalid(cx, entry_offset + 2))?;
            let val_count = endian
                .read_u32(src, entry_offset + 4)
                .ok_or_else(|| invalid(cx, entry_offset + 4))?;
            entries.push(RawEntry {
                tag,
                field_type,
                count: val_count,
                value_offset_pos: entry_offset + 8,
                entry_offset,
            });
        }

        let next_ifd = endian
            .read_u32(src, next_ptr_pos)
            .ok_or_else(|| invalid(cx, next_ptr_pos))?;
        raw_ifds.push((
            ByteRange {
                start: cur_offset as u64,
                end: ifd_end as u64,
            },
            entries,
        ));
        cur_offset = usize::try_from(next_ifd).map_err(|_| invalid(cx, next_ptr_pos))?;
    }

    // L1 の Record は開始オフセット昇順・非重複で並べる
    let mut span_order: Vec<(usize, ByteRange)> = raw_ifds
        .iter()
        .enumerate()
        .map(|(i, (r, _))| (i, *r))
        .collect();
    span_order.sort_by_key(|(_, r)| (r.start, r.end));

    let mut records = Vec::with_capacity(1 + span_order.len());
    records.push(Record {
        kind: b"HEADER",
        bytes: ByteRange { start: 0, end: 8 },
    });
    let mut prev_end = 8u64;
    let mut record_index_for_ifd = vec![0u64; raw_ifds.len()];
    for (orig_idx, range) in span_order {
        if range.start < prev_end {
            return Err(invalid(cx, range.start as usize));
        }
        prev_end = range.end;
        let rec_idx = records.len() as u64;
        if let Some(slot) = record_index_for_ifd.get_mut(orig_idx) {
            *slot = rec_idx;
        }
        records.push(Record {
            kind: b"IFD",
            bytes: range,
        });
    }

    let mut pages = Vec::with_capacity(raw_ifds.len());
    for (ifd_idx, (range, entries)) in raw_ifds.into_iter().enumerate() {
        let rec_idx = record_index_for_ifd.get(ifd_idx).copied().unwrap_or(1);
        let source = Ref::at_record(rec_idx).with_bytes(range.start, range.end);
        let decoded = decode_ifd(src, endian, &entries, source, cx)?;
        pages.push(decoded);
    }

    Ok(TiffFile {
        records,
        endian,
        pages,
    })
}

fn value_slice<'a>(
    src: &'a [u8],
    endian: Endian,
    entry: RawEntry,
    cx: &mut Cx,
) -> Result<&'a [u8], Error> {
    let Some(unit) = type_byte_width(entry.field_type) else {
        return Ok(&[]);
    };
    let byte_len = usize::try_from(entry.count)
        .ok()
        .and_then(|c| c.checked_mul(unit))
        .ok_or_else(|| invalid(cx, entry.entry_offset))?;
    if byte_len <= 4 {
        src.get(entry.value_offset_pos..entry.value_offset_pos + byte_len)
            .ok_or_else(|| invalid(cx, entry.value_offset_pos))
    } else {
        let off32 = endian
            .read_u32(src, entry.value_offset_pos)
            .ok_or_else(|| invalid(cx, entry.value_offset_pos))?;
        let off = usize::try_from(off32).map_err(|_| invalid(cx, entry.value_offset_pos))?;
        let end = off
            .checked_add(byte_len)
            .ok_or_else(|| invalid(cx, entry.value_offset_pos))?;
        src.get(off..end)
            .ok_or_else(|| invalid(cx, entry.value_offset_pos))
    }
}

fn check_tag_alloc(count: u32, elem_size: u64, offset: usize, cx: &mut Cx) -> Result<usize, Error> {
    let n = usize::try_from(count).map_err(|_| invalid(cx, offset))?;
    let bytes = u64::from(count).saturating_mul(elem_size);
    if bytes > MAX_ALLOC {
        return Err(invalid(cx, offset));
    }
    cx.check(Kind::Work, u64::from(count).max(1))
        .map_err(|s| stopped(cx, s, None))?;
    cx.check(Kind::DecodedBytes, bytes)
        .map_err(|s| stopped(cx, s, None))?;
    Ok(n)
}

fn read_u64_values(
    src: &[u8],
    endian: Endian,
    entry: RawEntry,
    cx: &mut Cx,
) -> Result<Vec<u64>, Error> {
    let bytes = value_slice(src, endian, entry, cx)?;
    let count = check_tag_alloc(entry.count, 8, entry.entry_offset, cx)?;
    let mut out = Vec::with_capacity(count);
    match entry.field_type {
        1 | 7 => {
            for &b in bytes.iter().take(count) {
                out.push(u64::from(b));
            }
        }
        3 => {
            for i in 0..count {
                let v = endian
                    .read_u16(bytes, i * 2)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                out.push(u64::from(v));
            }
        }
        4 => {
            for i in 0..count {
                let v = endian
                    .read_u32(bytes, i * 4)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                out.push(u64::from(v));
            }
        }
        _ => return Err(invalid(cx, entry.entry_offset)),
    }
    Ok(out)
}

fn read_u16_values(
    src: &[u8],
    endian: Endian,
    entry: RawEntry,
    cx: &mut Cx,
) -> Result<Vec<u16>, Error> {
    let bytes = value_slice(src, endian, entry, cx)?;
    let count = check_tag_alloc(entry.count, 2, entry.entry_offset, cx)?;
    let mut out = Vec::with_capacity(count);
    match entry.field_type {
        1 | 7 => {
            for &b in bytes.iter().take(count) {
                out.push(u16::from(b));
            }
        }
        3 => {
            for i in 0..count {
                let v = endian
                    .read_u16(bytes, i * 2)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                out.push(v);
            }
        }
        4 => {
            for i in 0..count {
                let v = endian
                    .read_u32(bytes, i * 4)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                let v16 = u16::try_from(v).map_err(|_| invalid(cx, entry.entry_offset))?;
                out.push(v16);
            }
        }
        _ => return Err(invalid(cx, entry.entry_offset)),
    }
    Ok(out)
}

fn read_f64_values(
    src: &[u8],
    endian: Endian,
    entry: RawEntry,
    cx: &mut Cx,
) -> Result<Vec<f64>, Error> {
    let bytes = value_slice(src, endian, entry, cx)?;
    let count = check_tag_alloc(entry.count, 8, entry.entry_offset, cx)?;
    let mut out = Vec::with_capacity(count);
    match entry.field_type {
        5 => {
            for i in 0..count {
                let num = endian
                    .read_u32(bytes, i * 8)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                let den = endian
                    .read_u32(bytes, i * 8 + 4)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                if den == 0 {
                    out.push(f64::NAN);
                } else {
                    out.push(f64::from(num) / f64::from(den));
                }
            }
        }
        10 => {
            for i in 0..count {
                let num = endian
                    .read_i32(bytes, i * 8)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                let den = endian
                    .read_i32(bytes, i * 8 + 4)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                if den == 0 {
                    out.push(f64::NAN);
                } else {
                    out.push(f64::from(num) / f64::from(den));
                }
            }
        }
        11 => {
            for i in 0..count {
                let v = endian
                    .read_f32(bytes, i * 4)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                out.push(f64::from(v));
            }
        }
        12 => {
            for i in 0..count {
                let v = endian
                    .read_f64(bytes, i * 8)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                out.push(v);
            }
        }
        1 => {
            for &b in bytes.iter().take(count) {
                out.push(f64::from(b));
            }
        }
        3 => {
            for i in 0..count {
                let v = endian
                    .read_u16(bytes, i * 2)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                out.push(f64::from(v));
            }
        }
        4 => {
            for i in 0..count {
                let v = endian
                    .read_u32(bytes, i * 4)
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                out.push(f64::from(v));
            }
        }
        _ => return Err(invalid(cx, entry.entry_offset)),
    }
    Ok(out)
}

fn read_ascii_string(
    src: &[u8],
    endian: Endian,
    entry: RawEntry,
    cx: &mut Cx,
) -> Result<String, Error> {
    let bytes = value_slice(src, endian, entry, cx)?;
    if bytes.len() as u64 > MAX_ALLOC {
        return Err(invalid(cx, entry.entry_offset));
    }
    cx.check(Kind::Work, (bytes.len() as u64).max(1))
        .map_err(|s| stopped(cx, s, None))?;
    cx.check(Kind::DecodedBytes, bytes.len() as u64)
        .map_err(|s| stopped(cx, s, None))?;
    Ok(String::from_utf8_lossy(bytes).into_owned())
}

#[allow(clippy::too_many_lines)]
fn decode_ifd(
    src: &[u8],
    endian: Endian,
    entries: &[RawEntry],
    source: Ref,
    cx: &mut Cx,
) -> Result<DecodedIfd, Error> {
    let mut raw_width: Option<u32> = None;
    let mut raw_height: Option<u32> = None;
    let mut bits_per_sample: Vec<u16> = Vec::new();
    let mut compression: u16 = 1;
    let mut photometric_raw: Option<u16> = None;
    let mut fill_order: u16 = 1;
    let mut strip_offsets: Option<Vec<u64>> = None;
    let mut orientation: u16 = 1;
    let mut samples_per_pixel: u16 = 1;
    let mut rows_per_strip: Option<u32> = None;
    let mut strip_byte_counts: Option<Vec<u64>> = None;
    let mut x_resolution: Option<f64> = None;
    let mut y_resolution: Option<f64> = None;
    let mut planar_raw: u16 = 1;
    let mut t6_options: u32 = 0;
    let mut resolution_unit_raw: u16 = 2;
    let mut predictor: u16 = 1;
    let mut color_map: Option<Vec<u16>> = None;
    let mut tile_width: Option<u32> = None;
    let mut tile_length: Option<u32> = None;
    let mut tile_offsets: Option<Vec<u64>> = None;
    let mut tile_byte_counts: Option<Vec<u64>> = None;
    let mut extra_samples: Vec<u16> = Vec::new();
    let mut sample_formats: Vec<u16> = Vec::new();
    let mut geo_tags = RawGeoTags::default();

    for &entry in entries {
        match entry.tag {
            256 => {
                let v = read_u64_values(src, endian, entry, cx)?
                    .first()
                    .copied()
                    .and_then(|x| u32::try_from(x).ok())
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                raw_width = Some(v);
            }
            257 => {
                let v = read_u64_values(src, endian, entry, cx)?
                    .first()
                    .copied()
                    .and_then(|x| u32::try_from(x).ok())
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
                raw_height = Some(v);
            }
            258 => bits_per_sample = read_u16_values(src, endian, entry, cx)?,
            259 => {
                compression = *read_u16_values(src, endian, entry, cx)?
                    .first()
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
            }
            262 => {
                photometric_raw = read_u16_values(src, endian, entry, cx)?.first().copied();
            }
            266 => {
                if let Some(v) = read_u16_values(src, endian, entry, cx)?.first().copied() {
                    fill_order = v;
                }
            }
            273 => strip_offsets = Some(read_u64_values(src, endian, entry, cx)?),
            274 => {
                if let Some(v) = read_u16_values(src, endian, entry, cx)?.first().copied() {
                    orientation = v;
                }
            }
            277 => {
                samples_per_pixel = *read_u16_values(src, endian, entry, cx)?
                    .first()
                    .ok_or_else(|| invalid(cx, entry.entry_offset))?;
            }
            278 => {
                let v = read_u64_values(src, endian, entry, cx)?
                    .first()
                    .copied()
                    .unwrap_or(u64::from(u32::MAX));
                rows_per_strip = Some(u32::try_from(v).unwrap_or(u32::MAX));
            }
            279 => strip_byte_counts = Some(read_u64_values(src, endian, entry, cx)?),
            282 => {
                x_resolution = read_f64_values(src, endian, entry, cx)?
                    .first()
                    .copied()
                    .filter(|v| v.is_finite() && *v > 0.0);
            }
            283 => {
                y_resolution = read_f64_values(src, endian, entry, cx)?
                    .first()
                    .copied()
                    .filter(|v| v.is_finite() && *v > 0.0);
            }
            284 => {
                if let Some(v) = read_u16_values(src, endian, entry, cx)?.first().copied() {
                    planar_raw = v;
                }
            }
            293 => {
                if let Some(v) = read_u64_values(src, endian, entry, cx)?.first().copied() {
                    t6_options = u32::try_from(v).unwrap_or(u32::MAX);
                }
            }
            296 => {
                if let Some(v) = read_u16_values(src, endian, entry, cx)?.first().copied() {
                    resolution_unit_raw = v;
                }
            }
            317 => {
                if let Some(v) = read_u16_values(src, endian, entry, cx)?.first().copied() {
                    predictor = v;
                }
            }
            320 => color_map = Some(read_u16_values(src, endian, entry, cx)?),
            322 => {
                tile_width = read_u64_values(src, endian, entry, cx)?
                    .first()
                    .copied()
                    .and_then(|x| u32::try_from(x).ok());
            }
            323 => {
                tile_length = read_u64_values(src, endian, entry, cx)?
                    .first()
                    .copied()
                    .and_then(|x| u32::try_from(x).ok());
            }
            324 => tile_offsets = Some(read_u64_values(src, endian, entry, cx)?),
            325 => tile_byte_counts = Some(read_u64_values(src, endian, entry, cx)?),
            338 => extra_samples = read_u16_values(src, endian, entry, cx)?,
            339 => sample_formats = read_u16_values(src, endian, entry, cx)?,
            33550 => geo_tags.pixel_scale = Some(read_f64_values(src, endian, entry, cx)?),
            33922 => geo_tags.tiepoints = Some(read_f64_values(src, endian, entry, cx)?),
            34264 => geo_tags.transformation = Some(read_f64_values(src, endian, entry, cx)?),
            34735 => geo_tags.key_directory = Some(read_u16_values(src, endian, entry, cx)?),
            34736 => geo_tags.double_params = Some(read_f64_values(src, endian, entry, cx)?),
            34737 => geo_tags.ascii_params = Some(read_ascii_string(src, endian, entry, cx)?),
            42113 => geo_tags.gdal_nodata = Some(read_ascii_string(src, endian, entry, cx)?),
            // メタデータ・文書情報タグは読み飛ばす（データを捨てていない）
            254 | 255 | 269 | 270 | 271 | 272 | 280 | 281 | 285 | 286 | 287 | 297 | 305 | 306
            | 315 | 318 | 319 | 340 | 341 | 33432 => {
                let _ = value_slice(src, endian, entry, cx)?;
            }
            other => {
                let _ = value_slice(src, endian, entry, cx)?;
                cx.report(
                    Diagnostic::new(codes::TIFF_TAG_UNSUPPORTED)
                        .with("tag", u64::from(other))
                        .at(source.clone()),
                );
            }
        }
    }

    let w = raw_width.filter(|&w| w > 0).ok_or_else(|| {
        let off = source.bytes().map_or(8, |r| r.start as usize);
        invalid(cx, off)
    })?;
    let h = raw_height.filter(|&h| h > 0).ok_or_else(|| {
        let off = source.bytes().map_or(8, |r| r.start as usize);
        invalid(cx, off)
    })?;

    if samples_per_pixel == 0 {
        let off = source.bytes().map_or(8, |r| r.start as usize);
        return Err(invalid(cx, off));
    }

    let bps = if bits_per_sample.is_empty() {
        1
    } else {
        *bits_per_sample.first().unwrap_or(&1)
    };

    let sample_format_raw = sample_formats.first().copied().unwrap_or(1);
    let sample_format = match sample_format_raw {
        2 => SampleFormat::Int,
        3 => SampleFormat::Float,
        _ => SampleFormat::UInt,
    };

    let photometric = match photometric_raw {
        Some(0) => Some(Photometric::WhiteIsZero),
        Some(1) | None => Some(Photometric::BlackIsZero),
        Some(2) => Some(Photometric::Rgb),
        Some(3) => Some(Photometric::Palette),
        Some(_) => None,
    };

    let planar_config = match planar_raw {
        1 => PlanarConfig::Chunky,
        2 => PlanarConfig::Planar,
        _ => {
            let off = source.bytes().map_or(8, |r| r.start as usize);
            return Err(invalid(cx, off));
        }
    };

    let resolution_unit = match resolution_unit_raw {
        1 => ResolutionUnit::None,
        3 => ResolutionUnit::Centimeter,
        _ => ResolutionUnit::Inch,
    };

    let (disp_w, disp_h, disp_xr, disp_yr) = if matches!(orientation, 5..=8) {
        (h, w, y_resolution, x_resolution)
    } else {
        (w, h, x_resolution, y_resolution)
    };

    let geo = geo_tags.parse(orientation, w, h, &source, cx);

    let info = PageInfo {
        width: disp_w,
        height: disp_h,
        samples_per_pixel,
        bits_per_sample: bps,
        compression,
        photometric,
        sample_format,
        planar_config,
        x_resolution: disp_xr,
        y_resolution: disp_yr,
        resolution_unit,
        geo,
        source: source.clone(),
    };

    let alpha = match extra_samples.first().copied() {
        Some(1) => AlphaKind::Associated,
        Some(_) => AlphaKind::Unassociated,
        None if photometric == Some(Photometric::Rgb) && samples_per_pixel == 4 => {
            AlphaKind::Unassociated
        }
        None if matches!(
            photometric,
            Some(Photometric::BlackIsZero | Photometric::WhiteIsZero)
        ) && samples_per_pixel == 2 =>
        {
            AlphaKind::Unassociated
        }
        None => AlphaKind::None,
    };

    // 形式・圧縮の対応検査
    if !matches!(compression, 1 | 4 | 5 | 8 | 32946 | 32773) {
        cx.report(
            Diagnostic::new(codes::TIFF_COMPRESSION_UNSUPPORTED)
                .with("compression", u64::from(compression))
                .at(source),
        );
        return Ok(DecodedIfd {
            info,
            orientation,
            raw_width: w,
            raw_height: h,
            alpha,
            color_map,
            data: PagePixels::Unsupported,
        });
    }

    if let Some(reason) = unsupported_format_reason(
        &bits_per_sample,
        bps,
        samples_per_pixel,
        photometric,
        &sample_formats,
        sample_format,
        predictor,
        fill_order,
        compression,
        t6_options,
        orientation,
        color_map.as_deref(),
    ) {
        cx.report(
            Diagnostic::new(codes::TIFF_FORMAT_UNSUPPORTED)
                .with("reason", reason)
                .at(source),
        );
        return Ok(DecodedIfd {
            info,
            orientation,
            raw_width: w,
            raw_height: h,
            alpha,
            color_map,
            data: PagePixels::Unsupported,
        });
    }

    let layout = if let (Some(tw), Some(tl), Some(toff), Some(tbc)) =
        (tile_width, tile_length, tile_offsets, tile_byte_counts)
    {
        DataLayout::Tiles {
            tile_w: tw,
            tile_h: tl,
            offsets: toff,
            byte_counts: tbc,
        }
    } else if let (Some(soff), Some(sbc)) = (strip_offsets, strip_byte_counts) {
        DataLayout::Strips {
            rows_per_strip: rows_per_strip.unwrap_or(h).max(1),
            offsets: soff,
            byte_counts: sbc,
        }
    } else {
        let off = source.bytes().map_or(8, |r| r.start as usize);
        return Err(invalid(cx, off));
    };

    let native = assemble_pixels(
        src,
        endian,
        w,
        h,
        bps,
        samples_per_pixel,
        planar_config,
        compression,
        predictor,
        fill_order,
        &layout,
        &source,
        cx,
    )?;

    Ok(DecodedIfd {
        info,
        orientation,
        raw_width: w,
        raw_height: h,
        alpha,
        color_map,
        data: PagePixels::Native(native),
    })
}

#[allow(clippy::too_many_arguments)]
fn unsupported_format_reason(
    bits_list: &[u16],
    bps: u16,
    spp: u16,
    photometric: Option<Photometric>,
    sample_formats: &[u16],
    sample_format: SampleFormat,
    predictor: u16,
    fill_order: u16,
    compression: u16,
    t6_options: u32,
    orientation: u16,
    color_map: Option<&[u16]>,
) -> Option<&'static str> {
    if photometric.is_none() {
        return Some("photometric");
    }
    if let Some(&first_sf) = sample_formats.first()
        && (!matches!(first_sf, 1..=3) || sample_formats.iter().any(|&sf| sf != first_sf))
    {
        return Some("sample-format");
    }
    if !bits_list.is_empty() && bits_list.iter().any(|&b| b != bps) {
        return Some("mixed-bits-per-sample");
    }
    if !matches!(fill_order, 1 | 2) {
        return Some("fill-order");
    }
    if !matches!(orientation, 1..=8) {
        return Some("orientation");
    }
    if !matches!(predictor, 1 | 2) {
        return Some("predictor");
    }
    if predictor == 2 && (!matches!(bps, 8 | 16) || sample_format == SampleFormat::Float) {
        return Some("predictor");
    }
    if compression == 4 {
        if bps != 1 || spp != 1 {
            return Some("group4-bits");
        }
        if (t6_options & 0b10) != 0 {
            return Some("group4-uncompressed");
        }
    }
    match photometric {
        Some(Photometric::WhiteIsZero | Photometric::BlackIsZero) => match sample_format {
            SampleFormat::UInt => {
                if spp == 1 && !matches!(bps, 1 | 4 | 8 | 16 | 32) {
                    return Some("bits-per-sample");
                }
                if spp == 2 && !matches!(bps, 8 | 16) {
                    return Some("bits-per-sample");
                }
                if spp > 2 {
                    return Some("samples-per-pixel");
                }
            }
            SampleFormat::Int => {
                if spp != 1 || !matches!(bps, 8 | 16 | 32) {
                    return Some("dem-sample-format");
                }
            }
            SampleFormat::Float => {
                if spp != 1 || !matches!(bps, 32 | 64) {
                    return Some("dem-sample-format");
                }
            }
        },
        Some(Photometric::Palette) => {
            if spp != 1 || !matches!(bps, 4 | 8) || sample_format != SampleFormat::UInt {
                return Some("palette-bits");
            }
            let needed = 3usize * (1usize << usize::from(bps));
            if color_map.is_none_or(|cm| cm.len() < needed) {
                return Some("palette-missing");
            }
        }
        Some(Photometric::Rgb) => {
            if !matches!(spp, 3 | 4)
                || !matches!(bps, 8 | 16)
                || sample_format != SampleFormat::UInt
            {
                return Some("rgb-format");
            }
        }
        None => return Some("photometric"),
    }
    None
}

enum DataLayout {
    Strips {
        rows_per_strip: u32,
        offsets: Vec<u64>,
        byte_counts: Vec<u64>,
    },
    Tiles {
        tile_w: u32,
        tile_h: u32,
        offsets: Vec<u64>,
        byte_counts: Vec<u64>,
    },
}

fn row_byte_len(width: u32, bps: u16, samples_per_row_pixel: u16) -> Option<usize> {
    let bits = u64::from(width)
        .checked_mul(u64::from(bps))?
        .checked_mul(u64::from(samples_per_row_pixel))?;
    let bytes = bits.div_ceil(8);
    if bytes > MAX_ALLOC {
        return None;
    }
    usize::try_from(bytes).ok()
}

#[allow(clippy::too_many_arguments)]
fn assemble_pixels(
    src: &[u8],
    endian: Endian,
    width: u32,
    height: u32,
    bps: u16,
    spp: u16,
    planar: PlanarConfig,
    compression: u16,
    predictor: u16,
    fill_order: u16,
    layout: &DataLayout,
    source: &Ref,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    let err_off = source.bytes().map_or(8, |r| r.start as usize);
    let chunky_row_bytes = row_byte_len(width, bps, spp).ok_or_else(|| invalid(cx, err_off))?;
    let total_bytes_u64 = (chunky_row_bytes as u64)
        .checked_mul(u64::from(height))
        .ok_or_else(|| invalid(cx, err_off))?;
    if total_bytes_u64 > MAX_ALLOC {
        return Err(invalid(cx, err_off));
    }

    match (layout, planar) {
        (
            DataLayout::Strips {
                rows_per_strip,
                offsets,
                byte_counts,
            },
            PlanarConfig::Chunky,
        ) => {
            let strips_needed = usize::try_from(height.div_ceil(*rows_per_strip))
                .map_err(|_| invalid(cx, err_off))?;
            if offsets.len() < strips_needed || byte_counts.len() < strips_needed {
                return Err(invalid(cx, err_off));
            }
            let mut out = Vec::with_capacity((total_bytes_u64 as usize).min(65536));
            for s_idx in 0..strips_needed {
                let start_row = u32::try_from(s_idx)
                    .ok()
                    .and_then(|i| i.checked_mul(*rows_per_strip))
                    .ok_or_else(|| invalid(cx, err_off))?;
                let rows = (height - start_row).min(*rows_per_strip);
                let expected = chunky_row_bytes
                    .checked_mul(usize::try_from(rows).unwrap_or(0))
                    .ok_or_else(|| invalid(cx, err_off))?;
                let raw_chunk =
                    slice_chunk(src, offsets.get(s_idx), byte_counts.get(s_idx), cx, err_off)?;
                let mut decoded = decode_and_predict(
                    raw_chunk,
                    compression,
                    fill_order,
                    predictor,
                    endian,
                    width,
                    rows,
                    bps,
                    spp,
                    expected,
                    source,
                    cx,
                )?;
                out.append(&mut decoded);
            }
            Ok(out)
        }
        (
            DataLayout::Strips {
                rows_per_strip,
                offsets,
                byte_counts,
            },
            PlanarConfig::Planar,
        ) => {
            if bps < 8 || !bps.is_multiple_of(8) {
                return Err(invalid(cx, err_off));
            }
            let sample_bytes = usize::from(bps / 8);
            let plane_row_bytes =
                row_byte_len(width, bps, 1).ok_or_else(|| invalid(cx, err_off))?;
            let strips_per_plane = usize::try_from(height.div_ceil(*rows_per_strip))
                .map_err(|_| invalid(cx, err_off))?;
            let total_strips = strips_per_plane
                .checked_mul(usize::from(spp))
                .ok_or_else(|| invalid(cx, err_off))?;
            if offsets.len() < total_strips || byte_counts.len() < total_strips {
                return Err(invalid(cx, err_off));
            }

            // まず全プレーンを展開し（予算を先に消費してから出力バッファーを確保する）、その後インターリーブする
            let mut planes: Vec<Vec<u8>> = Vec::with_capacity(usize::from(spp));
            for p in 0..usize::from(spp) {
                let mut plane_buf =
                    Vec::with_capacity((plane_row_bytes * height as usize).min(65536));
                for s_idx in 0..strips_per_plane {
                    let idx = p * strips_per_plane + s_idx;
                    let start_row = (s_idx as u32) * (*rows_per_strip);
                    let rows = (height - start_row).min(*rows_per_strip);
                    let expected = plane_row_bytes * (rows as usize);
                    let raw_chunk =
                        slice_chunk(src, offsets.get(idx), byte_counts.get(idx), cx, err_off)?;
                    let mut decoded = decode_and_predict(
                        raw_chunk,
                        compression,
                        fill_order,
                        predictor,
                        endian,
                        width,
                        rows,
                        bps,
                        1,
                        expected,
                        source,
                        cx,
                    )?;
                    plane_buf.append(&mut decoded);
                }
                planes.push(plane_buf);
            }

            let pixel_count = (width as usize) * (height as usize);
            cx.check(Kind::Work, pixel_count as u64)
                .map_err(|s| stopped(cx, s, Some(source.clone())))?;
            cx.check(Kind::DecodedBytes, total_bytes_u64)
                .map_err(|s| stopped(cx, s, Some(source.clone())))?;
            let mut out = vec![0u8; total_bytes_u64 as usize];
            let spp_usize = usize::from(spp);
            for px in 0..pixel_count {
                for (ch, plane) in planes.iter().enumerate() {
                    let src_pos = px * sample_bytes;
                    let dst_pos = (px * spp_usize + ch) * sample_bytes;
                    if let (Some(src_s), Some(dst_s)) = (
                        plane.get(src_pos..src_pos + sample_bytes),
                        out.get_mut(dst_pos..dst_pos + sample_bytes),
                    ) {
                        dst_s.copy_from_slice(src_s);
                    }
                }
            }
            Ok(out)
        }
        (
            DataLayout::Tiles {
                tile_w,
                tile_h,
                offsets,
                byte_counts,
            },
            planar_mode,
        ) => {
            if *tile_w == 0 || *tile_h == 0 {
                return Err(invalid(cx, err_off));
            }
            let tiles_across =
                usize::try_from(width.div_ceil(*tile_w)).map_err(|_| invalid(cx, err_off))?;
            let tiles_down =
                usize::try_from(height.div_ceil(*tile_h)).map_err(|_| invalid(cx, err_off))?;
            let tiles_per_plane = tiles_across
                .checked_mul(tiles_down)
                .ok_or_else(|| invalid(cx, err_off))?;
            let planes_count = match planar_mode {
                PlanarConfig::Chunky => 1usize,
                PlanarConfig::Planar => usize::from(spp),
            };
            let tile_spp = match planar_mode {
                PlanarConfig::Chunky => spp,
                PlanarConfig::Planar => 1,
            };
            let needed = tiles_per_plane
                .checked_mul(planes_count)
                .ok_or_else(|| invalid(cx, err_off))?;
            if offsets.len() < needed || byte_counts.len() < needed {
                return Err(invalid(cx, err_off));
            }
            if planar_mode == PlanarConfig::Planar && (bps < 8 || !bps.is_multiple_of(8)) {
                return Err(invalid(cx, err_off));
            }

            let tile_row_bytes =
                row_byte_len(*tile_w, bps, tile_spp).ok_or_else(|| invalid(cx, err_off))?;
            let tile_expected_u64 = (tile_row_bytes as u64)
                .checked_mul(u64::from(*tile_h))
                .ok_or_else(|| invalid(cx, err_off))?;
            if tile_expected_u64 > MAX_ALLOC {
                return Err(invalid(cx, err_off));
            }
            let tile_expected =
                usize::try_from(tile_expected_u64).map_err(|_| invalid(cx, err_off))?;

            // 先に最初のタイルを展開し、画像バッファー確保前にも DecodedBytes を検査する
            let mut out: Vec<u8> = Vec::new();
            for plane_idx in 0..planes_count {
                for ty in 0..tiles_down {
                    for tx in 0..tiles_across {
                        let idx = plane_idx * tiles_per_plane + ty * tiles_across + tx;
                        let raw_chunk =
                            slice_chunk(src, offsets.get(idx), byte_counts.get(idx), cx, err_off)?;
                        let tile_buf = decode_and_predict(
                            raw_chunk,
                            compression,
                            fill_order,
                            predictor,
                            endian,
                            *tile_w,
                            *tile_h,
                            bps,
                            tile_spp,
                            tile_expected,
                            source,
                            cx,
                        )?;
                        if out.is_empty() {
                            cx.check(Kind::DecodedBytes, total_bytes_u64)
                                .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                            out.resize(total_bytes_u64 as usize, 0);
                        }
                        cx.check(
                            Kind::Work,
                            u64::from(*tile_w)
                                .saturating_mul(u64::from(*tile_h))
                                .div_ceil(16)
                                .max(1),
                        )
                        .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                        copy_tile_into_image(
                            &tile_buf,
                            &mut out,
                            *tile_w,
                            *tile_h,
                            tile_row_bytes,
                            tx,
                            ty,
                            width,
                            height,
                            bps,
                            spp,
                            planar_mode,
                            plane_idx,
                            chunky_row_bytes,
                        );
                    }
                }
            }
            Ok(out)
        }
    }
}

fn slice_chunk<'a>(
    src: &'a [u8],
    offset: Option<&u64>,
    byte_count: Option<&u64>,
    cx: &mut Cx,
    err_off: usize,
) -> Result<&'a [u8], Error> {
    let off = usize::try_from(*offset.ok_or_else(|| invalid(cx, err_off))?)
        .map_err(|_| invalid(cx, err_off))?;
    let len = usize::try_from(*byte_count.ok_or_else(|| invalid(cx, err_off))?)
        .map_err(|_| invalid(cx, err_off))?;
    let end = off.checked_add(len).ok_or_else(|| invalid(cx, off))?;
    src.get(off..end).ok_or_else(|| invalid(cx, off))
}

#[allow(clippy::too_many_arguments)]
fn decode_and_predict(
    raw_chunk: &[u8],
    compression: u16,
    fill_order: u16,
    predictor: u16,
    endian: Endian,
    width: u32,
    rows: u32,
    bps: u16,
    spp: u16,
    expected_bytes: usize,
    source: &Ref,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    let err_off = source.bytes().map_or(8, |r| r.start as usize);
    let mut buf = decompress_chunk(
        raw_chunk,
        compression,
        fill_order,
        width,
        rows,
        bps,
        expected_bytes,
        cx,
    )
    .map_err(|f| match f {
        Fail::Invalid => invalid(cx, err_off),
        Fail::Stop(s) => stopped(cx, s, Some(source.clone())),
    })?;
    if predictor == 2 {
        apply_predictor_2(&mut buf, width, rows, bps, spp, endian, cx)
            .map_err(|s| stopped(cx, s, Some(source.clone())))?;
    }
    Ok(buf)
}

#[allow(clippy::too_many_arguments)]
fn copy_tile_into_image(
    tile_buf: &[u8],
    out: &mut [u8],
    tile_w: u32,
    tile_h: u32,
    tile_row_bytes: usize,
    tx: usize,
    ty: usize,
    img_w: u32,
    img_h: u32,
    bps: u16,
    spp: u16,
    planar: PlanarConfig,
    plane_idx: usize,
    chunky_row_bytes: usize,
) {
    let x0 = (tx as u32).saturating_mul(tile_w);
    let y0 = (ty as u32).saturating_mul(tile_h);
    if x0 >= img_w || y0 >= img_h {
        return;
    }
    let copy_w = (img_w - x0).min(tile_w);
    let copy_h = (img_h - y0).min(tile_h);

    match planar {
        PlanarConfig::Chunky => {
            let bits_per_px = usize::from(bps) * usize::from(spp);
            if bits_per_px.is_multiple_of(8) {
                let bytes_per_px = bits_per_px / 8;
                let row_copy_bytes = (copy_w as usize) * bytes_per_px;
                for r in 0..(copy_h as usize) {
                    let src_start = r * tile_row_bytes;
                    let dst_start =
                        (y0 as usize + r) * chunky_row_bytes + (x0 as usize) * bytes_per_px;
                    if let (Some(s), Some(d)) = (
                        tile_buf.get(src_start..src_start + row_copy_bytes),
                        out.get_mut(dst_start..dst_start + row_copy_bytes),
                    ) {
                        d.copy_from_slice(s);
                    }
                }
            } else {
                // 1-bit・4-bit タイル（tile_w は TIFF 6.0 により 16 の倍数なのでバイト境界に揃う）
                for r in 0..(copy_h as usize) {
                    let src_row = tile_buf
                        .get(r * tile_row_bytes..(r + 1) * tile_row_bytes)
                        .unwrap_or(&[]);
                    let dst_row = out
                        .get_mut(
                            (y0 as usize + r) * chunky_row_bytes
                                ..(y0 as usize + r + 1) * chunky_row_bytes,
                        )
                        .unwrap_or(&mut []);
                    for dx in 0..(copy_w as usize) {
                        let val = get_packed_sample(src_row, dx, bps);
                        set_packed_sample(dst_row, x0 as usize + dx, bps, val);
                    }
                }
            }
        }
        PlanarConfig::Planar => {
            let sample_bytes = usize::from(bps / 8);
            let spp_usize = usize::from(spp);
            for r in 0..(copy_h as usize) {
                for dx in 0..(copy_w as usize) {
                    let src_pos = r * tile_row_bytes + dx * sample_bytes;
                    let px_idx = (y0 as usize + r) * (img_w as usize) + (x0 as usize + dx);
                    let dst_pos = (px_idx * spp_usize + plane_idx) * sample_bytes;
                    if let (Some(s), Some(d)) = (
                        tile_buf.get(src_pos..src_pos + sample_bytes),
                        out.get_mut(dst_pos..dst_pos + sample_bytes),
                    ) {
                        d.copy_from_slice(s);
                    }
                }
            }
        }
    }
}

pub(crate) fn get_packed_sample(row: &[u8], x: usize, bps: u16) -> u8 {
    match bps {
        1 => {
            let b = row.get(x / 8).copied().unwrap_or(0);
            (b >> (7 - (x % 8))) & 1
        }
        4 => {
            let b = row.get(x / 2).copied().unwrap_or(0);
            if x.is_multiple_of(2) {
                (b >> 4) & 0x0F
            } else {
                b & 0x0F
            }
        }
        _ => 0,
    }
}

fn set_packed_sample(row: &mut [u8], x: usize, bps: u16, val: u8) {
    match bps {
        1 => {
            if let Some(slot) = row.get_mut(x / 8) {
                let shift = 7 - (x % 8);
                *slot = (*slot & !(1 << shift)) | ((val & 1) << shift);
            }
        }
        4 => {
            if let Some(slot) = row.get_mut(x / 2) {
                if x.is_multiple_of(2) {
                    *slot = (*slot & 0x0F) | ((val & 0x0F) << 4);
                } else {
                    *slot = (*slot & 0xF0) | (val & 0x0F);
                }
            }
        }
        _ => {}
    }
}
