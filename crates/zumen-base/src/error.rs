//! 読み書きが失敗した理由。L0 の読み取りから形式のコーデックまで、同じ型で返す。

use crate::cx::Stop;

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
