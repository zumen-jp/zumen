//! JWW（Jw_cad の図面）のファイルモデル（L1）と、そのコーデック（決定 0008・0133）。
//!
//! JWW は、見出しの欄と、MFC の CArchive で書いた図形の一覧・ブロックの定義の一覧と、版 700 からは画像の一覧を持つ。
//! 見出し・図形・ブロックの定義・画像を、レコードとして元のバイト範囲のまま読む。値は戻さず、戻すのは図面モデルへの変換で行う。

#[cfg(feature = "drawing")]
mod drawing;
mod parse;

use core::ops::Range;

use zumen_base::{Kind, Ref};
use zumen_sdk::{ByteRange, Codec, Cx, Error, FileModel, RecordInfo, Signature, Stop};

/// JWW のコーデック
#[derive(Clone, Copy, Debug)]
pub struct Jww;

/// JWW の始まり
const MAGIC: &[u8] = b"JwwData.";

/// レコードの種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    /// 始まりから図形の一覧の件数の前まで
    Header,
    /// 図形（線・円弧・点・文字・ソリッド・ブロックの参照・寸法）
    Entity,
    /// ブロックの定義の、中の図形の一覧の件数まで。中の図形は、後ろに続くレコード
    Block,
    /// 空のオブジェクト
    Null,
    /// 画像の件数（版 700 から）
    ImageCount,
    /// 埋め込んだ画像。名前・大きさ・中身
    Image,
    /// 読めなかった残り
    Trailer,
}

/// オブジェクトの前の印の形。書くときに同じ形を使う
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    /// 新しいクラス（`FF FF`・スキーマ・名前）
    New,
    /// 読んだクラスの番号（2 バイト）
    Known,
    /// 読んだクラスの番号（`7F FF` と 4 バイト）
    Big,
}

/// CArchive のクラス
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Class {
    name: Range<usize>,
    schema: u16,
}

impl Class {
    /// 名前の範囲
    pub fn name(&self) -> Range<usize> {
        self.name.clone()
    }

    /// スキーマの番号
    pub fn schema(&self) -> u16 {
        self.schema
    }
}

/// レコード
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    span: Range<usize>,
    kind: RecordKind,
    tag: Option<(Tag, usize)>,
    body: Range<usize>,
    block: Option<usize>,
}

impl Record {
    /// 元のバイト範囲
    pub fn span(&self) -> Range<usize> {
        self.span.clone()
    }

    /// 種類
    pub fn kind(&self) -> RecordKind {
        self.kind
    }

    /// オブジェクトの印の形と、クラスの位置（[`JwwFile::classes`] の中）。図形とブロックの定義にある
    pub fn tag(&self) -> Option<(Tag, usize)> {
        self.tag
    }

    /// 中身の範囲。図形とブロックの定義は印の後ろ、画像は中身のバイト列
    pub fn body(&self) -> Range<usize> {
        self.body.clone()
    }

    /// 入っているブロックの定義のレコードの位置。ブロックの定義の中の図形にある
    pub fn block(&self) -> Option<usize> {
        self.block
    }
}

/// JWW のファイルモデル（L1）。元のバイト列と、そのレコード
#[derive(Clone, Debug, PartialEq)]
pub struct JwwFile {
    src: Vec<u8>,
    version: u32,
    classes: Vec<Class>,
    records: Vec<Record>,
}

impl JwwFile {
    /// 版（`JwwData.` の後ろの 4 バイト）
    pub fn version(&self) -> u32 {
        self.version
    }

    /// 読んだ順のクラス
    pub fn classes(&self) -> &[Class] {
        &self.classes
    }

    /// レコード
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// 元のバイト列の範囲。外なら空
    pub fn bytes(&self, range: Range<usize>) -> &[u8] {
        self.src.get(range).unwrap_or_default()
    }

    /// レコードの種類の名前。図形とブロックの定義はクラスの名前
    pub fn kind_name(&self, record: &Record) -> &[u8] {
        match record.kind {
            RecordKind::Header => b"header",
            RecordKind::Entity | RecordKind::Block => record
                .tag
                .and_then(|(_, class)| self.classes.get(class))
                .map(|class| self.bytes(class.name()))
                .unwrap_or_default(),
            RecordKind::Null => b"null",
            RecordKind::ImageCount => b"image-count",
            RecordKind::Image => b"image",
            RecordKind::Trailer => b"trailer",
        }
    }
}

impl FileModel for JwwFile {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let record = self.records.get(index)?;
        Some(RecordInfo {
            kind: self.kind_name(record),
            bytes: Some(ByteRange {
                start: record.span.start as u64,
                end: record.span.end as u64,
            }),
        })
    }
}

impl Codec for Jww {
    type File = JwwFile;

    const FORMAT: &'static str = "jww";

    const SIGNATURES: &'static [Signature] = &[Signature::Bytes {
        offset: 0,
        bytes: MAGIC,
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<JwwFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop, None))?;
        let read = parse::read(src, cx)?;
        Ok(JwwFile {
            src: src.to_vec(),
            version: read.version,
            classes: read.classes,
            records: read.records,
        })
    }

    fn write(file: &JwwFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        let mut end = 0;
        for record in &file.records {
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
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
