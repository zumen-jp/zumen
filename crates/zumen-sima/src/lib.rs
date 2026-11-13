//! SIMA 共通フォーマット（カンマ区切り `.sim` と JPGIS 版 XML `jsima_jpgis_1_0_2`）を読み書きし、
//! 図面（[`Drawing2D`]）・土木（[`CivilModel`]）・地理（[`GeoModel`]）の領域モデルと相互に変換する（`#374`）。

mod csv;
mod lift;
mod lower;
mod model;
mod xml;

pub use lower::{
    LoweredSima, lower_civil_to_sima, lower_drawing_to_sima, lower_geo_to_sima, lower_sima_to_sima,
};

use zumen_base::{ByteRange, Diagnostic, Kind, Limit, Ref, Stop, codes};
use zumen_civil::CivilModel;
use zumen_drawing::Drawing2D;
use zumen_geo::GeoModel;
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};
use zumen_xml::Document;

/// JPGIS 版 SIMA XML Schema Ver.1.0.2 の名前空間
pub const NAMESPACE_201206: &str = "http://www.jsima.or.jp/JSIMASchema/201206";
/// JPGIS 版 SIMA XML Schema 旧版の名前空間
pub const NAMESPACE_201004: &str = "http://www.jsima.or.jp/JSIMASchema/201004";
/// JPGIS Ver.2.1 標準スキーマの名前空間
pub const JPGIS_NAMESPACE: &str = "http://www.gsi.go.jp/GIS/jpgis/standardSchemas2.1_2009-05";

/// 予算超過または中断の診断を報告して `Error::Stopped` を返す
pub(crate) fn stopped(cx: &mut Cx<'_>, stop: Stop, at: Option<Ref>) -> Error {
    let diag = stop.to_diagnostic();
    cx.report(match at {
        Some(place) => diag.at(place),
        None => diag,
    });
    Error::Stopped(stop)
}

/// SIMA 共通フォーマットのコーデック
pub struct Sima;

#[derive(Clone, Debug)]
pub(crate) struct Record {
    pub kind: Vec<u8>,
    pub bytes: ByteRange,
}

#[derive(Debug)]
enum Inner {
    Csv {
        src: Vec<u8>,
        utf8: bool,
        records: Vec<Record>,
    },
    Xml {
        doc: Document,
        records: Vec<Record>,
    },
}

/// 読んだ SIMA の L1 モデル
#[derive(Debug)]
pub struct SimaFile {
    inner: Inner,
}

impl SimaFile {
    /// 元のバイト列
    pub fn src(&self) -> &[u8] {
        match &self.inner {
            Inner::Csv { src, .. } => src,
            Inner::Xml { doc, .. } => doc.src(),
        }
    }

    fn records(&self) -> &[Record] {
        match &self.inner {
            Inner::Csv { records, .. } | Inner::Xml { records, .. } => records,
        }
    }

    pub(crate) fn parse_data(&self, cx: &mut Cx) -> Result<model::SimaData, Error> {
        match &self.inner {
            Inner::Csv {
                src, utf8, records, ..
            } => csv::parse_csv_data(src, *utf8, records, cx),
            Inner::Xml { doc, .. } => xml::parse_xml_data(doc, cx),
        }
    }
}

impl FileModel for SimaFile {
    fn record_count(&self) -> usize {
        self.records().len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let rec = self.records().get(index)?;
        Some(RecordInfo {
            kind: &rec.kind,
            bytes: Some(rec.bytes),
        })
    }
}

impl Codec for Sima {
    type File = SimaFile;

    const FORMAT: &'static str = "sima";

    const SIGNATURES: &'static [Signature] = &[
        Signature::Bytes {
            offset: 0,
            bytes: b"G00,",
        },
        Signature::Bytes {
            offset: 0,
            bytes: b"\xEF\xBB\xBFG00,",
        },
        Signature::XmlRoot {
            name: b"GI",
            namespace: Some(NAMESPACE_201206.as_bytes()),
        },
        Signature::XmlRoot {
            name: b"GI",
            namespace: Some(NAMESPACE_201004.as_bytes()),
        },
    ];

    fn read(src: &[u8], cx: &mut Cx) -> Result<SimaFile, Error> {
        if let Limit::Finite(max) = cx.budget.limits().get(Kind::InputBytes)
            && cx
                .budget
                .used(Kind::InputBytes)
                .saturating_add(src.len() as u64)
                > max
        {
            cx.check(Kind::InputBytes, src.len() as u64)
                .map_err(|s| stopped(cx, s, None))?;
        }
        cx.check(Kind::Work, 1).map_err(|s| stopped(cx, s, None))?;
        if src.len() as u64 > model::MAX_ALLOC {
            cx.check(Kind::InputBytes, src.len() as u64)
                .map_err(|s| stopped(cx, s, None))?;
            cx.report(
                Diagnostic::new(codes::SIMA_FILE_INVALID)
                    .with("offset", 0u64)
                    .at(Ref::at_bytes(0, 0)),
            );
            return Err(Error::Invalid);
        }
        if looks_like_xml(src, cx)? {
            let doc = zumen_xml::read(src, cx)?;
            let records = xml::validate_and_collect_records(&doc, cx)?;
            return Ok(SimaFile {
                inner: Inner::Xml { doc, records },
            });
        }
        let (utf8, records) = csv::read_csv_records(src, cx)?;
        Ok(SimaFile {
            inner: Inner::Csv {
                src: src.to_vec(),
                utf8,
                records,
            },
        })
    }

    fn write(file: &SimaFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        let bytes = file.src();
        cx.check(Kind::DecodedBytes, bytes.len() as u64)
            .map_err(|s| stopped(cx, s, None))?;
        out.extend_from_slice(bytes);
        Ok(())
    }
}

fn is_ascii_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

fn looks_like_xml(src: &[u8], cx: &mut Cx) -> Result<bool, Error> {
    if src.starts_with(b"\xFF\xFE") || src.starts_with(b"\xFE\xFF") {
        return Ok(true);
    }
    // BOM なしの UTF-16BE / UTF-16LE（先頭の空白 2 バイト列 + `<`）も XML として渡す
    if matches!(src.get(..2), Some([0x00, b]) if *b == b'<' || is_ascii_ws(*b)) {
        let mut idx = 0usize;
        while let Some(pair) = src.get(idx..idx.saturating_add(2)) {
            let [first, second] = pair else {
                break;
            };
            if (idx / 2).is_multiple_of(32) && idx > 0 {
                cx.check(Kind::Work, 1)
                    .map_err(|s| stopped(cx, s, Some(Ref::at_bytes(0, idx as u64))))?;
            }
            if *first == 0x00 && *second == b'<' {
                return Ok(true);
            }
            if *first == 0x00 && is_ascii_ws(*second) {
                idx += 2;
                continue;
            }
            break;
        }
        return Ok(false);
    }
    if matches!(src.get(..2), Some([b, 0x00]) if *b == b'<' || is_ascii_ws(*b)) {
        let mut idx = 0usize;
        while let Some(pair) = src.get(idx..idx.saturating_add(2)) {
            let [first, second] = pair else {
                break;
            };
            if (idx / 2).is_multiple_of(32) && idx > 0 {
                cx.check(Kind::Work, 1)
                    .map_err(|s| stopped(cx, s, Some(Ref::at_bytes(0, idx as u64))))?;
            }
            if *first == b'<' && *second == 0x00 {
                return Ok(true);
            }
            if *second == 0x00 && is_ascii_ws(*first) {
                idx += 2;
                continue;
            }
            break;
        }
        return Ok(false);
    }
    let rest = src.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(src);
    for (idx, &b) in rest.iter().enumerate() {
        if idx > 0 && idx.is_multiple_of(64) {
            cx.check(Kind::Work, 1)
                .map_err(|s| stopped(cx, s, Some(Ref::at_bytes(0, idx as u64))))?;
        }
        if !is_ascii_ws(b) {
            return Ok(b == b'<');
        }
    }
    Ok(false)
}

impl Lift<Drawing2D> for SimaFile {
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        let data = self.parse_data(cx)?;
        lift::lift_drawing(&data, cx)
    }
}

impl Lift<CivilModel> for SimaFile {
    fn lift(&self, cx: &mut Cx) -> Result<CivilModel, Error> {
        let data = self.parse_data(cx)?;
        lift::lift_civil(&data, cx)
    }
}

impl Lift<GeoModel> for SimaFile {
    fn lift(&self, cx: &mut Cx) -> Result<GeoModel, Error> {
        let data = self.parse_data(cx)?;
        lift::lift_geo(&data, cx)
    }
}
