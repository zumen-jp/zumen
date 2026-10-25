//! ASCII の DXF（DXF Reference「About ASCII DXF Files」「Group Code Value Types」）。
//!
//! ファイルは、グループ・コードの行と値の行の組（タグ）が続いたもの。グループ・コード 0 のタグから、
//! 次のグループ・コード 0 のタグの前までを 1 つのレコードにする。`SECTION`・`ENDSEC` も、
//! 未知の種類のものも、同じようにレコードとして持つ。`EOF` の後のバイトは、読まずにそのまま持つ。
//!
//! レコードは元のバイト範囲を持ち、隣のレコードと隙間なく並ぶ。書くときは、元のバイトを並べる。
//! 改行（CR LF か LF）、グループ・コードの前の空白、値の文字コードは、元のバイトに残る。
//!
//! 変更は [`DxfTxn`] で行う。確定すると、変えたレコードだけを書き直す。書き直すときも、
//! タグのグループ・コードの行と改行は元のバイトを使い、値だけを差し替える。

use core::ops::Range;
use std::collections::BTreeMap;

use zumen_base::{Code, Cx, Diagnostic, Kind, Ref, SpecRef, codes};
mod version;

use zumen_sdk::{ByteRange, Codec, Error, FileModel, RecordInfo, Signature, Transact, Transaction};

/// バイナリの DXF の先頭（DXF Reference「Binary DXF Files」）
const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

const ASCII_FILES: SpecRef = SpecRef {
    spec: "dxf_2026",
    section: "About ASCII DXF Files",
};

/// ASCII の DXF のコーデック
#[derive(Clone, Copy, Debug)]
pub struct Dxf;

/// 読んだ DXF。元のバイト列と、その上のレコードの区切り
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DxfFile {
    src: Vec<u8>,
    records: Vec<Record>,
    trailer: Range<usize>,
}

/// グループ・コード 0 のタグから始まるタグの並び。先頭がグループ・コード 0 でないのは、
/// ファイルの最初のレコードだけ
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    span: Range<usize>,
    tags: Vec<Tag>,
    edited: bool,
}

/// グループ・コードと値の組
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    code: i16,
    /// グループ・コードの行の先頭から、値の行の改行の終わりまで
    span: Range<usize>,
    value: Range<usize>,
    /// 変更した値。改行は含まない
    edited: Option<Box<[u8]>>,
}

impl Tag {
    /// グループ・コード
    pub fn code(&self) -> i16 {
        self.code
    }
}

impl Record {
    /// タグ
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }
}

impl DxfFile {
    /// レコード
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// タグの値の生のバイト列。改行は含まない
    pub fn value<'a>(&'a self, tag: &'a Tag) -> &'a [u8] {
        match &tag.edited {
            Some(value) => value,
            None => self.src.get(tag.value.clone()).unwrap_or_default(),
        }
    }

    /// 最初の `code` のタグの値を、実数として読む（DXF Reference「Group Code Value Types」）
    pub fn real(&self, record: &Record, code: i16) -> Result<f64, ViewError> {
        let tag = record
            .tags
            .iter()
            .find(|t| t.code == code)
            .ok_or(ViewError::Missing(code))?;
        parse_real(self.value(tag)).ok_or(ViewError::NotReal(code))
    }

    /// LINE の型付きの見方（DXF Reference「LINE」）。LINE でなければ `None`
    pub fn line<'a>(&'a self, record: &'a Record) -> Option<Result<Line<'a>, ViewError>> {
        (self.kind(record) == b"LINE").then(|| {
            let layer = record
                .tags
                .iter()
                .find(|t| t.code == 8)
                .ok_or(ViewError::Missing(8))?;
            let point = |x: i16| -> Result<[f64; 3], ViewError> {
                // Z のないものは 0 とみなす（推定。docs/formats/dxf.md）
                let z = match self.real(record, x + 20) {
                    Err(ViewError::Missing(_)) => 0.0,
                    z => z?,
                };
                Ok([self.real(record, x)?, self.real(record, x + 10)?, z])
            };
            Ok(Line {
                layer: self.value(layer),
                start: point(10)?,
                end: point(11)?,
            })
        })
    }

    /// レコードの種類（グループ・コード 0 の値。前後の空白を除く）。先頭がグループ・コード 0 でなければ空
    pub fn kind(&self, record: &Record) -> &[u8] {
        match record.tags.first() {
            // グループ・コード 0 の値は変えられないので、元のバイトから読む
            Some(tag) if tag.code == 0 => self
                .src
                .get(tag.value.clone())
                .unwrap_or_default()
                .trim_ascii(),
            _ => &[],
        }
    }

    /// `EOF` の後に続くバイト列
    pub fn trailer(&self) -> &[u8] {
        self.src.get(self.trailer.clone()).unwrap_or_default()
    }
}

impl FileModel for DxfFile {
    fn record_count(&self) -> usize {
        self.records.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let record = self.records.get(index)?;
        Some(RecordInfo {
            kind: self.kind(record),
            bytes: (!record.edited).then_some(ByteRange {
                start: record.span.start as u64,
                end: record.span.end as u64,
            }),
        })
    }
}

impl Codec for Dxf {
    type File = DxfFile;

    const FORMAT: &'static str = "dxf";

    const SIGNATURES: &'static [Signature] = &[Signature::Lines {
        lines: &[b"0", b"SECTION"],
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<DxfFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        if src.starts_with(BINARY_SENTINEL) {
            cx.report(Diagnostic::new(codes::DXF_BINARY_UNSUPPORTED));
            return Err(Error::Unsupported);
        }
        let mut lines = Lines { src, pos: 0, no: 0 };
        let mut records: Vec<Record> = Vec::new();
        while let Some(code_line) = lines.next() {
            let Some(value_line) = lines.next() else {
                return Err(invalid(cx, &code_line));
            };
            let Some(code) = parse_code(src.get(code_line.content.clone()).unwrap_or_default())
            else {
                return Err(invalid(cx, &code_line));
            };
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let tag = Tag {
                code,
                span: code_line.start..value_line.next,
                value: value_line.content.clone(),
                edited: None,
            };
            match records.last_mut() {
                Some(record) if code != 0 => {
                    record.span.end = value_line.next;
                    record.tags.push(tag);
                }
                _ => {
                    cx.check(Kind::Records, 1)
                        .map_err(|stop| stopped(cx, stop))?;
                    records.push(Record {
                        span: code_line.start..value_line.next,
                        tags: vec![tag],
                        edited: false,
                    });
                }
            }
            let value = src.get(value_line.content).unwrap_or_default();
            if code == 0 && value.trim_ascii() == b"EOF" {
                break;
            }
        }
        Ok(DxfFile {
            src: src.to_vec(),
            records,
            trailer: lines.pos..src.len(),
        })
    }

    fn write(file: &DxfFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        for record in &file.records {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            if !record.edited {
                out.extend_from_slice(file.src.get(record.span.clone()).unwrap_or_default());
                continue;
            }
            for tag in &record.tags {
                let src = |range: Range<usize>| file.src.get(range).unwrap_or_default();
                out.extend_from_slice(src(tag.span.start..tag.value.start));
                out.extend_from_slice(file.value(tag));
                out.extend_from_slice(src(tag.value.end..tag.span.end));
            }
        }
        out.extend_from_slice(file.trailer());
        Ok(())
    }
}

/// LINE の型付きの見方
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line<'a> {
    /// 画層の名前（グループ・コード 8）。生のバイト列
    pub layer: &'a [u8],
    /// 始点（10・20・30）
    pub start: [f64; 3],
    /// 終点（11・21・31）
    pub end: [f64; 3],
}

/// 型付きの見方で値を読めなかった理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewError {
    /// このグループ・コードのタグがない
    Missing(i16),
    /// このグループ・コードの値が、有限の実数として読めない
    NotReal(i16),
}

/// DXF の変更をためるトランザクション。[`Transaction::commit`] で、まとめて確かめてから反映する。
/// 変更の操作が失敗したら、そのトランザクションは確定できない
#[derive(Debug)]
pub struct DxfTxn<'a> {
    file: &'a mut DxfFile,
    staged: BTreeMap<usize, Record>,
    rejected: Option<Diagnostic>,
}

impl DxfTxn<'_> {
    /// `record` 番目のレコードの、`tag` 番目のタグの値を、生のバイト列で置き換える。
    /// 改行を含む値と、グループ・コード 0 の値の変更は、タグとレコードの区切りが変わるので受け付けない
    pub fn set_value(&mut self, record: usize, tag: usize, value: &[u8]) -> Result<(), Error> {
        let staged = self.stage(record)?;
        let Some(found) = staged.tags.get_mut(tag) else {
            return Err(self.reject(
                Diagnostic::new(codes::DXF_TAG_MISSING)
                    .with("record", record as u64)
                    .with("tag", tag as u64),
            ));
        };
        let code = found.code;
        if code == 0 || value.iter().any(|&b| b == b'\r' || b == b'\n') {
            return Err(self.reject(on_tag(codes::DXF_VALUE_UNSAFE, record, code)));
        }
        found.edited = Some(value.into());
        Ok(())
    }

    /// `record` 番目のレコードの、最初の `code` のタグの値を、実数で置き換える。
    /// 読み直すと同じ値に戻る最短の 10 進の表記で書く。指数は使わない
    pub fn set_real(&mut self, record: usize, code: i16, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(self.reject(on_tag(codes::DXF_REAL_UNSUPPORTED, record, code)));
        }
        let staged = self.stage(record)?;
        let Some(tag) = staged.tags.iter().position(|t| t.code == code) else {
            return Err(self.reject(on_tag(codes::DXF_GROUP_CODE_MISSING, record, code)));
        };
        self.set_value(record, tag, &format_real(value))
    }

    /// 変更する前のレコードを写して、ためる
    fn stage(&mut self, record: usize) -> Result<&mut Record, Error> {
        if self.rejected.is_some() {
            return Err(Error::Rejected);
        }
        if !self.staged.contains_key(&record) {
            let Some(found) = self.file.records.get(record) else {
                return Err(self.reject(
                    Diagnostic::new(codes::DXF_RECORD_MISSING).with("record", record as u64),
                ));
            };
            self.staged.insert(record, found.clone());
        }
        self.staged.get_mut(&record).ok_or(Error::Rejected)
    }

    /// 最初の失敗を、確定のときに診断として返すために覚える
    fn reject(&mut self, diagnostic: Diagnostic) -> Error {
        self.rejected.get_or_insert(diagnostic);
        Error::Rejected
    }
}

impl Transaction for DxfTxn<'_> {
    fn commit(self, cx: &mut Cx) -> Result<(), Error> {
        if let Some(diagnostic) = self.rejected {
            cx.report(diagnostic);
            return Err(Error::Rejected);
        }
        // 予算を先にすべて確かめ、反映の途中では止めない
        cx.check(Kind::Work, self.staged.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        for (index, mut record) in self.staged {
            if let Some(target) = self.file.records.get_mut(index) {
                record.edited = true;
                *target = record;
            }
        }
        Ok(())
    }
}

impl Transact for DxfFile {
    type Txn<'a> = DxfTxn<'a>;

    fn transaction(&mut self) -> DxfTxn<'_> {
        DxfTxn {
            file: self,
            staged: BTreeMap::new(),
            rejected: None,
        }
    }
}

fn on_tag(code: Code, record: usize, group_code: i16) -> Diagnostic {
    Diagnostic::new(code)
        .with("record", record as u64)
        .with("code", i64::from(group_code))
        .at(Ref::at_record(record as u64))
}

/// 実数の値。前後の空白を除いて読み、有限でないものは受け付けない（推定。docs/formats/dxf.md）
fn parse_real(value: &[u8]) -> Option<f64> {
    let real: f64 = core::str::from_utf8(value.trim_ascii())
        .ok()?
        .parse()
        .ok()?;
    real.is_finite().then_some(real)
}

/// 有限の実数を、読み直すと同じ値に戻る最短の 10 進の表記にする。整数の値にも小数点を付ける
fn format_real(value: f64) -> Vec<u8> {
    let mut text = value.to_string();
    if !text.contains('.') {
        text.push_str(".0");
    }
    text.into_bytes()
}

fn stopped(cx: &mut Cx, stop: zumen_base::Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

fn invalid(cx: &mut Cx, line: &TextLine) -> Error {
    cx.report(
        Diagnostic::new(codes::DXF_TAG_INVALID)
            .with("line", line.no)
            .at(Ref::at_bytes(line.start as u64, line.next as u64))
            .cite(ASCII_FILES),
    );
    Error::Invalid
}

/// グループ・コードの行。前後の空白を許す。仕様が決めない所の扱いは推定で、`docs/formats/dxf.md` に記録した
fn parse_code(line: &[u8]) -> Option<i16> {
    core::str::from_utf8(line.trim_ascii()).ok()?.parse().ok()
}

/// 1 行。`content` は改行を除いた範囲、`next` は次の行の先頭
struct TextLine {
    no: u64,
    start: usize,
    content: Range<usize>,
    next: usize,
}

struct Lines<'a> {
    src: &'a [u8],
    pos: usize,
    no: u64,
}

impl Iterator for Lines<'_> {
    type Item = TextLine;

    fn next(&mut self) -> Option<TextLine> {
        let start = self.pos;
        let rest = self.src.get(start..).filter(|rest| !rest.is_empty())?;
        let (mut end, next) = match rest.iter().position(|&b| b == b'\n') {
            Some(n) => (start + n, start + n + 1),
            None => (self.src.len(), self.src.len()),
        };
        if end > start && self.src.get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        self.pos = next;
        self.no += 1;
        Some(TextLine {
            no: self.no,
            start,
            content: start..end,
            next,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_codes_allow_padding_and_signs() {
        assert_eq!(parse_code(b"  0"), Some(0));
        assert_eq!(parse_code(b"10 "), Some(10));
        assert_eq!(parse_code(b"-1"), Some(-1));
        assert_eq!(parse_code(b""), None);
        assert_eq!(parse_code(b"1 0"), None);
        assert_eq!(parse_code(b"99999"), None);
    }

    #[test]
    fn reals_round_trip_with_a_decimal_point() {
        for value in [0.0, -0.0, 1.0, 100.0, 0.1, -2.5, 1e20, 1e-7, f64::MAX] {
            let text = format_real(value);
            assert!(text.contains(&b'.'), "{value}");
            assert_eq!(parse_real(&text).map(f64::to_bits), Some(value.to_bits()));
        }
        assert_eq!(format_real(100.0), b"100.0");
        assert_eq!(parse_real(b" 1.0E+20 "), Some(1e20));
        assert_eq!(parse_real(b"inf"), None);
        assert_eq!(parse_real(b"NaN"), None);
        assert_eq!(parse_real(b"1,5"), None);
    }

    #[test]
    fn lines_strip_cr_lf_and_keep_the_last_line() {
        let src = b"a\r\nb\nc";
        let lines: Vec<_> = Lines { src, pos: 0, no: 0 }
            .map(|l| (l.no, l.content, l.next))
            .collect();
        assert_eq!(lines, vec![(1, 0..1, 3), (2, 3..4, 5), (3, 5..6, 6)]);
    }
}
