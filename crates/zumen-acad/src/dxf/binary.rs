//! バイナリの DXF（DXF Reference「Binary DXF Files」）。
//!
//! 先頭の印の後に、グループ・コードと値の組が続く。グループ・コードは R12 までは 1 バイト
//! （255 なら続く 2 バイトがグループ・コード）、R13 からは 2 バイトの符号つき整数。値の型は
//! グループ・コードで決まり、文字列とハンドルは NUL で終わるバイト列、整数は 2・4・8 バイト、
//! 実数は 8 バイト、真偽は 1 バイト、バイナリは長さの 1 バイトとその長さのバイト列。数はどれも
//! リトル・エンディアン。値の幅の割り当ては推定で、試料のすべての版で確かめた（docs/formats/dxf.md）。
//!
//! 読むときは、各タグを ASCII の DXF の表現（グループ・コードの行と値の行）にした文を作り、
//! タグはその文の上の範囲を持つ。レコードの範囲だけは、元のバイナリのバイト範囲を持つ。
//! 書くときは、変えていないレコードは元のバイトを並べ、変えたレコードはタグを符号化し直す。

use core::ops::Range;

use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_sdk::Error;

use super::{BINARY_SENTINEL, Record, Tag, format_real, parse_real, stopped_at};
use crate::db::{DataType, caret, uncaret};

/// 元のバイナリのバイト列
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Binary {
    pub(super) src: Vec<u8>,
    /// グループ・コードが 2 バイトか（R13 から）
    wide: bool,
    /// `EOF` の後に続くバイト列の範囲
    pub(super) trailer: Range<usize>,
}

impl Binary {
    pub(super) fn snapshot(&self, cx: &mut Cx<'_>) -> Result<Self, Error> {
        Ok(Self {
            src: crate::snapshot::bytes(&self.src, cx)?,
            wide: self.wide,
            trailer: self.trailer.clone(),
        })
    }
}

/// 読んだ結果。ASCII の表現の文、レコード、元のバイナリ
pub(super) struct Decoded {
    pub(super) text: Vec<u8>,
    pub(super) records: Vec<Record>,
    pub(super) binary: Binary,
}

/// 整数の値のバイト数（推定。docs/formats/dxf.md）
fn int_width(code: i16) -> usize {
    match code {
        90..=99 | 420..=429 | 440..=459 | 1071 => 4,
        160..=169 => 8,
        _ => 2,
    }
}

struct Cursor<'a> {
    src: &'a [u8],
    pos: usize,
}

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let bytes = self.src.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(bytes)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }

    fn code(&mut self, wide: bool) -> Option<i16> {
        match self.array::<1>()? {
            [255] if !wide => Some(i16::from_le_bytes(self.array()?)),
            [low] if !wide => Some(i16::from(low)),
            [low] => Some(i16::from_le_bytes([low, self.array::<1>()?[0]])),
        }
    }

    /// NUL で終わるバイト列。NUL は含まない
    fn string(&mut self) -> Option<&[u8]> {
        let rest = self.src.get(self.pos..)?;
        let len = rest.iter().position(|&b| b == 0)?;
        let bytes = rest.get(..len)?;
        self.pos += len + 1;
        Some(bytes)
    }

    /// `code` の値を、ASCII の DXF の値の行の表現にする
    fn value(&mut self, code: i16) -> Option<Vec<u8>> {
        Some(match DataType::of(code) {
            // 制御文字は、ASCII の DXF と同じく `^J` のように書く（推定。docs/formats/dxf.md）
            DataType::Text | DataType::Handle => caret(self.string()?),
            DataType::Real => {
                let real = f64::from_le_bytes(self.array()?);
                if !real.is_finite() {
                    return None;
                }
                format_real(real)
            }
            DataType::Int => {
                let int = match int_width(code) {
                    4 => i64::from(i32::from_le_bytes(self.array()?)),
                    8 => i64::from_le_bytes(self.array()?),
                    _ => i64::from(i16::from_le_bytes(self.array()?)),
                };
                int.to_string().into_bytes()
            }
            DataType::Bool => self.array::<1>()?[0].to_string().into_bytes(),
            DataType::Binary => {
                let len = self.array::<1>()?[0];
                let mut hex = Vec::with_capacity(usize::from(len) * 2);
                for byte in self.take(usize::from(len))? {
                    hex.extend_from_slice(format!("{byte:02X}").as_bytes());
                }
                hex
            }
        })
    }
}

/// `src` は先頭の印で始まる
pub(super) fn read(src: &[u8], cx: &mut Cx) -> Result<Decoded, Error> {
    let start = BINARY_SENTINEL.len();
    // 最初のタグは `0 SECTION`。グループ・コードの幅は、その値の位置で決める（推定。docs/formats/dxf.md）
    let wide = src.get(start..start + 3) == Some(b"\0\0S".as_slice());
    let mut cursor = Cursor { src, pos: start };
    let mut text = Vec::new();
    let mut records: Vec<Record> = Vec::new();
    while cursor.pos < src.len() {
        let at = cursor.pos;
        let invalid = |cx: &mut Cx| {
            cx.report(
                Diagnostic::new(codes::DXF_BINARY_INVALID)
                    .with("offset", at as u64)
                    .at(Ref::at_bytes(at as u64, src.len() as u64)),
            );
            Error::Invalid
        };
        let Some((code, value)) = cursor
            .code(wide)
            .and_then(|code| Some((code, cursor.value(code)?)))
        else {
            return Err(invalid(cx));
        };
        let end = cursor.pos;
        let place = Ref::at_bytes(at as u64, end as u64);
        cx.check(Kind::Work, 1)
            .map_err(|stop| stopped_at(cx, stop, place.clone()))?;
        let tag_start = text.len();
        text.extend_from_slice(format!("{code:>3}\n").as_bytes());
        let value_start = text.len();
        text.extend_from_slice(&value);
        let value_end = text.len();
        text.push(b'\n');
        let tag = Tag {
            code,
            span: tag_start..text.len(),
            value: value_start..value_end,
            edited: None,
        };
        match records.last_mut() {
            Some(record) if code != 0 => {
                record.span.end = end;
                record.tags.push(tag);
            }
            _ => {
                cx.check(Kind::Records, 1)
                    .map_err(|stop| stopped_at(cx, stop, place))?;
                records.push(Record {
                    span: at..end,
                    tags: vec![tag],
                    edited: false,
                });
            }
        }
        if code == 0 && value == b"EOF" {
            break;
        }
    }
    Ok(Decoded {
        text,
        records,
        binary: Binary {
            src: src.to_vec(),
            wide,
            trailer: cursor.pos..src.len(),
        },
    })
}

impl Binary {
    /// 先頭の印
    pub(super) fn sentinel(&self) -> &[u8] {
        self.src.get(..BINARY_SENTINEL.len()).unwrap_or_default()
    }

    /// タグを符号化して `out` に足す。値を `code` の型のバイト列にできなければ `None`
    pub(super) fn encode(&self, code: i16, value: &[u8], out: &mut Vec<u8>) -> Option<()> {
        match (self.wide, u8::try_from(code)) {
            (false, Ok(code)) if code < 255 => out.push(code),
            (false, _) => {
                out.push(255);
                out.extend_from_slice(&code.to_le_bytes());
            }
            (true, _) => out.extend_from_slice(&code.to_le_bytes()),
        }
        match DataType::of(code) {
            DataType::Text | DataType::Handle => {
                let text = uncaret(value)?;
                if text.contains(&0) {
                    return None;
                }
                out.extend_from_slice(&text);
                out.push(0);
            }
            DataType::Real => out.extend_from_slice(&parse_real(value)?.to_le_bytes()),
            DataType::Int => {
                let int: i64 = core::str::from_utf8(value.trim_ascii())
                    .ok()?
                    .parse()
                    .ok()?;
                match int_width(code) {
                    4 => out.extend_from_slice(&i32::try_from(int).ok()?.to_le_bytes()),
                    8 => out.extend_from_slice(&int.to_le_bytes()),
                    _ => out.extend_from_slice(&i16::try_from(int).ok()?.to_le_bytes()),
                }
            }
            DataType::Bool => {
                let byte: u8 = core::str::from_utf8(value.trim_ascii())
                    .ok()?
                    .parse()
                    .ok()?;
                out.push(byte);
            }
            DataType::Binary => {
                let hex = value.trim_ascii();
                if !hex.len().is_multiple_of(2) {
                    return None;
                }
                out.push(u8::try_from(hex.len() / 2).ok()?);
                for pair in hex.chunks(2) {
                    let pair = core::str::from_utf8(pair).ok()?;
                    out.push(u8::from_str_radix(pair, 16).ok()?);
                }
            }
        }
        Some(())
    }

    /// 値を `code` の型のバイト列にできるか
    pub(super) fn encodable(&self, code: i16, value: &[u8]) -> bool {
        self.encode(code, value, &mut Vec::new()).is_some()
    }
}
