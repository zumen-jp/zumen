//! PDF（ISO 32000-2）のファイルモデル（L1）と、そのコーデック（決定 0008・0255）。
//!
//! ファイルの構造（7.5）を、見出し・間接オブジェクト・相互参照表・トレーラー・`startxref`・注記のレコードに、元の
//! バイト範囲のまま分ける。オブジェクトの中身は解釈しない。図面モデル（L2）への移しは、ページの内容を
//! hayro-interpret で解釈して行う（機能 `drawing`）。節の番号は ISO 32000-2:2020。

#[cfg(feature = "drawing")]
mod color;
#[cfg(feature = "drawing")]
mod device;
mod file;
#[cfg(feature = "drawing")]
mod lift;
#[cfg(feature = "drawing")]
mod page;
#[cfg(feature = "drawing")]
mod strings;

use std::sync::Arc;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{ByteRange, Codec, Cx, Error, FileModel, RecordInfo, Signature, Stop};

pub use file::{Record, RecordKind};

/// PDF のコーデック
#[derive(Clone, Copy, Debug)]
pub struct Pdf;

/// ファイルの見出しの始まり（7.5.2）
const MAGIC: &[u8] = b"%PDF-";

/// 見出しを探す範囲。見出しの前に 1,024 バイトまでの余計なバイトがあっても読む（Acrobat と pdf.js と同じ、推定）
const HEADER_WINDOW: usize = 1024;

/// PDF のファイルモデル（L1）。元のバイト列と、そのレコード
#[derive(Clone, Debug, PartialEq)]
pub struct PdfFile {
    /// 元のバイト列。図面モデルへの移しで、hayro-syntax と分け合う
    src: Arc<Vec<u8>>,
    records: Vec<Record>,
}

impl PdfFile {
    /// レコード。ファイルの順
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// 元のバイト列の範囲。外なら空
    pub fn bytes(&self, start: u64, end: u64) -> &[u8] {
        let (Ok(start), Ok(end)) = (usize::try_from(start), usize::try_from(end)) else {
            return &[];
        };
        self.src.get(start..end).unwrap_or_default()
    }
}

impl FileModel for PdfFile {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let record = self.records.get(index)?;
        Some(RecordInfo {
            kind: record.kind.name(),
            bytes: Some(ByteRange {
                start: record.start,
                end: record.end,
            }),
        })
    }
}

impl Codec for Pdf {
    type File = PdfFile;

    const FORMAT: &'static str = "pdf";

    const SIGNATURES: &'static [Signature] = &[Signature::Bytes {
        offset: 0,
        bytes: MAGIC,
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<PdfFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop, None))?;
        let window = src.get(..src.len().min(HEADER_WINDOW)).unwrap_or_default();
        if !window.windows(MAGIC.len()).any(|bytes| bytes == MAGIC) {
            cx.report(Diagnostic::new(codes::PDF_FILE_INVALID));
            return Err(Error::Invalid);
        }
        let records = file::records(src, cx)?;
        Ok(PdfFile {
            src: Arc::new(src.to_vec()),
            records,
        })
    }

    fn write(file: &PdfFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        let mut end = 0;
        for record in &file.records {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record)))?;
            // レコードの間は白空白だけ
            out.extend_from_slice(file.bytes(end, record.start));
            out.extend_from_slice(file.bytes(record.start, record.end));
            end = record.end;
        }
        out.extend_from_slice(file.bytes(end, file.src.len() as u64));
        Ok(())
    }
}

/// 止めた理由を報告する。扱っていたレコードがあれば、そのバイト範囲を場所にする（決定 0119）
fn stopped(cx: &mut Cx, stop: Stop, place: Option<&Record>) -> Error {
    let diagnostic = stop.to_diagnostic();
    cx.report(match place {
        Some(record) => diagnostic.at(Ref::at_bytes(record.start, record.end)),
        None => diagnostic,
    });
    Error::Stopped(stop)
}
