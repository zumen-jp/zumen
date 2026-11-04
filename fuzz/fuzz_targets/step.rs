//! ISO 10303-21（L0）の fuzz の target（#501）。交換構造を、フィーチャのコメントを読まない書き方（IFC）と読む書き方
//! （SXF）で読み、文字列のエスケープを戻す。値の入れ子が予算の深さで止まることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_base::codes;
use zumen_step::{Dialect, Exchange, Value};

/// フィーチャのコメントを読まない書き方
const PLAIN: Dialect = Dialect {
    record_invalid: codes::IFC_RECORD_INVALID,
    file_invalid: codes::IFC_FILE_INVALID,
    features: &[],
    plain_if_described: None,
};

/// フィーチャのコメントを読む書き方
const FEATURES: Dialect = Dialect {
    record_invalid: codes::SXF_RECORD_INVALID,
    file_invalid: codes::SXF_FILE_INVALID,
    features: &[
        (b"/*SXF3.1", b"SXF3.1*/"),
        (b"/*SXF3", b"SXF3*/"),
        (b"/*SXF", b"SXF*/"),
    ],
    plain_if_described: Some(b"AP202"),
};

/// 0x80 以上のバイトを、UTF-8 として読む
fn eight_bit(rest: &[u8]) -> (String, usize) {
    let len = rest.iter().take_while(|b| **b >= 0x80).count();
    (
        String::from_utf8_lossy(rest.get(..len).unwrap_or_default()).into_owned(),
        len,
    )
}

/// 値の中の文字列を戻し、戻した文字のバイト数を返す。入れ子は、読むときに予算の深さで止めてある
fn decode(src: &[u8], value: &Value) -> usize {
    match value {
        Value::Text { raw, .. } => {
            zumen_step::decode(src.get(raw.clone()).unwrap_or_default(), eight_bit).len()
        }
        Value::Typed(part) => part.args().iter().map(|arg| decode(src, arg)).sum(),
        Value::List(values) => values.iter().map(|value| decode(src, value)).sum(),
        _ => 0,
    }
}

fn decode_all(src: &[u8], exchange: &Exchange) -> usize {
    exchange
        .records
        .iter()
        .flat_map(|record| record.parts())
        .flat_map(|part| part.args())
        .map(|arg| decode(src, arg))
        .sum()
}

fuzz_target!(|data: &[u8]| {
    let read = zumen_sdk::testkit::survives_with(data, |src, cx| {
        let plain = zumen_step::read(src, &PLAIN, cx)?;
        let features = zumen_step::read(src, &FEATURES, cx)?;
        Ok(decode_all(src, &plain) + decode_all(src, &features))
    });
    if let Err(message) = read {
        panic!("{message}");
    }
});
