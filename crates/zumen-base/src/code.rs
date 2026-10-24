//! 診断の code と、台帳で決まる性質（決定 0009）。
//!
//! code は、台帳（`catalog/codes.toml`）から生成した [`crate::codes`] の定数だけを使う。
//! この crate の外では、新しい code を作れない。

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};

/// 診断の重さ。code ごとに決まる。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// 何も失っていない。別の表現に置き換えた、既定値を仮定した、など
    Info,
    /// 近似した、見た目だけにした、落とした、壊れた所を直した。結果は使える
    Warning,
    /// 処理が失敗した、または結果が欠けている
    Error,
}

/// 変換の結果の段のうち、`native` 以外のもの。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fidelity {
    /// 別の表現で同じ意味
    Equivalent,
    /// 許容差つきの近似。許容差は診断の値に入れる
    Approximated,
    /// 見た目だけ
    Drawn,
    /// 落とした
    Dropped,
}

/// 診断の値の型
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParamType {
    /// 符号のない整数
    UInt,
    /// 符号のある整数
    Int,
    /// 浮動小数
    Float,
    /// 図面に記録された名前などの文字列。翻訳しない
    Text,
    /// 列挙の値のキー。名前はカタログで引く
    Key,
}

/// 台帳で決めた、値の名前と型
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Param {
    name: &'static str,
    ty: ParamType,
}

impl Param {
    pub(crate) const fn new(name: &'static str, ty: ParamType) -> Self {
        Self { name, ty }
    }

    /// 値の名前
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// 値の型
    pub const fn ty(&self) -> ParamType {
        self.ty
    }
}

/// 診断の code。名前で比べる。
#[derive(Clone, Copy, Debug)]
pub struct Code {
    name: &'static str,
    severity: Severity,
    fidelity: Option<Fidelity>,
    params: &'static [Param],
    group_by: &'static [&'static str],
}

impl Code {
    pub(crate) const fn new(
        name: &'static str,
        severity: Severity,
        fidelity: Option<Fidelity>,
        params: &'static [Param],
        group_by: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            severity,
            fidelity,
            params,
            group_by,
        }
    }

    /// 変わらない名前（`budget.records.exceeded` など）
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// 重さ
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// 変換の結果の段。変換の結果を表さない code では `None`
    pub const fn fidelity(&self) -> Option<Fidelity> {
        self.fidelity
    }

    /// 台帳で決めた値
    pub const fn params(&self) -> &'static [Param] {
        self.params
    }

    /// まとめるときに比べる値の名前
    pub const fn group_by(&self) -> &'static [&'static str] {
        self.group_by
    }

    /// 名前で値を引く
    pub fn param(&self, name: &str) -> Option<Param> {
        self.params.iter().find(|param| param.name == name).copied()
    }
}

impl PartialEq for Code {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Code {}

impl PartialOrd for Code {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Code {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(other.name)
    }
}

impl Hash for Code {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use crate::codes;

    #[test]
    fn all_codes_are_sorted_unique_and_well_formed() {
        let names: Vec<&str> = codes::ALL.iter().map(|code| code.name()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(names, sorted);
        for name in names {
            assert_eq!(name.split('.').count(), 3, "{name}");
        }
    }

    #[test]
    fn looks_up_params_by_name() {
        let code = codes::BUDGET_RECORDS_EXCEEDED;
        assert!(code.param("limit").is_some());
        assert!(code.param("missing").is_none());
    }
}
