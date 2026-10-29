//! 診断と、そのまとめ方（決定 0009）。

use core::cmp::Ordering;
use std::collections::BTreeMap;

use crate::code::{Code, Fidelity, Severity};

/// 診断の値。人向けに整形した文字列は持たない。
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// 符号のない整数
    UInt(u64),
    /// 符号のある整数
    Int(i64),
    /// 浮動小数
    Float(f64),
    /// 図面に記録された名前などの文字列。翻訳しない
    Text(String),
    /// 列挙の値のキー。名前はカタログで引く
    Key(&'static str),
}

impl Value {
    fn rank(&self) -> u8 {
        match self {
            Value::UInt(_) => 0,
            Value::Int(_) => 1,
            Value::Float(_) => 2,
            Value::Text(_) => 3,
            Value::Key(_) => 4,
        }
    }

    /// 全順序で比べる。浮動小数は `total_cmp` で比べる（決定 0007）。
    fn total_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Value::UInt(a), Value::UInt(b)) => a.cmp(b),
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
            (Value::Text(a), Value::Text(b)) => a.cmp(b),
            (Value::Key(a), Value::Key(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::UInt(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Value::Int(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Float(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::Text(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Text(value.to_owned())
    }
}

/// L1 のバイト範囲（`start..end`）
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteRange {
    /// 先頭の位置
    pub start: u64,
    /// 終わりの位置（含まない）
    pub end: u64,
}

/// 形式が持つ、変わらない鍵（決定 0156）。書き戻しても、前のレコードが増えても変わらない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    /// AutoCAD の系統のハンドル
    Handle(u64),
    /// SXF のインスタンスの番号（`#n`）
    Instance(u64),
}

/// 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所）と、形式が持つ鍵
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ref {
    id: Option<String>,
    record: Option<u64>,
    bytes: Option<ByteRange>,
    key: Option<Key>,
}

impl Ref {
    /// L1 のレコードの番号で指す
    pub fn at_record(index: u64) -> Self {
        Self {
            record: Some(index),
            ..Self::default()
        }
    }

    /// L1 のバイト範囲で指す
    pub fn at_bytes(start: u64, end: u64) -> Self {
        Self {
            bytes: Some(ByteRange { start, end }),
            ..Self::default()
        }
    }

    /// ID を足す
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// レコードの番号を足す
    pub fn with_record(mut self, index: u64) -> Self {
        self.record = Some(index);
        self
    }

    /// バイト範囲を足す
    pub fn with_bytes(mut self, start: u64, end: u64) -> Self {
        self.bytes = Some(ByteRange { start, end });
        self
    }

    /// 形式が持つ鍵を足す
    pub fn with_key(mut self, key: Key) -> Self {
        self.key = Some(key);
        self
    }

    /// ID
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// L1 のレコードの番号
    pub fn record(&self) -> Option<u64> {
        self.record
    }

    /// L1 のバイト範囲
    pub fn bytes(&self) -> Option<ByteRange> {
        self.bytes
    }

    /// 形式が持つ鍵
    pub fn key(&self) -> Option<Key> {
        self.key
    }
}

/// 仕様の節。仕様の本文は写さない。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpecRef {
    /// 仕様の一覧での名前
    pub spec: &'static str,
    /// 節
    pub section: &'static str,
}

/// 診断。`severity` と `fidelity` は code から決まる。
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    code: Code,
    count: u64,
    at: Vec<Ref>,
    cite: Option<SpecRef>,
    params: Vec<(&'static str, Value)>,
}

impl Diagnostic {
    /// 件数 1 の診断を作る
    pub fn new(code: Code) -> Self {
        Self {
            code,
            count: 1,
            at: Vec::new(),
            cite: None,
            params: Vec::new(),
        }
    }

    /// 値を入れる。名前は台帳の値の名前にする。同じ名前の値は置き換える
    pub fn with(mut self, name: &'static str, value: impl Into<Value>) -> Self {
        let value = value.into();
        match self.params.iter_mut().find(|(key, _)| *key == name) {
            Some((_, slot)) => *slot = value,
            None => self.params.push((name, value)),
        }
        self
    }

    /// 場所を足す
    pub fn at(mut self, place: Ref) -> Self {
        self.at.push(place);
        self
    }

    /// 仕様の節を入れる
    pub fn cite(mut self, spec: SpecRef) -> Self {
        self.cite = Some(spec);
        self
    }

    /// code
    pub fn code(&self) -> Code {
        self.code
    }

    /// 重さ
    pub fn severity(&self) -> Severity {
        self.code.severity()
    }

    /// 変換の結果の段
    pub fn fidelity(&self) -> Option<Fidelity> {
        self.code.fidelity()
    }

    /// まとめた件数
    pub fn count(&self) -> u64 {
        self.count
    }

    /// 場所。入力の順に、先頭の数件
    pub fn places(&self) -> &[Ref] {
        &self.at
    }

    /// 仕様の節
    pub fn spec(&self) -> Option<SpecRef> {
        self.cite
    }

    /// 値。入れた順
    pub fn params(&self) -> &[(&'static str, Value)] {
        &self.params
    }

    /// 名前で値を引く
    pub fn param(&self, name: &str) -> Option<&Value> {
        self.params
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }
}

/// まとめるときの鍵。code の名前と、台帳で決めた値
#[derive(Clone, Debug)]
struct GroupKey {
    code: &'static str,
    values: Vec<Option<Value>>,
}

impl GroupKey {
    fn of(diagnostic: &Diagnostic) -> Self {
        let code = diagnostic.code();
        Self {
            code: code.name(),
            values: code
                .group_by()
                .iter()
                .map(|name| diagnostic.param(name).cloned())
                .collect(),
        }
    }
}

impl PartialEq for GroupKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for GroupKey {}

impl PartialOrd for GroupKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GroupKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.code.cmp(other.code).then_with(|| {
            let mut pairs = self.values.iter().zip(&other.values);
            pairs
                .find_map(|pair| match pair {
                    (Some(a), Some(b)) => Some(a.total_cmp(b)).filter(|o| o.is_ne()),
                    (None, None) => None,
                    (None, Some(_)) => Some(Ordering::Less),
                    (Some(_), None) => Some(Ordering::Greater),
                })
                .unwrap_or_else(|| self.values.len().cmp(&other.values.len()))
        })
    }
}

/// 診断を、届いた順にまとめて持つ。
///
/// 同じ code で、台帳で決めた値が同じ診断は 1 つにまとめ、件数と先頭の数件の場所を持つ。
/// まとめた診断の値は、最初に届いたものを残す。
#[derive(Clone, Debug)]
pub struct Diagnostics {
    list: Vec<Diagnostic>,
    index: BTreeMap<GroupKey, usize>,
    max_places: usize,
}

impl Diagnostics {
    /// 診断ごとに持つ場所の数の既定値
    pub const DEFAULT_MAX_PLACES: usize = 16;

    /// 空のものを作る
    pub fn new() -> Self {
        Self::with_max_places(Self::DEFAULT_MAX_PLACES)
    }

    /// 診断ごとに持つ場所の数を決めて作る
    pub fn with_max_places(max_places: usize) -> Self {
        Self {
            list: Vec::new(),
            index: BTreeMap::new(),
            max_places,
        }
    }

    /// 診断を足す。まとめられるものはまとめる
    pub fn report(&mut self, diagnostic: Diagnostic) {
        let key = GroupKey::of(&diagnostic);
        if let Some(existing) = self
            .index
            .get(&key)
            .and_then(|&position| self.list.get_mut(position))
        {
            existing.count = existing.count.saturating_add(diagnostic.count);
            let room = self.max_places.saturating_sub(existing.at.len());
            existing.at.extend(diagnostic.at.into_iter().take(room));
            return;
        }
        let mut diagnostic = diagnostic;
        diagnostic.at.truncate(self.max_places);
        self.index.insert(key, self.list.len());
        self.list.push(diagnostic);
    }

    /// 届いた順に返す
    pub fn iter(&self) -> core::slice::Iter<'_, Diagnostic> {
        self.list.iter()
    }

    /// まとめたあとの診断の数
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// 診断がないか
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// `error` の診断があるか
    pub fn has_errors(&self) -> bool {
        self.list
            .iter()
            .any(|diagnostic| diagnostic.severity() == Severity::Error)
    }

    /// 診断を取り出す
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.list
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = core::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::{Param, ParamType};
    use crate::codes;

    /// 値でまとめる code。試験のためだけに作る
    const BY_KIND: Code = Code::new(
        "test.kind.dropped",
        Severity::Warning,
        Some(Fidelity::Dropped),
        &[Param::new("kind", ParamType::Key)],
        &["kind"],
    );

    #[test]
    fn merges_the_same_code_and_keeps_the_first_places() {
        let mut diag = Diagnostics::with_max_places(2);
        for index in 0..3 {
            diag.report(
                Diagnostic::new(codes::ZIP_PATH_UNSAFE)
                    .with("name", format!("../{index}"))
                    .at(Ref::at_record(index)),
            );
        }
        assert_eq!(diag.len(), 1);
        let merged = diag.iter().next().unwrap();
        assert_eq!(merged.count(), 3);
        assert_eq!(merged.places(), [Ref::at_record(0), Ref::at_record(1)]);
        assert_eq!(merged.param("name"), Some(&Value::from("../0")));
    }

    #[test]
    fn separates_by_the_values_in_group_by_and_keeps_arrival_order() {
        let mut diag = Diagnostics::new();
        diag.report(Diagnostic::new(BY_KIND).with("kind", Value::Key("spline")));
        diag.report(Diagnostic::new(BY_KIND).with("kind", Value::Key("hatch")));
        diag.report(Diagnostic::new(BY_KIND).with("kind", Value::Key("spline")));
        let kinds: Vec<(Option<&Value>, u64)> =
            diag.iter().map(|d| (d.param("kind"), d.count())).collect();
        assert_eq!(
            kinds,
            [
                (Some(&Value::Key("spline")), 2),
                (Some(&Value::Key("hatch")), 1)
            ]
        );
    }

    #[test]
    fn severity_and_fidelity_come_from_the_code() {
        let diagnostic = Diagnostic::new(BY_KIND);
        assert_eq!(diagnostic.severity(), Severity::Warning);
        assert_eq!(diagnostic.fidelity(), Some(Fidelity::Dropped));
        let mut diag = Diagnostics::new();
        diag.report(diagnostic);
        assert!(!diag.has_errors());
        diag.report(Diagnostic::new(codes::CALL_REQUEST_CANCELLED));
        assert!(diag.has_errors());
    }

    #[test]
    fn replaces_a_value_with_the_same_name() {
        let diagnostic = Diagnostic::new(codes::BUDGET_RECORDS_EXCEEDED)
            .with("limit", 1_u64)
            .with("limit", 2_u64);
        assert_eq!(diagnostic.params(), [("limit", Value::UInt(2))]);
    }

    #[test]
    fn orders_floats_totally() {
        let nan = Value::Float(f64::NAN);
        assert_eq!(nan.total_cmp(&nan), Ordering::Equal);
        assert_eq!(
            Value::Float(-0.0).total_cmp(&Value::Float(0.0)),
            Ordering::Less
        );
        assert_eq!(
            Value::UInt(1).total_cmp(&Value::Text("a".into())),
            Ordering::Less
        );
    }
}
