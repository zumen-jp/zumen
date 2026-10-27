//! 図面の領域モデル（L2）の最小の部分（決定 0022）。形式に依存しない。
//!
//! 実寸の空間（[`ModelSpace`]）と用紙（[`Sheet`]）を分け、点と長さは型でどちらの空間かを区別する。
//! 用紙は mm で、X が右、Y が上、原点は用紙の左下（決定 0019）。実寸は空間ごとの文書の単位で持つ。
//! [`View`] は、実寸の空間を用紙に置くアフィン変換と、用紙上の切り抜きを持つ。
//!
//! 点どうしの計測とビューでの配置は、空間か用紙の ID が合わなければ値を返さない。別の部分図を混ぜて計測しない。
//!
//! レイヤは図面の一覧（[`Layer`]）に置き、要素は ID で指す。描く順は、空間と用紙ごとの一覧（`order`）で持ち、
//! 要素の一覧は入力の順のままにする。
//!
//! 要素は、いまは線分と文字だけ。寸法・記号・ハッチング・下敷き・スタイルの解決などは #28 で足す。
//! どの要素も、元の形式のレコードを指す出所（[`Ref`]）を持つ。

use zumen_base::Ref;

/// 図面。実寸の空間と用紙を、順序のある一覧で持つ
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawing2D {
    /// 実寸の空間
    pub spaces: Vec<ModelSpace>,
    /// 用紙
    pub sheets: Vec<Sheet>,
    /// レイヤ。順序は元の形式の定義の順
    pub layers: Vec<Layer>,
}

impl Drawing2D {
    /// ID のレイヤ
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }
}

/// 実寸の空間の ID。同じ図面の中で重ならない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpaceId(pub u32);

/// 用紙の ID。同じ図面の中で重ならない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SheetId(pub u32);

/// レイヤの ID。同じ図面の中で重ならない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(pub u32);

/// レイヤ。同じ名前でも、定義が別なら別のレイヤにする
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// ID
    pub id: LayerId,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 表示・印刷・編集の可否。要素が名前で指すだけで、ファイルに定義がなければ `None`
    pub state: Option<LayerState>,
    /// 定義の出所。定義がなければ既定の値
    pub source: Ref,
}

/// レイヤの表示・印刷・編集の可否。どれも、ビューごとの指定（[`View::hidden_layers`]）より前の値
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerState {
    /// 表示する
    pub visible: bool,
    /// 印刷する
    pub printable: bool,
    /// 編集できる
    pub editable: bool,
}

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
    /// 幅と高さ。元の形式から決まらなければ `None`
    pub size: Option<[PaperLength; 2]>,
    /// 用紙上の要素。順序は入力の順
    pub elements: Vec<Element<PaperPoint>>,
    /// 実寸の空間を用紙に置くビュー。順序は入力の順
    pub views: Vec<View>,
    /// 描く順。要素とビューを、先に描くものから並べる
    pub order: Vec<SheetItem>,
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
fn norm(x: f64, y: f64) -> f64 {
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

/// 要素。`P` は要素が属する空間の点
#[derive(Clone, Debug, PartialEq)]
pub struct Element<P> {
    /// 元の形式のレコード
    pub source: Ref,
    /// レイヤ
    pub layer: LayerId,
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
    /// 文字
    Text(Text<P>),
}

/// 文字。書式の解釈はまだしない
#[derive(Clone, Debug, PartialEq)]
pub struct Text<P> {
    /// 元の内容。制御文字や書式の符号も、そのまま持つ
    pub content: String,
    /// 宣言された高さ
    pub height: Height,
    /// 配置
    pub placement: Placement<P>,
    /// 回転（度、反時計回り）
    pub rotation: f64,
    /// 字形の幅の倍率
    pub width_factor: f64,
    /// 斜体の角度（度）
    pub oblique: f64,
    /// 左右に反転する
    pub backward: bool,
    /// 上下に反転する
    pub upside_down: bool,
    /// 元の文字スタイルの名前
    pub style: String,
}

/// 宣言された高さ。値・空間・意味を組にする
#[derive(Clone, Debug, PartialEq)]
pub struct Height {
    /// 値と空間
    pub value: Length,
    /// 値が何の高さか
    pub meaning: HeightMeaning,
}

/// 宣言された高さの意味
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeightMeaning {
    /// 文字枠の高さ
    Frame,
    /// 大文字の高さ
    CapHeight,
    /// em
    Em,
    /// 形式の宣言値で、字形との関係が決まらないもの。形式と欄を指すキー
    Declared(&'static str),
}

/// 文字の配置
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement<P> {
    /// 1 点に、水平と垂直の位置合わせで置く
    Anchor {
        /// 基点
        point: P,
        /// 水平の位置合わせ
        horizontal: Horizontal,
        /// 垂直の位置合わせ
        vertical: Vertical,
    },
    /// 2 点の間に置く
    Between {
        /// 1 点目
        first: P,
        /// 2 点目
        second: P,
        /// 2 点の間への合わせ方
        fit: Fit,
    },
}

/// 水平の位置合わせ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Horizontal {
    /// 左
    Left,
    /// 中央
    Center,
    /// 右
    Right,
    /// 文字の外形の中央（DXF の TEXT の 72 = 4）
    Middle,
}

/// 垂直の位置合わせ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vertical {
    /// 基準線
    Baseline,
    /// 下
    Bottom,
    /// 中央
    Middle,
    /// 上
    Top,
}

/// 2 点の間への合わせ方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// 縦横の比を保ち、高さも変える
    Aligned,
    /// 高さを保ち、幅だけを変える
    Fit,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: SpaceId = SpaceId(0);
    const SHEET: SheetId = SheetId(0);

    fn at(space: SpaceId, x: f64, y: f64) -> ModelPoint {
        ModelPoint { space, x, y }
    }

    /// 対応表の E1: 実寸を X に 0.01 倍、Y に 0.02 倍して（20,30）mm へ移す
    fn e1() -> View {
        View {
            space: MODEL,
            sheet: SHEET,
            transform: Affine {
                m: [0.01, 0.0, 20.0, 0.0, 0.02, 30.0],
            },
            clip: Vec::new(),
            hidden_layers: Vec::new(),
            source: Ref::default(),
        }
    }

    #[test]
    fn e1_an_anisotropic_view_places_the_corners() {
        let view = e1();
        let place = |x: f64, y: f64| view.place(at(MODEL, x, y));
        let paper = |x: f64, y: f64| Some(PaperPoint { sheet: SHEET, x, y });
        assert_eq!(place(0.0, 0.0), paper(20.0, 30.0));
        assert_eq!(place(1000.0, 500.0), paper(30.0, 40.0));
        // 実寸の大きさは変えない
        assert_eq!(
            at(MODEL, 0.0, 0.0).distance(&at(MODEL, 1000.0, 0.0)),
            Some(ModelLength {
                space: MODEL,
                value: 1000.0
            })
        );
    }

    #[test]
    fn e2_an_anisotropic_view_converts_lengths_by_direction() {
        let view = e1();
        let frame = ModelLength {
            space: MODEL,
            value: 250.0,
        };
        // 縦の文字枠の高さは 5 mm、横なら 2.5 mm。1 つの平均の縮尺で換算しない
        assert_eq!(view.paper_length(frame, [0.0, 1.0]), Some(PaperLength(5.0)));
        assert_eq!(view.paper_length(frame, [2.0, 0.0]), Some(PaperLength(2.5)));
        assert_eq!(view.paper_length(frame, [0.0, 0.0]), None);
        assert_eq!(view.paper_length(frame, [f64::NAN, 1.0]), None);
    }

    #[test]
    fn spaces_and_sheets_are_not_mixed() {
        let other = SpaceId(1);
        let view = e1();
        assert_eq!(at(MODEL, 0.0, 0.0).distance(&at(other, 3.0, 4.0)), None);
        assert_eq!(view.place(at(other, 0.0, 0.0)), None);
        let length = ModelLength {
            space: other,
            value: 1.0,
        };
        assert_eq!(view.paper_length(length, [1.0, 0.0]), None);
        let on = |sheet: u32, x: f64| PaperPoint {
            sheet: SheetId(sheet),
            x,
            y: 0.0,
        };
        assert_eq!(on(0, 0.0).distance(&on(0, 3.0)), Some(PaperLength(3.0)));
        assert_eq!(on(0, 0.0).distance(&on(1, 3.0)), None);
    }
}
