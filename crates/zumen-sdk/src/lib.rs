//! zumen の形式を書くための部品（決定 0008・0017）。コアの形式も、外部のプラグインも、これで書く。
//!
//! - [`Codec`]・[`FileModel`] は、L1 のファイルモデルと、その符号化の 1 つ
//! - [`Transact`]・[`Transaction`] は、L1 の変更を確かめてからまとめて反映する仕組み
//! - [`Lift`] は、L1 から L2 の領域モデルを作る変換
//! - [`Apply`] は、L2 の変更を元の L1 へその場で書き戻す
//! - [`Signature`] は、形式の判定の規則。コーデックがデータとして持つ
//! - [`testkit`] は、往復・決定性・レコードの範囲を確かめる試験の道具
//!
//! trait の形は、AutoCAD の系統の試作（#3）で育てている。`Lower` は、別の形式への変換を作るときに足す。

pub mod testkit;

pub use zumen_base::{ByteRange, Cx, Stop};

use zumen_base::Diagnostics;

/// 読み書きが失敗した理由。詳しいことは、診断として文脈に足してある
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// 予算の上限か、中断の合図で止めた
    Stopped(Stop),
    /// 入力が壊れていて読めない
    Invalid,
    /// 形式は分かったが、このコーデックでは扱えない
    Unsupported,
    /// 変更が形式の規則に合わないので、何も反映しなかった
    Rejected,
}

impl From<Stop> for Error {
    fn from(stop: Stop) -> Self {
        Error::Stopped(stop)
    }
}

/// L1 から L2 の領域モデル `M` を作る（決定 0008・0022）。形式と領域の組ごとに 1 つ。
///
/// 移せなかったもの・形を変えたものは、忠実度つきの診断として、元のレコードを指して文脈に足す。
/// 移した要素は、元のレコードを指す出所を持つ
pub trait Lift<M>: FileModel {
    /// 領域モデルを作る。予算の上限か中断の合図で止めたときだけ失敗する
    fn lift(&self, cx: &mut Cx) -> Result<M, Error>;
}

/// 診断を混ぜずに読む・変換する。止めたときだけ、止めた理由を文脈に足す
pub fn quietly<T>(cx: &mut Cx, run: impl FnOnce(&mut Cx) -> Result<T, Error>) -> Result<T, Error> {
    let mut quiet = Diagnostics::new();
    let result = run(&mut Cx::new(cx.budget, &mut quiet, cx.cancel));
    if let Err(Error::Stopped(stop)) = &result {
        cx.report(stop.to_diagnostic());
    }
    result
}

/// L2 の領域モデルへの変更 `C` を、元の L1 にその場で書き戻す（決定 0008・0139）
pub trait Apply<C>: FileModel {
    /// 当てた結果（図面モデルでは、変更ごとの新しい ID とハッシュ）
    type Output;

    /// すべての変更を確かめてから反映する。1 つでも書き戻せなければ、何も反映せずに
    /// [`Error::Rejected`] を返す。理由は、当てられない変更ごとの診断として文脈に足す
    fn apply(&mut self, changes: &C, cx: &mut Cx) -> Result<Self::Output, Error>;
}

/// L1: 形式の系統のモデルと、その符号化の 1 つ（決定 0008）
pub trait Codec {
    /// このコーデックが読み書きするファイルモデル
    type File: FileModel;

    /// 形式の台帳（`formats/ledger.toml`）のキー
    const FORMAT: &'static str;

    /// 形式の判定の規則。どれか 1 つに合えば、この形式とみなす
    const SIGNATURES: &'static [Signature];

    /// バイト列を読む
    fn read(src: &[u8], cx: &mut Cx) -> Result<Self::File, Error>;

    /// ファイルモデルを書く。変えていないレコードは、元のバイトのまま書く
    fn write(file: &Self::File, out: &mut Vec<u8>, cx: &mut Cx) -> Result<(), Error>;
}

/// L1 のファイルモデル。未知のものも含めて、全レコードを元のバイト範囲とともに辿れる
pub trait FileModel {
    /// レコードの数
    fn record_count(&self) -> usize;

    /// `index` 番目のレコード
    fn record(&self, index: usize) -> Option<RecordInfo<'_>>;
}

/// トランザクションで変更できる L1 のファイルモデル（決定 0008）
pub trait Transact: FileModel {
    /// 変更をためるトランザクション。変更の操作は、形式ごとに持つ
    type Txn<'a>: Transaction
    where
        Self: 'a;

    /// トランザクションを始める。確定せずに捨てると、何も反映しない
    fn transaction(&mut self) -> Self::Txn<'_>;
}

/// ためた変更を、まとめて確かめてから反映する
pub trait Transaction {
    /// すべての変更を確かめてから反映する。失敗したら、何も反映しない。
    /// 反映したレコードは、元のバイト範囲を持たなくなり、書くときに書き直す
    fn commit(self, cx: &mut Cx) -> Result<(), Error>;
}

/// 形式に依存しない、レコードの見出し
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordInfo<'a> {
    /// 形式が決めるレコードの種類（DXF なら `LINE` など）。生のバイト列
    pub kind: &'a [u8],
    /// 元のバイト範囲。読んだ後に作ったレコードと、変更したレコードにはない
    pub bytes: Option<ByteRange>,
}

/// 形式の判定の規則（決定 0007 の形式の判定）。TypeScript の判定も、このデータから作る
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signature {
    /// `offset` から `bytes` が続く
    Bytes {
        /// 先頭からの位置
        offset: usize,
        /// 続くバイト列
        bytes: &'static [u8],
    },
    /// 先頭の行が、前後の ASCII の空白を除いて `lines` と一致する。空の行は飛ばさない
    Lines {
        /// 先頭から順の行
        lines: &'static [&'static [u8]],
    },
}

impl Signature {
    /// 先頭のバイト列 `head` がこの規則に合うか
    pub fn matches(&self, head: &[u8]) -> bool {
        match *self {
            Signature::Bytes { offset, bytes } => offset
                .checked_add(bytes.len())
                .and_then(|end| head.get(offset..end))
                .is_some_and(|found| found == bytes),
            Signature::Lines { lines } => {
                let mut found = head.split(|&b| b == b'\n');
                lines.iter().all(|expected| {
                    found
                        .next()
                        .is_some_and(|line| line.trim_ascii() == *expected)
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_signature_needs_the_whole_pattern() {
        let sig = Signature::Bytes {
            offset: 2,
            bytes: b"CD",
        };
        assert!(sig.matches(b"ABCDE"));
        assert!(!sig.matches(b"ABC"));
        assert!(!sig.matches(b"ABXD"));
    }

    #[test]
    fn lines_signature_ignores_surrounding_spaces() {
        let sig = Signature::Lines {
            lines: &[b"0", b"SECTION"],
        };
        assert!(sig.matches(b"  0\r\nSECTION\r\n  2\r\nHEADER\r\n"));
        assert!(!sig.matches(b"0\n"));
        assert!(!sig.matches(b"\n0\nSECTION\n"));
    }
}
