//! 文字と属性の値を、文字にする（XML 1.0 の 2.11・3.3.3・4.6）。

use crate::Encoding;

/// 戻し方
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// 要素の中の文字
    Text,
    /// `<![CDATA[…]]>` の内側。参照を戻さない
    CData,
    /// 属性の値。空白の文字を空白にする
    Attribute,
}

/// 定義済みの実体
const PREDEFINED: [(&str, char); 5] = [
    ("lt", '<'),
    ("gt", '>'),
    ("amp", '&'),
    ("apos", '\''),
    ("quot", '"'),
];

/// 文字の参照（`#…`）か定義済みの実体の名前の文字。どちらでもなければ `None`
pub(crate) fn reference(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix('x') {
            Some(hex) if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
                u32::from_str_radix(hex, 16).ok()?
            }
            Some(_) => return None,
            None if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) => {
                number.parse().ok()?
            }
            None => return None,
        };
        return char::from_u32(code).filter(|c| *c != '\0');
    }
    PREDEFINED
        .iter()
        .find(|(entity, _)| *entity == name)
        .map(|(_, c)| *c)
}

pub(crate) fn decode(encoding: Encoding, raw: &[u8], mode: Mode) -> String {
    let decoded = match encoding {
        Encoding::Utf8 => String::from_utf8_lossy(raw),
        Encoding::ShiftJis => encoding_rs::SHIFT_JIS.decode_without_bom_handling(raw).0,
    };
    let mut out = String::with_capacity(decoded.len());
    let mut rest = decoded.as_ref();
    while let Some(c) = rest.chars().next() {
        rest = rest.get(c.len_utf8()..).unwrap_or_default();
        match c {
            '\r' => {
                rest = rest.strip_prefix('\n').unwrap_or(rest);
                out.push(if mode == Mode::Attribute { ' ' } else { '\n' });
            }
            '\n' | '\t' if mode == Mode::Attribute => out.push(' '),
            '&' if mode != Mode::CData => {
                let found = rest
                    .find(';')
                    .and_then(|end| Some((reference(rest.get(..end)?)?, end)));
                match found {
                    Some((c, end)) => {
                        out.push(c);
                        rest = rest.get(end + 1..).unwrap_or_default();
                    }
                    None => out.push('&'),
                }
            }
            c => out.push(c),
        }
    }
    out
}
