//! 拡張 DM（台帳の `extdm_furoku7`。作業規程の準則の付録 7「公共測量標準図式」の数値地形図データファイル
//! 仕様）を読み、地理の領域モデル（決定 0233）に移す。
//!
//! L1 は 84 バイトの固定長のレコード（第 96 条）の列で、行ごとに元のバイトの範囲を持つ。見出しのレコード
//! （インデックス・図郭・グループヘッダー・要素・グリッドヘッダー・不整三角網ヘッダー）が持つ数から、後に続く実データの
//! レコードを分ける（`docs/formats/extdm.md`）。

mod lift;
mod record;

use std::ops::Range;

use zumen_base::{ByteRange, Kind};
use zumen_geo::GeoModel;
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};

/// 拡張 DM のコーデック。見分ける印がないので、判定の規則を持たない。`open` で形式を名指して読む
pub struct ExtDm;

/// 読んだ拡張 DM。レコードの列を持つ
#[derive(Debug)]
pub struct ExtDmFile {
    src: Vec<u8>,
    records: Vec<Line>,
    /// 文字が UTF-8 か。UTF-8 として正しくなければ Shift_JIS とする（第 99 条）
    utf8: bool,
}

/// 1 つのレコード
#[derive(Clone, Debug)]
pub(crate) struct Line {
    /// 区切り（CR LF）を除いたバイトの範囲
    pub(crate) range: Range<usize>,
    /// 見出しのレコードなら、レコードタイプ（`I`・`M`・`H`・`E1`〜`E8`・`G`・`T`）。実データのレコードは `None`
    pub(crate) kind: Option<&'static [u8]>,
}

impl ExtDmFile {
    /// 元のバイト列
    pub fn src(&self) -> &[u8] {
        &self.src
    }

    pub(crate) fn bytes(&self, index: usize) -> &[u8] {
        self.records
            .get(index)
            .and_then(|line| self.src.get(line.range.clone()))
            .unwrap_or_default()
    }

    pub(crate) fn records(&self) -> &[Line] {
        &self.records
    }

    pub(crate) fn utf8(&self) -> bool {
        self.utf8
    }
}

impl FileModel for ExtDmFile {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let line = self.records.get(index)?;
        Some(RecordInfo {
            kind: line.kind.unwrap_or(b""),
            bytes: Some(ByteRange {
                start: line.range.start as u64,
                end: line.range.end as u64,
            }),
        })
    }
}

impl Codec for ExtDm {
    type File = ExtDmFile;

    const FORMAT: &'static str = "extdm";

    const SIGNATURES: &'static [Signature] = &[];

    fn read(src: &[u8], cx: &mut Cx) -> Result<ExtDmFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let start = if src.starts_with(b"\xEF\xBB\xBF") {
            3
        } else {
            0
        };
        let mut ranges = Vec::new();
        let mut begin = start;
        for (index, byte) in src.iter().enumerate().skip(start) {
            if *byte == b'\n' {
                let end = if index > begin && src.get(index - 1) == Some(&b'\r') {
                    index - 1
                } else {
                    index
                };
                ranges.push(begin..end);
                begin = index + 1;
            }
        }
        if begin < src.len() {
            ranges.push(begin..src.len());
        }
        cx.check(Kind::Records, ranges.len() as u64)?;
        let records = record::classify(src, ranges);
        Ok(ExtDmFile {
            utf8: std::str::from_utf8(src).is_ok(),
            src: src.to_vec(),
            records,
        })
    }

    /// 書き戻しはしないので、読んだバイト列をそのまま書く
    fn write(file: &ExtDmFile, out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        out.extend_from_slice(&file.src);
        Ok(())
    }
}

impl Lift<GeoModel> for ExtDmFile {
    fn lift(&self, cx: &mut Cx) -> Result<GeoModel, Error> {
        lift::lift(self, cx)
    }
}
