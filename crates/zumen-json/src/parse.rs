//! JSON を読む（RFC 8259）。

use std::collections::BTreeSet;

use crate::value::{Number, Object, Value};

/// 配列とオブジェクトの入れ子の深さの上限
pub const MAX_DEPTH: usize = 64;

/// UTF-8 の BOM。先頭にあれば読み飛ばす（RFC 8259 の 8.1 が許す）
const BOM: &str = "\u{feff}";

/// JSON として読めないこと
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    /// 問題を見つけたバイトの位置
    pub offset: u64,
}

/// UTF-8 のバイト列から、1 つの JSON の値を読む。
///
/// 次のものは読まない: UTF-8 でないもの、文法に合わないもの、オブジェクトのキーが重なるもの、
/// 入れ子が [`MAX_DEPTH`] より深いもの、値のあとに空白以外が続くもの、対になっていないサロゲートの `\u` エスケープ。
pub fn parse(bytes: &[u8]) -> Result<Value, SyntaxError> {
    let text = core::str::from_utf8(bytes).map_err(|error| SyntaxError {
        offset: error.valid_up_to() as u64,
    })?;
    let mut parser = Parser {
        text,
        pos: if text.starts_with(BOM) { BOM.len() } else { 0 },
    };
    parser.skip_whitespace();
    let value = parser.value(0)?;
    parser.skip_whitespace();
    if parser.pos == text.len() {
        Ok(value)
    } else {
        Err(parser.error())
    }
}

struct Parser<'a> {
    text: &'a str,
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.pos).copied()
    }

    fn error(&self) -> SyntaxError {
        SyntaxError {
            offset: self.pos as u64,
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.peek() {
            self.pos += 1;
        }
    }

    fn skip_digits(&mut self) {
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
    }

    fn literal(&mut self, literal: &str, value: Value) -> Result<Value, SyntaxError> {
        if self
            .text
            .get(self.pos..)
            .is_some_and(|rest| rest.starts_with(literal))
        {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err(self.error())
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, SyntaxError> {
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::String),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.error()),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, SyntaxError> {
        if depth >= MAX_DEPTH {
            return Err(self.error());
        }
        self.pos += 1;
        let mut members = Vec::new();
        let mut keys = BTreeSet::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(Object::from_unique(members)));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.error());
            }
            let key_at = self.error();
            let key = self.string()?;
            if !keys.insert(key.clone()) {
                return Err(key_at);
            }
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(self.error());
            }
            self.pos += 1;
            self.skip_whitespace();
            let value = self.value(depth + 1)?;
            members.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(Object::from_unique(members)));
                }
                _ => return Err(self.error()),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, SyntaxError> {
        if depth >= MAX_DEPTH {
            return Err(self.error());
        }
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_whitespace();
            items.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error()),
            }
        }
    }

    fn string(&mut self) -> Result<String, SyntaxError> {
        self.pos += 1;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(byte) = self.peek() {
                if byte == b'"' || byte == b'\\' || byte < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            // 区切りは ASCII なので、ここは文字の境目になる
            out.push_str(self.text.get(start..self.pos).ok_or_else(|| self.error())?);
            match self.peek() {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    out.push(self.escape()?);
                }
                // 制御文字か、入力の終わり
                _ => return Err(self.error()),
            }
        }
    }

    fn escape(&mut self) -> Result<char, SyntaxError> {
        let c = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => return self.unicode_escape(),
            _ => return Err(self.error()),
        };
        self.pos += 1;
        Ok(c)
    }

    /// `\u` のあとの 4 桁と、サロゲートの対
    fn unicode_escape(&mut self) -> Result<char, SyntaxError> {
        let at = SyntaxError {
            offset: self.pos.saturating_sub(1) as u64,
        };
        self.pos += 1;
        let unit = self.hex4()?;
        let code = match unit {
            0xD800..=0xDBFF => {
                if !self
                    .text
                    .get(self.pos..)
                    .is_some_and(|rest| rest.starts_with("\\u"))
                {
                    return Err(at);
                }
                self.pos += 2;
                let low = self.hex4()?;
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(at);
                }
                0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
            }
            0xDC00..=0xDFFF => return Err(at),
            _ => unit,
        };
        char::from_u32(code).ok_or(at)
    }

    fn hex4(&mut self) -> Result<u32, SyntaxError> {
        let mut unit = 0;
        for _ in 0..4 {
            let digit = self
                .peek()
                .and_then(|byte| char::from(byte).to_digit(16))
                .ok_or_else(|| self.error())?;
            unit = unit * 16 + digit;
            self.pos += 1;
        }
        Ok(unit)
    }

    fn number(&mut self) -> Result<Value, SyntaxError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.skip_digits(),
            _ => return Err(self.error()),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error());
            }
            self.skip_digits();
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error());
            }
            self.skip_digits();
        }
        let text = self.text.get(start..self.pos).ok_or_else(|| self.error())?;
        Ok(Value::Number(Number::from_valid(text)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offset(text: &[u8]) -> u64 {
        parse(text).unwrap_err().offset
    }

    #[test]
    fn reads_every_kind_of_value() {
        let value = parse(
            br#" {"a": [1, -2.5e3, true, false, null], "b": "x\n\u00e9\ud83d\ude00", "c": {}} "#,
        )
        .unwrap();
        let Value::Object(object) = &value else {
            panic!("{value:?}")
        };
        let keys: Vec<&str> = object.iter().map(|(key, _)| key).collect();
        assert_eq!(keys, ["a", "b", "c"]);
        assert_eq!(
            object.get("b"),
            Some(&Value::String("x\né\u{1f600}".to_string()))
        );
        let Some(Value::Array(items)) = object.get("a") else {
            panic!()
        };
        assert_eq!(items.len(), 5);
        assert_eq!(items[1], Value::Number(Number::from_valid("-2.5e3")));
        assert_eq!(object.get("c"), Some(&Value::Object(Object::default())));
    }

    #[test]
    fn keeps_utf8_text_as_is() {
        assert_eq!(
            parse("\"平面図\"".as_bytes()).unwrap(),
            Value::String("平面図".to_string())
        );
    }

    #[test]
    fn skips_a_leading_bom_and_counts_offsets_from_the_start() {
        assert_eq!(parse("\u{feff}1".as_bytes()).unwrap(), parse(b"1").unwrap());
        assert_eq!(offset("\u{feff}[1,]".as_bytes()), 6);
        assert_eq!(offset("1\u{feff}".as_bytes()), 1);
    }

    #[test]
    fn rejects_bad_utf8_at_the_first_bad_byte() {
        assert_eq!(offset(b"\"a\xff\""), 2);
        assert_eq!(offset(b"\xef\xbb"), 0);
    }

    #[test]
    fn rejects_grammar_errors() {
        assert_eq!(offset(b""), 0);
        assert_eq!(offset(b"  "), 2);
        assert_eq!(offset(b"[1,]"), 3);
        assert_eq!(offset(b"{\"a\":1,}"), 7);
        assert_eq!(offset(b"{\"a\" 1}"), 5);
        assert_eq!(offset(b"{a:1}"), 1);
        assert_eq!(offset(b"[1 2]"), 3);
        assert_eq!(offset(b"tru"), 0);
        assert_eq!(offset(b"nul"), 0);
        assert_eq!(offset(b"1 2"), 2);
        assert_eq!(offset(b"\"a"), 2);
        assert_eq!(offset(b"\"a\tb\""), 2);
        assert_eq!(offset(b"\"\\x\""), 2);
        assert_eq!(offset(b"\"\\u12\""), 5);
        assert_eq!(offset(b"'a'"), 0);
        assert_eq!(offset(b"NaN"), 0);
        assert_eq!(offset(b"Infinity"), 0);
    }

    #[test]
    fn rejects_numbers_outside_the_grammar() {
        assert_eq!(offset(b"01"), 1);
        assert_eq!(offset(b"-"), 1);
        assert_eq!(offset(b"+1"), 0);
        assert_eq!(offset(b"1."), 2);
        assert_eq!(offset(b".5"), 0);
        assert_eq!(offset(b"1e"), 2);
        assert_eq!(offset(b"1e+"), 3);
        assert!(parse(b"-0").is_ok());
        assert!(parse(b"0.5e-10").is_ok());
        assert!(parse(b"1E+2").is_ok());
    }

    #[test]
    fn rejects_unpaired_surrogates() {
        assert_eq!(offset(br#""\ud83d""#), 1);
        assert_eq!(offset(br#""\ud83dx""#), 1);
        assert_eq!(offset(br#""\ud83d\u0041""#), 1);
        assert_eq!(offset(br#""\ude00""#), 1);
    }

    #[test]
    fn rejects_duplicate_keys_at_the_second_key() {
        assert_eq!(offset(br#"{"a":1,"b":2,"a":3}"#), 13);
        assert!(parse(br#"{"a":{"a":1}}"#).is_ok());
    }

    #[test]
    fn limits_the_depth() {
        let deep = |depth: usize| {
            let mut text = "[".repeat(depth);
            text.push_str(&"]".repeat(depth));
            text
        };
        assert!(parse(deep(MAX_DEPTH).as_bytes()).is_ok());
        assert_eq!(offset(deep(MAX_DEPTH + 1).as_bytes()), MAX_DEPTH as u64);
        let objects = "{\"a\":".repeat(MAX_DEPTH + 1) + "1" + &"}".repeat(MAX_DEPTH + 1);
        assert!(parse(objects.as_bytes()).is_err());
    }
}
