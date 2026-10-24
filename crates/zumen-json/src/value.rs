//! 読んだ JSON の値。

use crate::read::MAX_SAFE_INTEGER;

/// JSON の値
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// `null`
    Null,
    /// `true`・`false`
    Bool(bool),
    /// 数。読んだときの表記のまま持つ
    Number(Number),
    /// 文字列
    String(String),
    /// 配列
    Array(Vec<Value>),
    /// オブジェクト
    Object(Object),
}

impl Value {
    /// JSON Pointer（RFC 6901）で指す値。根は空の文字列で指す
    pub fn pointer(&self, pointer: &str) -> Option<&Value> {
        if pointer.is_empty() {
            return Some(self);
        }
        let rest = pointer.strip_prefix('/')?;
        rest.split('/').try_fold(self, |value, token| {
            let token = token.replace("~1", "/").replace("~0", "~");
            match value {
                Value::Object(object) => object.get(&token),
                Value::Array(items) => {
                    let canonical = token == "0" || !token.starts_with('0');
                    let index: usize = token.parse().ok().filter(|_| canonical)?;
                    items.get(index)
                }
                _ => None,
            }
        })
    }
}

/// JSON の数。RFC 8259 の文法に合う表記を、そのまま持つ。
///
/// 同じ数でも、表記が違えば等しくない（`1` と `1.0` など）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Number(Box<str>);

impl Number {
    /// 文法を確かめた表記から作る
    pub(crate) fn from_valid(text: &str) -> Self {
        Self(text.into())
    }

    /// 表記
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// ±(2^53 − 1) の中の整数として読む（決定 0012）。
    ///
    /// 小数部が 0 だけの表記（`1.0` など）も整数とみなす。指数のある表記は、整数とみなさない。
    pub fn to_safe_integer(&self) -> Option<i64> {
        let (negative, unsigned) = match self.0.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, &*self.0),
        };
        if unsigned.contains(['e', 'E']) {
            return None;
        }
        let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        if !fraction.bytes().all(|b| b == b'0') {
            return None;
        }
        // 2^53 − 1 は 16 桁。それより長い整数部は、上限を超える
        if whole.len() > 16 {
            return None;
        }
        let magnitude: i64 = whole.parse().ok()?;
        if magnitude > MAX_SAFE_INTEGER {
            return None;
        }
        Some(if negative { -magnitude } else { magnitude })
    }

    /// 浮動小数として読む。最も近い値に丸め、有限でなければ `None`
    pub fn to_f64(&self) -> Option<f64> {
        self.0.parse::<f64>().ok().filter(|x| x.is_finite())
    }
}

/// JSON のオブジェクト。キーは重ならず、読んだ順に並ぶ
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Object(Vec<(String, Value)>);

impl Object {
    /// 重ならないキーの並びから作る
    pub(crate) fn from_unique(members: Vec<(String, Value)>) -> Self {
        Self(members)
    }

    /// キーで値を引く
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    /// キーと値を、読んだ順に返す
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(name, value)| (name.as_str(), value))
    }

    /// キーの数
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// キーがないか
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn members(&self) -> &[(String, Value)] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::parse;

    fn number(text: &str) -> crate::Number {
        match parse(text.as_bytes()).unwrap() {
            crate::Value::Number(number) => number,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn reads_safe_integers() {
        assert_eq!(number("0").to_safe_integer(), Some(0));
        assert_eq!(number("-0").to_safe_integer(), Some(0));
        assert_eq!(number("42").to_safe_integer(), Some(42));
        assert_eq!(number("-42").to_safe_integer(), Some(-42));
        assert_eq!(number("1.000").to_safe_integer(), Some(1));
        assert_eq!(
            number("9007199254740991").to_safe_integer(),
            Some(9_007_199_254_740_991)
        );
        assert_eq!(
            number("-9007199254740991").to_safe_integer(),
            Some(-9_007_199_254_740_991)
        );
        assert_eq!(number("9007199254740992").to_safe_integer(), None);
        assert_eq!(number("12345678901234567890").to_safe_integer(), None);
        assert_eq!(number("1.5").to_safe_integer(), None);
        assert_eq!(number("1e2").to_safe_integer(), None);
    }

    #[test]
    fn reads_floats_and_rejects_overflow() {
        assert_eq!(number("0.1").to_f64(), Some(0.1));
        assert_eq!(number("-2.5e-3").to_f64(), Some(-0.0025));
        assert_eq!(number("1e400").to_f64(), None);
        assert_eq!(number("1e-400").to_f64(), Some(0.0));
    }

    #[test]
    fn points_into_values() {
        let value = parse(br#"{"a":[{"b/c":1,"d~e":2}],"":3}"#).unwrap();
        assert_eq!(value.pointer(""), Some(&value));
        assert_eq!(
            value
                .pointer("/a/0/b~1c")
                .map(|v| matches!(v, crate::Value::Number(_))),
            Some(true)
        );
        assert!(value.pointer("/a/0/d~0e").is_some());
        assert!(value.pointer("/").is_some());
        assert!(value.pointer("/a/1").is_none());
        assert!(value.pointer("/a/00").is_none());
        assert!(value.pointer("a").is_none());
    }
}
