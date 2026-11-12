//! XML の文書の先頭から、最初の要素の名前と名前空間を読む（[`crate::Signature::XmlRoot`]）。
//!
//! XML 宣言・注記・処理命令・文書型宣言を飛ばし、最初の開始タグだけを見る。UTF-16 は XML 1.0 の付録 F の署名で
//! バイト順を選び、原本のコード単位を直接たどる。宣言の正否と、開始タグより後の本文は、コーデックで検査する。

use core::ops::Range;

/// 最初の開始タグの名前と名前空間が、規則に合うか。タグが先頭のバイト列の中で終わらなければ、合わない
pub(crate) fn matches(head: &[u8], name: &[u8], namespace: Option<&[u8]>) -> bool {
    let Some((mut input, mut at)) = Input::new(head) else {
        return false;
    };
    loop {
        at = input.skip_space(at);
        let after = if input.starts(at, b"<?") {
            let after = input.after_pattern(at + 2 * input.width, b"?>");
            if at == 0
                && let Some(after) = after
            {
                input.declaration_encoding(at, after - 2 * input.width);
            }
            after
        } else if input.starts(at, b"<!--") {
            input.after_pattern(at + 4 * input.width, b"-->")
        } else if input.starts(at, b"<!DOCTYPE") {
            input.after_doctype(at + 9 * input.width)
        } else {
            return input.starts(at, b"<")
                && input.start_tag(at + input.width, name, namespace) == Some(true);
        };
        let Some(after) = after else {
            return false;
        };
        at = after;
    }
}

/// 原本を借りるビュー。位置はバイト単位で、UTF-16 でもマークアップをコード単位の境界で探す
struct Input<'h> {
    head: &'h [u8],
    width: usize,
    little: bool,
    shift_jis: bool,
}

impl<'h> Input<'h> {
    fn new(head: &'h [u8]) -> Option<(Self, usize)> {
        // XML 1.0 付録 F。UTF-32 の署名を、先頭の 2 バイトだけで UTF-16 とみなさない
        if matches!(
            head,
            [0, 0, 0xfe, 0xff, ..]
                | [0xff, 0xfe, 0, 0, ..]
                | [0, 0, 0xff, 0xfe, ..]
                | [0xfe, 0xff, 0, 0, ..]
                | [0, 0, 0, 0x3c, ..]
                | [0x3c, 0, 0, 0, ..]
                | [0, 0, 0x3c, 0, ..]
                | [0, 0x3c, 0, 0, ..]
        ) {
            return None;
        }
        let (width, little, at) = match head {
            [0xff, 0xfe, ..] => (2, true, 2),
            [0xfe, 0xff, ..] => (2, false, 2),
            [0x3c, 0, 0x3f, 0, ..] => (2, true, 0),
            [0, 0x3c, 0, 0x3f, ..] => (2, false, 0),
            [0xef, 0xbb, 0xbf, ..] => (1, false, 3),
            _ => (1, false, 0),
        };
        Some((
            Self {
                head,
                width,
                little,
                shift_jis: false,
            },
            at,
        ))
    }

    fn unit(&self, at: usize) -> Option<u16> {
        let &first = self.head.get(at)?;
        if self.width == 1 {
            return Some(u16::from(first));
        }
        let &second = self.head.get(at.checked_add(1)?)?;
        Some(if self.little {
            u16::from_le_bytes([first, second])
        } else {
            u16::from_be_bytes([first, second])
        })
    }

    fn starts(&self, at: usize, pattern: &[u8]) -> bool {
        pattern.iter().enumerate().all(|(index, byte)| {
            index
                .checked_mul(self.width)
                .and_then(|offset| at.checked_add(offset))
                .and_then(|at| self.unit(at))
                == Some(u16::from(*byte))
        })
    }

    fn skip_space(&self, mut at: usize) -> usize {
        while self.unit(at).is_some_and(is_space) {
            at += self.width;
        }
        at
    }

    fn find(&self, mut at: usize, unit: u16) -> Option<usize> {
        while let Some(found) = self.unit(at) {
            if found == unit {
                return Some(at);
            }
            at += self.width;
        }
        None
    }

    fn after_pattern(&self, mut at: usize, pattern: &[u8]) -> Option<usize> {
        while self.unit(at).is_some() {
            if self.starts(at, pattern) {
                return Some(at + pattern.len() * self.width);
            }
            at += self.width;
        }
        None
    }

    fn after_doctype(&self, mut at: usize) -> Option<usize> {
        let mut depth = 0usize;
        loop {
            let unit = self.unit(at)?;
            if self.shift_jis && is_shift_jis_lead(unit) {
                at = self.after_character(at)?;
                continue;
            }
            at += self.width;
            match unit {
                0x22 | 0x27 => at = self.find(at, unit)? + self.width,
                0x5b => depth = depth.checked_add(1)?,
                0x5d => depth = depth.checked_sub(1)?,
                0x3e if depth == 0 => return Some(at),
                _ => {}
            }
        }
    }

    fn name_end(&self, mut at: usize) -> Option<usize> {
        loop {
            let unit = self.unit(at)?;
            if !is_name(unit) {
                return Some(at);
            }
            at = self.after_character(at)?;
        }
    }

    /// WHATWG Shift_JIS 12.3.1。2 バイト目の [・] を名前や DTD の区切りとしない。
    fn after_character(&self, at: usize) -> Option<usize> {
        if self.shift_jis && is_shift_jis_lead(self.unit(at)?) {
            let trail = self.unit(at.checked_add(1)?)?;
            matches!(trail, 0x40..=0x7e | 0x80..=0xfc).then(|| at + 2)
        } else {
            at.checked_add(self.width)
        }
    }

    /// 宣言のラベルだけを借りて読む。正否の検査と UTF-8 BOM の制約はコーデックの責務。
    fn declaration_encoding(&mut self, start: usize, end: usize) {
        if self.width != 1
            || !self.starts(start, b"<?xml")
            || !self.unit(start + 5).is_some_and(is_space)
        {
            return;
        }
        let mut at = start + 5;
        while at < end {
            at = self.skip_space(at);
            let Some(name_end) = self.name_end(at) else {
                return;
            };
            let attribute = at..name_end;
            at = self.skip_space(name_end);
            if !self.starts(at, b"=") {
                return;
            }
            at = self.skip_space(at + 1);
            let Some(quote @ (0x22 | 0x27)) = self.unit(at) else {
                return;
            };
            let value = at + 1;
            let Some(value_end) = self.find(value, quote) else {
                return;
            };
            if value_end > end {
                return;
            }
            if self.equals(attribute, b"encoding") {
                self.shift_jis = self.head.get(value..value_end).is_some_and(|label| {
                    encoding_rs::Encoding::for_label(label) == Some(encoding_rs::SHIFT_JIS)
                });
                return;
            }
            at = value_end + 1;
        }
    }

    /// 名前と宣言の値を比較する。UTF-16 のときの規則は UTF-8 で表した名前として比べ、確保はしない
    fn equals(&self, range: Range<usize>, expected: &[u8]) -> bool {
        let Some(bytes) = self.head.get(range) else {
            return false;
        };
        if self.width == 1 {
            if self.shift_jis {
                return equals_shift_jis(bytes, expected);
            }
            return bytes == expected;
        }
        let Ok(expected) = core::str::from_utf8(expected) else {
            return false;
        };
        let (pairs, trailing) = bytes.as_chunks::<2>();
        if !trailing.is_empty() {
            return false;
        }
        let units = pairs.iter().map(|pair| {
            if self.little {
                u16::from_le_bytes(*pair)
            } else {
                u16::from_be_bytes(*pair)
            }
        });
        char::decode_utf16(units).eq(expected.chars().map(Ok))
    }

    fn start_tag(
        &self,
        at: usize,
        expected_name: &[u8],
        expected_namespace: Option<&[u8]>,
    ) -> Option<bool> {
        let end = self.name_end(at)?;
        let colon = (at..end)
            .step_by(self.width)
            .find(|at| self.unit(*at) == Some(0x3a));
        let (prefix, local) = match colon {
            Some(colon) => (Some(at..colon), colon + self.width..end),
            None => (None, at..end),
        };
        if local.is_empty() {
            return None;
        }
        let name_matches = self.equals(local, expected_name);
        let mut at = end;
        let mut namespace = None;
        loop {
            at = self.skip_space(at);
            if self.starts(at, b">") || self.starts(at, b"/>") {
                return Some(
                    name_matches
                        && expected_namespace.is_none_or(|expected| {
                            namespace.is_some_and(|range| self.equals(range, expected))
                        }),
                );
            }
            let end = self.name_end(at)?;
            let attribute = at..end;
            at = self.skip_space(end);
            if !self.starts(at, b"=") {
                return None;
            }
            at = self.skip_space(at + self.width);
            let quote = self.unit(at)?;
            if !matches!(quote, 0x22 | 0x27) {
                return None;
            }
            let start = at + self.width;
            let end = self.find(start, quote)?;
            let declares = match &prefix {
                None => self.equals(attribute, b"xmlns"),
                Some(prefix) => {
                    self.starts(attribute.start, b"xmlns:")
                        && self
                            .head
                            .get(attribute.start + 6 * self.width..attribute.end)
                            == self.head.get(prefix.clone())
                }
            };
            if declares && start < end {
                namespace = Some(start..end);
            }
            at = end + self.width;
        }
    }
}

fn is_shift_jis_lead(unit: u16) -> bool {
    matches!(unit, 0x81..=0x9f | 0xe0..=0xfc)
}

/// 入力を確保せず、固定 4 バイトの出力を期待名と順に比べる。不正な符号化を置換しない。
fn equals_shift_jis(mut bytes: &[u8], expected: &[u8]) -> bool {
    if expected.is_ascii() {
        return bytes == expected;
    }
    let Ok(expected) = core::str::from_utf8(expected) else {
        return false;
    };
    let mut expected = expected.chars();
    let mut decoder = encoding_rs::SHIFT_JIS.new_decoder_without_bom_handling();
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
        if !text.chars().all(|value| expected.next() == Some(value)) {
            return false;
        }
        let Some(rest) = bytes.get(read..) else {
            return false;
        };
        bytes = rest;
        match result {
            encoding_rs::DecoderResult::InputEmpty => {
                return bytes.is_empty() && expected.next().is_none();
            }
            encoding_rs::DecoderResult::OutputFull if read != 0 || written != 0 => {}
            _ => return false,
        }
    }
}

fn is_space(unit: u16) -> bool {
    matches!(unit, 0x09 | 0x0a | 0x0c | 0x0d | 0x20)
}

fn is_name(unit: u16) -> bool {
    !is_space(unit)
        && !matches!(
            unit,
            0x3c | 0x3e | 0x2f | 0x3d | 0x21 | 0x3f | 0x22 | 0x27 | 0x26 | 0x3b | 0x5b | 0x5d
        )
}
