//! XML 1.0 の NameStartChar / NameChar と、名前空間 1.0 の QName。

/// 名前の文字を順に確かめる。接頭辞と局所名は空にせず、コロンを一つまで認める。
struct QName {
    initial: bool,
    colon: bool,
}

impl QName {
    fn new() -> Self {
        Self {
            initial: true,
            colon: false,
        }
    }

    fn push(&mut self, value: char) -> bool {
        if value == ':' {
            if self.initial || self.colon {
                return false;
            }
            self.colon = true;
            self.initial = true;
            return true;
        }
        let start = matches!(value as u32,
            0x41..=0x5a | 0x5f | 0x61..=0x7a | 0xc0..=0xd6 | 0xd8..=0xf6
            | 0xf8..=0x2ff | 0x370..=0x37d | 0x37f..=0x1fff | 0x200c..=0x200d
            | 0x2070..=0x218f | 0x2c00..=0x2fef | 0x3001..=0xd7ff
            | 0xf900..=0xfdcf | 0xfdf0..=0xfffd | 0x10000..=0xeffff);
        let character = start
            || matches!(value as u32, 0x2d | 0x2e | 0x30..=0x39 | 0xb7 | 0x300..=0x36f | 0x203f..=0x2040);
        if (self.initial && !start) || !character {
            return false;
        }
        self.initial = false;
        true
    }
}

pub(crate) fn valid_utf8(bytes: &[u8]) -> bool {
    let Ok(name) = core::str::from_utf8(bytes) else {
        return false;
    };
    let mut state = QName::new();
    name.chars().all(|value| state.push(value)) && !state.initial
}

/// WHATWG Shift_JIS の一文字の境界。名前や DTD の括弧を、2 バイト目と取り違えない。
pub(crate) fn shift_jis_width(bytes: &[u8], at: usize) -> Option<usize> {
    let byte = *bytes.get(at)?;
    if matches!(byte, 0x81..=0x9f | 0xe0..=0xfc) {
        let trail = *bytes.get(at.checked_add(1)?)?;
        matches!(trail, 0x40..=0x7e | 0x80..=0xfc).then_some(2)
    } else {
        Some(1)
    }
}

/// 名前を確保せず、固定サイズの出力へ復号して検査する。不正な符号化は置換しない。
pub(crate) fn valid_shift_jis(mut bytes: &[u8]) -> bool {
    let mut decoder = encoding_rs::SHIFT_JIS.new_decoder_without_bom_handling();
    let mut state = QName::new();
    loop {
        let mut output = [0; 4];
        let (result, read, written) =
            decoder.decode_to_utf8_without_replacement(bytes, &mut output, true);
        let Some(text) = output
            .get(..written)
            .and_then(|value| core::str::from_utf8(value).ok())
        else {
            return false;
        };
        if !text.chars().all(|value| state.push(value)) {
            return false;
        }
        let Some(rest) = bytes.get(read..) else {
            return false;
        };
        bytes = rest;
        match result {
            encoding_rs::DecoderResult::InputEmpty => return bytes.is_empty() && !state.initial,
            encoding_rs::DecoderResult::OutputFull if read != 0 || written != 0 => {}
            _ => return false,
        }
    }
}
