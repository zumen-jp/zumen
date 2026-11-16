//! 追加する値だけを、元の文字コードと改行にする。
//! XML 1.0 §2.2・§2.4・§2.11・§4.1: https://www.w3.org/TR/xml/

use super::*;

pub(super) struct Writer {
    pub(super) encoding: Encoding,
    pub(super) newline: &'static str,
}

impl Writer {
    pub(super) fn new(doc: &Document, cx: &mut Cx<'_>, at: &Ref) -> Result<Self, Error> {
        spend(cx, Kind::Work, doc.src().len() as u64, at)?;
        let mut writer = Self {
            encoding: doc.encoding(),
            newline: "\n",
        };
        for index in (0..doc.src().len()).step_by(writer.width()) {
            match writer.ascii(doc.src(), index) {
                Some(b'\r') => {
                    writer.newline =
                        if writer.ascii(doc.src(), index + writer.width()) == Some(b'\n') {
                            "\r\n"
                        } else {
                            "\r"
                        };
                    break;
                }
                Some(b'\n') => break,
                _ => {}
            }
        }
        Ok(writer)
    }

    pub(super) fn width(&self) -> usize {
        match self.encoding {
            Encoding::Utf16Le | Encoding::Utf16Be => 2,
            _ => 1,
        }
    }

    pub(super) fn ascii(&self, bytes: &[u8], index: usize) -> Option<u8> {
        let word = match self.encoding {
            Encoding::Utf16Le => {
                u16::from_le_bytes([*bytes.get(index)?, *bytes.get(index.checked_add(1)?)?])
            }
            Encoding::Utf16Be => {
                u16::from_be_bytes([*bytes.get(index)?, *bytes.get(index.checked_add(1)?)?])
            }
            _ => u16::from(*bytes.get(index)?),
        };
        (word <= 127).then_some(word as u8)
    }

    pub(super) fn encode(&self, value: &str, cx: &mut Cx<'_>, at: &Ref) -> Result<Vec<u8>, Error> {
        spend(cx, Kind::Work, value.len() as u64, at)?;
        // UTF-16 と Shift_JIS の変換・復号で使う一時領域も、確保前に数える。
        spend(
            cx,
            Kind::DecodedBytes,
            (value.len() as u64).saturating_mul(8).saturating_add(32),
            at,
        )?;
        Ok(match self.encoding {
            Encoding::Utf8 => value.as_bytes().to_vec(),
            Encoding::ShiftJis => {
                let (encoded, _, changed) = encoding_rs::SHIFT_JIS.encode(value);
                if changed
                    || encoding_rs::SHIFT_JIS
                        .decode_without_bom_handling(&encoded)
                        .0
                        != value
                {
                    return Err(invalid(cx, codes::XML_EDIT_ENCODING_FAILED, at));
                }
                encoded.into_owned()
            }
            Encoding::Utf16Le => value.encode_utf16().flat_map(u16::to_le_bytes).collect(),
            Encoding::Utf16Be => value.encode_utf16().flat_map(u16::to_be_bytes).collect(),
        })
    }

    pub(super) fn text(&self, value: &str, cx: &mut Cx<'_>, at: &Ref) -> Result<Vec<u8>, Error> {
        spend(cx, Kind::Work, value.len() as u64, at)?;
        spend(
            cx,
            Kind::DecodedBytes,
            (value.len() as u64).saturating_mul(6),
            at,
        )?;
        let mut escaped = String::new();
        for character in value.chars() {
            if !matches!(character as u32, 9 | 10 | 13 | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
            {
                cx.report(
                    Diagnostic::new(codes::XML_EDIT_CHARACTER_INVALID)
                        .with("code", u64::from(character as u32))
                        .at(at.clone()),
                );
                return Err(Error::Rejected);
            }
            match character {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '\r' => escaped.push_str("&#13;"),
                '\n' => escaped.push_str(self.newline),
                c => escaped.push(c),
            }
        }
        self.encode(&escaped, cx, at)
    }

    pub(super) fn push(
        &self,
        output: &mut Vec<u8>,
        value: &str,
        cx: &mut Cx<'_>,
        at: &Ref,
    ) -> Result<(), Error> {
        append(output, &self.encode(value, cx, at)?, cx, at)
    }
}

pub(super) fn append(
    output: &mut Vec<u8>,
    bytes: &[u8],
    cx: &mut Cx<'_>,
    at: &Ref,
) -> Result<(), Error> {
    spend(cx, Kind::Work, bytes.len() as u64, at)?;
    spend(
        cx,
        Kind::DecodedBytes,
        (bytes.len() as u64).saturating_mul(2),
        at,
    )?;
    output.extend_from_slice(bytes);
    Ok(())
}
