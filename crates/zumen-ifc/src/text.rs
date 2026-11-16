//! IFC のエスケープに書かれた CP932 を、ファイル単位の手がかりと明示指定で読む（#540）。
//!
//! ISO 10303-21 の 6.4.3「String encoding」のエスケープを扱う。規格本文・プレビューは参照せず、
//! buildingSMART の IFC String Encoding の公開説明を使う。
//! https://technical.buildingsmart.org/resources/ifcimplementationguidance/string-encoding/

use crate::{Cx, Diagnostic, Error, IfcFile, Kind, Record, at_bytes, codes, stopped};

/// IFC のエスケープ内の 8 ビット値の読み方。元のバイト列と書き方は変えない。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextEncoding {
    /// ファイル内の `\X\80`〜`\X\9F` を手がかりに CP932 を選ぶ。
    #[default]
    Auto,
    /// エスケープを従来の ISO 8859-1/Unicode として読む。
    Iso8859_1,
    /// 手がかりがなくても、8 ビットのエスケープを CP932 として試す。
    ShiftJis,
}

impl IfcFile {
    /// エスケープ内の文字コードを指定する。以降の lift・形の取得・L1 の編集に使う。
    /// すでに作った領域モデルは変えない。書き戻したバイト列を読み直すときも、同じ指定を渡す。
    pub fn set_text_encoding(&mut self, encoding: TextEncoding) {
        self.text_encoding = encoding;
    }

    /// 利用者が指定した文字コード。自動判定の結果とは区別する。
    pub fn text_encoding(&self) -> TextEncoding {
        self.text_encoding
    }

    pub(crate) fn shift_jis(&self) -> bool {
        match self.text_encoding {
            TextEncoding::Auto => self.text_hint,
            TextEncoding::Iso8859_1 => false,
            TextEncoding::ShiftJis => true,
        }
    }

    pub(crate) fn decode_text(&self, raw: &[u8]) -> (String, bool) {
        let decoded = decode(raw, self.shift_jis());
        (decoded.text, decoded.invalid)
    }

    /// 読み替える元文字列を一度ずつ数える。共有プロパティや同じ値の取得回数では増やさない。
    pub(crate) fn report_text_encoding(&self, cx: &mut Cx) -> Result<(), Error> {
        if !self.shift_jis() {
            return Ok(());
        }
        visit_texts(&self.records, &self.src, cx, |raw, cx| {
            let bytes = self.bytes(raw.clone());
            cx.check(Kind::Work, bytes.len() as u64)
                .and_then(|()| {
                    cx.check(Kind::DecodedBytes, (bytes.len() as u64).saturating_mul(12))
                })
                .map_err(|stop| stopped(cx, stop, Some(raw.clone())))?;
            if decode(bytes, true).shift_jis {
                cx.report(Diagnostic::new(codes::IFC_TEXT_REPAIRED).at(at_bytes(raw)));
            }
            Ok(false)
        })?;
        cx.check(Kind::Work, 0)
            .map_err(|stop| stopped(cx, stop, None))
    }
}

/// L1 では展開せず、文字列の中の手がかりだけを一度調べる。コメント・二重の逆斜線は対象外。
pub(crate) fn hint(records: &[Record], src: &[u8], cx: &mut Cx) -> Result<bool, Error> {
    visit_texts(records, src, cx, |raw, cx| {
        cx.check(Kind::Work, raw.len() as u64)
            .map_err(|stop| stopped(cx, stop, Some(raw.clone())))?;
        Ok(has_hint(src.get(raw).unwrap_or_default()))
    })
}

fn visit_texts(
    records: &[Record],
    src: &[u8],
    cx: &mut Cx,
    mut visit: impl FnMut(core::ops::Range<usize>, &mut Cx) -> Result<bool, Error>,
) -> Result<bool, Error> {
    // すでに L0 が読んだ文だけを走査する。文字列とコメントを区別し、追加の配列を作らない。
    for record in records {
        let span = record.span();
        cx.check(Kind::Work, span.len() as u64)
            .map_err(|stop| stopped(cx, stop, Some(span.clone())))?;
        let bytes = src.get(span.clone()).unwrap_or_default();
        let mut i = 0;
        while i < bytes.len() {
            if bytes.get(i..i + 2) == Some(b"/*") {
                i += 2;
                while i < bytes.len() && bytes.get(i..i + 2) != Some(b"*/") {
                    i += 1;
                }
                i = i.saturating_add(2);
            } else if bytes.get(i) == Some(&b'\'') {
                let start = i + 1;
                i = start;
                while i < bytes.len() {
                    if bytes.get(i) != Some(&b'\'') {
                        i += 1;
                    } else if bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                    } else {
                        break;
                    }
                }
                if i < bytes.len() && visit(span.start + start..span.start + i, cx)? {
                    return Ok(true);
                }
                i += 1;
            } else {
                i += 1;
            }
        }
    }
    Ok(false)
}

fn has_hint(mut raw: &[u8]) -> bool {
    while let Some((&byte, tail)) = raw.split_first() {
        if raw.starts_with(b"\\\\") {
            raw = raw.get(2..).unwrap_or_default();
        } else if let Some(after) = raw.strip_prefix(b"\\X\\")
            && let Some(value) = after.get(..2).and_then(hex)
        {
            if (0x80..=0x9f).contains(&value) {
                return true;
            }
            raw = after.get(2..).unwrap_or_default();
        } else if byte == b'\\' && raw.starts_with(b"\\S\\") && raw.len() >= 4 {
            raw = raw.get(4..).unwrap_or_default();
        } else {
            raw = tail;
        }
    }
    false
}

pub(crate) struct Decoded {
    pub text: String,
    pub invalid: bool,
    pub shift_jis: bool,
}

pub(crate) fn decode(raw: &[u8], shift_jis: bool) -> Decoded {
    if shift_jis
        && let Some(bytes) = legacy_bytes(raw)
        && let Some(text) =
            encoding_rs::SHIFT_JIS.decode_without_bom_handling_and_without_replacement(&bytes)
    {
        return Decoded {
            text: text.into_owned(),
            invalid: false,
            shift_jis: true,
        };
    }
    let (text, invalid) = standard(raw);
    Decoded {
        text,
        invalid,
        shift_jis: false,
    }
}

/// 8 ビット値だけで書かれた文字列。Unicode・生 UTF-8・不正なエスケープは全体を従来の読み方へ戻す。
fn legacy_bytes(mut raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(raw.len());
    let mut eight_bit = false;
    while let Some((&byte, tail)) = raw.split_first() {
        if !byte.is_ascii() {
            return None;
        }
        if raw.starts_with(b"''") || raw.starts_with(b"\\\\") {
            out.push(byte);
            raw = raw.get(2..).unwrap_or_default();
        } else if let Some(after) = raw.strip_prefix(b"\\X\\") {
            let value = u8::try_from(hex(after.get(..2)?)?).ok()?;
            out.push(value);
            eight_bit |= value >= 0x80;
            raw = after.get(2..).unwrap_or_default();
        } else if let Some(after) = raw.strip_prefix(b"\\S\\") {
            let (&value, after) = after.split_first()?;
            if !(32..=126).contains(&value) {
                return None;
            }
            out.push(value + 128);
            eight_bit = true;
            raw = after;
        } else if let Some(after) = raw.strip_prefix(b"\\X2\\") {
            let count = after.iter().take_while(|b| b.is_ascii_hexdigit()).count();
            if !count.is_multiple_of(4) {
                return None;
            }
            raw = after.get(count..)?.strip_prefix(b"\\X0\\")?;
            for unit in after.get(..count)?.as_chunks::<4>().0 {
                let value = u8::try_from(hex(unit)?).ok()?;
                out.push(value);
                eight_bit |= value >= 0x80;
            }
        } else if let [b'\\', b'P', b'A', b'\\', after @ ..] = raw {
            raw = after;
        } else if byte == b'\\' {
            return None;
        } else {
            out.push(byte);
            raw = tail;
        }
    }
    eight_bit.then_some(out)
}

fn hex(raw: &[u8]) -> Option<u32> {
    raw.iter().try_fold(0, |value, byte| {
        char::from(*byte)
            .to_digit(16)
            .map(|digit| value * 16 + digit)
    })
}

/// 既存の ISO 8859-1/Unicode の読み取り。生の 8 ビット列は UTF-8 として読む。
pub(crate) fn standard(raw: &[u8]) -> (String, bool) {
    let mut invalid = false;
    let mut written = 0usize;
    let text = zumen_step::decode(raw, |rest| {
        for len in 2..=4 {
            if let Some(text) = rest.get(..len).and_then(|b| core::str::from_utf8(b).ok()) {
                if text == "\u{FFFD}" {
                    written += 1;
                }
                return (text.to_owned(), len);
            }
        }
        invalid = true;
        (char::REPLACEMENT_CHARACTER.to_string(), 1)
    });
    let replaced = text.matches(char::REPLACEMENT_CHARACTER).count() > written;
    (text, invalid || replaced)
}
