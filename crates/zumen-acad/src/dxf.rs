//! DXF（DXF Reference「About ASCII DXF Files」「Group Code Value Types」「Binary DXF Files」）。
//!
//! ファイルは、グループ・コードの行と値の行の組（タグ）が続いたもの。グループ・コード 0 のタグから、
//! 次のグループ・コード 0 のタグの前までを 1 つのレコードにする。`SECTION`・`ENDSEC` も、
//! 未知の種類のものも、同じようにレコードとして持つ。`EOF` の後のバイトは、読まずにそのまま持つ。
//!
//! レコードは元のバイト範囲を持ち、隣のレコードと隙間なく並ぶ。書くときは、元のバイトを並べる。
//! 改行（CR LF か LF）、グループ・コードの前の空白、値の文字コードは、元のバイトに残る。
//!
//! 変更は [`DxfTxn`] で行う。確定すると、変えたレコードだけを書き直す。書き直すときも、
//! タグのグループ・コードの行と改行は元のバイトを使い、値だけを差し替える。足したタグは、
//! グループ・コードを 3 桁に右寄せし、ファイルの最初の行と同じ改行で書く。
//!
//! バイナリの DXF は、タグを ASCII の表現にした文の上で同じように扱う（[`binary`]）。
//! レコードの範囲は元のバイナリのバイト範囲で、書くときは変えていないレコードの元のバイトを並べ、
//! 変えたレコードのタグをバイナリに符号化し直す。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Code, Cx, Diagnostic, Kind, Ref, SpecRef, codes};

use crate::db::ViewError;
mod binary;
mod objects;
mod version;
mod write_back;

use zumen_sdk::{ByteRange, Codec, Error, FileModel, RecordInfo, Signature, Transact, Transaction};

/// バイナリの DXF の先頭（DXF Reference「Binary DXF Files」）
const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

const ASCII_FILES: SpecRef = SpecRef {
    spec: "dxf_2026",
    section: "About ASCII DXF Files",
};

/// DXF のコーデック。ASCII とバイナリの DXF を読み書きする
#[derive(Clone, Copy, Debug)]
pub struct Dxf;

/// 読んだ DXF。元のバイト列と、その上のレコードの区切り。バイナリの DXF では、`src` は
/// タグを ASCII の表現にした文で、元のバイト列は `binary` が持つ
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DxfFile {
    src: Vec<u8>,
    records: Vec<Record>,
    trailer: Range<usize>,
    binary: Option<binary::Binary>,
}

/// グループ・コード 0 のタグから始まるタグの並び。先頭がグループ・コード 0 でないのは、
/// ファイルの最初のレコードだけ
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// 元のファイルのバイト範囲。バイナリの DXF では、元のバイナリの範囲
    span: Range<usize>,
    tags: Vec<Tag>,
    edited: bool,
}

/// グループ・コードと値の組
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    code: i16,
    /// グループ・コードの行の先頭から、値の行の改行の終わりまで。足したタグは空
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

    /// レコードの種類（グループ・コード 0 の値。前後の空白を除く）。先頭がグループ・コード 0 でなければ空
    pub fn kind<'a>(&'a self, record: &'a Record) -> &'a [u8] {
        match record.tags.first() {
            // グループ・コード 0 の値は変えられない。足したレコードでは、足した値を読む
            Some(tag) if tag.code == 0 => self.value(tag).trim_ascii(),
            _ => &[],
        }
    }

    /// バイナリの DXF か
    pub fn is_binary(&self) -> bool {
        self.binary.is_some()
    }

    /// `EOF` の後に続くバイト列。バイナリの DXF では空
    pub fn trailer(&self) -> &[u8] {
        self.src.get(self.trailer.clone()).unwrap_or_default()
    }

    /// 足したタグに使う改行。最初の行が CR LF なら CR LF、ほかは LF（推定。docs/formats/dxf.md）
    fn newline(&self) -> &'static [u8] {
        match self.src.iter().position(|&b| b == b'\n') {
            Some(n) if n > 0 && self.src.get(n - 1) == Some(&b'\r') => b"\r\n",
            _ => b"\n",
        }
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

    const SIGNATURES: &'static [Signature] = &[
        Signature::Lines {
            lines: &[b"0", b"SECTION"],
        },
        Signature::Bytes {
            offset: 0,
            bytes: BINARY_SENTINEL,
        },
    ];

    fn read(src: &[u8], cx: &mut Cx) -> Result<DxfFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        if src.starts_with(BINARY_SENTINEL) {
            let decoded = binary::read(src, cx)?;
            let end = decoded.text.len();
            return Ok(DxfFile {
                src: decoded.text,
                records: decoded.records,
                trailer: end..end,
                binary: Some(decoded.binary),
            });
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
            // 止めたら、そのタグのバイト範囲を場所に入れる（決定 0119）
            let place = Ref::at_bytes(code_line.start as u64, value_line.next as u64);
            cx.check(Kind::Work, 1)
                .map_err(|stop| stopped_at(cx, stop, place.clone()))?;
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
                        .map_err(|stop| stopped_at(cx, stop, place))?;
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
            binary: None,
        })
    }

    fn write(file: &DxfFile, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error> {
        if let Some(binary) = &file.binary {
            return write_binary(file, binary, out, cx);
        }
        for record in &file.records {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            if !record.edited {
                out.extend_from_slice(file.src.get(record.span.clone()).unwrap_or_default());
                continue;
            }
            for tag in &record.tags {
                if tag.span.is_empty() {
                    let newline = file.newline();
                    out.extend_from_slice(format!("{:>3}", tag.code).as_bytes());
                    out.extend_from_slice(newline);
                    out.extend_from_slice(file.value(tag));
                    out.extend_from_slice(newline);
                    continue;
                }
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

/// バイナリの DXF を書く。値は変更のときに確かめたので、符号化できる
fn write_binary(
    file: &DxfFile,
    binary: &binary::Binary,
    out: &mut Vec<u8>,
    cx: &mut Cx,
) -> Result<(), Error> {
    out.extend_from_slice(binary.sentinel());
    for record in &file.records {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if !record.edited {
            out.extend_from_slice(binary.src.get(record.span.clone()).unwrap_or_default());
            continue;
        }
        for tag in &record.tags {
            if binary.encode(tag.code, file.value(tag), out).is_none() {
                return Err(Error::Invalid);
            }
        }
    }
    out.extend_from_slice(binary.src.get(binary.trailer.clone()).unwrap_or_default());
    Ok(())
}

/// DXF の変更をためるトランザクション。[`Transaction::commit`] で、まとめて確かめてから反映する。
/// 変更の操作が失敗したら、そのトランザクションは確定できない。レコードの位置は、
/// 確定するまで元のファイルの位置で名指しする
#[derive(Debug)]
pub struct DxfTxn<'a> {
    file: &'a mut DxfFile,
    staged: BTreeMap<usize, Record>,
    /// 元の位置の前に足すレコード
    inserted: BTreeMap<usize, Vec<Record>>,
    removed: BTreeSet<usize>,
    rejected: Option<Diagnostic>,
}

impl DxfTxn<'_> {
    /// `record` 番目のレコードの、`tag` 番目のタグの値を、生のバイト列で置き換える。
    /// 改行を含む値と、グループ・コード 0 の値の変更は、タグとレコードの区切りが変わるので受け付けない
    pub fn set_value(&mut self, record: usize, tag: usize, value: &[u8]) -> Result<(), Error> {
        let staged = self.stage(record)?;
        let Some(found) = staged.tags.get(tag) else {
            return Err(self.reject(missing_tag(record, tag)));
        };
        let code = found.code;
        if code == 0 || has_newline(value) {
            return Err(self.reject(on_tag(codes::DXF_VALUE_UNSAFE, record, code)));
        }
        if !encodable(self.file, code, value) {
            return Err(self.reject(on_tag(codes::DXF_BINARY_VALUE_UNSUPPORTED, record, code)));
        }
        let staged = self.staged.get_mut(&record);
        if let Some(found) = staged.and_then(|r| r.tags.get_mut(tag)) {
            found.edited = Some(value.into());
        }
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

    /// `record` 番目のレコードの `before` 番目のタグの前に、タグを足す。`before` がタグの数なら最後に足す。
    /// グループ・コード 0 のタグと改行を含む値は、タグとレコードの区切りが変わるので受け付けない
    pub fn insert_tag(
        &mut self,
        record: usize,
        before: usize,
        code: i16,
        value: &[u8],
    ) -> Result<(), Error> {
        let staged = self.stage(record)?;
        if before == 0 || before > staged.tags.len() {
            return Err(self.reject(missing_tag(record, before)));
        }
        if code == 0 || has_newline(value) {
            return Err(self.reject(on_tag(codes::DXF_VALUE_UNSAFE, record, code)));
        }
        if !encodable(self.file, code, value) {
            return Err(self.reject(on_tag(codes::DXF_BINARY_VALUE_UNSUPPORTED, record, code)));
        }
        if let Some(staged) = self.staged.get_mut(&record) {
            staged.tags.insert(before, fresh_tag(code, value));
        }
        Ok(())
    }

    /// `record` 番目のレコードの、`tag` 番目のタグを除く。グループ・コード 0 のタグは除けない
    pub fn remove_tag(&mut self, record: usize, tag: usize) -> Result<(), Error> {
        let staged = self.stage(record)?;
        let Some(found) = staged.tags.get(tag) else {
            return Err(self.reject(missing_tag(record, tag)));
        };
        if tag == 0 {
            let code = found.code;
            return Err(self.reject(on_tag(codes::DXF_VALUE_UNSAFE, record, code)));
        }
        staged.tags.remove(tag);
        Ok(())
    }

    /// 元の `before` 番目のレコードの前に、タグの並び `tags` のレコードを足す。
    /// 最初のタグはグループ・コード 0、ほかは 0 以外でなければならない。構造を区切る
    /// `SECTION`・`ENDSEC`・`EOF` のレコードは足せない。ハンドル（5）は呼び出し側が決める（[`Self::new_handle`]）
    pub fn insert_record(&mut self, before: usize, tags: &[(i16, &[u8])]) -> Result<(), Error> {
        self.check_open()?;
        if before >= self.file.records.len() {
            return Err(self.reject(missing_record(before)));
        }
        let unsafe_tag = tags
            .iter()
            .enumerate()
            .find(|(i, (code, value))| (*i == 0) != (*code == 0) || has_newline(value));
        match (tags.first(), unsafe_tag) {
            (_, Some((_, &(code, _)))) => {
                return Err(self.reject(on_tag(codes::DXF_VALUE_UNSAFE, before, code)));
            }
            (None, None) => {
                return Err(self.reject(on_tag(codes::DXF_VALUE_UNSAFE, before, 0)));
            }
            (Some((_, kind)), None) if structural(kind.trim_ascii()) => {
                return Err(self.reject(unsafe_record(before)));
            }
            _ => {}
        }
        let file = &*self.file;
        if let Some(&(code, _)) = tags.iter().find(|(c, v)| !encodable(file, *c, v)) {
            return Err(self.reject(on_tag(codes::DXF_BINARY_VALUE_UNSUPPORTED, before, code)));
        }
        let record = Record {
            span: 0..0,
            tags: tags
                .iter()
                .map(|&(code, value)| fresh_tag(code, value))
                .collect(),
            edited: true,
        };
        self.inserted.entry(before).or_default().push(record);
        Ok(())
    }

    /// 元の `record` 番目のレコードを除く。構造を区切る `SECTION`・`ENDSEC`・`EOF` のレコードは除けない
    pub fn remove_record(&mut self, record: usize) -> Result<(), Error> {
        self.check_open()?;
        let Some(found) = self.file.records.get(record) else {
            return Err(self.reject(missing_record(record)));
        };
        if self.removed.contains(&record) {
            return Err(self.reject(missing_record(record)));
        }
        if structural(self.file.kind(found)) {
            return Err(self.reject(unsafe_record(record)));
        }
        self.staged.remove(&record);
        self.removed.insert(record);
        Ok(())
    }

    /// 見出しの `$HANDSEED` の値を新しいハンドルとして返し、`$HANDSEED` を 1 つ進める
    /// （DXF Reference「HEADER Section Group Codes」の $HANDSEED）。値は 16 進の文字列で、大文字で書く
    pub fn new_handle(&mut self) -> Result<u64, Error> {
        self.check_open()?;
        let header = self.file.records.iter().position(|r| {
            self.file.kind(r) == b"SECTION"
                && r.tags
                    .get(1)
                    .is_some_and(|t| t.code == 2 && self.file.value(t).trim_ascii() == b"HEADER")
        });
        let Some(header) = header else {
            return Err(self.reject(Diagnostic::new(codes::DXF_HANDLE_SEED_MISSING)));
        };
        let file = &*self.file;
        let Some(found) = self.staged.get(&header).or(file.records.get(header)) else {
            return Err(Error::Rejected);
        };
        // `9 $HANDSEED` の次の、グループ・コード 5 のタグ
        let seed = found.tags.windows(2).position(|pair| {
            matches!(pair, [name, value] if name.code == 9 && value.code == 5
                && file.value(name).trim_ascii() == b"$HANDSEED")
        });
        let read = |tag: usize| {
            let text = core::str::from_utf8(file.value(found.tags.get(tag)?).trim_ascii()).ok()?;
            u64::from_str_radix(text, 16).ok().filter(|&h| h < u64::MAX)
        };
        let Some((tag, handle)) = seed.and_then(|i| Some((i + 1, read(i + 1)?))) else {
            return Err(self.reject(Diagnostic::new(codes::DXF_HANDLE_SEED_MISSING)));
        };
        self.set_value(header, tag, format!("{:X}", handle + 1).as_bytes())?;
        Ok(handle)
    }

    /// 失敗したトランザクションは、ほかの変更も受け付けない
    fn check_open(&self) -> Result<(), Error> {
        match self.rejected {
            Some(_) => Err(Error::Rejected),
            None => Ok(()),
        }
    }

    /// 変更する前のレコードを写して、ためる
    fn stage(&mut self, record: usize) -> Result<&mut Record, Error> {
        self.check_open()?;
        if self.removed.contains(&record) {
            return Err(self.reject(missing_record(record)));
        }
        if !self.staged.contains_key(&record) {
            let Some(found) = self.file.records.get(record) else {
                return Err(self.reject(missing_record(record)));
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
        let inserted: usize = self.inserted.values().map(Vec::len).sum();
        let work = self.staged.len() + inserted + self.removed.len();
        cx.check(Kind::Work, work as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let (mut staged, mut added) = (self.staged, self.inserted);
        let old = core::mem::take(&mut self.file.records);
        for (index, record) in old.into_iter().enumerate() {
            self.file
                .records
                .extend(added.remove(&index).unwrap_or_default());
            if self.removed.contains(&index) {
                continue;
            }
            self.file.records.push(match staged.remove(&index) {
                Some(edited) => Record {
                    edited: true,
                    ..edited
                },
                None => record,
            });
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
            inserted: BTreeMap::new(),
            removed: BTreeSet::new(),
            rejected: None,
        }
    }
}

fn missing_record(record: usize) -> Diagnostic {
    Diagnostic::new(codes::DXF_RECORD_MISSING).with("record", record as u64)
}

fn missing_tag(record: usize, tag: usize) -> Diagnostic {
    Diagnostic::new(codes::DXF_TAG_MISSING)
        .with("record", record as u64)
        .with("tag", tag as u64)
}

fn unsafe_record(record: usize) -> Diagnostic {
    Diagnostic::new(codes::DXF_RECORD_UNSAFE)
        .with("record", record as u64)
        .at(Ref::at_record(record as u64))
}

/// 足したり除いたりすると、セクションの区切りが変わるレコードの種類
fn structural(kind: &[u8]) -> bool {
    matches!(kind, b"SECTION" | b"ENDSEC" | b"EOF")
}

/// バイナリの DXF なら、値をグループ・コードの型のバイト列にできるか。ASCII の DXF では常に真
fn encodable(file: &DxfFile, code: i16, value: &[u8]) -> bool {
    file.binary
        .as_ref()
        .is_none_or(|binary| binary.encodable(code, value))
}

fn has_newline(value: &[u8]) -> bool {
    value.iter().any(|&b| b == b'\r' || b == b'\n')
}

/// 足すタグ。元のバイトを持たないので、範囲は空にする
fn fresh_tag(code: i16, value: &[u8]) -> Tag {
    Tag {
        code,
        span: 0..0,
        value: 0..0,
        edited: Some(value.into()),
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

/// 止めた理由を、扱っていた場所と一緒に報告する（決定 0119）
fn stopped_at(cx: &mut Cx, stop: zumen_base::Stop, place: Ref) -> Error {
    cx.report(stop.to_diagnostic().at(place));
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
