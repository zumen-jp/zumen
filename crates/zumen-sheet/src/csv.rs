//! CSV（RFC 4180）。区切りの字・二重引用符の囲み・CRLF と LF と CR の改行を読む。文字コードは、指定がなければ、
//! BOM か UTF-8 として正しければ UTF-8、そうでなければ Shift_JIS とする。

use zumen_base::{Cx, Diagnostic, Kind as Budget, codes};
use zumen_sdk::Error;

use crate::{Book, Cell, Encoding, Kind, Sheet, stopped};

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// 文字に直す。直せないバイトがあれば `csv.encoding.invalid` を足し、置き換えの字にする
fn decode(src: &[u8], encoding: Option<Encoding>, cx: &mut Cx) -> (String, Encoding) {
    let (body, bom) = match src.strip_prefix(BOM) {
        Some(body) => (body, true),
        None => (src, false),
    };
    let encoding = encoding.unwrap_or(if bom || std::str::from_utf8(body).is_ok() {
        Encoding::Utf8
    } else {
        Encoding::ShiftJis
    });
    let (text, errors) = match encoding {
        Encoding::Utf8 => encoding_rs::UTF_8.decode_without_bom_handling(body),
        Encoding::ShiftJis => encoding_rs::SHIFT_JIS.decode_without_bom_handling(body),
    };
    if errors {
        cx.report(Diagnostic::new(codes::CSV_ENCODING_INVALID).with("encoding", encoding.name()));
    }
    (text.into_owned(), encoding)
}

/// CSV を読む。`delimiter` は区切りの字（`,` や タブ）。閉じない引用符があれば `csv.quote.invalid` を足し、
/// 終わりまでを値とする
pub fn read(
    src: &[u8],
    encoding: Option<Encoding>,
    delimiter: char,
    cx: &mut Cx,
) -> Result<Book, Error> {
    cx.check(Budget::InputBytes, src.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let (text, encoding) = decode(src, encoding, cx);
    cx.check(Budget::DecodedBytes, text.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut sheet = Sheet::default();
    let mut chars = text.chars().peekable();
    let (mut row, mut column) = (0u32, 0u32);
    let mut field = String::new();
    // 引用符で囲んだ値か。行の始まりの行番号（1 から）
    let (mut quoted, mut line, mut start_line) = (false, 1u64, 1u64);
    let mut any = false;
    let push = |sheet: &mut Sheet, row: u32, column: u32, field: &mut String| {
        if !field.is_empty() {
            sheet.cells.push(Cell {
                row,
                column,
                kind: Kind::Text,
                value: std::mem::take(field),
                formula: None,
            });
        }
    };
    while let Some(c) = chars.next() {
        any = true;
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => {
                    if c == '\n' || (c == '\r' && chars.peek() != Some(&'\n')) {
                        line += 1;
                    }
                    field.push(c);
                }
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => {
                quoted = true;
                start_line = line;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                line += 1;
                push(&mut sheet, row, column, &mut field);
                cx.check(Budget::Records, 1)
                    .map_err(|stop| stopped(cx, stop))?;
                row = row.saturating_add(1);
                column = 0;
                any = false;
            }
            c if c == delimiter => {
                push(&mut sheet, row, column, &mut field);
                column = column.saturating_add(1);
            }
            _ => field.push(c),
        }
    }
    if quoted {
        cx.report(Diagnostic::new(codes::CSV_QUOTE_INVALID).with("line", start_line));
    }
    if any {
        push(&mut sheet, row, column, &mut field);
    }
    sheet.measure();
    Ok(Book {
        sheets: vec![sheet],
        encoding: Some(encoding),
    })
}
