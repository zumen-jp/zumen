//! 入りきらない入力を、範囲で読む（決定 0232）。
//!
//! コアは入力の全体を受けず、ホストが渡した範囲の表（[`Source`]）から読む。足りない範囲があれば、
//! [`Missing::Need`] で要る範囲を返し、ホストはその範囲を渡してから同じ要求をもう一度呼ぶ。全体を渡した入力も、
//! 範囲が 1 つの [`Source`] として同じ道で読む。

use crate::{ByteRange, Cx, Error, Signature, Stop};

/// ホストが渡した、入力の範囲の表
#[derive(Clone, Copy, Debug)]
pub struct Source<'a> {
    /// 入力の全体の大きさ
    pub size: u64,
    /// 渡された範囲。初めの位置とバイト列の組で、初めの位置の順に並べる
    pub parts: &'a [(u64, &'a [u8])],
}

impl<'a> Source<'a> {
    /// `start` から `end` までのバイト列。1 つの渡された範囲に収まっていなければ、要る範囲を返す
    pub fn get(&self, start: u64, end: u64) -> Result<&'a [u8], ByteRange> {
        let wanted = ByteRange {
            start,
            end: end.min(self.size),
        };
        for (at, bytes) in self.parts {
            let Some(from) = start.checked_sub(*at) else {
                continue;
            };
            let (Ok(from), Ok(to)) = (
                usize::try_from(from),
                usize::try_from(wanted.end.saturating_sub(*at)),
            ) else {
                continue;
            };
            if let Some(found) = bytes.get(from..to) {
                return Ok(found);
            }
        }
        Err(wanted)
    }
}

/// 範囲で読めなかった理由
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Missing {
    /// 要る範囲。渡してからもう一度呼ぶ
    Need(Vec<ByteRange>),
    /// 読めない。理由は、診断として文脈に足してある
    Failed(Error),
}

impl From<Error> for Missing {
    fn from(error: Error) -> Self {
        Missing::Failed(error)
    }
}

impl From<Stop> for Missing {
    fn from(stop: Stop) -> Self {
        Missing::Failed(Error::Stopped(stop))
    }
}

/// 範囲で読む形式（決定 0232）。[`crate::Codec`] と同じく、形式ごとに 1 つ
pub trait Ranged {
    /// 読んだファイルモデル
    type File;

    /// 形式の台帳（`formats/ledger.toml`）のキー
    const FORMAT: &'static str;

    /// 形式の判定の規則。入力の初めの部分で見分ける
    const SIGNATURES: &'static [Signature];

    /// 範囲の表から読む
    fn read_ranged(src: &Source<'_>, cx: &mut Cx) -> Result<Self::File, Missing>;
}
