//! `Cx` の予算とバイト範囲の記録つきの JSON パーサー（RFC 8259）。

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Cx, Error};

use crate::stopped;

/// 単一の確保の上限（32 ビット wasm でも安全に扱える範囲）
pub(crate) const MAX_ALLOC: u64 = i32::MAX as u64;

/// 元の入力でのバイト範囲を持つ JSON の値
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpannedValue {
    pub(crate) span: ByteRange,
    pub(crate) kind: JsonKind,
}

/// JSON の値の種類
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JsonKind {
    Null,
    Bool(bool),
    Number {
        raw: String,
        value: f64,
        integer: Option<i64>,
    },
    String(String),
    Array(Vec<SpannedValue>),
    Object(Vec<(String, SpannedValue)>),
}

impl SpannedValue {
    pub(crate) fn at(&self) -> Ref {
        Ref::at_bytes(self.span.start, self.span.end)
    }

    /// オブジェクトから最後の同名メンバーを引く（重複キーは後勝ち）
    pub(crate) fn get(&self, name: &str) -> Option<&SpannedValue> {
        match &self.kind {
            JsonKind::Object(entries) => entries
                .iter()
                .rev()
                .find_map(|(k, v)| (k == name).then_some(v)),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match &self.kind {
            JsonKind::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        match &self.kind {
            JsonKind::Number { value, .. } => Some(*value),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[SpannedValue]> {
        match &self.kind {
            JsonKind::Array(items) => Some(items.as_slice()),
            _ => None,
        }
    }
}

pub(crate) fn invalid_at(offset: usize, total_len: usize, cx: &mut Cx<'_>) -> Error {
    let start = offset.min(total_len) as u64;
    let end = offset.saturating_add(1).min(total_len) as u64;
    cx.report(
        Diagnostic::new(codes::GEOJSON_FILE_INVALID)
            .with("offset", start)
            .at(Ref::at_bytes(start, end)),
    );
    Error::Invalid
}

pub(crate) fn parse_spanned(src: &[u8], cx: &mut Cx<'_>) -> Result<SpannedValue, Error> {
    let start = if src.starts_with(b"\xEF\xBB\xBF") {
        3
    } else {
        0
    };
    let mut parser = Parser { src, pos: start };
    parser.skip_ws(cx)?;
    let value = parser.value(cx)?;
    parser.skip_ws(cx)?;
    if parser.pos != src.len() {
        return Err(parser.invalid(parser.pos, cx));
    }
    Ok(value)
}

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn at_pos(&self, offset: usize) -> Ref {
        let start = offset.min(self.src.len()) as u64;
        let end = offset.saturating_add(1).min(self.src.len()) as u64;
        Ref::at_bytes(start, end)
    }

    fn check(&self, cx: &mut Cx<'_>, kind: Kind, amount: u64, offset: usize) -> Result<(), Error> {
        cx.check(kind, amount)
            .map_err(|s| stopped(cx, s, Some(self.at_pos(offset))))
    }

    fn enter(&self, cx: &mut Cx<'_>, offset: usize) -> Result<(), Error> {
        cx.enter()
            .map_err(|s| stopped(cx, s, Some(self.at_pos(offset))))
    }

    fn invalid(&self, offset: usize, cx: &mut Cx<'_>) -> Error {
        invalid_at(offset, self.src.len(), cx)
    }

    fn skip_ws(&mut self, cx: &mut Cx<'_>) -> Result<(), Error> {
        let mut skipped = 0u64;
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            let at = self.pos;
            self.pos += 1;
            skipped += 1;
            if skipped.is_multiple_of(256) {
                self.check(cx, Kind::Work, 1, at)?;
            }
        }
        Ok(())
    }

    fn expect_byte(&mut self, expected: u8, cx: &mut Cx<'_>) -> Result<(), Error> {
        if self.peek() == Some(expected) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.invalid(self.pos, cx))
        }
    }

    fn expect_bytes(&mut self, expected: &[u8], cx: &mut Cx<'_>) -> Result<(), Error> {
        let end = self.pos.saturating_add(expected.len());
        if self.src.get(self.pos..end) == Some(expected) {
            self.pos = end;
            Ok(())
        } else {
            Err(self.invalid(self.pos, cx))
        }
    }

    fn value(&mut self, cx: &mut Cx<'_>) -> Result<SpannedValue, Error> {
        let start = self.pos;
        self.check(cx, Kind::Work, 1, start)?;
        self.check(cx, Kind::DecodedBytes, 32, start)?;
        let Some(b) = self.peek() else {
            return Err(self.invalid(start, cx));
        };
        let kind = match b {
            b'n' => {
                self.expect_bytes(b"null", cx)?;
                JsonKind::Null
            }
            b't' => {
                self.expect_bytes(b"true", cx)?;
                JsonKind::Bool(true)
            }
            b'f' => {
                self.expect_bytes(b"false", cx)?;
                JsonKind::Bool(false)
            }
            b'"' => JsonKind::String(self.string(cx)?),
            b'[' => JsonKind::Array(self.array(cx)?),
            b'{' => JsonKind::Object(self.object(cx)?),
            b'-' | b'0'..=b'9' => self.number(cx)?,
            _ => return Err(self.invalid(start, cx)),
        };
        Ok(SpannedValue {
            span: ByteRange {
                start: start as u64,
                end: self.pos as u64,
            },
            kind,
        })
    }

    fn array(&mut self, cx: &mut Cx<'_>) -> Result<Vec<SpannedValue>, Error> {
        self.enter(cx, self.pos)?;
        let res = self.array_inner(cx);
        cx.leave();
        res
    }

    fn array_inner(&mut self, cx: &mut Cx<'_>) -> Result<Vec<SpannedValue>, Error> {
        self.pos += 1;
        self.skip_ws(cx)?;
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(items);
        }
        loop {
            items.push(self.value(cx)?);
            self.skip_ws(cx)?;
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_ws(cx)?;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(items);
                }
                _ => return Err(self.invalid(self.pos, cx)),
            }
        }
    }

    fn object(&mut self, cx: &mut Cx<'_>) -> Result<Vec<(String, SpannedValue)>, Error> {
        self.enter(cx, self.pos)?;
        let res = self.object_inner(cx);
        cx.leave();
        res
    }

    fn object_inner(&mut self, cx: &mut Cx<'_>) -> Result<Vec<(String, SpannedValue)>, Error> {
        self.pos += 1;
        self.skip_ws(cx)?;
        let mut entries = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(entries);
        }
        loop {
            self.check(cx, Kind::Work, 1, self.pos)?;
            if self.peek() != Some(b'"') {
                return Err(self.invalid(self.pos, cx));
            }
            let key = self.string(cx)?;
            self.skip_ws(cx)?;
            self.expect_byte(b':', cx)?;
            self.skip_ws(cx)?;
            let val = self.value(cx)?;
            entries.push((key, val));
            self.skip_ws(cx)?;
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_ws(cx)?;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(entries);
                }
                _ => return Err(self.invalid(self.pos, cx)),
            }
        }
    }

    fn string(&mut self, cx: &mut Cx<'_>) -> Result<String, Error> {
        let open = self.pos;
        self.pos += 1;
        let mut out = String::new();
        let mut plain_start = self.pos;
        let mut plain_scanned = 0u64;
        loop {
            let Some(b) = self.peek() else {
                return Err(self.invalid(open, cx));
            };
            match b {
                b'"' => {
                    self.flush_utf8(&mut out, plain_start, self.pos, plain_scanned, cx)?;
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.flush_utf8(&mut out, plain_start, self.pos, plain_scanned, cx)?;
                    let esc_at = self.pos;
                    self.pos += 1;
                    let ch = self.escape(cx)?;
                    let ch_len = ch.len_utf8() as u64;
                    if (out.len() as u64).saturating_add(ch_len) > MAX_ALLOC {
                        return Err(self.invalid(esc_at, cx));
                    }
                    self.check(cx, Kind::DecodedBytes, ch_len, esc_at)?;
                    out.push(ch);
                    plain_start = self.pos;
                    plain_scanned = 0;
                }
                0x00..=0x1f => return Err(self.invalid(self.pos, cx)),
                _ => {
                    let at = self.pos;
                    self.pos += 1;
                    plain_scanned += 1;
                    if plain_scanned.is_multiple_of(256) {
                        self.check(cx, Kind::Work, 1, at)?;
                    }
                }
            }
        }
    }

    fn flush_utf8(
        &self,
        out: &mut String,
        start: usize,
        end: usize,
        already_scanned: u64,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        if start == end {
            return Ok(());
        }
        let len = (end - start) as u64;
        if (out.len() as u64).saturating_add(len) > MAX_ALLOC {
            return Err(self.invalid(start, cx));
        }
        self.check(cx, Kind::DecodedBytes, len, start)?;
        if !already_scanned.is_multiple_of(256) {
            self.check(cx, Kind::Work, 1, start)?;
        }
        let Some(slice) = self.src.get(start..end) else {
            return Err(self.invalid(start, cx));
        };
        let Ok(text) = std::str::from_utf8(slice) else {
            return Err(self.invalid(start, cx));
        };
        out.push_str(text);
        Ok(())
    }

    fn escape(&mut self, cx: &mut Cx<'_>) -> Result<char, Error> {
        self.check(cx, Kind::Work, 1, self.pos)?;
        let Some(b) = self.peek() else {
            return Err(self.invalid(self.pos, cx));
        };
        self.pos += 1;
        match b {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{0008}'),
            b'f' => Ok('\u{000c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => {
                let first = self.hex4(cx)?;
                if (0xd800..=0xdbff).contains(&first) {
                    let pair_at = self.pos;
                    if self.src.get(self.pos..self.pos.saturating_add(2)) != Some(b"\\u") {
                        return Err(self.invalid(pair_at, cx));
                    }
                    self.pos += 2;
                    let second = self.hex4(cx)?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(self.invalid(pair_at, cx));
                    }
                    let code = 0x10000
                        + ((u32::from(first) - 0xd800) << 10)
                        + (u32::from(second) - 0xdc00);
                    char::from_u32(code).ok_or_else(|| self.invalid(pair_at, cx))
                } else {
                    char::from_u32(u32::from(first)).ok_or_else(|| self.invalid(self.pos - 4, cx))
                }
            }
            _ => Err(self.invalid(self.pos - 1, cx)),
        }
    }

    fn hex4(&mut self, cx: &mut Cx<'_>) -> Result<u16, Error> {
        let start = self.pos;
        let end = start.saturating_add(4);
        let Some(bytes) = self.src.get(start..end) else {
            return Err(self.invalid(start, cx));
        };
        let mut acc = 0u16;
        for &b in bytes {
            let digit = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return Err(self.invalid(start, cx)),
            };
            acc = (acc << 4) | u16::from(digit);
        }
        self.pos = end;
        Ok(acc)
    }

    fn scan_digits(&mut self, cx: &mut Cx<'_>) -> Result<(), Error> {
        let mut count = 0u64;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            let at = self.pos;
            self.pos += 1;
            count += 1;
            if count.is_multiple_of(256) {
                self.check(cx, Kind::Work, 1, at)?;
            }
        }
        Ok(())
    }

    fn number(&mut self, cx: &mut Cx<'_>) -> Result<JsonKind, Error> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
            }
            Some(b'1'..=b'9') => {
                self.pos += 1;
                self.scan_digits(cx)?;
            }
            _ => return Err(self.invalid(start, cx)),
        }
        let mut is_int = true;
        if self.peek() == Some(b'.') {
            is_int = false;
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.invalid(self.pos, cx));
            }
            self.scan_digits(cx)?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_int = false;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.invalid(self.pos, cx));
            }
            self.scan_digits(cx)?;
        }
        let len = (self.pos - start) as u64;
        if len > MAX_ALLOC {
            return Err(self.invalid(start, cx));
        }
        self.check(cx, Kind::DecodedBytes, len, start)?;
        let Some(slice) = self.src.get(start..self.pos) else {
            return Err(self.invalid(start, cx));
        };
        let Ok(raw) = std::str::from_utf8(slice) else {
            return Err(self.invalid(start, cx));
        };
        let Ok(parsed) = raw.parse::<f64>() else {
            return Err(self.invalid(start, cx));
        };
        if !parsed.is_finite() {
            return Err(self.invalid(start, cx));
        }
        let value = if parsed == 0.0 { 0.0 } else { parsed };
        let integer = if is_int {
            raw.parse::<i64>().ok()
        } else {
            None
        };
        Ok(JsonKind::Number {
            raw: raw.to_string(),
            value,
            integer,
        })
    }
}
