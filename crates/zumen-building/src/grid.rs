//! 評価前の通り芯の指定。配置と軸の向きの適用、Building と Hash への統合は別の対象。
use crate::GridSet;

/// 通り芯の配置の種類の指定。軸がこの配置に適合するかは評価しない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridLayout {
    /// RECTANGULAR
    Rectangular,
    /// RADIAL
    Radial,
    /// TRIANGULAR
    Triangular,
    /// IRREGULAR
    Irregular,
    /// USERDEFINED
    UserDefined,
    /// NOTDEFINED
    NotDefined,
}

/// 通り芯の軸の指定。曲線の定義と出所は別に持つ
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridAxisDefinition {
    /// U・V・W のどの列に属するか
    pub set: GridSet,
    /// 元の軸の名前。省略を空の文字列にしない
    pub tag: Option<String>,
    /// 基の曲線と同じ向きか。曲線の点や媒介変数は反転しない
    pub same_sense: bool,
}
