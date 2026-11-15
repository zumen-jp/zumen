//! 評価前の掃引円盤。曲線の定義と単位は、同じ取得結果の CurveGraph が持つ。

use crate::CurveRef;

/// 円盤を曲線に沿って動かす定義。配置・近似・輪郭や立体の評価は行わない
#[derive(Clone, Debug, PartialEq)]
pub struct DiskSweep {
    /// 同じ取得結果の CurveGraph の経路。元のレコードの番号ではない
    pub directrix: CurveRef,
    /// 文書の長さの単位で持つ正の外半径
    pub radius: f64,
    /// 穴があるときの正の内半径。外半径より小さい
    pub inner_radius: Option<f64>,
    /// 元の曲線の媒介変数で表す始点。省略はそのまま保持する
    pub start_parameter: Option<f64>,
    /// 元の曲線の媒介変数で表す終点。省略はそのまま保持する
    pub end_parameter: Option<f64>,
}
