//! TIFF 6.0 と OGC GeoTIFF 1.1（`19-008r4`）の読み取り、領域モデル（`Drawing2D`・`GeoModel`・`PointCloud`）への持ち上げ、
//! ならびに指定ページの RGBA8 PNG 復号（#877）。
//!
//! 読み取りだけを行い、書き出しは行わない。仕様の詳細は `docs/formats/tiff.md` にある。

mod decompress;
mod geotiff;
mod ifd;
mod lift;
mod raster;

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, Stop, codes};
use zumen_drawing::Drawing2D;
use zumen_geo::GeoModel;
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};

pub use geotiff::{GeoInfo, GeoTransform, RasterType};
pub use ifd::{Endian, PageInfo, Photometric, PlanarConfig, ResolutionUnit, SampleFormat};
pub use raster::RasterPng;

/// 1 回の確保で許す最大バイト数（32 ビット環境での再確保倍増でも `isize::MAX` を超えない 1 GiB − 1）
pub(crate) const MAX_ALLOC: u64 = (i32::MAX as u64) / 2;

/// PNG 出力で許す最大画素数（`zumen-render` と同じ 16,777,216 画素）
pub const MAX_PNG_PIXELS: u64 = 16 * 1024 * 1024;

pub(crate) fn stopped(cx: &mut Cx<'_>, stop: Stop, at: Option<Ref>) -> Error {
    let diag = stop.to_diagnostic();
    cx.report(match at {
        Some(r) => diag.at(r),
        None => diag,
    });
    Error::Stopped(stop)
}

pub(crate) fn invalid(cx: &mut Cx<'_>, offset: usize) -> Error {
    let end = (offset as u64).saturating_add(1);
    cx.report(
        Diagnostic::new(codes::TIFF_FILE_INVALID)
            .with("offset", offset as u64)
            .at(Ref::at_bytes(offset as u64, end)),
    );
    Error::Invalid
}

#[derive(Clone, Debug)]
pub(crate) struct Record {
    pub kind: &'static [u8],
    pub bytes: ByteRange,
}

/// TIFF・GeoTIFF の L1 ファイルモデル
#[derive(Clone, Debug)]
pub struct TiffFile {
    records: Vec<Record>,
    endian: Endian,
    pages: Vec<ifd::DecodedIfd>,
}

impl TiffFile {
    /// バイト順
    pub fn endian(&self) -> Endian {
        self.endian
    }

    /// 総ページ（IFD）数
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// 0 始まりのページ情報
    pub fn page(&self, index: usize) -> Option<&PageInfo> {
        self.pages.get(index).map(|p| &p.info)
    }

    /// 点群（DEM 標高格子）として持ち上げ可能な IFD を持つか
    pub fn has_dem_grid(&self) -> bool {
        self.pages.iter().any(ifd::DecodedIfd::is_dem_candidate)
    }

    /// 1 始まりの指定ページを RGBA8・非インターレースの PNG に符号化する。
    ///
    /// `max_side` を指定すると、元の長辺が `max_side` を超えるときだけ縦横比を保って縮小する。
    pub fn encode_raster_png(
        &self,
        page_1based: usize,
        max_side: Option<u32>,
        cx: &mut Cx<'_>,
    ) -> Result<RasterPng, Error> {
        raster::encode_page_png(self, page_1based, max_side, cx)
    }
}

impl FileModel for TiffFile {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let rec = self.records.get(index)?;
        Some(RecordInfo {
            kind: rec.kind,
            bytes: Some(rec.bytes),
        })
    }
}

/// TIFF・GeoTIFF のコーデック
pub struct Tiff;

impl Codec for Tiff {
    type File = TiffFile;

    const FORMAT: &'static str = "tiff";
    const SIGNATURES: &'static [Signature] = &[
        Signature::Bytes {
            offset: 0,
            bytes: b"II*\0",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"MM\0*",
        },
    ];

    fn read(src: &[u8], cx: &mut Cx<'_>) -> Result<Self::File, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|s| stopped(cx, s, None))?;
        if src.len() as u64 > MAX_ALLOC {
            return Err(invalid(cx, 0));
        }
        ifd::read_tiff(src, cx)
    }

    fn write(_file: &Self::File, _out: &mut Vec<u8>, _cx: &mut Cx<'_>) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

impl Lift<Drawing2D> for TiffFile {
    fn lift(&self, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
        lift::lift_drawing(self, cx)
    }
}

impl Lift<GeoModel> for TiffFile {
    fn lift(&self, cx: &mut Cx<'_>) -> Result<GeoModel, Error> {
        lift::lift_geo(self, cx)
    }
}

impl Lift<PointCloud> for TiffFile {
    fn lift(&self, cx: &mut Cx<'_>) -> Result<PointCloud, Error> {
        lift::lift_pointcloud(self, cx)
    }
}
