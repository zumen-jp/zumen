//! 点と、文書の点の表し方（決定 0160）。

use std::borrow::Cow;

/// 点。`P` は点の属する空間の点
#[derive(Clone, Debug, PartialEq)]
pub struct Point<P> {
    /// 位置
    pub position: P,
    /// 回転（度、反時計回り）
    pub rotation: f64,
    /// 描き方
    pub look: PointLook,
    /// 作図の補助の点（JWW の仮点）。表示する側が描くかを決め、印刷はしない
    pub construction: bool,
}

/// 点の描き方
#[derive(Clone, Debug, PartialEq)]
pub enum PointLook {
    /// 文書の点の表し方（[`crate::Drawing2D::point_style`]）に従う
    Document,
    /// 既定義の印。印の形は、表示する側がキーから引く
    Marker {
        /// 印の表のキー
        key: MarkerKey,
        /// 尺度
        scale: f64,
    },
}

/// 既定義の印の表のキー
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarkerKey {
    /// 名前空間（`sxf.marker`・`jww.point`）
    pub namespace: Cow<'static, str>,
    /// 名前空間の中の元のコード
    pub code: String,
}

/// 文書の点の表し方
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointStyle {
    /// 基本の形
    pub mark: PointMark,
    /// 円で囲む
    pub circle: bool,
    /// 四角で囲む
    pub square: bool,
    /// 大きさ
    pub size: PointSize,
}

/// 点の基本の形
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointMark {
    /// 小さな点
    Dot,
    /// なし
    None,
    /// 十字
    Plus,
    /// バツ
    Cross,
    /// 上向きの短い線
    Tick,
}

/// 点の大きさ
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointSize {
    /// 点のある空間の長さ
    Length(f64),
    /// 画面の高さに対する百分率
    Screen(f64),
}
