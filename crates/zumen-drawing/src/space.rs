//! 実寸の空間と用紙、それぞれの点と長さ、単位、ビューとアフィン変換。

use zumen_base::Ref;

use crate::{Element, LayerId, SheetId, SpaceId};

/// 文書の長さの単位。分からないものを mm とみなさない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// ミリメートル
    Millimeter,
    /// センチメートル
    Centimeter,
    /// メートル
    Meter,
    /// インチ
    Inch,
    /// フィート
    Foot,
    /// 元の形式が単位を決めていないか、長さの単位でない
    Unknown,
}

impl Unit {
    /// 1 単位の mm の数。不明なら `None`
    pub fn millimeters(self) -> Option<f64> {
        match self {
            Unit::Millimeter => Some(1.0),
            Unit::Centimeter => Some(10.0),
            Unit::Meter => Some(1000.0),
            Unit::Inch => Some(25.4),
            Unit::Foot => Some(304.8),
            Unit::Unknown => None,
        }
    }
}

/// 実寸の空間
#[derive(Clone, Debug, PartialEq)]
pub struct ModelSpace {
    /// ID
    pub id: SpaceId,
    /// 文書の長さの単位
    pub unit: Unit,
    /// 要素。順序は入力の順
    pub elements: Vec<Element<ModelPoint>>,
    /// 描く順。`elements` の添字を、先に描くものから並べる
    pub order: Vec<usize>,
}

/// 用紙
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    /// ID
    pub id: SheetId,
    /// 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。元の形式が名前を持たなければ `None`
    pub name: Option<String>,
    /// 幅と高さ。元の形式から決まらなければ `None`
    pub size: Option<[PaperLength; 2]>,
    /// 用紙上の要素。順序は入力の順
    pub elements: Vec<Element<PaperPoint>>,
    /// 実寸の空間を用紙に置くビュー。順序は入力の順
    pub views: Vec<View>,
    /// 描く順。要素とビューを、先に描くものから並べる
    pub order: Vec<SheetItem>,
    /// 元の形式の用紙のレコード（DXF・DWG の LAYOUT）。鍵があれば、ID を鍵で作る（決定 0156）
    pub source: Ref,
}

/// 用紙の上に描くもの
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetItem {
    /// `elements` の添字
    Element(usize),
    /// `views` の添字
    View(usize),
}

/// 実寸の空間の点
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelPoint {
    /// 属する空間
    pub space: SpaceId,
    /// X（文書の単位）
    pub x: f64,
    /// Y（文書の単位）
    pub y: f64,
}

impl ModelPoint {
    /// 2 点の距離。空間が違えば `None`
    pub fn distance(&self, other: &ModelPoint) -> Option<ModelLength> {
        (self.space == other.space).then(|| ModelLength {
            space: self.space,
            value: norm(other.x - self.x, other.y - self.y),
        })
    }
}

/// 用紙の点
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperPoint {
    /// 属する用紙
    pub sheet: SheetId,
    /// X（mm）
    pub x: f64,
    /// Y（mm）
    pub y: f64,
}

impl PaperPoint {
    /// 2 点の距離。用紙が違えば `None`
    pub fn distance(&self, other: &PaperPoint) -> Option<PaperLength> {
        (self.sheet == other.sheet).then(|| PaperLength(norm(other.x - self.x, other.y - self.y)))
    }
}

/// ベクトルの長さ。`hypot` は環境で値が違いうるので、IEEE 754 で決まる平方根で求める（決定 0007）
pub(crate) fn norm(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

/// 用紙の長さ（mm）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperLength(pub f64);

/// 実寸の長さ
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModelLength {
    /// 属する空間
    pub space: SpaceId,
    /// 長さ（文書の単位）
    pub value: f64,
}

/// 用紙か実寸のどちらかの長さ
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// 用紙の長さ
    Paper(PaperLength),
    /// 実寸の長さ
    Model(ModelLength),
}

/// 実寸の空間を用紙に置くビュー
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    /// 置く空間
    pub space: SpaceId,
    /// 置く先の用紙
    pub sheet: SheetId,
    /// 実寸の点から用紙の点への変換
    pub transform: Affine,
    /// 用紙上の切り抜き。閉じた多角形の頂点
    pub clip: Vec<PaperPoint>,
    /// このビューで表示しないレイヤ
    pub hidden_layers: Vec<LayerId>,
    /// 出所
    pub source: Ref,
}

impl View {
    /// 実寸の点を用紙に置く。点の空間がビューの空間でなければ `None`
    pub fn place(&self, point: ModelPoint) -> Option<PaperPoint> {
        (point.space == self.space).then(|| {
            let (x, y) = self.transform.apply(point.x, point.y);
            PaperPoint {
                sheet: self.sheet,
                x,
                y,
            }
        })
    }

    /// 実寸の向き `direction` に測った長さの、用紙の上の長さ。非等方の変換では、向きによって値が変わる。
    /// 長さの空間がビューの空間でないか、向きが 0 か有限でなければ `None`
    pub fn paper_length(&self, length: ModelLength, direction: [f64; 2]) -> Option<PaperLength> {
        let [dx, dy] = direction;
        let size = norm(dx, dy);
        if length.space != self.space || !size.is_finite() || size == 0.0 {
            return None;
        }
        let [a, b, _, d, e, _] = self.transform.m;
        let [ux, uy] = [dx / size, dy / size];
        Some(PaperLength(
            length.value * norm(a * ux + b * uy, d * ux + e * uy),
        ))
    }
}

/// 2 次元のアフィン変換。`(x, y)` を `(a x + b y + c, d x + e y + f)` に写す
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    /// `[a, b, c, d, e, f]`
    pub m: [f64; 6],
}

impl Affine {
    /// 点を写す
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.m;
        (a * x + b * y + c, d * x + e * y + f)
    }
}
