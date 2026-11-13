//! GeoJSON（RFC 7946、台帳の `rfc_7946`）を読み書きし、地理の領域モデル（[`GeoModel`]）と相互に変換する（`#876`）。
//!
//! 見分ける固有の先頭バイト列がないので、`SIGNATURES` は空とし、API の `open` では `format: "geojson"` を名指して読む
//! （`docs/formats/geojson.md`）。

mod json;
mod lift;
mod lower;

pub use lower::{LoweredGeoJson, lower_to_geojson};

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, Stop, codes};
use zumen_geo::GeoModel;
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};

use crate::json::SpannedValue;

/// 予算超過または中断の診断を報告して `Error::Stopped` を返す
pub(crate) fn stopped(cx: &mut Cx<'_>, stop: Stop, at: Option<Ref>) -> Error {
    let diag = stop.to_diagnostic();
    cx.report(match at {
        Some(place) => diag.at(place),
        None => diag,
    });
    Error::Stopped(stop)
}

/// GeoJSON のコーデック
pub struct GeoJson;

/// 読んだ GeoJSON の L1 モデル
#[derive(Debug)]
pub struct GeoJsonFile {
    src: Vec<u8>,
    root: SpannedValue,
    records: Vec<Record>,
}

#[derive(Clone, Debug)]
struct Record {
    kind: Vec<u8>,
    bytes: ByteRange,
}

impl GeoJsonFile {
    /// 元のバイト列
    pub fn src(&self) -> &[u8] {
        &self.src
    }
}

impl FileModel for GeoJsonFile {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let rec = self.records.get(index)?;
        Some(RecordInfo {
            kind: &rec.kind,
            bytes: Some(rec.bytes),
        })
    }
}

impl Codec for GeoJson {
    type File = GeoJsonFile;

    const FORMAT: &'static str = "geojson";

    const SIGNATURES: &'static [Signature] = &[];

    fn read(src: &[u8], cx: &mut Cx) -> Result<GeoJsonFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|s| stopped(cx, s, None))?;
        if src.len() as u64 > json::MAX_ALLOC {
            return Err(json::invalid_at(0, src.len(), cx));
        }
        let root = json::parse_spanned(src, cx)?;
        let Some(root_type) = root.get("type").and_then(SpannedValue::as_str) else {
            cx.report(
                Diagnostic::new(codes::GEOJSON_FILE_INVALID)
                    .with("offset", root.span.start)
                    .at(root.at()),
            );
            return Err(Error::Invalid);
        };
        if !lift::is_geojson_root_type(root_type) {
            cx.report(
                Diagnostic::new(codes::GEOJSON_FILE_INVALID)
                    .with("offset", root.span.start)
                    .at(root.at()),
            );
            return Err(Error::Invalid);
        }

        let mut records = Vec::new();
        if root_type == "FeatureCollection" {
            let Some(items) = root.get("features").and_then(SpannedValue::as_array) else {
                cx.report(
                    Diagnostic::new(codes::GEOJSON_FILE_INVALID)
                        .with("offset", root.span.start)
                        .at(root.at()),
                );
                return Err(Error::Invalid);
            };
            if items.is_empty() {
                cx.check(Kind::Records, 1)
                    .map_err(|s| stopped(cx, s, Some(root.at())))?;
                records.push(Record {
                    kind: root_type.as_bytes().to_vec(),
                    bytes: root.span,
                });
            } else {
                for item in items {
                    cx.check(Kind::Records, 1)
                        .map_err(|s| stopped(cx, s, Some(item.at())))?;
                    let kind = item
                        .get("type")
                        .and_then(SpannedValue::as_str)
                        .unwrap_or("")
                        .as_bytes()
                        .to_vec();
                    records.push(Record {
                        kind,
                        bytes: item.span,
                    });
                }
            }
        } else {
            cx.check(Kind::Records, 1)
                .map_err(|s| stopped(cx, s, Some(root.at())))?;
            records.push(Record {
                kind: root_type.as_bytes().to_vec(),
                bytes: root.span,
            });
        }

        cx.check(Kind::DecodedBytes, src.len() as u64)
            .map_err(|s| stopped(cx, s, None))?;
        Ok(GeoJsonFile {
            src: src.to_vec(),
            root,
            records,
        })
    }

    fn write(file: &GeoJsonFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        cx.check(Kind::DecodedBytes, file.src.len() as u64)
            .map_err(|s| stopped(cx, s, None))?;
        out.extend_from_slice(&file.src);
        Ok(())
    }
}

impl Lift<GeoModel> for GeoJsonFile {
    fn lift(&self, cx: &mut Cx) -> Result<GeoModel, Error> {
        lift::lift(&self.root, cx)
    }
}
