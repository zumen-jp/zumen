//! 形式の登録（決定 0017）。コーデックの形式を、台帳（`formats/ledger.toml`）と同じキーと権利で窓口に知らせる。
//!
//! 窓口（`zumen-abi` など）は、登録の一覧から形式を判定し、capabilities の形式の一覧を作る。登録の権利と台帳の
//! 権利が合うことは、窓口の試験で確かめる。

use crate::{Codec, Signature};

/// 形式の権利（台帳の `rights`）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rights {
    /// 仕様が公開され、実装を制限する記述がない
    Public,
    /// 実装の公開に許諾が要る
    Permission,
    /// 権利が分からない
    Unknown,
}

impl Rights {
    /// 台帳の `rights` の値
    pub fn key(self) -> &'static str {
        match self {
            Rights::Public => "public",
            Rights::Permission => "permission",
            Rights::Unknown => "unknown",
        }
    }
}

/// 形式の登録
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registration {
    /// 台帳のキー（[`Codec::FORMAT`]）
    pub format: &'static str,
    /// 台帳の権利
    pub rights: Rights,
    /// 境界の形式の系統（capabilities の `family`）
    pub family: &'static str,
    /// 形式の判定の規則（[`Codec::SIGNATURES`]）
    pub signatures: &'static [Signature],
    /// 書き戻せるか
    pub write: bool,
    /// ID の規則の版（決定 0156）
    pub id_rules: u64,
}

impl Registration {
    /// コーデック `C` の登録。キーと判定の規則はコーデックから取る
    pub const fn of<C: Codec>(
        rights: Rights,
        family: &'static str,
        write: bool,
        id_rules: u64,
    ) -> Self {
        Registration {
            format: C::FORMAT,
            rights,
            family,
            signatures: C::SIGNATURES,
            write,
            id_rules,
        }
    }

    /// `bytes` が、この形式の判定の規則のどれかに合うか
    pub fn matches(&self, bytes: &[u8]) -> bool {
        self.signatures.iter().any(|rule| rule.matches(bytes))
    }
}

/// 登録の一覧の順に判定の規則を試し、`bytes` に合う最初の形式を返す
pub fn detect<'r>(registry: &'r [Registration], bytes: &[u8]) -> Option<&'r Registration> {
    registry
        .iter()
        .find(|registration| registration.matches(bytes))
}
