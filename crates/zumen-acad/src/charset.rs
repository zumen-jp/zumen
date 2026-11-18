//! DXF と DWG のオブジェクト表に記録された文字の符号化（決定 0175・0192）。

use std::borrow::Cow;

use crate::db::{AcadDb, Data};

mod windows;

/// UTF-8 で書く最初のバージョン（2007）
const UTF8: &[u8] = b"AC1021";

/// 文字の値の符号化。2007 以降のバージョンは UTF-8 で、それより前のバージョンは `$DWGCODEPAGE` のコード・ページで決まる
/// （DXF Reference の HEADER Section Group Codes）
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Charset {
    /// UTF-8（2007 以降のバージョン）
    #[default]
    Utf8,
    /// ANSI_932。WHATWG の Shift_JIS（Windows の 932 と同じ表）として戻す
    ShiftJis,
    /// Windows の 1 バイトのコード・ページ（ANSI_874・ANSI_1250〜ANSI_1258）。値はコード・ページの番号で、その表
    /// （Unicode の `CP<番号>.TXT`）で戻す。表に割り当てのないバイトは戻さない（#327）
    Windows(u16),
    /// 対応表のないコード・ページか、コード・ページがない。ASCII の文字だけを読む
    Other,
}

impl Charset {
    /// 文字の値を戻す。戻せないバイト列と、対応表のないコード・ページの ASCII でない文字は `None`
    pub fn decode(self, value: &[u8]) -> Option<String> {
        match self {
            Charset::Utf8 => core::str::from_utf8(value).ok().map(str::to_owned),
            _ if value.is_ascii() => core::str::from_utf8(value).ok().map(str::to_owned),
            #[cfg(feature = "cp932")]
            Charset::ShiftJis => encoding_rs::SHIFT_JIS
                .decode_without_bom_handling_and_without_replacement(value)
                .map(Cow::into_owned),
            Charset::Windows(page) => value.iter().map(|&byte| from_windows(page, byte)).collect(),
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
        if let Self::Windows(page) = self {
            return text
                .chars()
                .map(|c| to_windows(page, c))
                .collect::<Option<Vec<u8>>>()
                .map(Cow::Owned);
        }
        None
    }

    /// 同系統の変換で、同じバイト列に戻る表の名前だけを使う。
    pub fn decode_name(self, bytes: &[u8]) -> Option<String> {
        let name = self.decode(bytes)?;
        (self.encoded(&name)?.as_ref() == bytes).then_some(name)
    }

    /// 文字列を符号化する。戻すと別の文字になる場合は、その最初の文字を返す（決定 0192）。
    #[cfg(feature = "drawing")]
    pub fn encode(self, value: &str) -> Result<Vec<u8>, char> {
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
    pub fn fold(self, name: &[u8]) -> Vec<u8> {
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
    pub fn same(self, a: &[u8], b: &[u8]) -> bool {
        a.len() == b.len() && self.fold(a) == self.fold(b)
    }
}

/// Windows のコード・ページ `page` の 1 バイトが表す文字。割り当てのないバイトと、表のない番号は `None`
fn from_windows(page: u16, byte: u8) -> Option<char> {
    if byte.is_ascii() {
        return Some(char::from(byte));
    }
    windows::table(page)?
        .get(usize::from(byte - 0x80))
        .filter(|unit| **unit != 0)
        .and_then(|unit| char::from_u32(u32::from(*unit)))
}

/// 文字を Windows のコード・ページ `page` の 1 バイトにする。表にない文字は `None`
fn to_windows(page: u16, c: char) -> Option<u8> {
    if c.is_ascii() {
        return u8::try_from(c).ok();
    }
    windows::table(page)?
        .iter()
        .position(|unit| *unit != 0 && u32::from(*unit) == u32::from(c))
        .and_then(|index| u8::try_from(0x80 + index).ok())
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

/// バージョン（`$ACADVER`）とコード・ページ（`$DWGCODEPAGE`）から文字の符号化を決める
pub fn charset(version: Option<&[u8]>, code_page: Option<&[u8]>) -> Charset {
    if version.is_some_and(|v| v >= UTF8) {
        Charset::Utf8
    } else if code_page.is_some_and(|page| {
        page.eq_ignore_ascii_case(b"ANSI_932") || page.eq_ignore_ascii_case(b"DOS932")
    }) {
        Charset::ShiftJis
    } else if let Some(page) = code_page.and_then(windows_page) {
        Charset::Windows(page)
    } else {
        Charset::Other
    }
}

/// `ANSI_<番号>` の名前のうち、表のある Windows の 1 バイトのコード・ページの番号
fn windows_page(name: &[u8]) -> Option<u16> {
    let (prefix, digits) = name.split_at_checked(5)?;
    if !prefix.eq_ignore_ascii_case(b"ANSI_") || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let page = core::str::from_utf8(digits).ok()?.parse().ok()?;
    windows::table(page).map(|_| page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_1252_reads_and_writes_every_assigned_byte_and_refuses_the_rest() {
        let page = charset(Some(b"AC1015"), Some(b"ansi_1252"));
        assert_eq!(page, Charset::Windows(1252));
        assert_eq!(charset(Some(b"AC1021"), Some(b"ANSI_1252")), Charset::Utf8);
        assert_eq!(
            page.decode(b"Bohrung \xD820 f\xFCr M6 Ma\xDFstab \x80\x9F")
                .as_deref(),
            Some("Bohrung Ø20 für M6 Maßstab €Ÿ")
        );
        for byte in [0x81_u8, 0x8D, 0x8F, 0x90, 0x9D] {
            assert_eq!(page.decode(&[b'A', byte]), None, "{byte:#X}");
        }
        assert_eq!(page.decode_name(b"Fl\xE4che").as_deref(), Some("Fläche"));
        assert_eq!(page.encoded("日"), None);
        assert_eq!(page.encoded("\u{81}"), None);
    }

    #[test]
    fn windows_code_pages_round_trip_every_assigned_byte() {
        // 割り当てのあるバイトの数（Unicode の CP<番号>.TXT の割り当てのない 0x80〜0xFF を除く）
        let pages = [
            (874, 225),
            (1250, 251),
            (1251, 255),
            (1252, 251),
            (1253, 239),
            (1254, 249),
            (1255, 233),
            (1256, 256),
            (1257, 244),
            (1258, 247),
        ];
        for (number, count) in pages {
            let name = format!("ANSI_{number}");
            let page = charset(Some(b"AC1015"), Some(name.as_bytes()));
            assert_eq!(page, Charset::Windows(number));
            let assigned: Vec<u8> = (0..=u8::MAX)
                .filter(|byte| from_windows(number, *byte).is_some())
                .collect();
            assert_eq!(assigned.len(), count, "{name}");
            let text = page.decode(&assigned).unwrap_or_default();
            assert_eq!(text.chars().count(), count, "{name}");
            assert_eq!(page.encoded(&text).as_deref(), Some(assigned.as_slice()));
            assert_eq!(page.encoded("日"), None, "{name}");
        }
        assert_eq!(
            Charset::Windows(1251)
                .decode(b"\xD1\xF2\xE5\xED\xE0")
                .as_deref(),
            Some("Стена")
        );
        for name in [
            &b"ANSI_936"[..],
            b"ANSI_1200",
            b"ANSI_",
            b"ANSI_+1251",
            b"DOS866",
        ] {
            assert_eq!(charset(Some(b"AC1015"), Some(name)), Charset::Other);
        }
    }
}
