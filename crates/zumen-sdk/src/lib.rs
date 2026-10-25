//! zumen の形式を書くための部品（決定 0008・0017）。核の形式も、外部のプラグインも、これで書く。
//!
//! - [`Codec`]・[`FileModel`] は、L1 のファイルモデルと、その符号化の 1 つ
//! - [`Signature`] は、形式の判定の規則。コーデックがデータとして持つ
//! - [`testkit`] は、往復・決定性・レコードの範囲を確かめる試験の道具
//!
//! trait の形は、AutoCAD の系統の試作（#3）で育てている。トランザクション・`Lift`・`Lower` は、
//! 試作の次の段階で足す。

pub mod testkit;

pub use zumen_base::{ByteRange, Cx, Stop};

/// 読み書きが失敗した理由。詳しいことは、診断として文脈に足してある
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// 予算の上限か、中断の合図で止めた
    Stopped(Stop),
    /// 入力が壊れていて読めない
    Invalid,
    /// 形式は分かったが、このコーデックでは扱えない
    Unsupported,
}

impl From<Stop> for Error {
    fn from(stop: Stop) -> Self {
        Error::Stopped(stop)
    }
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

/// 形式に依存しない、レコードの見出し
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordInfo<'a> {
    /// 形式が決めるレコードの種類（DXF なら `LINE` など）。生のバイト列
    pub kind: &'a [u8],
    /// 元のバイト範囲。読んだ後に作ったレコードにはない
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
