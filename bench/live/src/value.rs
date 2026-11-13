//! 値の型付けと書式。今の実装（`crates/zumen-acad/src/dxf.rs` と `dxf/objects.rs`）と同じ規則で読み書きする。
//! 開くときは値を写さずに借りて読み、変更のときだけ `zumen_acad::Value` にする。

use zumen_acad::{Data, DataType, Value};

/// 借りて読んだ値
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Val<'a> {
    /// 文字列。型どおりに読めなかった値も、元のままここに置く
    Text(&'a [u8]),
    /// 整数
    Int(i64),
    /// 有限の実数
    Real(f64),
    /// 真偽
    Bool(bool),
    /// ハンドル
    Handle(u64),
    /// バイナリ。偶数個の 16 進の数字のまま
    Binary(&'a [u8]),
}

impl Val<'_> {
    /// 持つ値にする
    pub fn to_data(self) -> Data {
        match self {
            Val::Text(raw) => Data::Text(raw.to_vec()),
            Val::Int(value) => Data::Int(value),
            Val::Real(value) => Data::Real(value),
            Val::Bool(value) => Data::Bool(value),
            Val::Handle(value) => Data::Handle(value),
            Val::Binary(hex) => Data::Binary(
                hex.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&[high, low]| {
                        let digit = |b: u8| char::from(b).to_digit(16).unwrap_or(0);
                        (digit(high) * 16 + digit(low)) as u8
                    })
                    .collect(),
            ),
        }
    }
}

/// グループ・コードの型で値を読む。型どおりに読めなければ、元のままの文字列にする
pub fn typed(code: i16, raw: &[u8]) -> Val<'_> {
    let text = raw.trim_ascii();
    let data = match DataType::of(code) {
        DataType::Text => Some(Val::Text(raw)),
        DataType::Int => parse_int(text).map(Val::Int),
        DataType::Real => parse_real(raw).map(Val::Real),
        DataType::Bool => match parse_int(text) {
            Some(0) => Some(Val::Bool(false)),
            Some(1) => Some(Val::Bool(true)),
            _ => None,
        },
        DataType::Handle => parse_hex(text).map(Val::Handle),
        DataType::Binary => is_binary(text).then_some(Val::Binary(text)),
    };
    data.unwrap_or(Val::Text(raw))
}

/// 持つ値として読む
pub fn value(code: i16, raw: &[u8]) -> Value {
    Value {
        code,
        data: typed(code, raw).to_data(),
    }
}

fn parse_int(text: &[u8]) -> Option<i64> {
    core::str::from_utf8(text).ok()?.parse().ok()
}

/// 有限の実数
pub fn parse_real(value: &[u8]) -> Option<f64> {
    let real: f64 = core::str::from_utf8(value.trim_ascii())
        .ok()?
        .parse()
        .ok()?;
    real.is_finite().then_some(real)
}

/// 16 進のハンドル。空のものと符号つきのものは読めないとする
pub fn parse_hex(text: &[u8]) -> Option<u64> {
    let text = core::str::from_utf8(text).ok()?;
    if text.is_empty() || text.starts_with(['+', '-']) {
        return None;
    }
    u64::from_str_radix(text, 16).ok()
}

/// 16 進の 2 文字ずつで 1 バイトのバイナリか
fn is_binary(text: &[u8]) -> bool {
    text.len().is_multiple_of(2) && text.iter().all(u8::is_ascii_hexdigit)
}

/// 有限の実数を、読み直すと同じ値に戻る最短の 10 進の表記にする。整数の値にも小数点を付ける
pub fn format_real(value: f64) -> Vec<u8> {
    let mut text = value.to_string();
    if !text.contains('.') {
        text.push_str(".0");
    }
    text.into_bytes()
}

/// 値の行に書くバイト列。有限でない実数は書けない
pub fn value_bytes(data: &Data) -> Option<Vec<u8>> {
    Some(match data {
        Data::Text(bytes) => bytes.clone(),
        Data::Real(real) if real.is_finite() => format_real(*real),
        Data::Real(_) => return None,
        Data::Int(number) => number.to_string().into_bytes(),
        Data::Bool(flag) => vec![if *flag { b'1' } else { b'0' }],
        Data::Handle(handle) => format!("{handle:X}").into_bytes(),
        Data::Binary(bytes) => {
            let hex = |n: u8| if n < 10 { b'0' + n } else { b'A' + (n - 10) };
            let mut digits = Vec::with_capacity(bytes.len().checked_mul(2)?);
            for byte in bytes {
                digits.push(hex(byte >> 4));
                digits.push(hex(byte & 15));
            }
            digits
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_follow_the_group_code_type() {
        assert_eq!(typed(8, b" 0 "), Val::Text(b" 0 "));
        assert_eq!(typed(10, b" 1.50 "), Val::Real(1.5));
        assert_eq!(typed(10, b"1,5"), Val::Text(b"1,5"));
        assert_eq!(typed(10, b"inf"), Val::Text(b"inf"));
        assert_eq!(typed(70, b"     1"), Val::Int(1));
        assert_eq!(typed(290, b"1"), Val::Bool(true));
        assert_eq!(typed(290, b"2"), Val::Text(b"2"));
        assert_eq!(typed(5, b"2f"), Val::Handle(0x2F));
        assert_eq!(typed(5, b"+2F"), Val::Text(b"+2F"));
        assert_eq!(typed(310, b"0aFF"), Val::Binary(b"0aFF"));
        assert_eq!(typed(310, b"0aF"), Val::Text(b"0aF"));
        assert_eq!(
            typed(310, b"0aFF").to_data(),
            Data::Binary(vec![0x0A, 0xFF])
        );
    }

    #[test]
    fn reals_get_a_decimal_point() {
        assert_eq!(format_real(100.0), b"100.0");
        assert_eq!(format_real(-0.25), b"-0.25");
        assert_eq!(value_bytes(&Data::Real(f64::NAN)), None);
        assert_eq!(value_bytes(&Data::Handle(0x2F)), Some(b"2F".to_vec()));
        assert_eq!(
            value_bytes(&Data::Binary(vec![0x0A, 0xFF])),
            Some(b"0AFF".to_vec())
        );
    }
}
