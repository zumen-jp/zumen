//! 復号済みページの RGBA8 変換・縮小（エリア平均）・PNG 符号化と、DEM 標高値の読み取り。

use zumen_base::crc::crc32;
use zumen_base::{Diagnostic, Kind, codes};
use zumen_sdk::{Cx, Error};

use crate::ifd::{
    AlphaKind, DecodedIfd, Endian, PagePixels, Photometric, SampleFormat, get_packed_sample,
};
use crate::{MAX_ALLOC, MAX_PNG_PIXELS, TiffFile, invalid, stopped};

/// `raster` 操作が返す PNG 画像とページ寸法
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RasterPng {
    /// 出力した PNG の幅（ピクセル）
    pub width: u32,
    /// 出力した PNG の高さ（ピクセル）
    pub height: u32,
    /// 元のページの幅（ピクセル）
    pub source_width: u32,
    /// 元のページの高さ（ピクセル）
    pub source_height: u32,
    /// 1 始まりのページ番号
    pub page: usize,
    /// 総ページ数
    pub pages: usize,
    /// RGBA8・非インターレースの PNG バイト列
    pub bytes: Vec<u8>,
}

pub(crate) fn encode_page_png(
    file: &TiffFile,
    page_1based: usize,
    max_side: Option<u32>,
    cx: &mut Cx,
) -> Result<RasterPng, Error> {
    if page_1based == 0 {
        return Err(invalid(cx, 0));
    }
    let page = file
        .pages
        .get(page_1based - 1)
        .ok_or_else(|| invalid(cx, 0))?;
    let PagePixels::Native(native) = &page.data else {
        cx.report(
            Diagnostic::new(codes::TIFF_FORMAT_UNSUPPORTED)
                .with("reason", "page-unsupported")
                .at(page.info.source.clone()),
        );
        return Err(Error::Unsupported);
    };

    let src_w = page.info.width;
    let src_h = page.info.height;
    let (out_w, out_h) = target_dimensions(src_w, src_h, max_side);
    let out_pixels = u64::from(out_w).saturating_mul(u64::from(out_h));
    if out_w == 0 || out_h == 0 || out_pixels > MAX_PNG_PIXELS {
        cx.report(
            Diagnostic::new(codes::TIFF_FORMAT_UNSUPPORTED)
                .with("reason", "png-pixels-exceeded")
                .at(page.info.source.clone()),
        );
        return Err(Error::Unsupported);
    }

    let rgba_bytes = out_pixels.saturating_mul(4);
    if rgba_bytes > MAX_ALLOC {
        return Err(Error::Unsupported);
    }
    cx.check(Kind::DecodedBytes, rgba_bytes)
        .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;

    let dem_range = if is_scalar_elevation_grid(page) {
        Some(compute_dem_min_max(file.endian, page, native, cx)?)
    } else {
        None
    };

    let rgba = if out_w == src_w && out_h == src_h {
        render_full_rgba(file.endian, page, native, dem_range, cx)?
    } else {
        render_downsampled_rgba(file.endian, page, native, dem_range, out_w, out_h, cx)?
    };

    let bytes = encode_rgba_png(out_w, out_h, &rgba, &page.info.source, cx)?;
    Ok(RasterPng {
        width: out_w,
        height: out_h,
        source_width: src_w,
        source_height: src_h,
        page: page_1based,
        pages: file.pages.len(),
        bytes,
    })
}

fn target_dimensions(src_w: u32, src_h: u32, max_side: Option<u32>) -> (u32, u32) {
    let mut w = f64::from(src_w);
    let mut h = f64::from(src_h);

    if let Some(limit) = max_side.filter(|&m| m > 0) {
        let max_cur = src_w.max(src_h);
        if max_cur > limit {
            let scale = f64::from(limit) / f64::from(max_cur);
            w *= scale;
            h *= scale;
        }
    }

    // MAX_PNG_PIXELS（16,777,216 画素）も超えないよう保護する
    let pixels = w * h;
    if pixels > MAX_PNG_PIXELS as f64 {
        let scale = libm::sqrt((MAX_PNG_PIXELS as f64) / pixels);
        w *= scale;
        h *= scale;
    }

    let out_w = (libm::floor(w + 0.5) as u32).max(1).min(src_w);
    let out_h = (libm::floor(h + 0.5) as u32).max(1).min(src_h);
    (out_w, out_h)
}

fn is_scalar_elevation_grid(page: &DecodedIfd) -> bool {
    if page.info.samples_per_pixel != 1 || page.color_map.is_some() {
        return false;
    }
    match page.info.sample_format {
        SampleFormat::Float | SampleFormat::Int => true,
        SampleFormat::UInt => page.info.bits_per_sample == 32,
    }
}

/// 向き反映後の表示座標 `(disp_x, disp_y)` を格納上の `(raw_x, raw_y)` に戻す
pub(crate) fn map_display_to_raw(
    orientation: u16,
    raw_w: u32,
    raw_h: u32,
    disp_x: u32,
    disp_y: u32,
) -> (usize, usize) {
    let rw_last = raw_w.saturating_sub(1);
    let rh_last = raw_h.saturating_sub(1);
    let (rx, ry) = match orientation {
        2 => (rw_last.saturating_sub(disp_x), disp_y),
        3 => (
            rw_last.saturating_sub(disp_x),
            rh_last.saturating_sub(disp_y),
        ),
        4 => (disp_x, rh_last.saturating_sub(disp_y)),
        5 => (disp_y, disp_x),
        6 => (disp_y, rh_last.saturating_sub(disp_x)),
        7 => (
            rw_last.saturating_sub(disp_y),
            rh_last.saturating_sub(disp_x),
        ),
        8 => (rw_last.saturating_sub(disp_y), disp_x),
        _ => (disp_x, disp_y),
    };
    (rx as usize, ry as usize)
}

/// 単バンド DEM 格子の `(disp_x, disp_y)` の数値を `f64` で読む。欠損値・非有限値は `None`。
pub(crate) fn read_dem_value(
    endian: Endian,
    page: &DecodedIfd,
    native: &[u8],
    disp_x: u32,
    disp_y: u32,
) -> Option<f64> {
    let (rx, ry) = map_display_to_raw(
        page.orientation,
        page.raw_width,
        page.raw_height,
        disp_x,
        disp_y,
    );
    let idx = ry * (page.raw_width as usize) + rx;
    let bps = page.info.bits_per_sample;
    let val = match (page.info.sample_format, bps) {
        (SampleFormat::Float, 32) => f64::from(endian.read_f32(native, idx * 4)?),
        (SampleFormat::Float, 64) => endian.read_f64(native, idx * 8)?,
        (SampleFormat::Int, 8) => f64::from(*native.get(idx)? as i8),
        (SampleFormat::Int, 16) => f64::from(endian.read_i16(native, idx * 2)?),
        (SampleFormat::Int, 32) => f64::from(endian.read_i32(native, idx * 4)?),
        (SampleFormat::UInt, 8) => f64::from(*native.get(idx)?),
        (SampleFormat::UInt, 16) => f64::from(endian.read_u16(native, idx * 2)?),
        (SampleFormat::UInt, 32) => f64::from(endian.read_u32(native, idx * 4)?),
        _ => return None,
    };
    if !val.is_finite() {
        return None;
    }
    if let Some(nodata) = page.info.geo.as_ref().and_then(|g| g.nodata) {
        let tol = libm::fabs(nodata) * 1e-7 + 1e-9;
        if libm::fabs(val - nodata) <= tol {
            return None;
        }
    }
    Some(val)
}

fn compute_dem_min_max(
    endian: Endian,
    page: &DecodedIfd,
    native: &[u8],
    cx: &mut Cx,
) -> Result<(f64, f64), Error> {
    let mut min_v = f64::INFINITY;
    let mut max_v = f64::NEG_INFINITY;
    for y in 0..page.info.height {
        cx.check(Kind::Work, u64::from(page.info.width))
            .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;
        for x in 0..page.info.width {
            if let Some(v) = read_dem_value(endian, page, native, x, y) {
                if v < min_v {
                    min_v = v;
                }
                if v > max_v {
                    max_v = v;
                }
            }
        }
    }
    if !min_v.is_finite() || !max_v.is_finite() {
        Ok((0.0, 1.0))
    } else {
        Ok((min_v, max_v))
    }
}

fn render_full_rgba(
    endian: Endian,
    page: &DecodedIfd,
    native: &[u8],
    dem_range: Option<(f64, f64)>,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    let w = page.info.width;
    let h = page.info.height;
    let raw_row_bytes = (u64::from(page.raw_width)
        * u64::from(page.info.bits_per_sample)
        * u64::from(page.info.samples_per_pixel))
    .div_ceil(8) as usize;

    let mut out = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for dy in 0..h {
        cx.check(Kind::Work, u64::from(w))
            .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;
        for dx in 0..w {
            let px = sample_rgba(endian, page, native, raw_row_bytes, dem_range, dx, dy);
            out.extend_from_slice(&px);
        }
    }
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn render_downsampled_rgba(
    endian: Endian,
    page: &DecodedIfd,
    native: &[u8],
    dem_range: Option<(f64, f64)>,
    out_w: u32,
    out_h: u32,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    let src_w = page.info.width;
    let src_h = page.info.height;
    let raw_row_bytes = (u64::from(page.raw_width)
        * u64::from(page.info.bits_per_sample)
        * u64::from(page.info.samples_per_pixel))
    .div_ceil(8) as usize;

    let mut out = Vec::with_capacity((out_w as usize) * (out_h as usize) * 4);
    for oy in 0..out_h {
        let sy0 = ((u64::from(oy) * u64::from(src_h)) / u64::from(out_h)) as u32;
        let sy1 = (((u64::from(oy + 1) * u64::from(src_h)).div_ceil(u64::from(out_h))) as u32)
            .max(sy0 + 1)
            .min(src_h);
        for ox in 0..out_w {
            let sx0 = ((u64::from(ox) * u64::from(src_w)) / u64::from(out_w)) as u32;
            let sx1 = (((u64::from(ox + 1) * u64::from(src_w)).div_ceil(u64::from(out_w))) as u32)
                .max(sx0 + 1)
                .min(src_w);

            let count = u64::from(sx1 - sx0) * u64::from(sy1 - sy0);
            cx.check(Kind::Work, count.max(1))
                .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;

            let mut sum_r = 0u64;
            let mut sum_g = 0u64;
            let mut sum_b = 0u64;
            let mut sum_a = 0u64;
            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    let [r, g, b, a] =
                        sample_rgba(endian, page, native, raw_row_bytes, dem_range, sx, sy);
                    sum_r += u64::from(r);
                    sum_g += u64::from(g);
                    sum_b += u64::from(b);
                    sum_a += u64::from(a);
                }
            }
            let half = count / 2;
            out.push(((sum_r + half) / count) as u8);
            out.push(((sum_g + half) / count) as u8);
            out.push(((sum_b + half) / count) as u8);
            out.push(((sum_a + half) / count) as u8);
        }
    }
    Ok(out)
}

fn scale_u16_to_u8(v: u16) -> u8 {
    // (v * 255 + 32895) >> 16 は 0..=65535 全域で round(v * 255 / 65535) と一致する
    (((u32::from(v) * 255) + 32895) >> 16) as u8
}

fn unpremultiply(c: u8, a: u8) -> u8 {
    if a == 0 {
        0
    } else if a == 255 {
        c
    } else {
        ((u32::from(c) * 255 + u32::from(a / 2)) / u32::from(a)).min(255) as u8
    }
}

fn sample_rgba(
    endian: Endian,
    page: &DecodedIfd,
    native: &[u8],
    raw_row_bytes: usize,
    dem_range: Option<(f64, f64)>,
    disp_x: u32,
    disp_y: u32,
) -> [u8; 4] {
    if let Some((min_v, max_v)) = dem_range {
        let Some(val) = read_dem_value(endian, page, native, disp_x, disp_y) else {
            return [0, 0, 0, 0];
        };
        let span = max_v - min_v;
        let gray = if span <= 1e-12 {
            128u8
        } else {
            let t = ((val - min_v) / span).clamp(0.0, 1.0);
            libm::floor(t * 255.0 + 0.5) as u8
        };
        return [gray, gray, gray, 255];
    }

    let (rx, ry) = map_display_to_raw(
        page.orientation,
        page.raw_width,
        page.raw_height,
        disp_x,
        disp_y,
    );
    let row = native
        .get(ry * raw_row_bytes..(ry + 1) * raw_row_bytes)
        .unwrap_or(&[]);
    let bps = page.info.bits_per_sample;
    let spp = page.info.samples_per_pixel;
    let invert = page.info.photometric == Some(Photometric::WhiteIsZero);

    match page.info.photometric {
        Some(Photometric::WhiteIsZero | Photometric::BlackIsZero) => match (spp, bps) {
            (1, 1) => {
                let bit = get_packed_sample(row, rx, 1);
                let g = match (invert, bit) {
                    (true, 0) | (false, 1) => 255,
                    _ => 0,
                };
                [g, g, g, 255]
            }
            (1, 4) => {
                let v4 = get_packed_sample(row, rx, 4);
                let mut g = v4 * 17;
                if invert {
                    g = 255 - g;
                }
                [g, g, g, 255]
            }
            (1, 8) => {
                let mut g = row.get(rx).copied().unwrap_or(0);
                if invert {
                    g = 255 - g;
                }
                [g, g, g, 255]
            }
            (1, 16) => {
                let v16 = endian.read_u16(row, rx * 2).unwrap_or(0);
                let mut g = scale_u16_to_u8(v16);
                if invert {
                    g = 255 - g;
                }
                [g, g, g, 255]
            }
            (2, 8) => {
                let mut g = row.get(rx * 2).copied().unwrap_or(0);
                let a = row.get(rx * 2 + 1).copied().unwrap_or(255);
                if invert {
                    g = 255 - g;
                }
                if page.alpha == AlphaKind::Associated {
                    g = unpremultiply(g, a);
                }
                [g, g, g, a]
            }
            (2, 16) => {
                let mut g = scale_u16_to_u8(endian.read_u16(row, rx * 4).unwrap_or(0));
                let a = scale_u16_to_u8(endian.read_u16(row, rx * 4 + 2).unwrap_or(65535));
                if invert {
                    g = 255 - g;
                }
                if page.alpha == AlphaKind::Associated {
                    g = unpremultiply(g, a);
                }
                [g, g, g, a]
            }
            _ => [0, 0, 0, 255],
        },
        Some(Photometric::Palette) => {
            let idx = match bps {
                4 => usize::from(get_packed_sample(row, rx, 4)),
                8 => usize::from(row.get(rx).copied().unwrap_or(0)),
                _ => 0,
            };
            let n = 1usize << usize::from(bps);
            if let Some(cm) = &page.color_map {
                // TIFF 6.0 Section 5: 16-bit ColorMap 値（v << 8 または v * 257）は上位 8 bit を取る
                let r = (cm.get(idx).copied().unwrap_or(0) >> 8) as u8;
                let g = (cm.get(n + idx).copied().unwrap_or(0) >> 8) as u8;
                let b = (cm.get(2 * n + idx).copied().unwrap_or(0) >> 8) as u8;
                [r, g, b, 255]
            } else {
                [0, 0, 0, 255]
            }
        }
        Some(Photometric::Rgb) => match (spp, bps) {
            (3, 8) => {
                let base = rx * 3;
                [
                    row.get(base).copied().unwrap_or(0),
                    row.get(base + 1).copied().unwrap_or(0),
                    row.get(base + 2).copied().unwrap_or(0),
                    255,
                ]
            }
            (4, 8) => {
                let base = rx * 4;
                let mut r = row.get(base).copied().unwrap_or(0);
                let mut g = row.get(base + 1).copied().unwrap_or(0);
                let mut b = row.get(base + 2).copied().unwrap_or(0);
                let a = if page.alpha == AlphaKind::None {
                    255
                } else {
                    row.get(base + 3).copied().unwrap_or(255)
                };
                if page.alpha == AlphaKind::Associated {
                    r = unpremultiply(r, a);
                    g = unpremultiply(g, a);
                    b = unpremultiply(b, a);
                }
                [r, g, b, a]
            }
            (3, 16) => {
                let base = rx * 6;
                [
                    scale_u16_to_u8(endian.read_u16(row, base).unwrap_or(0)),
                    scale_u16_to_u8(endian.read_u16(row, base + 2).unwrap_or(0)),
                    scale_u16_to_u8(endian.read_u16(row, base + 4).unwrap_or(0)),
                    255,
                ]
            }
            (4, 16) => {
                let base = rx * 8;
                let mut r = scale_u16_to_u8(endian.read_u16(row, base).unwrap_or(0));
                let mut g = scale_u16_to_u8(endian.read_u16(row, base + 2).unwrap_or(0));
                let mut b = scale_u16_to_u8(endian.read_u16(row, base + 4).unwrap_or(0));
                let a = if page.alpha == AlphaKind::None {
                    255
                } else {
                    scale_u16_to_u8(endian.read_u16(row, base + 6).unwrap_or(65535))
                };
                if page.alpha == AlphaKind::Associated {
                    r = unpremultiply(r, a);
                    g = unpremultiply(g, a);
                    b = unpremultiply(b, a);
                }
                [r, g, b, a]
            }
            _ => [0, 0, 0, 255],
        },
        None => [0, 0, 0, 255],
    }
}

const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
const STORED_BLOCK_MAX: usize = 65535;
const ADLER_MOD: u32 = 65521;

fn encode_rgba_png(
    width: u32,
    height: u32,
    rgba: &[u8],
    source: &zumen_base::Ref,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    let row_bytes = (width as usize) * 4;
    let raw_len = (height as usize) * (1 + row_bytes);
    let blocks = raw_len.div_ceil(STORED_BLOCK_MAX).max(1);
    let idat_len = 2 + raw_len + blocks * 5 + 4;
    let total_len = 8 + (12 + 13) + (12 + 1) + (12 + 4) + (12 + idat_len) + 12;

    cx.check(Kind::DecodedBytes, total_len as u64)
        .map_err(|s| stopped(cx, s, Some(source.clone())))?;
    cx.check(Kind::Work, height as u64)
        .map_err(|s| stopped(cx, s, Some(source.clone())))?;

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&PNG_SIGNATURE);

    let mut ihdr = [0u8; 13];
    if let Some(s) = ihdr.get_mut(0..4) {
        s.copy_from_slice(&width.to_be_bytes());
    }
    if let Some(s) = ihdr.get_mut(4..8) {
        s.copy_from_slice(&height.to_be_bytes());
    }
    if let Some(s) = ihdr.get_mut(8..13) {
        s.copy_from_slice(&[8, 6, 0, 0, 0]);
    }
    write_png_chunk(&mut out, *b"IHDR", &ihdr);
    write_png_chunk(&mut out, *b"sRGB", &[0]);
    write_png_chunk(&mut out, *b"gAMA", &45455u32.to_be_bytes());

    let mut idat = Vec::with_capacity(idat_len);
    idat.extend_from_slice(&[0x78, 0x01]);
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;

    // フィルター 0 を先頭に付けた行ストリームを 65535 バイト単位の stored block に分割する
    let mut block_buf = Vec::with_capacity(STORED_BLOCK_MAX);
    let mut emitted = 0usize;
    for y in 0..(height as usize) {
        let row = rgba.get(y * row_bytes..(y + 1) * row_bytes).unwrap_or(&[]);
        for &byte in core::iter::once(&0u8).chain(row.iter()) {
            block_buf.push(byte);
            emitted += 1;
            if block_buf.len() == STORED_BLOCK_MAX || emitted == raw_len {
                let final_flag = u8::from(emitted == raw_len);
                let len = block_buf.len() as u16;
                idat.push(final_flag);
                idat.extend_from_slice(&len.to_le_bytes());
                idat.extend_from_slice(&(!len).to_le_bytes());
                idat.extend_from_slice(&block_buf);
                update_adler(&mut s1, &mut s2, &block_buf);
                block_buf.clear();
            }
        }
    }

    let adler = (s2 << 16) | s1;
    idat.extend_from_slice(&adler.to_be_bytes());
    write_png_chunk(&mut out, *b"IDAT", &idat);
    write_png_chunk(&mut out, *b"IEND", &[]);
    Ok(out)
}

fn update_adler(s1: &mut u32, s2: &mut u32, data: &[u8]) {
    for part in data.chunks(5552) {
        for &b in part {
            *s1 += u32::from(b);
            *s2 += *s1;
        }
        *s1 %= ADLER_MOD;
        *s2 %= ADLER_MOD;
    }
}

fn write_png_chunk(out: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(&kind);
    out.extend_from_slice(data);
    let crc = if let Some(slice) = out.get(start..) {
        crc32(slice)
    } else {
        0
    };
    out.extend_from_slice(&crc.to_be_bytes());
}
