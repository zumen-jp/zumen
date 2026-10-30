//! 下敷きと、その資源・置き方・切り抜き。

use zumen_base::Ref;

use crate::Length;

/// 下敷き（決定 0114）。資源を参照名で指し、コアは開かない。描く順は、ほかの要素と同じく空間と用紙の一覧に従う
#[derive(Clone, Debug, PartialEq)]
pub struct Underlay<P> {
    /// 資源の種類
    pub kind: UnderlayKind,
    /// 資源
    pub resource: Resource,
    /// 資源の中のページなど。元の形式の名前のまま持つ
    pub page: Option<String>,
    /// 置き方
    pub frame: UnderlayFrame<P>,
    /// 切り抜き
    pub clip: Option<Clip>,
    /// 表示する
    pub visible: bool,
}

/// 下敷きの資源の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnderlayKind {
    /// 画素の画像
    Image,
    /// PDF
    Pdf,
    /// DWF
    Dwf,
    /// DGN
    Dgn,
}

/// 図面が指す資源。内容はホストが渡して解決する
#[derive(Clone, Debug, PartialEq)]
pub struct Resource {
    /// 元の参照名（ファイルのパスなど）。翻訳も正規化もしない
    pub path: String,
    /// 参照を定義する元の形式のレコード
    pub definition: Ref,
}

/// 下敷きの置き方
#[derive(Clone, Debug, PartialEq)]
pub enum UnderlayFrame<P> {
    /// 画素の格子。元の形式が持つ画素の数と、左下・右下・左上の角
    Pixels {
        /// 画素の数（横・縦）
        size: [f64; 2],
        /// 左下の角
        origin: P,
        /// 右下の角
        right: P,
        /// 左上の角
        top: P,
    },
    /// 内容の全体を、左下・右下・左上の角の平行四辺形に収める。元の形式が画素の数を持たないもの（決定 0125）
    Corners {
        /// 左下の角
        origin: P,
        /// 右下の角
        right: P,
        /// 左上の角
        top: P,
    },
    /// 資源の内容の座標を、尺度・回転・移動で写す
    Scaled {
        /// 内容の原点を置く点
        origin: P,
        /// 内容の 1 単位の、X と Y の長さ
        scale: [Length; 2],
        /// 回転（度、反時計回り）
        rotation: f64,
    },
}

/// 下敷きと記号の切り抜き。頂点の座標は、使う側（下敷きは元の形式の下敷きの座標、記号は定義の空間の座標）が決める
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// 境界の頂点。2 つなら、向かい合う角の長方形
    pub vertices: Vec<[f64; 2]>,
    /// 境界の内側を隠す
    pub inverted: bool,
}
