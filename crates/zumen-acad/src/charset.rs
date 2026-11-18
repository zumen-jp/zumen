//! DXF と DWG のオブジェクト表に記録された文字の符号化（決定 0175・0192）。

use std::borrow::Cow;

use crate::db::{AcadDb, Data};

#[cfg(feature = "cp932")]
mod dbcs;
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
    /// Windows のコード・ページ。値はコード・ページの番号で、その表（Unicode の `CP<番号>.TXT`）で戻す。表に割り当ての
    /// ないバイトは戻さない（#327）。1 バイトの ANSI_874・ANSI_1250〜ANSI_1258・DOS437〜DOS869 と、`cp932` の feature
    /// があるときの 2 バイトの ANSI_936・ANSI_949・ANSI_950・ANSI_1361（Johab）。ISO8859-1 などの名前も、AutoCAD と
    /// 同じ番号にする
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
            #[cfg(feature = "cp932")]
            Charset::Windows(page) if dbcs::table(page).is_some() => {
                dbcs::table(page)?.decode(value)
            }
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
        #[cfg(feature = "cp932")]
        if let Self::Windows(page) = self
            && let Some(table) = dbcs::table(page)
        {
            return table.encode(text).map(Cow::Owned);
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

    /// 名前を、大文字と小文字を区別せずに比べるための形にする。ASCII の文字だけを小文字にし、Shift_JIS などの
    /// 2 バイトの文字の 2 バイト目は変えない（読むときに、戻した文字の ASCII だけを小文字にするのと同じ）
    pub fn fold(self, name: &[u8]) -> Vec<u8> {
        if !self.double_byte() {
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
                trail = self.leads(byte);
            }
        }
        folded
    }

    /// 2 バイトの文字を持つ符号化か
    fn double_byte(self) -> bool {
        match self {
            Charset::ShiftJis => true,
            #[cfg(feature = "cp932")]
            Charset::Windows(page) => dbcs::table(page).is_some(),
            _ => false,
        }
    }

    /// `byte` が 2 バイトの文字の先頭のバイトか
    fn leads(self, byte: u8) -> bool {
        match self {
            Charset::ShiftJis => matches!(byte, 0x81..=0x9F | 0xE0..=0xFC),
            #[cfg(feature = "cp932")]
            Charset::Windows(page) => dbcs::table(page).is_some_and(|table| table.leads(byte)),
            _ => false,
        }
    }

    /// 2 つの名前が、大文字と小文字を区別せずに同じか
    #[cfg(feature = "drawing")]
    pub fn same(self, a: &[u8], b: &[u8]) -> bool {
        a.len() == b.len() && self.fold(a) == self.fold(b)
    }
}

/// MTEXT の書式の符号 `\M+<番号><16 進 4 桁>` の 2 バイト `pair` が表す 1 文字。番号は 1 が ANSI_932、2 が
/// ANSI_950、3 が ANSI_949、4 が ANSI_1361（Johab）、5 が ANSI_936（ezdxf の `decode_mif_to_unicode` と ODA の Font
/// Handling による）。そのコード・ページの 2 バイトの文字でないものは `None`
#[cfg(feature = "drawing")]
pub(crate) fn multibyte(number: char, pair: [u8; 2]) -> Option<char> {
    let charset = match number {
        '1' => Charset::ShiftJis,
        '2' => Charset::Windows(950),
        '3' => Charset::Windows(949),
        '4' => Charset::Windows(1361),
        '5' => Charset::Windows(936),
        _ => return None,
    };
    if !charset.leads(pair[0]) {
        return None;
    }
    let text = charset.decode(&pair)?;
    let mut chars = text.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
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

/// AutoCAD が Windows のコード・ページとして読む、`ANSI_<番号>` でない名前と、その番号。AutoCAD 2027 で、自作の
/// DXF の文字を Windows の MultiByteToWideChar と同じに読んだ（docs/formats/dxf.md）。ISO8859-3・4・5・6・8 と
/// MAC-ROMAN は、端末のコード・ページで読んだので、ここに入れない
const ALIASES: [(&[u8], u16); 8] = [
    (b"ISO8859-1", 1252),
    (b"ISO8859-2", 1250),
    (b"ISO8859-7", 1253),
    (b"ISO8859-9", 1254),
    (b"BIG5", 950),
    (b"KSC5601", 949),
    (b"GB2312", 936),
    (b"JOHAB", 1361),
];

/// `ANSI_<番号>`・`DOS<番号>` などの名前のうち、表のある Windows のコード・ページの番号
fn windows_page(name: &[u8]) -> Option<u16> {
    let page = match ALIASES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(name))
    {
        Some((_, page)) => *page,
        None => {
            let (dos, digits) = [b"ANSI_".as_slice(), b"DOS"]
                .into_iter()
                .find_map(|prefix| {
                    let (head, digits) = name.split_at_checked(prefix.len())?;
                    head.eq_ignore_ascii_case(prefix)
                        .then_some((prefix == b"DOS", digits))
                })?;
            if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
                return None;
            }
            let page: u16 = core::str::from_utf8(digits).ok()?.parse().ok()?;
            // DOS の表（437〜869）は DOS の名前だけで、Windows の表は ANSI_ の名前だけで引く
            if dos != (437..=869).contains(&page) {
                return None;
            }
            page
        }
    };
    #[cfg(feature = "cp932")]
    if dbcs::table(page).is_some() {
        return Some(page);
    }
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
            (1255, 234),
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
            &b"ANSI_1200"[..],
            b"ANSI_",
            b"ANSI_+1251",
            b"ANSI_850",
            b"DOS",
            b"DOS1252",
            b"ISO8859-5",
            b"MAC-ROMAN",
            b"ASCII",
        ] {
            assert_eq!(charset(Some(b"AC1015"), Some(name)), Charset::Other);
        }
    }

    #[test]
    fn dos_and_iso_names_read_as_windows_code_pages() {
        // DOS の名前は同じ番号の表で、割り当てのあるバイトの数（Windows の MultiByteToWideChar が C1 制御文字と私用
        // 領域でない文字に戻す 0x80〜0xFF）
        let pages = [
            (437, 128),
            (850, 128),
            (852, 128),
            (855, 128),
            (857, 125),
            (860, 128),
            (861, 128),
            (863, 128),
            (864, 122),
            (865, 128),
            (866, 128),
            (869, 119),
        ];
        for (number, count) in pages {
            let name = format!("dos{number}");
            let page = charset(Some(b"AC1015"), Some(name.as_bytes()));
            assert_eq!(page, Charset::Windows(number));
            let assigned: Vec<u8> = (0x80..=u8::MAX)
                .filter(|byte| from_windows(number, *byte).is_some())
                .collect();
            assert_eq!(assigned.len(), count, "{name}");
            let text = page.decode(&assigned).unwrap_or_default();
            assert_eq!(page.encoded(&text).as_deref(), Some(assigned.as_slice()));
        }
        let dos = Charset::Windows(866);
        assert_eq!(
            dos.decode(b"\x91\xE2\xA5\xAD\xA0 %").as_deref(),
            Some("Стена %")
        );
        assert_eq!(Charset::Windows(864).decode(b"%").as_deref(), Some("%"));
        assert_eq!(Charset::Windows(857).decode(b"\xD5"), None);
        // ISO8859 の一部と、2 バイトの名前は、Windows のコード・ページとして読む
        for (name, number) in [
            (&b"ISO8859-1"[..], 1252),
            (b"iso8859-2", 1250),
            (b"ISO8859-7", 1253),
            (b"ISO8859-9", 1254),
        ] {
            assert_eq!(
                charset(Some(b"AC1015"), Some(name)),
                Charset::Windows(number)
            );
        }
        #[cfg(feature = "cp932")]
        for (name, number) in [(&b"BIG5"[..], 950), (b"KSC5601", 949), (b"gb2312", 936)] {
            assert_eq!(
                charset(Some(b"AC1015"), Some(name)),
                Charset::Windows(number)
            );
        }
    }

    #[cfg(feature = "cp932")]
    #[test]
    fn windows_double_byte_code_pages_match_the_microsoft_tables() {
        // 割り当てのあるバイト列の数（Unicode の CP<番号>.TXT の 0x80 以上）と、書き戻すと別の 2 バイトになる数
        // （CP950 は、同じ文字を表す 2 バイトが 10 組ある）
        for (number, count, moved) in [
            (936, 21_792, 0),
            (949, 17_048, 0),
            (950, 13_503, 10),
            (1361, 17_064, 0),
        ] {
            let page = Charset::Windows(number);
            let name = format!("ANSI_{number}");
            assert_eq!(charset(Some(b"AC1015"), Some(name.as_bytes())), page);
            let mut assigned = 0;
            let mut elsewhere = 0;
            for lead in 0x80..=u8::MAX {
                let mut sequences = vec![vec![lead]];
                sequences.extend((0..=u8::MAX).map(|trail| vec![lead, trail]));
                for bytes in sequences {
                    let Some(text) = page.decode(&bytes) else {
                        continue;
                    };
                    if text.chars().count() != 1 {
                        continue;
                    }
                    assigned += 1;
                    let encoded = page.encoded(&text);
                    assert!(encoded.is_some(), "{name} {bytes:02X?}");
                    if encoded.as_deref() != Some(bytes.as_slice()) {
                        elsewhere += 1;
                    }
                }
            }
            assert_eq!((assigned, elsewhere), (count, moved), "{name}");
        }
        let gbk = Charset::Windows(936);
        let uhc = Charset::Windows(949);
        let big5 = Charset::Windows(950);
        assert_eq!(
            gbk.decode(b"\xD6\xD0\xCE\xC4 \x80").as_deref(),
            Some("中文 €")
        );
        assert_eq!(uhc.decode(b"\xC7\xD1\xB1\xB9").as_deref(), Some("한국"));
        assert_eq!(big5.decode(b"\xA4\xA4\xA4\xE5").as_deref(), Some("中文"));
        // WHATWG の表だけにある 2 バイト・私用領域・GB 18030 の 4 バイト・途中で切れた 2 バイトは戻さない
        for (page, bytes) in [
            (gbk, &b"\xA2\xE3"[..]),
            (gbk, b"\xAA\xA1"),
            (gbk, b"\x81\x30\x81\x30"),
            (gbk, b"A\xD6"),
            (big5, b"\xC6\xA1"),
            (big5, b"\x87\x40"),
            (big5, b"\xFA\x40"),
            (uhc, b"\xC9\xA1"),
        ] {
            assert_eq!(page.decode(bytes), None, "{page:?} {bytes:02X?}");
        }
        // Microsoft の表に合わせて直す 2 バイト
        assert_eq!(big5.decode(b"\xF9\xFE").as_deref(), Some("\u{2593}"));
        assert_eq!(
            big5.encoded("\u{2593}\u{4EDD}").as_deref(),
            Some(&b"\xF9\xFE\xC9\x69"[..])
        );
        assert_eq!(gbk.encoded("\u{FE10}"), None);
        assert_eq!(gbk.encoded("\u{E000}"), None);
        assert_eq!(uhc.encoded("ภ"), None);
        // 2 バイト目の ASCII の英字は、名前の大小を比べるときに変えない
        assert_eq!(big5.fold(b"\xA4\x41A"), b"\xA4\x41a");
        assert_eq!(big5.decode_name(b"\xA4\x41").as_deref(), Some("乙"));
    }

    #[cfg(feature = "cp932")]
    #[test]
    fn johab_matches_windows() {
        let johab = Charset::Windows(1361);
        assert_eq!(charset(Some(b"AC1015"), Some(b"JOHAB")), johab);
        assert_eq!(charset(Some(b"AC1015"), Some(b"ANSI_1361")), johab);
        // 音節と、1 つだけの字母（Windows は結合用の字母 U+1100〜 に戻す）、記号・漢字（KS X 1001 の行と列に直して
        // 戻す）と、4 行目の古い字母。値は Windows の WideCharToMultiByte が書くもの
        for (bytes, text) in [
            (&b"\xD0\x65\x8A\x82"[..], "한국"),
            (b"\x88\x61\x8B\xBD\xB8\x93", "가깋잽"),
            (
                b"\x88\x41\x84\x61\x84\x42\x84\x5D",
                "\u{1100}\u{1161}\u{11A8}\u{11C2}",
            ),
            (b"\xD9\x31\xE0\x31\xF9\xFE\xDA\xD4", "\u{3000}伽詰\u{115F}"),
        ] {
            assert_eq!(johab.decode(bytes).as_deref(), Some(text), "{bytes:02X?}");
            assert_eq!(johab.encoded(text).as_deref(), Some(bytes), "{text}");
        }
        // 詰め物だけ・字母の組み合わせの一部・4 行目の現代の字母・私用領域・先頭でないバイトは戻さない
        for bytes in [
            &b"\x84\x41"[..],
            b"\x88\x42",
            b"\x84\x62",
            b"\xDA\xA1",
            b"\xD8\x31",
            b"\xD4\x41",
            b"\xD9\x7F",
        ] {
            assert_eq!(johab.decode(bytes), None, "{bytes:02X?}");
        }
        // 互換の字母は表せない
        assert_eq!(johab.encoded("\u{3131}"), None);
        // 2 バイト目の ASCII の英字は、名前の大小を比べるときに変えない
        assert_eq!(johab.fold(b"\x88\x61A"), b"\x88\x61a");
    }
}
