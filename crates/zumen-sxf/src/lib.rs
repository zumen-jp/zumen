//! SXF（SFC・P21）のファイルモデル（L1）と、そのコーデック（決定 0008・0120）。
//!
//! SFC と P21 は、どちらも ISO 10303-21 の交換構造（`ISO-10303-21;` で始まり、見出しとデータの節を持つ）に入る。
//! 文（`;` で終わるもの）と、SFC のフィーチャのコメント（`/*SXF` から `SXF*/` まで）を、レコードとして元のバイト範囲のまま読む。
//! 交換構造の読み取りは `zumen-step` が行い、SFC のフィーチャのコメントの印と診断を [`Dialect`] で渡す（決定 0207）。
//! 値は元のバイト列の範囲で持ち、文字列のエスケープと文字コードは戻さない。戻すのは図面モデルへの変換で行う。
//! SFC と P21 は図面モデル（L2）に変換できる（決定 0121・0132）。

#[cfg(feature = "drawing")]
mod drawing;
#[cfg(feature = "drawing")]
mod p21;

use core::ops::Range;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{ByteRange, Codec, Cx, Error, FileModel, RecordInfo, Signature, Stop};
use zumen_step::Dialect;
pub use zumen_step::{Form, Part, Quote, Record, Value};

/// SXF のコーデック
#[derive(Clone, Copy, Debug)]
pub struct Sxf;

/// SXF の入れ方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// フィーチャをコメントに書く SFC
    Sfc,
    /// STEP の AP202 のインスタンスで書く P21
    P21,
}

/// SFC のフィーチャのコメントの印。印は版を表す（推定。docs/formats/sxf.md）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    /// `/*SXF` … `SXF*/`
    Sxf,
    /// `/*SXF3` … `SXF3*/`
    Sxf3,
    /// `/*SXF3.1` … `SXF3.1*/`
    Sxf31,
}

impl Marker {
    /// 長いものから順に並べた、印・開き・閉じ。[`DIALECT`] の `features` と同じ順
    const ALL: [(Marker, &'static [u8], &'static [u8]); 3] = [
        (Marker::Sxf31, b"/*SXF3.1", b"SXF3.1*/"),
        (Marker::Sxf3, b"/*SXF3", b"SXF3*/"),
        (Marker::Sxf, b"/*SXF", b"SXF*/"),
    ];
}

/// SXF の交換構造の読み方。AP202 と書いた P21 では、`/*SXF` もただのコメントとする（推定。docs/formats/sxf.md）
const DIALECT: Dialect = Dialect {
    record_invalid: codes::SXF_RECORD_INVALID,
    file_invalid: codes::SXF_FILE_INVALID,
    features: &[
        (b"/*SXF3.1", b"SXF3.1*/"),
        (b"/*SXF3", b"SXF3*/"),
        (b"/*SXF", b"SXF*/"),
    ],
    plain_if_described: Some(b"AP202"),
};

/// SFC のフィーチャのコメントの印を持つレコード
pub trait SfcRecord {
    /// SFC のフィーチャのコメントの印。文にはない
    fn marker(&self) -> Option<Marker>;
}

impl SfcRecord for Record {
    fn marker(&self) -> Option<Marker> {
        self.feature()
            .and_then(|index| Marker::ALL.get(index))
            .map(|(marker, _, _)| *marker)
    }
}

/// SXF のファイルモデル（L1）。元のバイト列と、そのレコード
#[derive(Clone, Debug, PartialEq)]
pub struct SxfFile {
    src: Vec<u8>,
    mode: Mode,
    records: Vec<Record>,
}

impl SxfFile {
    /// SFC か P21 か
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// レコード
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// 元のバイト列の範囲。外なら空
    pub fn bytes(&self, range: Range<usize>) -> &[u8] {
        self.src.get(range).unwrap_or_default()
    }

    /// レコードの種類。語と名前と引数のものはその名前、複合のインスタンスは最初の部分の名前
    pub fn kind(&self, record: &Record) -> &[u8] {
        record
            .parts()
            .first()
            .map(|part| self.bytes(part.name()))
            .unwrap_or_default()
    }
}

impl FileModel for SxfFile {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let record = self.records.get(index)?;
        Some(RecordInfo {
            kind: self.kind(record),
            bytes: Some(ByteRange {
                start: record.span().start as u64,
                end: record.span().end as u64,
            }),
        })
    }
}

/// SFC と P21 に共通の始まり。IFC なども同じ行で始まるので、スキーマは読んでから確かめる
const MAGIC: &[u8] = b"ISO-10303-21;";

/// SXF の図面のスキーマ（推定。docs/formats/sxf.md）
const SCHEMA: &[u8] = b"ASSOCIATIVE_DRAUGHTING";

impl Codec for Sxf {
    type File = SxfFile;

    const FORMAT: &'static str = "sxf";

    const SIGNATURES: &'static [Signature] = &[Signature::Lines { lines: &[MAGIC] }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<SxfFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop, None))?;
        let read = zumen_step::read(src, &DIALECT, cx)?;
        let schema = read.schema.map(|range| src.get(range).unwrap_or_default());
        if let Some(schema) = schema {
            let name = schema
                .split(|&b| b == b'{' || b.is_ascii_whitespace())
                .find(|word| !word.is_empty())
                .unwrap_or_default();
            if !name.eq_ignore_ascii_case(SCHEMA) {
                cx.report(
                    Diagnostic::new(codes::SXF_SCHEMA_UNSUPPORTED)
                        .with("schema", String::from_utf8_lossy(schema).into_owned()),
                );
                return Err(Error::Unsupported);
            }
        }
        cx.check(Kind::Work, 1 + src.len() as u64 / 1024)
            .map_err(|stop| stopped(cx, stop, None))?;
        let described = read
            .description
            .and_then(|range| src.get(range))
            .is_some_and(|text| {
                text.windows(b"feature_mode".len())
                    .any(|window| window.eq_ignore_ascii_case(b"feature_mode"))
            });
        let found_feature = read.records.iter().any(|record| record.feature().is_some());
        Ok(SxfFile {
            src: src.to_vec(),
            mode: if described || found_feature {
                Mode::Sfc
            } else {
                Mode::P21
            },
            records: read.records,
        })
    }

    fn write(file: &SxfFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        let mut end = 0;
        for record in &file.records {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            // レコードの間（空白・コメント・読めなかった文）は、そのまま写す
            out.extend_from_slice(file.bytes(end..record.span().start));
            out.extend_from_slice(file.bytes(record.span()));
            end = record.span().end;
        }
        out.extend_from_slice(file.bytes(end..file.src.len()));
        Ok(())
    }
}

/// 止めた理由を報告する。扱っていたバイト範囲があれば、場所にする（決定 0119）
fn stopped(cx: &mut Cx, stop: Stop, place: Option<Range<usize>>) -> Error {
    let diagnostic = stop.to_diagnostic();
    cx.report(match place {
        Some(range) => diagnostic.at(Ref::at_bytes(range.start as u64, range.end as u64)),
        None => diagnostic,
    });
    Error::Stopped(stop)
}
