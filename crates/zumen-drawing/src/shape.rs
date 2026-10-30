//! 要素と、その形。折れ線・曲線・ハッチングの境界の辺のように、形どうしで使う部品もここに置く。

use zumen_base::Ref;

use crate::{
    Callout, Dimension, FormattedText, Hatch, LayerId, Leader, Length, Style, Symbol, Table, Text,
    Underlay,
};

/// 要素。`P` は要素が属する空間の点
#[derive(Clone, Debug, PartialEq)]
pub struct Element<P> {
    /// 元の形式のレコード
    pub source: Ref,
    /// レイヤ
    pub layer: LayerId,
    /// 色・線種・線幅
    pub style: Style,
    /// 形と意味
    pub shape: Shape<P>,
}

/// 要素の形と意味
#[derive(Clone, Debug, PartialEq)]
pub enum Shape<P> {
    /// 線分
    Line {
        /// 始点
        start: P,
        /// 終点
        end: P,
    },
    /// 円
    Circle {
        /// 中心
        center: P,
        /// 半径
        radius: Length,
    },
    /// 曲線（円弧・楕円弧・スプライン）。元の定義のまま持ち、線分に近似しない（決定 0111）
    Curve(Edge<P>),
    /// 膨らみを持つ頂点を結んだ折れ線
    Polyline(Polyline<P>),
    /// 文字
    Text(Text<P>),
    /// 書式つきの文字
    Formatted(FormattedText<P>),
    /// 寸法
    Dimension(Dimension<P>),
    /// 引出線
    Leader(Leader<P>),
    /// 引出線つきの注記（バルーンを含む）
    Callout(Callout<P>),
    /// 表
    Table(Table<P>),
    /// 下敷きの画像・PDF など
    Underlay(Underlay<P>),
    /// 記号
    Symbol(Symbol<P>),
    /// ハッチング
    Hatch(Hatch<P>),
}

/// 折れ線の頂点
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex<P> {
    /// 点
    pub point: P,
    /// 次の頂点までの膨らみ。弧の中心角の 1/4 の正接で、正なら反時計回り。直線なら 0
    pub bulge: f64,
}

/// 膨らみを持つ頂点を結んだ折れ線
#[derive(Clone, Debug, PartialEq)]
pub struct Polyline<P> {
    /// 頂点。順序は入力の順
    pub vertices: Vec<Vertex<P>>,
    /// 終点と始点を結ぶ
    pub closed: bool,
    /// 一定の幅。なければ幅を持たない
    pub width: Option<Length>,
    /// 頂点ごとの、次の頂点までの始まりと終わりの幅。元の形式が持たなければ空
    pub vertex_widths: Vec<[Length; 2]>,
    /// 線種の模様を頂点ごとに切らず、折れ線の全体で続ける（DXF・DWG の線種の生成）。元の形式が持たなければ偽
    /// （決定 0157）
    pub line_type_generation: bool,
}

/// 辺と曲線。角度は度で、反時計回り
#[derive(Clone, Debug, PartialEq)]
pub enum Edge<P> {
    /// 線分
    Line {
        /// 始点
        start: P,
        /// 終点
        end: P,
    },
    /// 円弧
    Arc {
        /// 中心
        center: P,
        /// 半径
        radius: Length,
        /// 始まりの角度
        start: f64,
        /// 終わりの角度
        end: f64,
        /// 反時計回りにたどる
        counterclockwise: bool,
    },
    /// 楕円弧
    Elliptic {
        /// 中心
        center: P,
        /// 長軸の端の点
        major: P,
        /// 短軸の長軸に対する比
        ratio: f64,
        /// 始まりの角度。楕円を円に引き延ばしたときの角度
        start: f64,
        /// 終わりの角度。楕円を円に引き延ばしたときの角度
        end: f64,
        /// 反時計回りにたどる
        counterclockwise: bool,
    },
    /// スプライン
    Spline(Spline<P>),
    /// クロソイド。原点（曲率が 0 の点）からの長さ s の点での接線の向きは、原点での向きから
    /// s²/2A² ラジアンだけ曲がる向きへ回る。元の定義のまま持ち、点の列に近似しない（決定 0123）
    Clothoid {
        /// 原点
        origin: P,
        /// パラメーター A
        parameter: Length,
        /// 原点での接線の向き
        angle: f64,
        /// 反時計回り（左）に曲がる
        counterclockwise: bool,
        /// 原点からの、始まりの長さ。原点より手前は負
        start: Length,
        /// 原点からの、終わりの長さ
        end: Length,
    },
}

/// スプライン。制御点と節点の定義と、当てはめる点の定義の、元の形式が持つものを持つ。片方から
/// 他方を計算しない
#[derive(Clone, Debug, PartialEq)]
pub struct Spline<P> {
    /// 次数
    pub degree: u32,
    /// 節点
    pub knots: Vec<f64>,
    /// 制御点
    pub control: Vec<P>,
    /// 制御点の重み。有理でなければ空
    pub weights: Vec<f64>,
    /// 当てはめる点
    pub fit: Vec<P>,
    /// 始まりの接線の向き。なければ `None`
    pub start_tangent: Option<[f64; 2]>,
    /// 終わりの接線の向き。なければ `None`
    pub end_tangent: Option<[f64; 2]>,
    /// 閉じている
    pub closed: bool,
    /// 周期的
    pub periodic: bool,
    /// 当てはめる点を通す許容差（要素の空間の長さ）。0 なら点を通る。元の形式が持たなければ `None`（決定 0157）
    pub fit_tolerance: Option<f64>,
    /// 当てはめる点から節点を決める取り方。当てはめる点で決まるスプラインでないか、元の形式が持たなければ
    /// `None`（決定 0157）
    pub knot_parameterization: Option<KnotParameterization>,
}

/// 当てはめる点から節点を決める取り方（DXF・DWG の節点のパラメーター化）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KnotParameterization {
    /// 隣の点との距離に比例させる
    Chord,
    /// 隣の点との距離の平方根に比例させる
    SquareRoot,
    /// 等しい間隔にする
    Uniform,
    /// 元の形式が持つ節点を使う
    Custom,
}
