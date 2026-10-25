//! ASCII の DXF（DXF Reference「About ASCII DXF Files」「Group Code Value Types」）。
//!
//! ファイルは、グループ・コードの行と値の行の組（タグ）が続いたもの。グループ・コード 0 のタグから、
//! 次のグループ・コード 0 のタグの前までを 1 つのレコードにする。`SECTION`・`ENDSEC` も、
//! 未知の種類のものも、同じようにレコードとして持つ。`EOF` の後のバイトは、読まずにそのまま持つ。
//!
//! レコードは元のバイト範囲を持ち、隣のレコードと隙間なく並ぶ。書くときは、元のバイトを並べる。
//! 改行（CR LF か LF）、グループ・コードの前の空白、値の文字コードは、元のバイトに残る。

use core::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, SpecRef, codes};
use zumen_sdk::{ByteRange, Codec, Error, FileModel, RecordInfo, Signature};

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
}

/// グループ・コードと値の組
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    code: i16,
    value: Range<usize>,
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
    pub fn value(&self, tag: &Tag) -> &[u8] {
        self.src.get(tag.value.clone()).unwrap_or_default()
    }

    /// レコードの種類（グループ・コード 0 の値。前後の空白を除く）。先頭がグループ・コード 0 でなければ空
    pub fn kind(&self, record: &Record) -> &[u8] {
        match record.tags.first() {
            Some(tag) if tag.code == 0 => self.value(tag).trim_ascii(),
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
            bytes: Some(ByteRange {
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
                value: value_line.content.clone(),
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
            out.extend_from_slice(file.src.get(record.span.clone()).unwrap_or_default());
        }
        out.extend_from_slice(file.trailer());
        Ok(())
    }
}

fn stopped(cx: &mut Cx, stop: zumen_base::Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

fn invalid(cx: &mut Cx, line: &Line) -> Error {
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
struct Line {
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
    type Item = Line;

    fn next(&mut self) -> Option<Line> {
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
        Some(Line {
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
    fn lines_strip_cr_lf_and_keep_the_last_line() {
        let src = b"a\r\nb\nc";
        let lines: Vec<_> = Lines { src, pos: 0, no: 0 }
            .map(|l| (l.no, l.content, l.next))
            .collect();
        assert_eq!(lines, vec![(1, 0..1, 3), (2, 3..4, 5), (3, 5..6, 6)]);
    }
}
