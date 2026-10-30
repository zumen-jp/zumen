//! 寸法と引出線。

use zumen_base::Ref;

use crate::{Horizontal, Length, Span, Vertical};

/// 寸法。測る空間は定義点の空間とする。保存された測定値・文字の上書き・表示用の部品は別に持ち、
/// 定義点から測り直した値で文字を書き換えない（決定 0022）
#[derive(Clone, Debug, PartialEq)]
pub struct Dimension<P> {
    /// 種類と定義点
    pub kind: DimensionKind<P>,
    /// 元の形式が保存した測定値。なければ `None`
    pub measurement: Option<Measured>,
    /// 寸法値の文字
    pub label: Label,
    /// 寸法値の文字の位置と向き
    pub text: DimensionText<P>,
    /// 元の寸法スタイルの名前
    pub style: String,
    /// 寸法を描いた部品の名前。表示用の構成要素で、なければ `None`
    pub block: Option<String>,
}

/// 寸法の種類と定義点。角度は度で、反時計回り
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DimensionKind<P> {
    /// 2 点を、`rotation` の向きに測った長さ
    Linear {
        /// 1 本目の補助線の点
        first: P,
        /// 2 本目の補助線の点
        second: P,
        /// 寸法線が通る点
        line: P,
        /// 測る向き（度）
        rotation: f64,
        /// 補助線の傾き（度）。傾けなければ 0
        oblique: f64,
    },
    /// 2 点を結ぶ向きに測った長さ
    Aligned {
        /// 1 本目の補助線の点
        first: P,
        /// 2 本目の補助線の点
        second: P,
        /// 寸法線が通る点
        line: P,
        /// 補助線の傾き（度）。傾けなければ 0
        oblique: f64,
    },
    /// 頂点から 2 点へ向かう線のなす角度
    Angular {
        /// 頂点
        vertex: P,
        /// 1 本目の線の点
        first: P,
        /// 2 本目の線の点
        second: P,
        /// 寸法の弧が通る点
        arc: P,
    },
    /// 2 本の線のなす角度
    AngularLines {
        /// 1 本目の線の始点と終点
        first: [P; 2],
        /// 2 本目の線の始点と終点
        second: [P; 2],
        /// 寸法の弧が通る点
        arc: P,
    },
    /// 円弧の長さ。角度に変えない（決定 0127）
    ArcLength {
        /// 円弧の中心
        center: P,
        /// 円弧の始めの点
        first: P,
        /// 円弧の終わりの点
        second: P,
        /// 寸法の弧が通る点
        arc: P,
    },
    /// 半径
    Radius {
        /// 中心
        center: P,
        /// 円の上の点
        chord: P,
        /// 引出線の長さ
        leader: Length,
    },
    /// 直径
    Diameter {
        /// 円の上の点
        chord: P,
        /// `chord` と向かい合う円の上の点
        opposite: P,
        /// 引出線の長さ
        leader: Length,
    },
    /// 原点からの X か Y の座標
    Ordinate {
        /// 原点
        origin: P,
        /// 測る点
        feature: P,
        /// 引出線の終点
        leader_end: P,
        /// 測る座標の軸
        axis: Axis,
    },
}

/// 座標の軸
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// X
    X,
    /// Y
    Y,
}

/// 元の形式が保存した値。単位と、倍率を掛けたかは形式ごとに違うので、形式と欄を指すキーと組にする
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    /// 値
    pub value: f64,
    /// 形式と欄を指すキー
    pub key: &'static str,
}

/// 寸法値の文字
#[derive(Clone, Debug, PartialEq)]
pub enum Label {
    /// 測定値を書く
    Measurement,
    /// 利用者が書いた文字。測定値に置き換える符号（DXF の `<>`）は `Piece::Measurement` の区間にする
    Override {
        /// 元の内容。測定値に置き換える符号と書式の符号も、そのまま持つ
        content: String,
        /// 内容を解釈した区間
        spans: Vec<Span>,
    },
    /// 寸法値を書かない
    Hidden,
}

/// 寸法値の文字の位置と向き
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimensionText<P> {
    /// 文字の中央の点。元の形式が持たなければ `None`
    pub position: Option<P>,
    /// 利用者が文字を動かした
    pub moved: bool,
    /// 回転（度、反時計回り）。寸法線に沿わせるなら `None`
    pub rotation: Option<f64>,
    /// 文字の位置合わせ。元の形式が持たなければ `None`
    pub attachment: Option<(Horizontal, Vertical)>,
}

/// 引出線。注記は別の要素で、出所で指す
#[derive(Clone, Debug, PartialEq)]
pub struct Leader<P> {
    /// 経路の頂点。矢印の側から並べる
    pub path: Vec<P>,
    /// 頂点を通る曲線で結ぶ。偽なら折れ線
    pub spline: bool,
    /// 始点に矢印を付ける。元の形式が決めず、寸法スタイルに従うなら `None`
    pub arrow: Option<bool>,
    /// 注記の要素の出所。注記がなければ `None`
    pub annotation: Option<Ref>,
    /// 元の寸法スタイルの名前
    pub style: String,
}
