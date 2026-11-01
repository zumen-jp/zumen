//! LAS 1.0〜1.4 と LAZ（台帳の `las_1_4_r15`・`laz_1_4`）の点群を読み、点群の領域モデル（決定 0232）に移す。
//!
//! L1 は、見出し・可変長のレコード（VLR・EVLR）と、点のレコードのバイト列で持つ。LAZ は、読むときに点のレコードへ
//! 展開する。入力は範囲で読める（[`Ranged`]）。見出しを読んでから、VLR・点・EVLR の範囲を求める。

mod lift;

use std::io::{Read, Seek, SeekFrom};

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, codes};
use zumen_pointcloud::PointCloud;
use zumen_sdk::ranged::{Missing, Ranged, Source};
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};

/// LAS・LAZ のコーデック
pub struct Las;

/// 読んだ見出し
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    /// 版（大・小）
    pub version: (u8, u8),
    /// 全体の符号化の印。4 ビット目が立てば、座標系を WKT で書く
    pub global_encoding: u16,
    /// 見出しの大きさ
    pub header_size: u16,
    /// 点のレコードの始まり
    pub point_offset: u32,
    /// VLR の数
    pub vlr_count: u32,
    /// 点の形式（0〜10）
    pub point_format: u8,
    /// LAZ で圧縮してある
    pub compressed: bool,
    /// 点のレコードの長さ
    pub record_length: u16,
    /// 点の数
    pub point_count: u64,
    /// 倍率（X・Y・Z）
    pub scale: [f64; 3],
    /// ずれ（X・Y・Z）
    pub offset: [f64; 3],
    /// 範囲（X・Y・Z の最小と最大）
    pub bounds: [[f64; 3]; 2],
    /// 最初の EVLR の位置。なければ 0
    pub evlr_start: u64,
    /// EVLR の数
    pub evlr_count: u32,
}

impl Header {
    /// 座標系を WKT で書く
    pub fn wkt(&self) -> bool {
        self.global_encoding & 0x10 != 0
    }
}

/// 可変長のレコード（VLR・EVLR）
#[derive(Clone, Debug, PartialEq)]
pub struct Vlr {
    /// 利用者の ID（`LASF_Projection` など）
    pub user: String,
    /// レコードの ID
    pub record: u16,
    /// 説明
    pub description: String,
    /// 中身
    pub data: Vec<u8>,
    /// EVLR（1.4 の、長さが 64 ビットのもの）
    pub extended: bool,
    /// 元のバイト範囲
    pub span: ByteRange,
}

/// 読んだ LAS・LAZ。LAZ の点は展開して持つ
#[derive(Clone, Debug, PartialEq)]
pub struct LasFile {
    /// 見出し
    pub header: Header,
    /// VLR と EVLR。ファイルの順
    pub vlrs: Vec<Vlr>,
    /// 点のレコード。長さは、点の数とレコードの長さの積
    pub points: Vec<u8>,
}

impl LasFile {
    /// 利用者の ID とレコードの ID で探す
    pub fn vlr(&self, user: &str, record: u16) -> Option<&Vlr> {
        self.vlrs
            .iter()
            .find(|vlr| vlr.user == user && vlr.record == record)
    }
}

impl FileModel for LasFile {
    fn record_count(&self) -> usize {
        self.vlrs.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let vlr = self.vlrs.get(index)?;
        Some(RecordInfo {
            kind: if vlr.extended { b"EVLR" } else { b"VLR" },
            bytes: Some(vlr.span),
        })
    }
}

/// 1.0〜1.2 の見出しの大きさ
const HEADER_1_0: u16 = 227;
/// 1.4 の見出しの大きさ
const HEADER_1_4: u16 = 375;
/// VLR の見出しの大きさ
const VLR_HEADER: u64 = 54;
/// EVLR の見出しの大きさ
const EVLR_HEADER: u64 = 60;

/// 点の形式ごとの、レコードの最小の長さ
pub fn base_length(format: u8) -> Option<u16> {
    [20, 28, 26, 34, 57, 63, 30, 36, 38, 59, 67]
        .get(usize::from(format))
        .copied()
}

impl Codec for Las {
    type File = LasFile;

    const FORMAT: &'static str = "las";

    const SIGNATURES: &'static [Signature] = &[Signature::Bytes {
        offset: 0,
        bytes: b"LASF",
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<LasFile, Error> {
        let parts = [(0, src)];
        let source = Source {
            size: src.len() as u64,
            parts: &parts,
        };
        match Las::read_ranged(&source, cx) {
            Ok(file) => Ok(file),
            Err(Missing::Failed(error)) => Err(error),
            // 全体を渡したので、足りない範囲は入力が短いことを示す
            Err(Missing::Need(_)) => {
                cx.report(Diagnostic::new(codes::LAS_FILE_INVALID));
                Err(Error::Invalid)
            }
        }
    }

    /// 書き出しはまだしない。範囲で読んだ入力は、元のバイト列の全体を持たない
    fn write(_file: &LasFile, _out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

impl Ranged for Las {
    type File = LasFile;

    const FORMAT: &'static str = "las";

    const SIGNATURES: &'static [Signature] = <Las as Codec>::SIGNATURES;

    fn read_ranged(src: &Source<'_>, cx: &mut Cx) -> Result<LasFile, Missing> {
        let head = src
            .get(0, u64::from(HEADER_1_4))
            .map_err(|range| Missing::Need(vec![range]))?;
        let Some(header) = header(head) else {
            cx.report(Diagnostic::new(codes::LAS_FILE_INVALID));
            return Err(Error::Invalid.into());
        };
        let Some(base) = base_length(header.point_format) else {
            cx.report(
                Diagnostic::new(codes::LAS_POINT_FORMAT_UNSUPPORTED)
                    .with("format", u64::from(header.point_format)),
            );
            return Err(Error::Unsupported.into());
        };
        if header.record_length < base {
            cx.report(Diagnostic::new(codes::LAS_FILE_INVALID));
            return Err(Error::Invalid.into());
        }
        let point_start = u64::from(header.point_offset);
        let has_evlrs = header.evlr_count > 0 && header.evlr_start >= point_start;
        let point_end = if header.compressed {
            if has_evlrs {
                header.evlr_start
            } else {
                src.size
            }
        } else {
            header
                .point_count
                .saturating_mul(u64::from(header.record_length))
                .saturating_add(point_start)
        };
        let mut wanted = vec![
            (u64::from(header.header_size), point_start),
            (point_start, point_end.min(src.size)),
        ];
        if has_evlrs {
            wanted.push((header.evlr_start, src.size));
        }
        let mut found = Vec::new();
        let mut needs = Vec::new();
        for (start, end) in wanted {
            match src.get(start, end.max(start)) {
                Ok(bytes) => found.push(bytes),
                Err(range) => needs.push(range),
            }
        }
        if !needs.is_empty() {
            return Err(Missing::Need(needs));
        }
        let [vlr_bytes, point_bytes, evlr_bytes @ ..] = found.as_slice() else {
            return Err(Error::Invalid.into());
        };
        let read = found.iter().map(|bytes| bytes.len() as u64).sum::<u64>();
        cx.check(
            Kind::InputBytes,
            read.saturating_add(u64::from(header.header_size)),
        )?;
        let mut vlrs = records(
            vlr_bytes,
            u64::from(header.header_size),
            header.vlr_count,
            false,
            cx,
        )?;
        if let Some(bytes) = evlr_bytes.first() {
            vlrs.extend(records(
                bytes,
                header.evlr_start,
                header.evlr_count,
                true,
                cx,
            )?);
        }
        let points = if header.compressed {
            decompress(&header, &vlrs, point_bytes, cx)?
        } else {
            let length = u64::from(header.record_length);
            let whole = point_bytes.len() as u64 / length * length;
            if whole < header.point_count.saturating_mul(length) {
                cx.report(
                    Diagnostic::new(codes::LAS_POINTS_INVALID)
                        .with("expected", header.point_count)
                        .with("found", whole / length)
                        .at(Ref::at_bytes(
                            point_start,
                            point_start + point_bytes.len() as u64,
                        )),
                );
            }
            point_bytes
                .get(..usize::try_from(whole).unwrap_or(0))
                .unwrap_or_default()
                .to_vec()
        };
        Ok(LasFile {
            header,
            vlrs,
            points,
        })
    }
}

impl Lift<PointCloud> for LasFile {
    fn lift(&self, cx: &mut Cx) -> Result<PointCloud, Error> {
        lift::lift(self, cx)
    }
}

/// 小さな読み手。足りなければ `None`
fn at<const N: usize>(bytes: &[u8], offset: usize) -> Option<[u8; N]> {
    bytes.get(offset..offset.checked_add(N)?)?.try_into().ok()
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    at(bytes, offset).map(u16::from_le_bytes)
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    at(bytes, offset).map(u32::from_le_bytes)
}

fn u64_at(bytes: &[u8], offset: usize) -> Option<u64> {
    at(bytes, offset).map(u64::from_le_bytes)
}

fn f64_at(bytes: &[u8], offset: usize) -> Option<f64> {
    at(bytes, offset).map(f64::from_le_bytes)
}

/// 0 で終わる ASCII の欄
fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(bytes.get(..end).unwrap_or_default()).into_owned()
}

/// 見出しを読む。LAS でなければ `None`
fn header(bytes: &[u8]) -> Option<Header> {
    if bytes.get(..4)? != b"LASF" {
        return None;
    }
    let version = (*bytes.get(24)?, *bytes.get(25)?);
    let header_size = u16_at(bytes, 94)?;
    if version.0 != 1 || header_size < HEADER_1_0 {
        return None;
    }
    let id = *bytes.get(104)?;
    let f = |offset| f64_at(bytes, offset);
    let mut header = Header {
        version,
        global_encoding: u16_at(bytes, 6)?,
        header_size,
        point_offset: u32_at(bytes, 96)?,
        vlr_count: u32_at(bytes, 100)?,
        // 上の 2 ビットは LAZ の印
        point_format: id & 0x3f,
        compressed: id & 0xc0 != 0,
        record_length: u16_at(bytes, 105)?,
        point_count: u64::from(u32_at(bytes, 107)?),
        scale: [f(131)?, f(139)?, f(147)?],
        offset: [f(155)?, f(163)?, f(171)?],
        bounds: [[f(187)?, f(203)?, f(219)?], [f(179)?, f(195)?, f(211)?]],
        evlr_start: 0,
        evlr_count: 0,
    };
    if version.1 >= 4 && header_size >= HEADER_1_4 {
        header.evlr_start = u64_at(bytes, 235)?;
        header.evlr_count = u32_at(bytes, 243)?;
        let count = u64_at(bytes, 247)?;
        if count > 0 || header.point_count == 0 {
            header.point_count = count;
        }
    }
    if u32::from(header_size) > header.point_offset {
        return None;
    }
    Some(header)
}

/// VLR か EVLR を `count` 個読む。範囲から外れたら、残りを読まずに知らせる
fn records(
    bytes: &[u8],
    start: u64,
    count: u32,
    extended: bool,
    cx: &mut Cx<'_>,
) -> Result<Vec<Vlr>, Error> {
    let mut out = Vec::new();
    let mut at_byte: usize = 0;
    let head = if extended { EVLR_HEADER } else { VLR_HEADER } as usize;
    for _ in 0..count {
        cx.check(Kind::Records, 1)?;
        let length = if extended {
            u64_at(bytes, at_byte + 20)
        } else {
            u16_at(bytes, at_byte + 20).map(u64::from)
        };
        let body = at_byte.checked_add(head);
        let end = length
            .and_then(|length| usize::try_from(length).ok())
            .zip(body)
            .and_then(|(length, body)| body.checked_add(length));
        let (Some(body), Some(end), Some(record)) = (body, end, u16_at(bytes, at_byte + 18)) else {
            break;
        };
        let (Some(user), Some(description), Some(data)) = (
            bytes.get(at_byte + 2..at_byte + 18),
            bytes.get(at_byte + 22..at_byte + 54),
            bytes.get(body..end),
        ) else {
            break;
        };
        out.push(Vlr {
            user: text(user),
            record,
            description: text(description),
            data: data.to_vec(),
            extended,
            span: ByteRange {
                start: start + at_byte as u64,
                end: start + end as u64,
            },
        });
        at_byte = end;
    }
    if out.len() < count as usize {
        cx.report(
            Diagnostic::new(codes::LAS_VLR_INVALID)
                .with("expected", u64::from(count))
                .with("found", out.len() as u64)
                .at(Ref::at_bytes(
                    start + at_byte as u64,
                    start + bytes.len() as u64,
                )),
        );
    }
    Ok(out)
}

/// LAZ の点を展開する。展開したバイトは `decoded-bytes` で数える
fn decompress(
    header: &Header,
    vlrs: &[Vlr],
    compressed: &[u8],
    cx: &mut Cx<'_>,
) -> Result<Vec<u8>, Error> {
    let invalid = |cx: &mut Cx<'_>, reason: &str| {
        cx.report(Diagnostic::new(codes::LAS_LAZ_INVALID).with("reason", reason));
        Error::Invalid
    };
    let Some(record) = vlrs
        .iter()
        .find(|vlr| vlr.user == laz::LazVlr::USER_ID && vlr.record == laz::LazVlr::RECORD_ID)
    else {
        return Err(invalid(cx, "laszip-vlr-missing"));
    };
    let Ok(vlr) = laz::LazVlr::from_buffer(&record.data) else {
        return Err(invalid(cx, "laszip-vlr-invalid"));
    };
    if vlr.items_size() != u64::from(header.record_length) {
        return Err(invalid(cx, "record-length"));
    }
    let size = header
        .point_count
        .saturating_mul(u64::from(header.record_length));
    cx.check(Kind::DecodedBytes, size)?;
    let Ok(size) = usize::try_from(size) else {
        return Err(invalid(cx, "too-large"));
    };
    let mut out = vec![0; size];
    let mut source = Shifted {
        bytes: compressed,
        base: u64::from(header.point_offset),
        position: u64::from(header.point_offset),
    };
    // 塊の表の位置はファイルの初めからなので、点のレコードの始まりからずらして読む
    let done = laz::LasZipDecompressor::new(&mut source, vlr)
        .map_err(|_| ())
        .and_then(|mut decompressor| decompressor.decompress_many(&mut out).map_err(|_| ()));
    if done.is_err() {
        return Err(invalid(cx, "decompress"));
    }
    Ok(out)
}

/// ファイルの途中から始まるバイト列を、ファイルの初めからの位置で読む
struct Shifted<'a> {
    bytes: &'a [u8],
    base: u64,
    position: u64,
}

impl Read for Shifted<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let from = self
            .position
            .checked_sub(self.base)
            .and_then(|from| usize::try_from(from).ok())
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        let rest = self.bytes.get(from..).unwrap_or_default();
        let n = rest.len().min(buf.len());
        if let (Some(to), Some(from)) = (buf.get_mut(..n), rest.get(..n)) {
            to.copy_from_slice(from);
        }
        self.position += n as u64;
        Ok(n)
    }
}

impl Seek for Shifted<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let end = self.base + self.bytes.len() as u64;
        let next = match pos {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::End(delta) => end.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
        };
        match next {
            Some(at) if at >= self.base => {
                self.position = at;
                Ok(at)
            }
            _ => Err(std::io::ErrorKind::InvalidInput.into()),
        }
    }
}
