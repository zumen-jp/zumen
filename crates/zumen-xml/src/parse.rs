//! XML 1.0 と XML の名前空間 1.0 の文書を読む（決定 0209）。
//!
//! 入れ子は再帰でなく、開いた要素の並びでたどる。要素を予算のレコード、入れ子を深さで数える（決定 0010）。

use core::ops::Range;
use std::collections::BTreeSet;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Cx, Error, Stop};

use crate::{Attribute, Document, Element, Encoding, Name, Namespace, Node, text};

/// 読めない理由
enum Fail {
    /// XML の形になっていない。範囲はその場所
    Invalid(Range<usize>),
    /// 予算か中断の合図で止めた。範囲は扱っていた場所
    Stop(Stop, Range<usize>),
}

/// XML の文書を読む。XML の形になっていなければ `xml.document.invalid`、UTF-8 と Shift_JIS のほかの文字コードなら
/// `xml.encoding.unsupported` を報告して読まない。文書型宣言は処理せず `xml.dtd.dropped` を報告する
pub fn read(src: &[u8], cx: &mut Cx) -> Result<Document, Error> {
    cx.check(Kind::InputBytes, src.len() as u64)
        .map_err(|stop| stopped(cx, stop, None))?;
    let mut p = Parser {
        src,
        pos: 0,
        elements: Vec::new(),
        scopes: Vec::new(),
        marks: Vec::new(),
        doctype: None,
    };
    let encoding = p.encoding(cx)?;
    match p.document(cx) {
        Ok(()) => Ok(Document {
            src: src.to_vec(),
            encoding,
            elements: p.elements,
            doctype: p.doctype,
        }),
        Err(Fail::Invalid(range)) => {
            cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(at(range)));
            Err(Error::Invalid)
        }
        Err(Fail::Stop(stop, range)) => Err(stopped(cx, stop, Some(range))),
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

/// 名前空間の宣言。接頭辞（既定の名前空間は `None`）と、値の範囲（`xmlns=""` は `None`）
type Scope = (Option<Range<usize>>, Option<Range<usize>>);

struct Parser<'s> {
    src: &'s [u8],
    pos: usize,
    elements: Vec<Element>,
    /// 開いている要素の名前空間の宣言。内側の宣言が後
    scopes: Vec<Scope>,
    /// 開いている要素ごとの、`scopes` の長さ。閉じるときに戻す
    marks: Vec<usize>,
    doctype: Option<Range<usize>>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn starts(&self, pattern: &[u8]) -> bool {
        self.src
            .get(self.pos..)
            .is_some_and(|rest| rest.starts_with(pattern))
    }

    fn bytes(&self, range: Range<usize>) -> &[u8] {
        self.src.get(range).unwrap_or_default()
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

    /// バイト順の印と XML 宣言から、文字コードを決める
    fn encoding(&mut self, cx: &mut Cx) -> Result<Encoding, Error> {
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
            return Ok(Encoding::Utf8);
        }
        let start = self.pos;
        let Some(end) = find(self.src, b"?>", start) else {
            cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(at(start..self.src.len())));
            return Err(Error::Invalid);
        };
        let src = self.src;
        let declaration = src.get(start + 5..end).unwrap_or_default();
        let Some(pairs) = pseudo_attributes(declaration) else {
            cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(at(start..end + 2)));
            return Err(Error::Invalid);
        };
        if pairs.first().map(|(name, _)| *name) != Some(&b"version"[..]) {
            cx.report(Diagnostic::new(codes::XML_DOCUMENT_INVALID).at(at(start..end + 2)));
            return Err(Error::Invalid);
        }
        self.pos = end + 2;
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
        cx.report(Diagnostic::new(codes::XML_DTD_DROPPED).at(at(start..self.pos)));
        Ok(())
    }

    /// 名前。`:` で接頭辞と分け、接頭辞を除いた名前の始まりを返す
    fn name(&mut self) -> Result<(Range<usize>, usize), Fail> {
        let start = self.pos;
        while self.peek().is_some_and(|b| !is_delimiter(b)) {
            self.pos += 1;
        }
        let raw = start..self.pos;
        let bytes = self.bytes(raw.clone());
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

    /// 文字か属性の値の参照を確かめる。DTD で宣言する実体の参照は、報告して残す
    fn references(&self, range: Range<usize>, cx: &mut Cx) -> Result<(), Fail> {
        let mut at = range.start;
        while let Some(amp) = find(self.src, b"&", at).filter(|amp| *amp < range.end) {
            let end = find(self.src, b";", amp)
                .filter(|end| *end < range.end)
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
                        .at(Ref::at_bytes(amp as u64, end as u64 + 1)),
                );
            }
            at = end + 1;
        }
        Ok(())
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
                    .scopes
                    .iter()
                    .rev()
                    .find(|(prefix, _)| prefix.is_none())
                    .and_then(|(_, value)| value.clone().map(Namespace::Declared));
            }
            return (self.bytes(raw.clone()) == b"xmlns").then_some(Namespace::Xmlns);
        }
        let prefix = self.bytes(raw.start..local - 1);
        match prefix {
            b"xml" => return Some(Namespace::Xml),
            b"xmlns" => return Some(Namespace::Xmlns),
            _ => {}
        }
        let found = self.scopes.iter().rev().find_map(|(declared, value)| {
            let declared = declared.clone()?;
            (self.bytes(declared) == prefix).then(|| value.clone())
        });
        match found {
            Some(value) => value.map(Namespace::Declared),
            None => {
                cx.report(
                    Diagnostic::new(codes::XML_NAMESPACE_MISSING)
                        .with("prefix", String::from_utf8_lossy(prefix).into_owned())
                        .at(at(raw.clone())),
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
                let (raw, _) = self.name()?;
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
                    self.scopes.truncate(mark);
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
        let (raw, local) = self.name()?;
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
                    let (name, name_local) = self.name()?;
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
                self.scopes.push((None, declared));
            } else if let Some(prefix) = bytes.strip_prefix(b"xmlns:") {
                if matches!(prefix, b"xml" | b"xmlns") {
                    continue;
                }
                if declared.is_none() {
                    return Err(Fail::Invalid(name.clone()));
                }
                self.scopes.push((Some(name.start + 6..name.end), declared));
            }
        }
        let namespace = self.resolve(&raw, local, true, cx);
        let attributes = pairs
            .into_iter()
            .map(|(name, name_local, value)| Attribute {
                name: Name {
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
            self.scopes.truncate(mark);
            cx.leave();
        } else {
            self.marks.push(mark);
        }
        Ok(!empty)
    }
}
