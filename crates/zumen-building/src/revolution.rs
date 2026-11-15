//! 評価前の面の断面の回転。配置・単位換算・輪郭と軸の交差判定は行わない。

use crate::{AreaProfile, AxisPlacement3, Measured};

/// 回転の軸。長さは併せて取得した文書の単位、方向は正規化しない元の比
#[derive(Clone, Debug, PartialEq)]
pub struct RevolutionAxis {
    /// 断面と同じ局所座標の原点
    pub location: [f64; 3],
    /// 0 でない元の方向比
    pub direction: [f64; 3],
}

/// 面の断面を軸のまわりに回す定義。形の評価や既定の配置の補完は行わない
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileRevolution {
    /// 面の断面。任意断面の外輪の値は併せて取得する
    pub profile: AreaProfile,
    /// 立体全体の局所の配置。省略と元の方向比を保持する
    pub position: Option<AxisPlacement3>,
    /// 断面と同じ平面にある回転軸
    pub axis: RevolutionAxis,
    /// 元の回転角と文書の平面角の単位。ラジアンへ換算しない
    pub angle: Measured,
}
