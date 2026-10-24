//! JSON を書く。

use core::fmt::Write as _;
use std::collections::BTreeMap;

use crate::number;
use crate::value::{Number, Value};

/// JSON を書ける値。スキーマから生成した型が実装する
pub trait ToJson {
    /// 値を書く
    fn to_json(&self, out: &mut Writer);
}

/// JSON を、空白を入れずに書く。
///
/// オブジェクトの中では、呼ぶ側が [`Writer::key`] と値を交互に書く。キーは書いた順に並ぶ。
/// 区切りの `,` は、この型が入れる。
#[derive(Debug, Default)]
pub struct Writer {
    out: String,
    /// 開いている配列とオブジェクトごとに、次の要素が最初のものか
    first: Vec<bool>,
    after_key: bool,
}

impl Writer {
    /// 空のものを作る
    pub fn new() -> Self {
        Self::default()
    }

    /// 書いたバイト列を取り出す
    pub fn into_bytes(self) -> Vec<u8> {
        self.out.into_bytes()
    }

    fn separate(&mut self) {
        if self.after_key {
            self.after_key = false;
            return;
        }
        if let Some(first) = self.first.last_mut() {
            if *first {
                *first = false;
            } else {
                self.out.push(',');
            }
        }
    }

    /// オブジェクトを始める
    pub fn begin_object(&mut self) {
        self.separate();
        self.out.push('{');
        self.first.push(true);
    }

    /// オブジェクトを終える
    pub fn end_object(&mut self) {
        self.first.pop();
        self.out.push('}');
    }

    /// 配列を始める
    pub fn begin_array(&mut self) {
        self.separate();
        self.out.push('[');
        self.first.push(true);
    }

    /// 配列を終える
    pub fn end_array(&mut self) {
        self.first.pop();
        self.out.push(']');
    }

    /// オブジェクトのキーを書く。次に書くものが、その値になる
    pub fn key(&mut self, key: &str) {
        self.separate();
        escape(&mut self.out, key);
        self.out.push(':');
        self.after_key = true;
    }

    /// 文字列を書く
    pub fn string(&mut self, text: &str) {
        self.separate();
        escape(&mut self.out, text);
    }

    /// 符号のない整数を書く。±(2^53 − 1) の中に収めるのは、呼ぶ側が守る（決定 0012）
    pub fn uint(&mut self, value: u64) {
        self.separate();
        let _ = write!(self.out, "{value}");
    }

    /// 符号のある整数を書く。±(2^53 − 1) の中に収めるのは、呼ぶ側が守る（決定 0012）
    pub fn int(&mut self, value: i64) {
        self.separate();
        let _ = write!(self.out, "{value}");
    }

    /// 浮動小数を、読み直すと同じ値に戻る最短の表記で書く。
    ///
    /// NaN と無限大は JSON に書けない（決定 0012）。呼ぶ側が有限の値にする。有限でなければ `null` を書く。
    pub fn float(&mut self, value: f64) {
        self.separate();
        if value.is_finite() {
            number::write_finite(&mut self.out, value);
        } else {
            self.out.push_str("null");
        }
    }

    /// 真偽を書く
    pub fn bool(&mut self, value: bool) {
        self.separate();
        self.out.push_str(if value { "true" } else { "false" });
    }

    /// `null` を書く
    pub fn null(&mut self) {
        self.separate();
        self.out.push_str("null");
    }

    /// 読んだ数を、読んだときの表記のまま書く
    pub fn number(&mut self, value: &Number) {
        self.separate();
        self.out.push_str(value.as_str());
    }

    /// 読んだ値を、そのまま書く
    pub fn value(&mut self, value: &Value) {
        match value {
            Value::Null => self.null(),
            Value::Bool(value) => self.bool(*value),
            Value::Number(value) => self.number(value),
            Value::String(value) => self.string(value),
            Value::Array(items) => self.array(items, Self::value),
            Value::Object(object) => {
                self.begin_object();
                for (key, value) in object.iter() {
                    self.key(key);
                    self.value(value);
                }
                self.end_object();
            }
        }
    }

    /// 配列を書く
    pub fn array<T>(&mut self, items: &[T], mut each: impl FnMut(&mut Self, &T)) {
        self.begin_array();
        for item in items {
            each(self, item);
        }
        self.end_array();
    }

    /// 名前から値への表を、名前の順にオブジェクトとして書く
    pub fn map<T>(&mut self, map: &BTreeMap<String, T>, mut each: impl FnMut(&mut Self, &T)) {
        self.begin_object();
        for (key, value) in map {
            self.key(key);
            each(self, value);
        }
        self.end_object();
    }
}

impl ToJson for Value {
    fn to_json(&self, out: &mut Writer) {
        out.value(self);
    }
}

/// 文字列を、引用符で囲んでエスケープして書く。
///
/// エスケープするのは `"`・`\`・制御文字（U+0000〜U+001F）だけ。ほかの文字は、UTF-8 のまま書く。
fn escape(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn written(write: impl FnOnce(&mut Writer)) -> String {
        let mut out = Writer::new();
        write(&mut out);
        String::from_utf8(out.into_bytes()).unwrap()
    }

    #[test]
    fn writes_nested_values_without_spaces() {
        let text = written(|out| {
            out.begin_object();
            out.key("v");
            out.uint(1);
            out.key("items");
            out.begin_array();
            out.int(-2);
            out.float(0.5);
            out.bool(true);
            out.null();
            out.begin_object();
            out.end_object();
            out.end_array();
            out.key("empty");
            out.array::<u64>(&[], |out, item| out.uint(*item));
            out.end_object();
        });
        assert_eq!(text, r#"{"v":1,"items":[-2,0.5,true,null,{}],"empty":[]}"#);
    }

    #[test]
    fn escapes_only_quotes_backslashes_and_control_characters() {
        let text = written(|out| out.string("a\"b\\c\n\u{1}\u{7f}é/\u{2028}"));
        assert_eq!(text, "\"a\\\"b\\\\c\\n\\u0001\u{7f}é/\u{2028}\"");
    }

    #[test]
    fn never_writes_nan_or_infinity() {
        let text = written(|out| {
            out.array(&[f64::NAN, f64::INFINITY, -0.0, 1e21], |out, x| {
                out.float(*x)
            });
        });
        assert_eq!(text, "[null,null,0,1e+21]");
    }

    #[test]
    fn writes_maps_in_key_order() {
        let map: BTreeMap<String, u64> = [("b".to_string(), 2), ("a".to_string(), 1)].into();
        assert_eq!(
            written(|out| out.map(&map, |out, v| out.uint(*v))),
            r#"{"a":1,"b":2}"#
        );
    }

    #[test]
    fn writes_read_values_back_as_they_were() {
        let source = r#"{"a":[1.0,-2e3,"x\u0000"],"b":{"c":null,"d":false}}"#;
        let value = parse(source.as_bytes()).unwrap();
        let text = written(|out| value.to_json(out));
        assert_eq!(text, source);
        assert_eq!(parse(text.as_bytes()).unwrap(), value);
    }
}
