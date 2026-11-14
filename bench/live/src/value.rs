//! 値の型付けと書式。`crates/zumen-acad/src/dxf` の共通関数をそのまま呼ぶ。
//! 開くときは値を写さずに借りて読み、変更のときだけ `zumen_acad::Value` にする。

pub use zumen_acad::dxf::{
    RawData as Val, format_real, parse_hex, parse_real, parse_value as value_of,
    raw_data as typed_of,
};
use zumen_acad::{Data, Value};

/// グループ・コードの型で値を読む。型どおりに読めなければ、元のままの文字列にする
pub fn typed(code: i16, raw: &[u8]) -> Val<'_> {
    typed_of(b"", code, raw)
}

/// 持つ値として読む
pub fn value(code: i16, raw: &[u8]) -> Value {
    value_of(b"", code, raw)
}

/// 値の行に書くバイト列。有限でない実数は書けない
pub fn value_bytes(data: &Data) -> Option<Vec<u8>> {
    zumen_acad::dxf::value_bytes(data).map(std::borrow::Cow::into_owned)
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
            typed(310, b"0aFF").into_data(),
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
