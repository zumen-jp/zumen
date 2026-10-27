//! SXF（SFC・P21）のファイルモデル（L1）と、そのコーデック（決定 0008・0120）。
//!
//! SFC と P21 は、どちらも ISO 10303-21 の交換構造（`ISO-10303-21;` で始まり、見出しとデータの節を持つ）に入る。
//! 文（`;` で終わるもの）と、SFC のフィーチャのコメント（`/*SXF` から `SXF*/` まで）を、レコードとして元のバイト範囲のまま読む。
//! 値は元のバイト列の範囲で持ち、文字列のエスケープと文字コードは戻さない。戻すのは図面モデルへの変換で行う。

mod parse;

use core::ops::Range;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{ByteRange, Codec, Cx, Error, FileModel, RecordInfo, Signature, Stop};

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
    /// 長いものから順に並べた、印・開き・閉じ
    const ALL: [(Marker, &'static [u8], &'static [u8]); 3] = [
        (Marker::Sxf31, b"/*SXF3.1", b"SXF3.1*/"),
        (Marker::Sxf3, b"/*SXF3", b"SXF3*/"),
        (Marker::Sxf, b"/*SXF", b"SXF*/"),
    ];
}

/// レコードの形
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// 引数のない語（`HEADER;` など）
    Keyword,
    /// 名前と引数（`FILE_NAME(…);`・`#1=LINE(…);`・SFC のフィーチャ）
    Entity,
    /// 部分を並べた複合のインスタンス（`#1=(A() B());`）
    Complex,
}

/// レコード。文か、SFC のフィーチャのコメント
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    span: Range<usize>,
    id: Option<u64>,
    marker: Option<Marker>,
    form: Form,
    parts: Vec<Part>,
}

impl Record {
    /// 元のバイト範囲。文は `;` まで、フィーチャはコメントの閉じまで
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    /// インスタンスの番号（`#n`）。見出しの文と語にはない
    pub fn id(&self) -> Option<u64> {
        self.id
    }

    /// SFC のフィーチャのコメントの印。文にはない
    pub fn marker(&self) -> Option<Marker> {
        self.marker
    }

    /// 形
    pub fn form(&self) -> Form {
        self.form
    }

    /// 部分。語と名前と引数のものは 1 つ、複合のインスタンスは書いた順
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }
}

/// 名前と引数
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    name: Range<usize>,
    args: Vec<Value>,
}

impl Part {
    /// 名前の範囲
    pub fn name(&self) -> Range<usize> {
        self.name.clone()
    }

    /// 引数
    pub fn args(&self) -> &[Value] {
        &self.args
    }
}

/// 文字列の囲み方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quote {
    /// `'…'`。P21 は `''` で `'` を表す。SFC は次の `'` で閉じる
    Plain,
    /// SFC の `\'…\'`。Shift_JIS の 2 バイト目と `\\` を飛ばし、後に `,` か `)` が続く `\'` で閉じる
    Backslash,
}

/// 引数の値。範囲は元のバイト列の中を指す
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `$`（値がない）
    Null,
    /// `*`（導く値）
    Derived,
    /// `#n`（インスタンスの参照）
    Ref(u64),
    /// `.E.` の内側
    Enum(Range<usize>),
    /// 文字列の内側。エスケープは戻さない
    Text {
        /// 内側の範囲
        raw: Range<usize>,
        /// 囲み方
        quote: Quote,
    },
    /// 数
    Number(Range<usize>),
    /// `"…"` の内側の 16 進
    Binary(Range<usize>),
    /// 型を付けた値（`LENGTH_MEASURE(1.)`）
    Typed(Part),
    /// `(…)`
    List(Vec<Value>),
    /// SFC の、囲まない値。前後の空白を除いた範囲
    Token(Range<usize>),
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
            .parts
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
                start: record.span.start as u64,
                end: record.span.end as u64,
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
        let read = parse::read(src, cx)?;
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
        Ok(SxfFile {
            src: src.to_vec(),
            mode: read.mode,
            records: read.records,
        })
    }

    fn write(file: &SxfFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        let mut end = 0;
        for record in &file.records {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
            // レコードの間（空白・コメント・読めなかった文）は、そのまま写す
            out.extend_from_slice(file.bytes(end..record.span.start));
            out.extend_from_slice(file.bytes(record.span()));
            end = record.span.end;
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
