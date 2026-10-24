//! 値を型に直す。スキーマから生成した型が使う（決定 0012）。
//!
//! 合わない値は [`Invalid`] にし、根からの道のりを JSON Pointer（RFC 6901）で返す。

use std::collections::BTreeMap;

use crate::value::Value;

/// JSON で整数として持てる最大の値（2^53 − 1）。JavaScript の数が正しく持てる範囲（決定 0012）
pub const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;

/// JSON の値から作れる型。スキーマから生成した型が実装する
pub trait FromJson: Sized {
    /// 値から作る
    fn from_json(value: &Value) -> Result<Self, Invalid>;
}

/// 値が型に合わない理由
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// JSON の型が違う
    Type,
    /// 値が、範囲か、決まった値の一覧に入らない
    Value,
    /// 必要な欄がない
    Missing,
    /// 知らない欄がある
    Unknown,
    /// 目印の欄の値が、どの形にも当たらない。その値を持つ
    Tag(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Key(String),
    Index(usize),
}

/// 値が型に合わないこと
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invalid {
    problem: Problem,
    /// 合わない所から根へ向かう道のり
    path: Vec<Step>,
}

impl Invalid {
    /// 今の値で合わなかったことを作る
    pub fn new(problem: Problem) -> Self {
        Self {
            problem,
            path: Vec::new(),
        }
    }

    /// 理由
    pub fn problem(&self) -> &Problem {
        &self.problem
    }

    /// 合わない所の JSON Pointer。根は空の文字列
    pub fn pointer(&self) -> String {
        let mut pointer = String::new();
        for step in self.path.iter().rev() {
            pointer.push('/');
            match step {
                Step::Key(key) => pointer.push_str(&key.replace('~', "~0").replace('/', "~1")),
                Step::Index(index) => pointer.push_str(&index.to_string()),
            }
        }
        pointer
    }

    /// オブジェクトのキーの中で起きたことにする
    pub fn in_key(mut self, key: &str) -> Self {
        self.path.push(Step::Key(key.to_string()));
        self
    }

    /// 配列の要素の中で起きたことにする
    pub fn in_index(mut self, index: usize) -> Self {
        self.path.push(Step::Index(index));
        self
    }
}

fn mismatch() -> Invalid {
    Invalid::new(Problem::Type)
}

/// 文字列を借りる
pub fn str(value: &Value) -> Result<&str, Invalid> {
    match value {
        Value::String(text) => Ok(text),
        _ => Err(mismatch()),
    }
}

/// 文字列
pub fn string(value: &Value) -> Result<String, Invalid> {
    str(value).map(str::to_string)
}

/// 真偽
pub fn boolean(value: &Value) -> Result<bool, Invalid> {
    match value {
        Value::Bool(value) => Ok(*value),
        _ => Err(mismatch()),
    }
}

/// `min` から `max` までの、符号のない整数。±(2^53 − 1) を超えるものは読まない
pub fn uint(value: &Value, min: u64, max: u64) -> Result<u64, Invalid> {
    let integer = int(value, i64::MIN, i64::MAX)?;
    u64::try_from(integer)
        .ok()
        .filter(|integer| (min..=max).contains(integer))
        .ok_or_else(|| Invalid::new(Problem::Value))
}

/// `min` から `max` までの、符号のある整数。±(2^53 − 1) を超えるものは読まない
pub fn int(value: &Value, min: i64, max: i64) -> Result<i64, Invalid> {
    let Value::Number(number) = value else {
        return Err(mismatch());
    };
    number
        .to_safe_integer()
        .filter(|integer| (min..=max).contains(integer))
        .ok_or_else(|| Invalid::new(Problem::Value))
}

/// 有限の浮動小数
pub fn number(value: &Value) -> Result<f64, Invalid> {
    let Value::Number(number) = value else {
        return Err(mismatch());
    };
    number.to_f64().ok_or_else(|| Invalid::new(Problem::Value))
}

/// 配列。要素は `item` で読む
pub fn array<T>(
    value: &Value,
    mut item: impl FnMut(&Value) -> Result<T, Invalid>,
) -> Result<Vec<T>, Invalid> {
    let Value::Array(items) = value else {
        return Err(mismatch());
    };
    items
        .iter()
        .enumerate()
        .map(|(index, value)| item(value).map_err(|error| error.in_index(index)))
        .collect()
}

/// 名前から値への表。値は `item` で読む
pub fn map<T>(
    value: &Value,
    mut item: impl FnMut(&Value) -> Result<T, Invalid>,
) -> Result<BTreeMap<String, T>, Invalid> {
    let Value::Object(object) = value else {
        return Err(mismatch());
    };
    object
        .iter()
        .map(|(key, value)| {
            item(value)
                .map(|item| (key.to_string(), item))
                .map_err(|error| error.in_key(key))
        })
        .collect()
}

/// オブジェクトの欄を 1 つずつ読み、読まなかった欄を知らない欄として返す。
///
/// 生成した型は、欄をスキーマの順に読み、最後に [`ObjectReader::finish`] を呼ぶ。
#[derive(Debug)]
pub struct ObjectReader<'a> {
    members: &'a [(String, Value)],
    read: Vec<bool>,
}

impl<'a> ObjectReader<'a> {
    /// オブジェクトを読み始める
    pub fn new(value: &'a Value) -> Result<Self, Invalid> {
        let Value::Object(object) = value else {
            return Err(mismatch());
        };
        Ok(Self {
            members: object.members(),
            read: vec![false; object.len()],
        })
    }

    fn find(&self, key: &str) -> Option<(usize, &'a Value)> {
        self.members
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == key)
            .map(|(index, (_, value))| (index, value))
    }

    /// 必要な欄を読む
    pub fn required<T>(
        &mut self,
        key: &str,
        read: impl FnOnce(&'a Value) -> Result<T, Invalid>,
    ) -> Result<T, Invalid> {
        self.optional(key, read)?
            .ok_or_else(|| Invalid::new(Problem::Missing).in_key(key))
    }

    /// 書かなくてもよい欄を読む。`null` は値がないことにしない
    pub fn optional<T>(
        &mut self,
        key: &str,
        read: impl FnOnce(&'a Value) -> Result<T, Invalid>,
    ) -> Result<Option<T>, Invalid> {
        let Some((index, value)) = self.find(key) else {
            return Ok(None);
        };
        if let Some(flag) = self.read.get_mut(index) {
            *flag = true;
        }
        read(value).map(Some).map_err(|error| error.in_key(key))
    }

    /// 目印の欄の文字列を、読んだことにせずに見る。タグつきの共用体が、どの形かを決めるのに使う
    pub fn tag(&self, key: &str) -> Result<&'a str, Invalid> {
        let (_, value) = self
            .find(key)
            .ok_or_else(|| Invalid::new(Problem::Missing).in_key(key))?;
        str(value).map_err(|error| error.in_key(key))
    }

    /// 決まった文字列の欄を読む
    pub fn constant(&mut self, key: &str, expected: &str) -> Result<(), Invalid> {
        let text = self.required(key, str)?;
        if text == expected {
            Ok(())
        } else {
            Err(Invalid::new(Problem::Value).in_key(key))
        }
    }

    /// 読まなかった欄があれば、最初のものを知らない欄として返す
    pub fn finish(self) -> Result<(), Invalid> {
        match self
            .members
            .iter()
            .zip(&self.read)
            .find(|(_, read)| !**read)
        {
            Some(((key, _), _)) => Err(Invalid::new(Problem::Unknown).in_key(key)),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn value(text: &str) -> Value {
        parse(text.as_bytes()).unwrap()
    }

    #[test]
    fn reads_scalars_and_reports_the_problem() {
        assert_eq!(string(&value(r#""a""#)), Ok("a".to_string()));
        assert_eq!(string(&value("1")).unwrap_err().problem(), &Problem::Type);
        assert_eq!(boolean(&value("true")), Ok(true));
        assert_eq!(uint(&value("3"), 1, 5), Ok(3));
        assert_eq!(uint(&value("3.0"), 1, 5), Ok(3));
        assert_eq!(
            uint(&value("6"), 1, 5).unwrap_err().problem(),
            &Problem::Value
        );
        assert_eq!(
            uint(&value("-1"), 0, 5).unwrap_err().problem(),
            &Problem::Value
        );
        assert_eq!(
            uint(&value("1.5"), 0, 5).unwrap_err().problem(),
            &Problem::Value
        );
        assert_eq!(
            uint(&value(r#""1""#), 0, 5).unwrap_err().problem(),
            &Problem::Type
        );
        assert_eq!(int(&value("-4"), -5, 5), Ok(-4));
        assert_eq!(number(&value("2.5e-1")), Ok(0.25));
        assert_eq!(
            number(&value("1e999")).unwrap_err().problem(),
            &Problem::Value
        );
    }

    #[test]
    fn points_to_the_bad_element() {
        let error = array(&value(r#"[{"a":{"b/c~":[1,"x"]}}]"#), |item| {
            let mut object = ObjectReader::new(item)?;
            object.required("a", |a| map(a, |list| array(list, |n| uint(n, 0, 9))))
        })
        .unwrap_err();
        assert_eq!(error.pointer(), "/0/a/b~1c~0/1");
        assert_eq!(error.problem(), &Problem::Type);
        assert_eq!(Invalid::new(Problem::Type).pointer(), "");
    }

    #[test]
    fn reads_fields_and_finds_unknown_ones() {
        let object_value = value(r#"{"op":"open","input":"a.dxf","extra":1}"#);
        let mut object = ObjectReader::new(&object_value).unwrap();
        assert_eq!(object.tag("op"), Ok("open"));
        object.constant("op", "open").unwrap();
        assert_eq!(object.required("input", string), Ok("a.dxf".to_string()));
        assert_eq!(object.optional("doc", string), Ok(None));
        let missing = object.required("doc", string).unwrap_err();
        assert_eq!(
            (missing.problem(), missing.pointer().as_str()),
            (&Problem::Missing, "/doc")
        );
        let unknown = object.finish().unwrap_err();
        assert_eq!(
            (unknown.problem(), unknown.pointer().as_str()),
            (&Problem::Unknown, "/extra")
        );
    }

    #[test]
    fn rejects_null_for_optional_fields_and_wrong_constants() {
        let object_value = value(r#"{"doc":null,"op":"close"}"#);
        let mut object = ObjectReader::new(&object_value).unwrap();
        assert_eq!(
            object.optional("doc", string).unwrap_err().pointer(),
            "/doc"
        );
        assert_eq!(
            object.constant("op", "open").unwrap_err().problem(),
            &Problem::Value
        );
        assert!(ObjectReader::new(&value("[]")).is_err());
        assert_eq!(
            ObjectReader::new(&value("{}"))
                .unwrap()
                .tag("op")
                .unwrap_err()
                .pointer(),
            "/op"
        );
    }
}
