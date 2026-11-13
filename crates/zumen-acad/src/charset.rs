//! DXF と DWG のオブジェクト表に記録された文字の符号化（決定 0175・0192）。

use std::borrow::Cow;

use crate::db::{AcadDb, Data};

/// UTF-8 で書く最初のバージョン（2007）
const UTF8: &[u8] = b"AC1021";

/// 文字の値の符号化。2007 以降のバージョンは UTF-8 で、それより前のバージョンは `$DWGCODEPAGE` のコード・ページで決まる
/// （DXF Reference の HEADER Section Group Codes）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Charset {
    Utf8,
    /// ANSI_932。WHATWG の Shift_JIS（Windows の 932 と同じ表）として戻す
    ShiftJis,
    /// 対応表のないコード・ページか、コード・ページがない。ASCII の文字だけを読む
    Other,
}

impl Charset {
    /// 文字の値を戻す。戻せないバイト列と、対応表のないコード・ページの ASCII でない文字は `None`
    pub(crate) fn decode(self, value: &[u8]) -> Option<String> {
        match self {
            Charset::Utf8 => core::str::from_utf8(value).ok().map(str::to_owned),
            _ if value.is_ascii() => core::str::from_utf8(value).ok().map(str::to_owned),
            #[cfg(feature = "cp932")]
            Charset::ShiftJis => encoding_rs::SHIFT_JIS
                .decode_without_bom_handling_and_without_replacement(value)
                .map(Cow::into_owned),
            _ => None,
        }
    }

    fn encoded(self, text: &str) -> Option<Cow<'_, [u8]>> {
        if self == Self::Utf8 || text.is_ascii() {
            return Some(Cow::Borrowed(text.as_bytes()));
        }
        #[cfg(feature = "cp932")]
        if self == Self::ShiftJis {
            let (bytes, _, unmappable) = encoding_rs::SHIFT_JIS.encode(text);
            return (!unmappable).then_some(bytes);
        }
        None
    }

    /// 同系統の変換で、同じバイト列に戻る表の名前だけを使う。
    pub(crate) fn decode_name(self, bytes: &[u8]) -> Option<String> {
        let name = self.decode(bytes)?;
        (self.encoded(&name)?.as_ref() == bytes).then_some(name)
    }

    /// 文字列を符号化する。戻すと別の文字になる場合は、その最初の文字を返す（決定 0192）。
    #[cfg(feature = "drawing")]
    pub(crate) fn encode(self, value: &str) -> Result<Vec<u8>, char> {
        if self == Self::Utf8 || value.is_ascii() {
            return Ok(value.as_bytes().to_vec());
        }
        // ¥（U+00A5）や −（U+2212）は、符号化できても別の文字に戻る。
        if let Some(bytes) = self.encoded(value)
            && self.decode(&bytes).as_deref() == Some(value)
        {
            return Ok(bytes.into_owned());
        }
        Err(value
            .chars()
            .find(|c| {
                let mut buffer = [0; 4];
                let ch = c.encode_utf8(&mut buffer);
                !self
                    .encoded(ch)
                    .is_some_and(|bytes| self.decode(&bytes).as_deref() == Some(ch))
            })
            .unwrap_or(char::REPLACEMENT_CHARACTER))
    }

    /// 名前を、大文字と小文字を区別せずに比べるための形にする。ASCII の文字だけを小文字にし、Shift_JIS の
    /// 2 バイトの文字の 2 バイト目は変えない（読むときに、戻した文字の ASCII だけを小文字にするのと同じ）
    pub(crate) fn fold(self, name: &[u8]) -> Vec<u8> {
        if self != Charset::ShiftJis {
            return name.to_ascii_lowercase();
        }
        let mut folded = Vec::with_capacity(name.len());
        let mut trail = false;
        for &byte in name {
            if trail {
                folded.push(byte);
                trail = false;
            } else {
                folded.push(byte.to_ascii_lowercase());
                trail = matches!(byte, 0x81..=0x9F | 0xE0..=0xFC);
            }
        }
        folded
    }

    /// 2 つの名前が、大文字と小文字を区別せずに同じか
    #[cfg(feature = "drawing")]
    pub(crate) fn same(self, a: &[u8], b: &[u8]) -> bool {
        a.len() == b.len() && self.fold(a) == self.fold(b)
    }
}

/// 表 `db` の文字の値の符号化
pub(crate) fn charset_of(db: &AcadDb) -> Charset {
    let variable = |name: &[u8]| {
        db.variable(name)
            .and_then(|values| match values.first().map(|v| &v.data) {
                Some(Data::Text(text)) => Some(text.trim_ascii()),
                _ => None,
            })
    };
    charset(variable(b"$ACADVER"), variable(b"$DWGCODEPAGE"))
}

pub(crate) fn charset(version: Option<&[u8]>, code_page: Option<&[u8]>) -> Charset {
    if version.is_some_and(|v| v >= UTF8) {
        Charset::Utf8
    } else if code_page.is_some_and(|page| {
        page.eq_ignore_ascii_case(b"ANSI_932") || page.eq_ignore_ascii_case(b"DOS932")
    }) {
        Charset::ShiftJis
    } else {
        Charset::Other
    }
}
