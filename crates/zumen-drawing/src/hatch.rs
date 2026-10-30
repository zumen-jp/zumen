//! ハッチングの輪と、塗り・パターン・グラデーション。

use zumen_base::Ref;

use crate::{Edge, Vertex};

/// ハッチング。境界の輪と、中を埋めるものを持つ。境界を作った要素は、輪ごとに出所で指す
#[derive(Clone, Debug, PartialEq)]
pub struct Hatch<P> {
    /// 境界の輪。順序は入力の順
    pub loops: Vec<HatchLoop<P>>,
    /// 輪の中の輪の埋め方
    pub islands: Islands,
    /// 中を埋めるもの
    pub fill: Fill,
    /// 境界を作った要素に合わせて、境界を作り直す
    pub associative: bool,
    /// 境界を探した点。順序は入力の順
    pub seeds: Vec<P>,
}

/// ハッチングの境界の輪
#[derive(Clone, Debug, PartialEq)]
pub struct HatchLoop<P> {
    /// 形
    pub boundary: Boundary<P>,
    /// 外側の輪
    pub external: bool,
    /// いちばん外の輪
    pub outermost: bool,
    /// 境界を探して作った輪
    pub derived: bool,
    /// 文字の枠
    pub text_box: bool,
    /// 輪を作った要素の出所。順序は入力の順
    pub sources: Vec<Ref>,
}

/// ハッチングの境界の輪の形
#[derive(Clone, Debug, PartialEq)]
pub enum Boundary<P> {
    /// 膨らみを持つ頂点を結んだ折れ線
    Polyline {
        /// 頂点。順序は入力の順
        vertices: Vec<Vertex<P>>,
        /// 終点と始点を結ぶ
        closed: bool,
    },
    /// 辺の並び
    Edges(Vec<Edge<P>>),
}

/// 輪の中の輪の埋め方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Islands {
    /// 外から数えて、奇数番目の輪の内側を埋める
    Nested,
    /// いちばん外の輪と、その内側の最初の輪の間だけを埋める
    Outer,
    /// 内側の輪を無視して、いちばん外の輪の内側をすべて埋める
    Ignore,
}

/// ハッチングの中を埋めるもの
#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    /// 要素の色で塗る
    Solid,
    /// 線の組で埋める
    Pattern(Pattern),
    /// グラデーションで塗る
    Gradient(Gradient),
}

/// ハッチングのパターン
#[derive(Clone, Debug, PartialEq)]
pub struct Pattern {
    /// 元の名前。翻訳しない
    pub name: String,
    /// 定義の出どころ
    pub origin: PatternOrigin,
    /// 定義に加えた回転（度）
    pub angle: f64,
    /// 定義に掛けた尺度か、利用者が決めた線の間隔
    pub scale: f64,
    /// 線の組を 90 度回して重ねる
    pub double: bool,
    /// 線の組。`angle` と `scale` を当てた後の値
    pub lines: Vec<PatternLine>,
}

/// パターンの定義の出どころ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternOrigin {
    /// 利用者が角度と間隔で決めた平行線
    UserDefined,
    /// 形式が名前で決めた定義
    Predefined,
    /// 利用者の定義のファイル
    Custom,
}

/// パターンの 1 組の平行線。長さは、要素の空間の文書の単位（[`PatternBasis::Element`](crate::PatternBasis::Element)）
#[derive(Clone, Debug, PartialEq)]
pub struct PatternLine {
    /// 線の向き（度）
    pub angle: f64,
    /// 1 本の線が通る点
    pub base: [f64; 2],
    /// 隣の線までのずれ
    pub offset: [f64; 2],
    /// 線と間の長さの列。正は線、負は間、0 は点。空なら実線
    pub dashes: Vec<f64>,
}

/// ハッチングのグラデーション
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// 元の名前。翻訳しない
    pub name: String,
    /// 回転（度）
    pub angle: f64,
    /// 中心からのずれ。0 は中心
    pub shift: f64,
    /// 色の RGB。順序は入力の順
    pub colors: Vec<[u8; 3]>,
    /// 1 色で塗るときの、色の濃さ。2 色なら `None`
    pub tint: Option<f64>,
}
