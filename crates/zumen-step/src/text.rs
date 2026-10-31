//! ISO 10303-21 の文字列のエスケープを戻す（決定 0207）。

/// 文字列の内側の、ISO 10303-21 のエスケープ（`''`・`\X2\…\X0\`・`\X4\…\X0\`・`\X\hh`・`\S\c`・`\P?\`・`\\`）を
/// 戻す。
///
/// 規格の外の 8 ビットのバイト（0x80 以上）は、`eight_bit` が読む。`eight_bit` は、そのバイトで始まる残りを受け、
/// 読んだ文字と、読んだバイトの数を返す。文字コードは形式が決める（SXF は Shift_JIS、IFC は UTF-8）。返した数が 0
/// なら、1 バイトを読んだとする
pub fn decode(raw: &[u8], mut eight_bit: impl FnMut(&[u8]) -> (String, usize)) -> String {
    let mut out: Vec<u16> = Vec::with_capacity(raw.len());
    let mut rest = raw;
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'\'' && tail.first() == Some(&b'\'') {
            out.push(u16::from(b'\''));
            rest = tail.get(1..).unwrap_or_default();
            continue;
        }
        if byte == b'\\' {
            if let Some((units, after)) = hex_run(rest, b"\\X2\\", 4) {
                out.extend(units.iter().map(|unit| *unit as u16));
                rest = after;
                continue;
            }
            if let Some((units, after)) = hex_run(rest, b"\\X4\\", 8) {
                for unit in units {
                    let c = char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER);
                    out.extend(c.encode_utf16(&mut [0; 2]).iter());
                }
                rest = after;
                continue;
            }
            if let Some(after) = rest.strip_prefix(b"\\X\\")
                && let Some(hex) = after
                    .get(..2)
                    .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
            {
                out.push(hex_value(hex) as u16);
                rest = after.get(2..).unwrap_or_default();
                continue;
            }
            if let Some(after) = rest.strip_prefix(b"\\S\\")
                && let Some((&c, after)) = after.split_first()
            {
                out.push(u16::from(c.wrapping_add(128)));
                rest = after;
                continue;
            }
            if let [b'\\', b'P', page, b'\\', after @ ..] = rest
                && (b'A'..=b'I').contains(page)
            {
                rest = after;
                continue;
            }
            if tail.first() == Some(&b'\\') {
                out.push(u16::from(b'\\'));
                rest = tail.get(1..).unwrap_or_default();
                continue;
            }
        }
        if byte >= 0x80 {
            let (text, len) = eight_bit(rest);
            out.extend(text.encode_utf16());
            rest = rest.get(len.max(1)..).unwrap_or_default();
            continue;
        }
        out.push(u16::from(byte));
        rest = tail;
    }
    String::from_utf16_lossy(&out)
}

/// `\X2\` などで始まり、`\X0\` で閉じる 16 進の並び。width 桁ずつの値と、閉じの後
fn hex_run<'a>(rest: &'a [u8], open: &[u8], width: usize) -> Option<(Vec<u32>, &'a [u8])> {
    let after = rest.strip_prefix(open)?;
    let run = after
        .iter()
        .take_while(|byte| byte.is_ascii_hexdigit())
        .count();
    if !run.is_multiple_of(width) {
        return None;
    }
    let (hex, close) = after.split_at(run);
    let close = close.strip_prefix(b"\\X0\\")?;
    Some((hex.chunks(width).map(hex_value).collect(), close))
}

fn hex_value(hex: &[u8]) -> u32 {
    hex.iter().fold(0, |value, byte| {
        value * 16 + char::from(*byte).to_digit(16).unwrap_or_default()
    })
}
