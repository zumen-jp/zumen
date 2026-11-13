//! LAZ 1.0〜1.4（OGC 24-070r1）の展開器。
//!
//! 外部 crate に頼らず、`Cx` の予算（`DecodedBytes`・`Records`・`Work`）と算術復号器の不変条件を検査しながら
//! 点のレコードを展開する（#510）。

mod coder;
mod v1v2;
mod v3v4;

use zumen_base::{Diagnostic, Kind, codes};
use zumen_sdk::{Cx, Error};

use crate::{Header, Vlr};
use coder::{IntDecoder, LazResult, RangeDecoder, SliceCursor};

/// LASzip VLR の利用者 ID。
pub(crate) const LASZIP_USER_ID: &str = "laszip encoded";
/// LASzip VLR のレコード ID。
pub(crate) const LASZIP_RECORD_ID: u16 = 22204;

const VARIABLE_CHUNK_SIZE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Compressor {
    PointWise,
    PointWiseChunked,
    LayeredChunked,
}

struct LazVlr {
    compressor: Compressor,
    chunk_size: u32,
    items: Vec<(u16, u16, u16)>,
}

impl LazVlr {
    fn parse(bytes: &[u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        let mut cur = SliceCursor::new(bytes);
        let compressor_raw = cur.u16_le().map_err(|_| Error::Invalid)?;
        let compressor = match compressor_raw {
            1 => Compressor::PointWise,
            2 => Compressor::PointWiseChunked,
            3 => Compressor::LayeredChunked,
            _ => return Err(Error::Invalid),
        };
        let coder = cur.u16_le().map_err(|_| Error::Invalid)?;
        if coder != 0 {
            return Err(Error::Invalid);
        }
        // version (4 bytes) + options (4 bytes)
        let _ = cur.take(8).map_err(|_| Error::Invalid)?;
        let chunk_size = match cur.u32_le().map_err(|_| Error::Invalid)? {
            0 => VARIABLE_CHUNK_SIZE,
            v => v,
        };
        // number_of_special_evlrs (8) + offset_to_special_evlrs (8)
        let _ = cur.take(16).map_err(|_| Error::Invalid)?;
        let num_items = cur.u16_le().map_err(|_| Error::Invalid)?;
        if num_items == 0 {
            return Err(Error::Invalid);
        }
        let mut items = Vec::new();
        for _ in 0..num_items {
            cx.check(Kind::Records, 1)?;
            let item_type = cur.u16_le().map_err(|_| Error::Invalid)?;
            let size = cur.u16_le().map_err(|_| Error::Invalid)?;
            let version = cur.u16_le().map_err(|_| Error::Invalid)?;
            if size == 0 {
                return Err(Error::Invalid);
            }
            items.push((item_type, size, version));
        }

        let first_ver = items.first().map(|it| it.2).unwrap_or(0);
        match (compressor, first_ver) {
            (Compressor::PointWise | Compressor::PointWiseChunked, 1 | 2) => {
                if items.iter().any(|it| it.2 != 1 && it.2 != 2) {
                    return Err(Error::Invalid);
                }
            }
            (Compressor::LayeredChunked, 3 | 4) => {
                if items.iter().any(|it| it.2 != 3 && it.2 != 4) {
                    return Err(Error::Invalid);
                }
            }
            _ => return Err(Error::Invalid),
        }

        Ok(Self {
            compressor,
            chunk_size,
            items,
        })
    }

    fn items_size(&self) -> u64 {
        self.items.iter().map(|it| u64::from(it.1)).sum()
    }

    fn variable_chunks(&self) -> bool {
        self.chunk_size == VARIABLE_CHUNK_SIZE
    }
}

/// 可変長チャンクの点数一覧をチャンク表から読む。
/// `number_of_chunks` を信じて一括確保せず、1 件ずつ `Kind::Records` を消費する。
fn read_variable_chunk_point_counts(
    compressed: &[u8],
    point_offset: u64,
    cx: &mut Cx<'_>,
) -> Result<Vec<usize>, Error> {
    let read_table_offset = |slice: &[u8]| -> LazResult<i64> {
        let mut c = SliceCursor::new(slice);
        c.i64_le()
    };
    let mut table_file_pos = read_table_offset(compressed).map_err(|_| Error::Invalid)?;
    let base_i64 = i64::try_from(point_offset).map_err(|_| Error::Invalid)?;
    if table_file_pos <= base_i64 {
        let tail_start = compressed.len().checked_sub(8).ok_or(Error::Invalid)?;
        let tail = compressed.get(tail_start..).ok_or(Error::Invalid)?;
        table_file_pos = read_table_offset(tail).map_err(|_| Error::Invalid)?;
        if table_file_pos <= base_i64 {
            return Err(Error::Invalid);
        }
    }
    let rel = usize::try_from(table_file_pos - base_i64).map_err(|_| Error::Invalid)?;
    let table_bytes = compressed.get(rel..).ok_or(Error::Invalid)?;
    let mut cur = SliceCursor::new(table_bytes);
    let version = cur.u32_le().map_err(|_| Error::Invalid)?;
    if version != 0 {
        return Err(Error::Invalid);
    }
    let num_chunks = cur.u32_le().map_err(|_| Error::Invalid)?;
    if num_chunks == 0 {
        return Ok(Vec::new());
    }

    let mut ic = IntDecoder::new(32, 2);
    let mut dec = RangeDecoder::from_cursor(cur).map_err(|_| Error::Invalid)?;
    let mut out = Vec::new();
    let mut prev_pts = 0i32;
    let mut prev_bytes = 0i32;
    for _ in 0..num_chunks {
        cx.check(Kind::Records, 1)?;
        let pts = ic
            .decode(&mut dec, prev_pts, 0)
            .map_err(|_| Error::Invalid)?;
        let bts = ic
            .decode(&mut dec, prev_bytes, 1)
            .map_err(|_| Error::Invalid)?;
        prev_pts = pts;
        prev_bytes = bts;
        let pts_usize = usize::try_from(pts as u32).map_err(|_| Error::Invalid)?;
        out.push(pts_usize);
    }
    Ok(out)
}

fn report_invalid(cx: &mut Cx<'_>, reason: &'static str) -> Error {
    cx.report(Diagnostic::new(codes::LAS_LAZ_INVALID).with("reason", reason));
    Error::Invalid
}

/// LAZ の点データを展開する。展開後のバイト数は `DecodedBytes` で数える。
pub(crate) fn decompress(
    header: &Header,
    vlrs: &[Vlr],
    compressed: &[u8],
    cx: &mut Cx<'_>,
) -> Result<Vec<u8>, Error> {
    let Some(record) = vlrs
        .iter()
        .find(|vlr| vlr.user == LASZIP_USER_ID && vlr.record == LASZIP_RECORD_ID)
    else {
        return Err(report_invalid(cx, "laszip-vlr-missing"));
    };

    let vlr = match LazVlr::parse(&record.data, cx) {
        Ok(v) => v,
        Err(Error::Invalid) => return Err(report_invalid(cx, "laszip-vlr-invalid")),
        Err(other) => return Err(other),
    };

    if vlr.items_size() != u64::from(header.record_length) {
        return Err(report_invalid(cx, "record-length"));
    }

    let total_bytes = header
        .point_count
        .saturating_mul(u64::from(header.record_length));
    cx.check(Kind::DecodedBytes, total_bytes)?;
    let Ok(total_size) = usize::try_from(total_bytes) else {
        return Err(report_invalid(cx, "too-large"));
    };
    let Ok(total_points) = usize::try_from(header.point_count) else {
        return Err(report_invalid(cx, "too-large"));
    };
    let record_size = usize::from(header.record_length);
    if total_points == 0 {
        return Ok(Vec::new());
    }

    let mut out = vec![0u8; total_size];
    let run = decompress_inner(
        header,
        &vlr,
        compressed,
        total_points,
        record_size,
        &mut out,
        cx,
    );
    match run {
        Ok(()) => Ok(out),
        Err(Error::Invalid) => Err(report_invalid(cx, "decompress")),
        Err(other) => Err(other),
    }
}

fn decompress_inner(
    header: &Header,
    vlr: &LazVlr,
    compressed: &[u8],
    total_points: usize,
    record_size: usize,
    out: &mut [u8],
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    match vlr.compressor {
        Compressor::PointWise => {
            let cursor = SliceCursor::new(compressed);
            let _ = v1v2::decompress_sequential_chunk(&vlr.items, cursor, record_size, out, cx)?;
            Ok(())
        }
        Compressor::PointWiseChunked => {
            if vlr.variable_chunks() {
                let chunk_counts = read_variable_chunk_point_counts(
                    compressed,
                    u64::from(header.point_offset),
                    cx,
                )?;
                let mut cursor = SliceCursor::new(compressed);
                let _ = cursor.i64_le().map_err(|_| Error::Invalid)?;
                let mut done = 0usize;
                let mut chunk_iter = chunk_counts.into_iter();
                while done < total_points {
                    let count = chunk_iter.next().ok_or(Error::Invalid)?;
                    if count == 0 {
                        return Err(Error::Invalid);
                    }
                    let n = count.min(total_points - done);
                    let dst = out
                        .get_mut(done * record_size..(done + n) * record_size)
                        .ok_or(Error::Invalid)?;
                    cursor = v1v2::decompress_sequential_chunk(
                        &vlr.items,
                        cursor,
                        record_size,
                        dst,
                        cx,
                    )?;
                    done += n;
                }
                Ok(())
            } else {
                let chunk_cap = usize::try_from(vlr.chunk_size).map_err(|_| Error::Invalid)?;
                if chunk_cap == 0 {
                    return Err(Error::Invalid);
                }
                let mut cursor = SliceCursor::new(compressed);
                let _ = cursor.i64_le().map_err(|_| Error::Invalid)?;
                let mut done = 0usize;
                while done < total_points {
                    let n = chunk_cap.min(total_points - done);
                    let dst = out
                        .get_mut(done * record_size..(done + n) * record_size)
                        .ok_or(Error::Invalid)?;
                    cursor = v1v2::decompress_sequential_chunk(
                        &vlr.items,
                        cursor,
                        record_size,
                        dst,
                        cx,
                    )?;
                    done += n;
                }
                Ok(())
            }
        }
        Compressor::LayeredChunked => {
            let mut cursor = SliceCursor::new(compressed);
            let _ = cursor.i64_le().map_err(|_| Error::Invalid)?;
            let mut done = 0usize;
            while done < total_points {
                let rem = total_points - done;
                let max_chunk = if vlr.variable_chunks() {
                    rem
                } else {
                    let cap = usize::try_from(vlr.chunk_size).map_err(|_| Error::Invalid)?;
                    cap.min(rem)
                };
                let dst = out.get_mut(done * record_size..).ok_or(Error::Invalid)?;
                let (next_cur, n) = v3v4::decompress_layered_chunk(
                    &vlr.items,
                    cursor,
                    record_size,
                    max_chunk,
                    dst,
                    cx,
                )?;
                if n == 0 {
                    return Err(Error::Invalid);
                }
                cursor = next_cur;
                done += n;
            }
            Ok(())
        }
    }
}
