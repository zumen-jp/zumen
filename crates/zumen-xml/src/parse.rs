//! XML 1.0 と XML の名前空間 1.0 の文書を読む（決定 0209）。
//!
//! 入れ子は再帰でなく、開いた要素の並びでたどる。要素を予算のレコード、入れ子を深さで数える（決定 0010）。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Cx, Error, Stop};

use crate::{Attribute, Document, Element, Encoding, Name, Namespace, Node, text, utf16};

/// 読めない理由
enum Fail {
    /// XML の形になっていない。範囲はその場所
    Invalid(Range<usize>),
    /// 予算か中断の合図で止めた。範囲は扱っていた場所
    Stop(Stop, Range<usize>),
}

/// XML の文書を読む。XML の形になっていなければ `xml.document.invalid`、UTF-8・Shift_JIS・UTF-16 のほかの文字コードなら
/// `xml.encoding.unsupported` を報告して読まない。文書型宣言は処理せず `xml.dtd.dropped` を報告する
pub fn read(src: &[u8], cx: &mut Cx) -> Result<Document, Error> {
    cx.check(Kind::InputBytes, src.len() as u64)
        .map_err(|stop| stopped(cx, stop, None))?;
    if utf16::is_utf32(src) {
        return Err(unsupported(cx, "UTF-32"));
    }
    if let Some(detection) = utf16::detect(src) {
        return read_utf16(src, detection, cx);
    }
    let mut p = Parser::new(src);
    let encoding = p.encoding(cx)?;
    match p.document(cx) {
        Ok(()) => Ok(Document {
            src: src.to_vec(),
            encoding,
            elements: p.elements,
            doctype: p.doctype,
            decoded: None,
        }),
        Err(Fail::Invalid(range)) => {
            cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(at(range)));
            Err(Error::Invalid)
        }
        Err(Fail::Stop(stop, range)) => Err(stopped(cx, stop, Some(range))),
    }
}

fn read_utf16(src: &[u8], detection: utf16::Detection, cx: &mut Cx) -> Result<Document, Error> {
    let decoded = utf16::Decoded::read(src, detection, cx)?;
    let mut p = Parser::new(&decoded.bytes);
    p.mapping = Some(&decoded);
    let encoding = p.encoding_expected(cx, Some(detection))?;
    match p.document(cx) {
        Ok(()) => {}
        Err(Fail::Invalid(range)) => {
            cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(p.place(range)));
            return Err(Error::Invalid);
        }
        Err(Fail::Stop(stop, range)) => {
            return Err(stopped(cx, stop, Some(decoded.original_range(range))));
        }
    }
    let mut elements = p.elements;
    let mut doctype = p.doctype;
    // 全公開範囲を原本へ戻す。要素番号と親子の順は動かさない。
    for element in &mut elements {
        let span = element.span.clone();
        map_charge(cx, &decoded, span.clone())?;
        element.span = decoded.original_range(span.clone());
        map_name(&mut element.name, &decoded);
        for attribute in &mut element.attributes {
            map_charge(cx, &decoded, attribute.name.raw.clone())?;
            map_name(&mut attribute.name, &decoded);
            attribute.value = decoded.original_range(attribute.value.clone());
        }
        for node in &mut element.children {
            let range = match node {
                Node::Text(range) | Node::CData(range) => range.clone(),
                Node::Element(_) => span.clone(),
            };
            map_charge(cx, &decoded, range)?;
            if let Node::Text(range) | Node::CData(range) = node {
                *range = decoded.original_range(range.clone());
            }
        }
    }
    if let Some(range) = &mut doctype {
        map_charge(cx, &decoded, range.clone())?;
        *range = decoded.original_range(range.clone());
    }
    Ok(Document {
        src: src.to_vec(),
        encoding,
        elements,
        doctype,
        decoded: Some(decoded),
    })
}

fn map_charge(cx: &mut Cx, decoded: &utf16::Decoded, range: Range<usize>) -> Result<(), Error> {
    cx.check(Kind::Work, 1)
        .map_err(|stop| stopped(cx, stop, Some(decoded.original_range(range))))
}

fn map_name(name: &mut Name, decoded: &utf16::Decoded) {
    name.raw = decoded.original_range(name.raw.clone());
    name.local = decoded.original_range(name.local..name.local).start;
    name.prefix_end = name
        .prefix_end
        .map(|end| decoded.original_range(end..end).start);
    if let Some(Namespace::Declared(range)) = &mut name.namespace {
        *range = decoded.original_range(range.clone());
    }
}

fn at(range: Range<usize>) -> Ref {
    Ref::at_bytes(range.start as u64, range.end as u64)
}

/// 止めた理由を報告する。扱っていたバイト範囲があれば、場所にする（決定 0119）
fn stopped(cx: &mut Cx, stop: Stop, place: Option<Range<usize>>) -> Error {
    let diagnostic = stop.to_diagnostic();
    cx.report(match place {
        Some(range) => diagnostic.at(at(range)),
        None => diagnostic,
    });
    Error::Stopped(stop)
}

fn unsupported(cx: &mut Cx, encoding: &str) -> Error {
    cx.report(Diagnostic::new(codes::XML_ENCODING_UNSUPPORTED).with("encoding", encoding));
    Error::Unsupported
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

/// 名前に使えないバイト。0x80 以上は、どの文字コードでも名前の文字として通す
fn is_delimiter(byte: u8) -> bool {
    is_space(byte)
        || matches!(
            byte,
            b'<' | b'>' | b'/' | b'=' | b'!' | b'?' | b'"' | b'\'' | b'&' | b';' | b'[' | b']'
        )
}

fn find(src: &[u8], pattern: &[u8], from: usize) -> Option<usize> {
    src.get(from..)?
        .windows(pattern.len())
        .position(|window| window == pattern)
        .map(|at| from + at)
}

/// `src` の `range` の中で、`byte` の最初の位置。見たバイトの数を `seen` に足す
fn find_in(src: &[u8], byte: u8, range: Range<usize>, seen: &mut usize) -> Option<usize> {
    let from = range.start;
    let rest = src.get(range).unwrap_or_default();
    let found = rest.iter().position(|b| *b == byte);
    *seen += found.map_or(rest.len(), |at| at + 1);
    found.map(|at| from + at)
}

/// XML 宣言の擬似属性（`version="1.0" encoding="…"`）の名前と値。形が違えば `None`
fn pseudo_attributes(declaration: &[u8]) -> Option<Vec<(&[u8], &[u8])>> {
    let mut pairs = Vec::new();
    let mut rest = declaration;
    loop {
        let trimmed = rest.trim_ascii_start();
        if trimmed.is_empty() {
            return Some(pairs);
        }
        if trimmed.len() == rest.len() && !pairs.is_empty() {
            return None;
        }
        let name_end = trimmed.iter().position(|b| *b == b'=' || is_space(*b))?;
        let (name, after) = trimmed.split_at(name_end);
        let after = after
            .trim_ascii_start()
            .strip_prefix(b"=")?
            .trim_ascii_start();
        let (&quote, after) = after.split_first()?;
        if !matches!(quote, b'"' | b'\'') {
            return None;
        }
        let end = after.iter().position(|b| *b == quote)?;
        let (value, after) = after.split_at(end);
        pairs.push((name, value));
        rest = after.get(1..).unwrap_or_default();
    }
}

/// 名前空間の宣言の接頭辞。既定の名前空間は `None`
type Prefix<'s> = Option<&'s [u8]>;

/// 名前空間の宣言の値の範囲。`xmlns=""` は `None`
type Declared = Option<Range<usize>>;

struct Parser<'s> {
    src: &'s [u8],
    mapping: Option<&'s utf16::Decoded>,
    pos: usize,
    elements: Vec<Element>,
    /// 開いている要素の名前空間の宣言を、接頭辞ごとに積む。内側の宣言が後。名前を引く手間は、宣言の数に比例せず、
    /// 接頭辞の種類の数の対数になる（#721）
    namespaces: BTreeMap<Prefix<'s>, Vec<Declared>>,
    /// 開いている要素の名前空間の宣言の接頭辞。内側の宣言が後
    scopes: Vec<Prefix<'s>>,
    /// 開いている要素ごとの、`scopes` の長さ。閉じるときに戻す
    marks: Vec<usize>,
    doctype: Option<Range<usize>>,
}

impl<'s> Parser<'s> {
    fn new(src: &'s [u8]) -> Self {
        Self {
            src,
            mapping: None,
            pos: 0,
            elements: Vec::new(),
            namespaces: BTreeMap::new(),
            scopes: Vec::new(),
            marks: Vec::new(),
            doctype: None,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn place(&self, range: Range<usize>) -> Ref {
        at(match self.mapping {
            Some(decoded) => decoded.original_range(range),
            None => range,
        })
    }

    fn starts(&self, pattern: &[u8]) -> bool {
        self.src
            .get(self.pos..)
            .is_some_and(|rest| rest.starts_with(pattern))
    }

    fn bytes(&self, range: Range<usize>) -> &'s [u8] {
        self.src.get(range).unwrap_or_default()
    }

    /// 名前空間を宣言する
    fn declare(&mut self, prefix: Prefix<'s>, value: Declared) {
        self.namespaces.entry(prefix).or_default().push(value);
        self.scopes.push(prefix);
    }

    /// `scopes` を `mark` の長さに戻し、そのあとの宣言を外す
    fn unwind(&mut self, mark: usize) {
        while self.scopes.len() > mark {
            let Some(prefix) = self.scopes.pop() else {
                break;
            };
            if let Some(stack) = self.namespaces.get_mut(&prefix) {
                stack.pop();
                if stack.is_empty() {
                    self.namespaces.remove(&prefix);
                }
            }
        }
    }

    /// 接頭辞のいちばん内側の宣言。宣言がなければ `None`
    fn declared(&self, prefix: Prefix<'s>) -> Option<&Declared> {
        self.namespaces.get(&prefix).and_then(|stack| stack.last())
    }

    /// 空白を飛ばす。飛ばしたら真
    fn skip_space(&mut self) -> bool {
        let start = self.pos;
        while self.peek().is_some_and(is_space) {
            self.pos += 1;
        }
        self.pos > start
    }

    fn work(&self, cx: &mut Cx, amount: u64, start: usize) -> Result<(), Fail> {
        cx.check(Kind::Work, amount)
            .map_err(|stop| Fail::Stop(stop, start..self.pos))
    }

    /// バイト順マークと XML 宣言から、文字コードを決める
    fn encoding(&mut self, cx: &mut Cx) -> Result<Encoding, Error> {
        self.encoding_expected(cx, None)
    }

    fn encoding_expected(
        &mut self,
        cx: &mut Cx,
        expected: Option<utf16::Detection>,
    ) -> Result<Encoding, Error> {
        let bom = self.starts(&[0xef, 0xbb, 0xbf]);
        if bom {
            self.pos = 3;
        } else if matches!(
            self.src,
            [0xfe, 0xff, ..] | [0xff, 0xfe, ..] | [0, _, ..] | [_, 0, ..]
        ) {
            return Err(unsupported(cx, "UTF-16"));
        }
        if !(self.starts(b"<?xml") && self.src.get(self.pos + 5).copied().is_some_and(is_space)) {
            if let Some(expected) = expected {
                if !expected.bom {
                    cx.report(
                        Diagnostic::new(codes::XML_DOCUMENT_INVALID)
                            .at(self.place(0..self.src.len().min(1))),
                    );
                    return Err(Error::Invalid);
                }
                return Ok(match expected.order {
                    utf16::Order::Little => Encoding::Utf16Le,
                    utf16::Order::Big => Encoding::Utf16Be,
                });
            }
            return Ok(Encoding::Utf8);
        }
        let start = self.pos;
        let Some(end) = find(self.src, b"?>", start) else {
            cx.report(
                Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(self.place(start..self.src.len())),
            );
            return Err(Error::Invalid);
        };
        let src = self.src;
        let declaration = src.get(start + 5..end).unwrap_or_default();
        let Some(pairs) = pseudo_attributes(declaration) else {
            cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(self.place(start..end + 2)));
            return Err(Error::Invalid);
        };
        if pairs.first().map(|(name, _)| *name) != Some(&b"version"[..]) {
            cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(self.place(start..end + 2)));
            return Err(Error::Invalid);
        }
        self.pos = end + 2;
        if let Some(expected) = expected {
            let label = pairs
                .iter()
                .find(|(name, _)| *name == b"encoding")
                .map(|(_, label)| *label);
            // XML 1.0 4.3.3 と RFC 2781 3.3。generic UTF-16 は BOM、LE/BE ラベルは BOM なし。
            let valid = if expected.bom {
                label.is_none_or(|label| label.eq_ignore_ascii_case(b"UTF-16"))
            } else {
                label.is_some_and(|label| {
                    label.eq_ignore_ascii_case(match expected.order {
                        utf16::Order::Little => b"UTF-16LE",
                        utf16::Order::Big => b"UTF-16BE",
                    })
                })
            };
            if !valid {
                cx.report(
                    Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(self.place(start..end + 2)),
                );
                return Err(Error::Invalid);
            }
            return Ok(match expected.order {
                utf16::Order::Little => Encoding::Utf16Le,
                utf16::Order::Big => Encoding::Utf16Be,
            });
        }
        let Some((_, label)) = pairs.iter().find(|(name, _)| *name == b"encoding") else {
            return Ok(Encoding::Utf8);
        };
        let found = encoding_rs::Encoding::for_label(label);
        let label = String::from_utf8_lossy(label);
        if found == Some(encoding_rs::UTF_8) {
            Ok(Encoding::Utf8)
        } else if found == Some(encoding_rs::SHIFT_JIS) && !bom {
            Ok(Encoding::ShiftJis)
        } else {
            Err(unsupported(cx, &label))
        }
    }

    /// 文書。ルートの要素の前と後には、空白・注記・処理命令と、前に 1 つの文書型宣言だけを置ける
    fn document(&mut self, cx: &mut Cx) -> Result<(), Fail> {
        let mut root = false;
        loop {
            self.skip_space();
            let start = self.pos;
            if self.pos >= self.src.len() {
                return if root {
                    Ok(())
                } else {
                    Err(Fail::Invalid(start..start))
                };
            }
            self.work(cx, 1, start)?;
            if self.starts(b"<?") {
                self.skip_to(b"?>", start)?;
            } else if self.starts(b"<!--") {
                self.skip_to(b"-->", start)?;
            } else if !root && self.doctype.is_none() && self.starts(b"<!DOCTYPE") {
                self.doctype(cx)?;
            } else if !root && self.peek() == Some(b'<') {
                self.tree(cx)?;
                root = true;
            } else {
                return Err(Fail::Invalid(start..self.src.len()));
            }
        }
    }

    /// `pattern` の後まで飛ばす。なければ、`start` から終わりまでを壊れているとする
    fn skip_to(&mut self, pattern: &[u8], start: usize) -> Result<(), Fail> {
        let end =
            find(self.src, pattern, self.pos + 2).ok_or(Fail::Invalid(start..self.src.len()))?;
        self.pos = end + pattern.len();
        Ok(())
    }

    /// 文書型宣言を、内部の部分集合ごと飛ばす。中は処理しない
    fn doctype(&mut self, cx: &mut Cx) -> Result<(), Fail> {
        let start = self.pos;
        self.pos += b"<!DOCTYPE".len();
        let mut depth = 0usize;
        loop {
            let byte = self.peek().ok_or(Fail::Invalid(start..self.src.len()))?;
            match byte {
                b'"' | b'\'' => {
                    let end = self
                        .src
                        .get(self.pos + 1..)
                        .and_then(|rest| rest.iter().position(|b| *b == byte))
                        .ok_or(Fail::Invalid(start..self.src.len()))?;
                    self.pos += end + 2;
                    continue;
                }
                b'<' if self.starts(b"<!--") => {
                    self.skip_to(b"-->", start)?;
                    continue;
                }
                b'[' => depth += 1,
                b']' => depth = depth.checked_sub(1).ok_or(Fail::Invalid(start..self.pos))?,
                b'>' if depth == 0 => break,
                _ => {}
            }
            self.pos += 1;
        }
        self.pos += 1;
        self.doctype = Some(start..self.pos);
        cx.report(Diagnostic::new(codes::XML_DTD_DROPPED).at(self.place(start..self.pos)));
        Ok(())
    }

    /// 名前。`:` で接頭辞と分け、接頭辞を除いた名前の始まりを返す
    fn name(&mut self, cx: &mut Cx) -> Result<(Range<usize>, usize), Fail> {
        let start = self.pos;
        while self.peek().is_some_and(|b| !is_delimiter(b)) {
            self.pos += 1;
        }
        let raw = start..self.pos;
        let bytes = self.bytes(raw.clone());
        if self.mapping.is_some() {
            self.work(cx, raw.len() as u64, start)?;
            if !utf16::valid_name(bytes) {
                return Err(Fail::Invalid(raw));
            }
        }
        let first = bytes.first().copied();
        if first.is_none_or(|b| b.is_ascii_digit() || matches!(b, b'-' | b'.' | b':')) {
            return Err(Fail::Invalid(start..self.pos.max(start + 1)));
        }
        let colons: Vec<usize> = bytes
            .iter()
            .enumerate()
            .filter_map(|(at, b)| (*b == b':').then_some(at))
            .collect();
        match colons.as_slice() {
            [] => Ok((raw, start)),
            [colon] if start + colon + 1 < self.pos => Ok((raw, start + colon + 1)),
            _ => Err(Fail::Invalid(raw)),
        }
    }

    /// 文字か属性の値の参照を確かめる。DTD で宣言する実体の参照は、報告して残す。`&` と `;` は `range` の中だけで
    /// 探すので、手間は範囲の長さに比例する（#721）。見たバイトの数を返す
    fn references(&self, range: Range<usize>, cx: &mut Cx) -> Result<usize, Fail> {
        let mut seen = 0;
        let mut at = range.start;
        while let Some(amp) = find_in(self.src, b'&', at..range.end, &mut seen) {
            let end = find_in(self.src, b';', amp + 1..range.end, &mut seen)
                .ok_or(Fail::Invalid(amp..range.end))?;
            let name = core::str::from_utf8(self.bytes(amp + 1..end))
                .map_err(|_| Fail::Invalid(amp..end + 1))?;
            if text::reference(name).is_none() {
                let valid = name
                    .bytes()
                    .next()
                    .is_some_and(|b| !b.is_ascii_digit() && !matches!(b, b'#' | b'-' | b'.'))
                    && name.bytes().all(|b| !is_delimiter(b));
                if !valid {
                    return Err(Fail::Invalid(amp..end + 1));
                }
                cx.report(
                    Diagnostic::new(codes::XML_ENTITY_UNSUPPORTED)
                        .with("name", name)
                        .at(self.place(amp..end + 1)),
                );
            }
            at = end + 1;
        }
        Ok(seen)
    }

    /// 接頭辞の名前空間。`element` なら、接頭辞のない名前は既定の名前空間に入る
    fn resolve(
        &self,
        raw: &Range<usize>,
        local: usize,
        element: bool,
        cx: &mut Cx,
    ) -> Option<Namespace> {
        if local == raw.start {
            if element {
                return self
                    .declared(None)
                    .and_then(|value| value.clone().map(Namespace::Declared));
            }
            return (self.bytes(raw.clone()) == b"xmlns").then_some(Namespace::Xmlns);
        }
        let prefix = self.bytes(raw.start..local - 1);
        match prefix {
            b"xml" => return Some(Namespace::Xml),
            b"xmlns" => return Some(Namespace::Xmlns),
            _ => {}
        }
        match self.declared(Some(prefix)) {
            Some(value) => value.clone().map(Namespace::Declared),
            None => {
                cx.report(
                    Diagnostic::new(codes::XML_NAMESPACE_MISSING)
                        .with("prefix", String::from_utf8_lossy(prefix).into_owned())
                        .at(self.place(raw.clone())),
                );
                None
            }
        }
    }

    /// ルートの要素と、その中を読む
    fn tree(&mut self, cx: &mut Cx) -> Result<(), Fail> {
        let mut open = Vec::new();
        if self.open(None, cx)? {
            open.push(0);
        }
        while let Some(&current) = open.last() {
            let start = self.pos;
            self.work(cx, 1, start)?;
            let Some(byte) = self.peek() else {
                let from = self.elements.get(current).map_or(start, |e| e.span.start);
                return Err(Fail::Invalid(from..start));
            };
            if byte != b'<' {
                let end = find(self.src, b"<", start).unwrap_or(self.src.len());
                self.pos = end;
                self.references(start..end, cx)?;
                self.child(current, Node::Text(start..end));
            } else if self.starts(b"</") {
                self.pos += 2;
                let (raw, _) = self.name(cx)?;
                self.skip_space();
                let expected = self
                    .elements
                    .get(current)
                    .map(|e| e.name.raw.clone())
                    .unwrap_or_default();
                if self.peek() != Some(b'>') || self.bytes(raw) != self.bytes(expected) {
                    return Err(Fail::Invalid(start..self.pos + 1));
                }
                self.pos += 1;
                if let Some(element) = self.elements.get_mut(current) {
                    element.span.end = self.pos;
                }
                if let Some(mark) = self.marks.pop() {
                    self.unwind(mark);
                }
                cx.leave();
                open.pop();
            } else if self.starts(b"<!--") {
                self.skip_to(b"-->", start)?;
            } else if self.starts(b"<![CDATA[") {
                let inner = start + b"<![CDATA[".len();
                let end =
                    find(self.src, b"]]>", inner).ok_or(Fail::Invalid(start..self.src.len()))?;
                self.pos = end + 3;
                self.child(current, Node::CData(inner..end));
            } else if self.starts(b"<?") {
                self.skip_to(b"?>", start)?;
            } else if self.starts(b"<!") {
                return Err(Fail::Invalid(start..start + 2));
            } else {
                let index = self.elements.len();
                if self.open(Some(current), cx)? {
                    open.push(index);
                }
            }
        }
        Ok(())
    }

    fn child(&mut self, parent: usize, node: Node) {
        if let Some(parent) = self.elements.get_mut(parent) {
            parent.children.push(node);
        }
    }

    /// 開始タグか空要素のタグを読み、要素を足す。中身を持つ要素なら真
    fn open(&mut self, parent: Option<usize>, cx: &mut Cx) -> Result<bool, Fail> {
        let start = self.pos;
        self.pos += 1;
        let (raw, local) = self.name(cx)?;
        let mut pairs: Vec<(Range<usize>, usize, Range<usize>)> = Vec::new();
        let empty = loop {
            let spaced = self.skip_space();
            match self.peek() {
                Some(b'>') => {
                    self.pos += 1;
                    break false;
                }
                Some(b'/') if self.starts(b"/>") => {
                    self.pos += 2;
                    break true;
                }
                Some(_) if spaced => {
                    let (name, name_local) = self.name(cx)?;
                    self.skip_space();
                    if self.peek() != Some(b'=') {
                        return Err(Fail::Invalid(name.start..self.pos + 1));
                    }
                    self.pos += 1;
                    self.skip_space();
                    let quote = self
                        .peek()
                        .filter(|b| matches!(b, b'"' | b'\''))
                        .ok_or(Fail::Invalid(name.start..self.pos + 1))?;
                    let value_start = self.pos + 1;
                    let value_end = self
                        .src
                        .get(value_start..)
                        .and_then(|rest| rest.iter().position(|b| *b == quote))
                        .map(|end| value_start + end)
                        .ok_or(Fail::Invalid(name.start..self.src.len()))?;
                    let value = value_start..value_end;
                    if self.bytes(value.clone()).contains(&b'<') {
                        return Err(Fail::Invalid(name.start..value_end + 1));
                    }
                    self.pos = value_end + 1;
                    self.references(value.clone(), cx)?;
                    pairs.push((name, name_local, value));
                }
                _ => return Err(Fail::Invalid(start..self.pos.max(start + 1))),
            }
        };
        cx.check(Kind::Records, 1)
            .map_err(|stop| Fail::Stop(stop, start..self.pos))?;
        self.work(cx, 1 + pairs.len() as u64, start)?;
        cx.enter()
            .map_err(|stop| Fail::Stop(stop, start..self.pos))?;
        let mut seen = BTreeSet::new();
        for (name, _, _) in &pairs {
            if !seen.insert(self.bytes(name.clone())) {
                return Err(Fail::Invalid(name.clone()));
            }
        }
        let mark = self.scopes.len();
        for (name, _, value) in &pairs {
            let bytes = self.bytes(name.clone());
            let declared = (!value.is_empty()).then(|| value.clone());
            if bytes == b"xmlns" {
                self.declare(None, declared);
            } else if let Some(prefix) = bytes.strip_prefix(b"xmlns:") {
                if matches!(prefix, b"xml" | b"xmlns") {
                    continue;
                }
                if declared.is_none() {
                    return Err(Fail::Invalid(name.clone()));
                }
                self.declare(Some(prefix), declared);
            }
        }
        let namespace = self.resolve(&raw, local, true, cx);
        let attributes = pairs
            .into_iter()
            .map(|(name, name_local, value)| Attribute {
                name: Name {
                    prefix_end: (name_local > name.start).then_some(name_local.saturating_sub(1)),
                    namespace: self.resolve(&name, name_local, false, cx),
                    raw: name,
                    local: name_local,
                },
                value,
            })
            .collect();
        let index = self.elements.len();
        self.elements.push(Element {
            span: start..self.pos,
            name: Name {
                prefix_end: (local > raw.start).then_some(local.saturating_sub(1)),
                raw,
                local,
                namespace,
            },
            attributes,
            children: Vec::new(),
            parent,
        });
        if let Some(parent) = parent.and_then(|parent| self.elements.get_mut(parent)) {
            parent.children.push(Node::Element(index));
        }
        if empty {
            self.unwind(mark);
            cx.leave();
        } else {
            self.marks.push(mark);
        }
        Ok(!empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics};

    /// 参照は範囲の中だけで探し、文書の残りを見ない。小さな範囲を多く確かめても、見たバイトの数は範囲の長さの
    /// 和を超えない（#721）
    #[test]
    fn references_look_only_inside_the_range() {
        let mut src = b"<a>".to_vec();
        src.extend(std::iter::repeat_n(b'x', 100_000));
        src.extend(b"&amp;</a>");
        let parser = Parser::new(&src);
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut seen = 0;
        let mut ranges = 0;
        for start in (3..100_003).step_by(10) {
            let Ok(found) = parser.references(start..start + 10, &mut cx) else {
                panic!("{start}");
            };
            seen += found;
            ranges += 10;
        }
        assert_eq!(seen, ranges);
        // 範囲の中の参照は、前と同じく読む
        let Ok(found) = parser.references(100_003..100_008, &mut cx) else {
            panic!("&amp;");
        };
        assert_eq!(found, 5);
        assert!(diag.is_empty());
    }

    /// 名前空間の宣言は接頭辞ごとに積み、閉じた要素の宣言を外す。読み終えたら、積みは空になる（#721）
    #[test]
    fn namespaces_are_stacked_by_prefix_and_unwound() {
        let src = b"<r xmlns:p='urn:r' xmlns='urn:d' xmlns:q='urn:q'>\
            <p:a xmlns:p='urn:a'><b xmlns=''/></p:a><c xmlns:s='urn:c'/></r>";
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut parser = Parser::new(src);
        assert!(parser.encoding(&mut cx).is_ok());
        // ルートの開始タグのあとは、接頭辞ごとに 1 つずつ
        assert!(matches!(parser.open(None, &mut cx), Ok(true)));
        assert_eq!(parser.namespaces.len(), 3);
        assert!(parser.namespaces.values().all(|stack| stack.len() == 1));
        // `p:a` の中では、`p` の宣言が 2 つ積まれ、内側で引く
        assert!(matches!(parser.open(Some(0), &mut cx), Ok(true)));
        let p = Some(&b"p"[..]);
        assert_eq!(parser.namespaces.get(&p).map(Vec::len), Some(2));
        let inner = parser.declared(p).cloned().flatten();
        assert_eq!(inner.map(|value| parser.bytes(value)), Some(&b"urn:a"[..]));
        assert!(diag.is_empty());

        // 読み終えたら、すべて外れる
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut parser = Parser::new(src);
        assert!(parser.encoding(&mut cx).is_ok());
        assert!(parser.document(&mut cx).is_ok());
        assert!(parser.namespaces.is_empty());
        assert!(parser.scopes.is_empty());
        assert!(parser.marks.is_empty());
        assert!(diag.is_empty());
    }
}
