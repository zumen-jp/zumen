//! PDF のテキスト文字列と日付を、Unicode の文字列に戻す。

use hayro_syntax::object::DateTime;

/// テキスト文字列（7.9.2.2）。先頭が FE FF なら UTF-16BE、EF BB BF なら UTF-8（PDF 2.0）、それ以外は
/// PDFDocEncoding（附属書 D の表 D.2）
pub(crate) fn text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        let units = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes(*pair));
        return char::decode_utf16(units)
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    bytes.iter().map(|&byte| pdf_doc_char(byte)).collect()
}

/// PDFDocEncoding の 1 バイト。ISO 8859-1 と違う所だけを表に持つ。定義のないバイトは U+FFFD
fn pdf_doc_char(byte: u8) -> char {
    const LOW: [char; 8] = ['˘', 'ˇ', 'ˆ', '˙', '˝', '˛', '˚', '˜'];
    const HIGH: [char; 33] = [
        '•', '†', '‡', '…', '—', '–', 'ƒ', '⁄', '‹', '›', '−', '‰', '„', '“', '”', '‘', '’', '‚',
        '™', 'ﬁ', 'ﬂ', 'Ł', 'Œ', 'Š', 'Ÿ', 'Ž', 'ı', 'ł', 'œ', 'š', 'ž', '\u{fffd}', '€',
    ];
    match byte {
        0x18..=0x1f => LOW
            .get(usize::from(byte - 0x18))
            .copied()
            .unwrap_or(char::REPLACEMENT_CHARACTER),
        0x80..=0xa0 => HIGH
            .get(usize::from(byte - 0x80))
            .copied()
            .unwrap_or(char::REPLACEMENT_CHARACTER),
        0x7f | 0xad => char::REPLACEMENT_CHARACTER,
        _ => char::from(byte),
    }
}

/// 日付（7.9.4）。`2026-10-23T09:30:00+09:00` の形（ISO 8601）。時差を書かない日付（UT との関係が分からない）は、
/// hayro-syntax が `Z`（UT）と見分けないので、`+00:00` にする（推定）
pub(crate) fn date(date: &DateTime) -> String {
    let sign = if date.utc_offset_hour < 0 { '-' } else { '+' };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{sign}{:02}:{:02}",
        date.year,
        date.month,
        date.day,
        date.hour,
        date.minute,
        date.second,
        date.utc_offset_hour.unsigned_abs(),
        date.utc_offset_minute
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_text_strings_and_names() {
        assert_eq!(text(b"Plan \x84 A"), "Plan — A");
        assert_eq!(text(&[0xfe, 0xff, 0x56, 0xf3, 0x97, 0x62]), "図面");
        assert_eq!(text(&[0xef, 0xbb, 0xbf, 0xe5, 0x9b, 0xb3]), "図");
        assert_eq!(text(b"\x18\xa0\xe9"), "˘€é");
    }

    #[test]
    fn writes_dates_in_iso_8601() {
        let date = DateTime {
            year: 2026,
            month: 10,
            day: 23,
            hour: 9,
            minute: 30,
            second: 0,
            utc_offset_hour: 9,
            utc_offset_minute: 0,
        };
        assert_eq!(super::date(&date), "2026-10-23T09:30:00+09:00");
        let date = DateTime {
            utc_offset_hour: -5,
            utc_offset_minute: 30,
            ..date
        };
        assert_eq!(super::date(&date), "2026-10-23T09:30:00-05:30");
    }
}
