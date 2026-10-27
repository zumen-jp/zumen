//! ISO 10303-21 の交換構造と、SFC のフィーチャのコメントを、レコードに分けて読む（決定 0120）。

use core::ops::Range;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Cx, Error, Stop};

use crate::{Form, Marker, Mode, Part, Quote, Record, Value, stopped};

/// 読んだ結果
pub(crate) struct Read {
    pub(crate) mode: Mode,
    pub(crate) records: Vec<Record>,
    /// `FILE_SCHEMA` の最初のスキーマの文字列の内側
    pub(crate) schema: Option<Range<usize>>,
}

/// 読めない理由
enum Fail {
    /// 文の形になっていない
    Syntax,
    /// 予算か中断の合図で止めた
    Stop(Stop),
}

impl From<Stop> for Fail {
    fn from(stop: Stop) -> Self {
        Fail::Stop(stop)
    }
}

/// どの節を読んでいるか
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    /// `ISO-10303-21;` の前
    Start,
    /// 節の外か、`HEADER;` の中
    Outside,
    /// `DATA;` の中
    Data,
    /// `END-ISO-10303-21;` の後
    Done,
}

const START: &[u8] = b"ISO-10303-21";
const END: &[u8] = b"END-ISO-10303-21";

/// 交換構造を読む。始まり・`DATA` の節・終わりのどれかがなければ、読めないとする
pub(crate) fn read(src: &[u8], cx: &mut Cx) -> Result<Read, Error> {
    let mut p = Parser {
        src,
        pos: 0,
        end: src.len(),
        sfc: false,
    };
    let mut records = Vec::new();
    let mut section = Section::Start;
    let mut saw_data = false;
    let mut description = None;
    let mut schema = None;
    // AP202 と書いた P21 では、`/*SXF` もただのコメントとする（推定。docs/formats/sxf.md）
    let mut features = true;
    let mut found_feature = false;
    while section != Section::Done {
        p.skip_white();
        if p.pos >= p.end {
            break;
        }
        let start = p.pos;
        if p.starts(b"/*") {
            if section == Section::Data && features && p.starts(b"/*SXF") {
                if let Some(record) = feature(&mut p, cx)? {
                    found_feature = true;
                    push(&mut records, record, cx)?;
                }
                continue;
            }
            match find(src, b"*/", start + 2, p.end) {
                Some(at) => p.pos = at + 2,
                None => {
                    invalid(cx, start..p.end);
                    p.pos = p.end;
                }
            }
            continue;
        }
        let record = match p.statement(cx) {
            Ok(record) => record,
            Err(Fail::Stop(stop)) => return Err(stopped(cx, stop, Some(start..p.pos))),
            Err(Fail::Syntax) if section == Section::Start => return Err(file_invalid(cx)),
            Err(Fail::Syntax) => {
                p.pos = find(src, b";", p.pos, p.end).map_or(p.end, |at| at + 1);
                invalid(cx, start..p.pos);
                continue;
            }
        };
        let name = record
            .parts
            .first()
            .and_then(|part| src.get(part.name()))
            .unwrap_or_default();
        let keyword = matches!(record.form, Form::Keyword | Form::Entity) && record.id.is_none();
        match section {
            Section::Start if keyword && name == START => section = Section::Outside,
            Section::Start => return Err(file_invalid(cx)),
            _ if keyword && name == END => section = Section::Done,
            _ if keyword && name == b"DATA" => {
                section = Section::Data;
                saw_data = true;
                features = !description
                    .clone()
                    .is_some_and(|range: Range<usize>| contains(src.get(range), b"AP202"));
            }
            _ if keyword && name == b"ENDSEC" => section = Section::Outside,
            Section::Outside if name == b"FILE_DESCRIPTION" => description = first_text(&record),
            Section::Outside if name == b"FILE_SCHEMA" => schema = first_text(&record),
            _ => {}
        }
        push(&mut records, record, cx)?;
    }
    if !saw_data || section != Section::Done {
        return Err(file_invalid(cx));
    }
    let described = description.is_some_and(|range| contains(src.get(range), b"feature_mode"));
    Ok(Read {
        mode: if described || found_feature {
            Mode::Sfc
        } else {
            Mode::P21
        },
        records,
        schema,
    })
}

/// レコードを 1 つ数えて足す
fn push(records: &mut Vec<Record>, record: Record, cx: &mut Cx) -> Result<(), Error> {
    cx.check(Kind::Records, 1)
        .map_err(|stop| stopped(cx, stop, Some(record.span())))?;
    records.push(record);
    Ok(())
}

/// SFC のフィーチャのコメントを読む。閉じがないか、中が読めなければ、診断してレコードにしない
fn feature(p: &mut Parser, cx: &mut Cx) -> Result<Option<Record>, Error> {
    let start = p.pos;
    let Some(&(marker, open, close)) = Marker::ALL.iter().find(|(_, open, _)| p.starts(open))
    else {
        return Ok(None);
    };
    let Some(closing) = find(p.src, close, start + open.len(), p.end) else {
        invalid(cx, start..p.end);
        p.pos = p.end;
        return Ok(None);
    };
    let span = start..closing + close.len();
    p.pos = span.end;
    let mut inner = Parser {
        src: p.src,
        pos: start + open.len(),
        end: closing,
        sfc: true,
    };
    match inner.feature(cx) {
        Ok((id, part)) => Ok(Some(Record {
            span,
            id: Some(id),
            marker: Some(marker),
            form: Form::Entity,
            parts: vec![part],
        })),
        Err(Fail::Syntax) => {
            invalid(cx, span);
            Ok(None)
        }
        Err(Fail::Stop(stop)) => Err(stopped(cx, stop, Some(span))),
    }
}

fn invalid(cx: &mut Cx, range: Range<usize>) {
    cx.report(
        Diagnostic::new(codes::SXF_RECORD_INVALID)
            .at(Ref::at_bytes(range.start as u64, range.end as u64)),
    );
}

fn file_invalid(cx: &mut Cx) -> Error {
    cx.report(Diagnostic::new(codes::SXF_FILE_INVALID));
    Error::Invalid
}

/// 最初の引数の並びの、最初の文字列
fn first_text(record: &Record) -> Option<Range<usize>> {
    match record.parts.first()?.args.first()? {
        Value::List(items) => match items.first()? {
            Value::Text { raw, .. } => Some(raw.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn contains(hay: Option<&[u8]>, needle: &[u8]) -> bool {
    hay.is_some_and(|hay| {
        hay.windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
    })
}

fn find(src: &[u8], pattern: &[u8], from: usize, end: usize) -> Option<usize> {
    src.get(from..end)?
        .windows(pattern.len())
        .position(|window| window == pattern)
        .map(|at| from + at)
}

/// Shift_JIS の 2 バイトの文字の 1 バイト目（推定。docs/formats/sxf.md）
fn is_lead_byte(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9f | 0xe0..=0xfc)
}

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'!'
}

fn is_name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

struct Parser<'s> {
    src: &'s [u8],
    pos: usize,
    /// 読む範囲の終わり。SFC のフィーチャでは、コメントの閉じの前
    end: usize,
    /// SFC のフィーチャの中か
    sfc: bool,
}

impl Parser<'_> {
    fn byte(&self, at: usize) -> Option<u8> {
        if at < self.end {
            self.src.get(at).copied()
        } else {
            None
        }
    }

    fn peek(&self) -> Option<u8> {
        self.byte(self.pos)
    }

    fn starts(&self, pattern: &[u8]) -> bool {
        self.src
            .get(self.pos..self.end)
            .is_some_and(|rest| rest.starts_with(pattern))
    }

    fn skip_white(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    /// 空白と、P21 では文の中のコメントを飛ばす
    fn skip_space(&mut self) {
        loop {
            self.skip_white();
            if self.sfc || !self.starts(b"/*") {
                return;
            }
            self.pos = find(self.src, b"*/", self.pos + 2, self.end).map_or(self.end, |at| at + 2);
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), Fail> {
        self.skip_space();
        if self.peek() != Some(byte) {
            return Err(Fail::Syntax);
        }
        self.pos += 1;
        Ok(())
    }

    fn name(&mut self) -> Result<Range<usize>, Fail> {
        self.skip_space();
        let start = self.pos;
        if !self.peek().is_some_and(is_name_start) {
            return Err(Fail::Syntax);
        }
        self.pos += 1;
        while self.peek().is_some_and(is_name) {
            self.pos += 1;
        }
        Ok(start..self.pos)
    }

    fn number(&mut self) -> Result<u64, Fail> {
        let mut value: u64 = 0;
        let start = self.pos;
        while let Some(digit) = self.peek().filter(u8::is_ascii_digit) {
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(u64::from(digit - b'0')))
                .ok_or(Fail::Syntax)?;
            self.pos += 1;
        }
        if self.pos == start {
            return Err(Fail::Syntax);
        }
        Ok(value)
    }

    /// `;` で終わる文
    fn statement(&mut self, cx: &mut Cx) -> Result<Record, Fail> {
        let start = self.pos;
        let id = if self.peek() == Some(b'#') {
            self.pos += 1;
            let id = self.number()?;
            self.expect(b'=')?;
            Some(id)
        } else {
            None
        };
        self.skip_space();
        let (form, parts) = if id.is_some() && self.peek() == Some(b'(') {
            self.pos += 1;
            let mut parts = Vec::new();
            loop {
                self.skip_space();
                if self.peek() == Some(b')') {
                    self.pos += 1;
                    break;
                }
                parts.push(self.part(cx)?);
            }
            if parts.is_empty() {
                return Err(Fail::Syntax);
            }
            (Form::Complex, parts)
        } else {
            let name = self.name()?;
            self.skip_space();
            if self.peek() == Some(b'(') {
                let args = self.list(cx)?;
                (Form::Entity, vec![Part { name, args }])
            } else if id.is_none() {
                (
                    Form::Keyword,
                    vec![Part {
                        name,
                        args: Vec::new(),
                    }],
                )
            } else {
                return Err(Fail::Syntax);
            }
        };
        self.expect(b';')?;
        Ok(Record {
            span: start..self.pos,
            id,
            marker: None,
            form,
            parts,
        })
    }

    /// SFC のフィーチャ `#n = 名前(引数)`。後に空白のほかは続かない
    fn feature(&mut self, cx: &mut Cx) -> Result<(u64, Part), Fail> {
        self.expect(b'#')?;
        let id = self.number()?;
        self.expect(b'=')?;
        let part = self.part(cx)?;
        self.skip_space();
        if self.pos < self.end {
            return Err(Fail::Syntax);
        }
        Ok((id, part))
    }

    fn part(&mut self, cx: &mut Cx) -> Result<Part, Fail> {
        let name = self.name()?;
        self.skip_space();
        let args = self.list(cx)?;
        Ok(Part { name, args })
    }

    /// `(…)`。入れ子を 1 つ数える
    fn list(&mut self, cx: &mut Cx) -> Result<Vec<Value>, Fail> {
        self.expect(b'(')?;
        cx.enter()?;
        let items = self.items(cx);
        cx.leave();
        items
    }

    fn items(&mut self, cx: &mut Cx) -> Result<Vec<Value>, Fail> {
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(b')') {
            self.pos += 1;
            return Ok(items);
        }
        loop {
            items.push(self.value(cx)?);
            self.skip_space();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b')') => {
                    self.pos += 1;
                    return Ok(items);
                }
                _ => return Err(Fail::Syntax),
            }
        }
    }

    fn value(&mut self, cx: &mut Cx) -> Result<Value, Fail> {
        cx.check(Kind::Work, 1)?;
        self.skip_space();
        let byte = self.peek().ok_or(Fail::Syntax)?;
        if byte == b'(' {
            return self.list(cx).map(Value::List);
        }
        if byte == b'\'' {
            return self.plain();
        }
        if self.sfc {
            if byte == b'\\' && self.byte(self.pos + 1) == Some(b'\'') {
                return self.backslash();
            }
            return Ok(self.token());
        }
        let start = self.pos;
        match byte {
            b'$' => {
                self.pos += 1;
                Ok(Value::Null)
            }
            b'*' => {
                self.pos += 1;
                Ok(Value::Derived)
            }
            b'#' => {
                self.pos += 1;
                self.number().map(Value::Ref)
            }
            b'.' => {
                self.pos += 1;
                while self
                    .peek()
                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
                {
                    self.pos += 1;
                }
                let inner = start + 1..self.pos;
                if inner.is_empty() || self.peek() != Some(b'.') {
                    return Err(Fail::Syntax);
                }
                self.pos += 1;
                Ok(Value::Enum(inner))
            }
            b'"' => {
                self.pos += 1;
                while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                    self.pos += 1;
                }
                let inner = start + 1..self.pos;
                if self.peek() != Some(b'"') {
                    return Err(Fail::Syntax);
                }
                self.pos += 1;
                Ok(Value::Binary(inner))
            }
            b'+' | b'-' | b'0'..=b'9' => self.real(),
            _ if is_name_start(byte) => self.part(cx).map(Value::Typed),
            _ => Err(Fail::Syntax),
        }
    }

    /// 符号・整数・小数点・指数の順の数
    fn real(&mut self) -> Result<Value, Fail> {
        let start = self.pos;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.pos += 1;
        }
        self.number()?;
        if self.peek() == Some(b'.') {
            self.pos += 1;
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'E' | b'e')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.number()?;
        }
        Ok(Value::Number(start..self.pos))
    }

    /// `'…'`。P21 は `''` を飛ばす
    fn plain(&mut self) -> Result<Value, Fail> {
        self.pos += 1;
        let start = self.pos;
        loop {
            match self.peek() {
                None => return Err(Fail::Syntax),
                Some(b'\'') if !self.sfc && self.byte(self.pos + 1) == Some(b'\'') => {
                    self.pos += 2;
                }
                Some(b'\'') => break,
                Some(_) => self.pos += 1,
            }
        }
        let raw = start..self.pos;
        self.pos += 1;
        Ok(Value::Text {
            raw,
            quote: Quote::Plain,
        })
    }

    /// SFC の `\'…\'`
    fn backslash(&mut self) -> Result<Value, Fail> {
        self.pos += 2;
        let start = self.pos;
        loop {
            let byte = self.peek().ok_or(Fail::Syntax)?;
            let next = self.byte(self.pos + 1);
            if is_lead_byte(byte) && next.is_some() {
                self.pos += 2;
                continue;
            }
            if byte == b'\\' && next == Some(b'\\') {
                self.pos += 2;
                continue;
            }
            if byte == b'\\' && next == Some(b'\'') {
                let mut after = self.pos + 2;
                while self.byte(after).is_some_and(|b| b.is_ascii_whitespace()) {
                    after += 1;
                }
                if matches!(self.byte(after), None | Some(b',' | b')')) {
                    let raw = start..self.pos;
                    self.pos += 2;
                    return Ok(Value::Text {
                        raw,
                        quote: Quote::Backslash,
                    });
                }
            }
            self.pos += 1;
        }
    }

    /// SFC の、囲まない値。`,` か `)` の前まで
    fn token(&mut self) -> Value {
        let start = self.pos;
        while self.peek().is_some_and(|b| b != b',' && b != b')') {
            self.pos += 1;
        }
        let mut end = self.pos;
        while end > start
            && self
                .src
                .get(end - 1)
                .is_some_and(|b| b.is_ascii_whitespace())
        {
            end -= 1;
        }
        Value::Token(start..end)
    }
}
