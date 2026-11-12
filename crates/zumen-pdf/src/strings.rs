//! PDF のテキスト文字列と日付を、Unicode の文字列に戻す。

/// 解釈した PDF の日付（7.9.4）
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParsedDate {
    /// ISO 8601 の日時（`2026-10-23T09:30:00+09:00` の形）
    pub iso: String,
    /// UT との時差（`Z`・`+`・`-`）が書かれていたか
    pub utc_offset_known: bool,
    /// 省略された月・日・時・分・秒や時差を既定値（`01`・`00`・`+00:00`）で補ったか
    pub approximated: bool,
}

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

/// 日付（7.9.4）。`(D:YYYYMMDDHHmmSSOHH'mm)`（末尾の `'` も許す）を読み、`2026-10-23T09:30:00+09:00` の形
/// （ISO 8601）にする。`D:` を省いた文字列も、全体が同じ形なら読む（PDF 1.7 の 7.9.4）。時差を書かない日付
/// （UT との関係が分からない）は、`utc_offset_known` を `false` にし、ISO 8601 の文字列は `+00:00` で補って
/// `approximated` を `true` にする。月・日・時・分・秒や時差の分を省いた日付も、7.9.4 の既定値で補って
/// `approximated` を `true` にする。暦にない日や余りがある文字列は `None` を返す
pub(crate) fn parse_date(bytes: &[u8]) -> Option<ParsedDate> {
    let mut rest = bytes.strip_prefix(b"D:").unwrap_or(bytes);
    let year = take_digits(&mut rest, 4, 0, 9999)?;

    let mut approximated = false;
    let month = take_optional_field(&mut rest, 1, 12, 1, &mut approximated)? as u8;
    let max_day = days_in_month(year, month);
    let day = take_optional_field(&mut rest, 1, u16::from(max_day), 1, &mut approximated)? as u8;
    let hour = take_optional_field(&mut rest, 0, 23, 0, &mut approximated)? as u8;
    let minute = take_optional_field(&mut rest, 0, 59, 0, &mut approximated)? as u8;
    let second = take_optional_field(&mut rest, 0, 59, 0, &mut approximated)? as u8;

    let (utc_offset_known, sign, offset_hour, offset_minute) = match rest.first().copied() {
        None => {
            approximated = true;
            (false, '+', 0u8, 0u8)
        }
        Some(b'Z') => {
            rest = rest.get(1..)?;
            if !rest.is_empty() {
                let z_hour = take_digits(&mut rest, 2, 0, 0)? as u8;
                let z_minute = take_offset_minutes(&mut rest, &mut false)? as u8;
                if z_hour != 0 || z_minute != 0 {
                    return None;
                }
            }
            (true, '+', 0, 0)
        }
        Some(mark @ (b'+' | b'-')) => {
            rest = rest.get(1..)?;
            let offset_hour = take_digits(&mut rest, 2, 0, 23)? as u8;
            let offset_minute = take_offset_minutes(&mut rest, &mut approximated)? as u8;
            let sign = if mark == b'-' && (offset_hour != 0 || offset_minute != 0) {
                '-'
            } else {
                '+'
            };
            (true, sign, offset_hour, offset_minute)
        }
        Some(_) => return None,
    };

    if !rest.is_empty() {
        return None;
    }

    let iso = format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}{sign}{offset_hour:02}:{offset_minute:02}"
    );
    Some(ParsedDate {
        iso,
        utc_offset_known,
        approximated,
    })
}

fn take_digits(rest: &mut &[u8], len: usize, min: u16, max: u16) -> Option<u16> {
    let head = rest.get(..len)?;
    let mut value = 0u16;
    for &byte in head {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u16::from(byte - b'0'))?;
    }
    if value < min || value > max {
        return None;
    }
    *rest = rest.get(len..)?;
    Some(value)
}

fn take_optional_field(
    rest: &mut &[u8],
    min: u16,
    max: u16,
    default: u16,
    approximated: &mut bool,
) -> Option<u16> {
    if matches!(rest.first(), None | Some(b'+' | b'-' | b'Z')) {
        *approximated = true;
        return Some(default);
    }
    take_digits(rest, 2, min, max)
}

fn take_offset_minutes(rest: &mut &[u8], approximated: &mut bool) -> Option<u16> {
    if rest.is_empty() {
        *approximated = true;
        return Some(0);
    }
    *rest = rest.strip_prefix(b"'")?;
    if rest.is_empty() {
        *approximated = true;
        return Some(0);
    }
    let minutes = take_digits(rest, 2, 0, 59)?;
    if let Some(after_quote) = rest.strip_prefix(b"'") {
        *rest = after_quote;
    }
    Some(minutes)
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => {
            29
        }
        2 => 28,
        _ => 0,
    }
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
    fn parses_exact_approximated_and_invalid_pdf_dates() {
        assert_eq!(
            parse_date(b"D:20261023093000+09'00'"),
            Some(ParsedDate {
                iso: "2026-10-23T09:30:00+09:00".to_owned(),
                utc_offset_known: true,
                approximated: false,
            })
        );
        assert_eq!(
            parse_date(b"D:20261023093000-05'30"),
            Some(ParsedDate {
                iso: "2026-10-23T09:30:00-05:30".to_owned(),
                utc_offset_known: true,
                approximated: false,
            })
        );
        assert_eq!(
            parse_date(b"D:20261023093000-00'45'"),
            Some(ParsedDate {
                iso: "2026-10-23T09:30:00-00:45".to_owned(),
                utc_offset_known: true,
                approximated: false,
            })
        );
        assert_eq!(
            parse_date(b"D:20261023093000Z"),
            Some(ParsedDate {
                iso: "2026-10-23T09:30:00+00:00".to_owned(),
                utc_offset_known: true,
                approximated: false,
            })
        );
        assert_eq!(
            parse_date(b"D:20261023093000Z00'00'"),
            Some(ParsedDate {
                iso: "2026-10-23T09:30:00+00:00".to_owned(),
                utc_offset_known: true,
                approximated: false,
            })
        );
        assert_eq!(
            parse_date(b"20261023093000+09'00'"),
            Some(ParsedDate {
                iso: "2026-10-23T09:30:00+09:00".to_owned(),
                utc_offset_known: true,
                approximated: false,
            })
        );

        // 時差なし（UT との関係が不明）と、秒・時差の分の省略
        assert_eq!(
            parse_date(b"D:20261023093000"),
            Some(ParsedDate {
                iso: "2026-10-23T09:30:00+00:00".to_owned(),
                utc_offset_known: false,
                approximated: true,
            })
        );
        assert_eq!(
            parse_date(b"D:199812231952-08'00'"),
            Some(ParsedDate {
                iso: "1998-12-23T19:52:00-08:00".to_owned(),
                utc_offset_known: true,
                approximated: true,
            })
        );
        assert_eq!(
            parse_date(b"D:2023+02"),
            Some(ParsedDate {
                iso: "2023-01-01T00:00:00+02:00".to_owned(),
                utc_offset_known: true,
                approximated: true,
            })
        );
        assert_eq!(
            parse_date(b"D:20240229000000Z"),
            Some(ParsedDate {
                iso: "2024-02-29T00:00:00+00:00".to_owned(),
                utc_offset_known: true,
                approximated: false,
            })
        );

        // 暦にない日、範囲外の時刻・時差、末尾の余り、読めない文字列
        for invalid in [
            b"".as_slice(),
            b"D:",
            b"D:20230229000000Z",
            b"D:20260431000000Z",
            b"D:20261301000000Z",
            b"D:20261023240000Z",
            b"D:20261023096000Z",
            b"D:20261023093000+24'00'",
            b"D:20261023093000Z01'00'",
            b"D:20261023093000+09'00'extra",
            b"2026/10/23 09:30:00",
        ] {
            assert_eq!(parse_date(invalid), None, "{invalid:?}");
        }
    }
}
