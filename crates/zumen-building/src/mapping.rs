//! 評価前の共有写像の配置と変換の値。

use crate::AxisPlacement3;

/// 3D の写像の尺度。省略時の既定値は適用しない
#[derive(Clone, Debug, PartialEq)]
pub enum MappingScale3 {
    /// 一様な尺度。省略したときの既定値は 1
    Uniform(Option<f64>),
    /// 軸ごとの尺度。x の省略時は 1、y と z の省略時は x の尺度を使う
    NonUniform {
        /// 元の x 方向の尺度
        x: Option<f64>,
        /// 元の y 方向の尺度
        y: Option<f64>,
        /// 元の z 方向の尺度
        z: Option<f64>,
    },
}

/// 平行移動・回転・鏡映・尺度を定義する元の 3D の属性。行列へ変換しない
#[derive(Clone, Debug, PartialEq)]
pub struct CartesianMapping3 {
    /// 要素の座標系で持つ移動先の原点
    pub local_origin: [f64; 3],
    /// x 軸を導く元の方向の比
    pub axis1: Option<[f64; 3]>,
    /// y 軸を導く元の方向の比
    pub axis2: Option<[f64; 3]>,
    /// z 軸を定める元の方向の比
    pub axis3: Option<[f64; 3]>,
    /// 正の有限な尺度。方向の比を正規化する前後の長さを掛けない
    pub scale: MappingScale3,
}

/// 写像元の基準位置と、写像先の変換。元の定義に値を適用しない
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeMapping3 {
    /// 写像元の基準位置
    pub origin: AxisPlacement3,
    /// 写像先の元属性
    pub target: CartesianMapping3,
}
