//! 評価前の平面の半空間。元の配置・側の指定・囲みの座標系を保持する。

use crate::{AxisPlacement3, CurveRef};

/// 半空間の囲み箱を置く座標系
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnclosureCoordinates {
    /// 基準面の局所の座標系
    BaseSurface,
    /// 形を持つ要素の座標系
    Object,
}

/// 半空間の囲み。箱は計算用の囲み、多角形は形を制限する境界
#[derive(Clone, Debug, PartialEq)]
pub enum HalfSpaceLimit {
    /// 囲みなし
    Unbounded,
    /// 計算用の囲み箱。半空間との積を評価した結果ではない
    Box {
        /// 箱の隅の元の座標
        corner: [f64; 3],
        /// 各軸方向の正の長さ
        dimensions: [f64; 3],
        /// 箱の座標系。形式のバージョンによる違いを意味の値として持つ
        coordinates: EnclosureCoordinates,
    },
    /// 局所の XY 平面の境界で制限する半空間
    Polygonal {
        /// 境界の配置。基準面の配置とは別に持つ
        position: AxisPlacement3,
        /// 取得結果の境界の CurveGraph 内の参照
        boundary: CurveRef,
    },
}

/// 平面を境とする半空間。幾何の値に配置や単位を適用しない
#[derive(Clone, Debug, PartialEq)]
pub struct PlaneHalfSpace {
    /// 基準面の元の 3D 配置。局所 z 軸が面の法線
    pub plane: AxisPlacement3,
    /// true なら法線と逆側、false なら法線が向く側が半空間の内部
    pub agreement: bool,
    /// 囲みの元の定義
    pub limit: HalfSpaceLimit,
}
