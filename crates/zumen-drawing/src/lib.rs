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
//! 色・線種・線幅は、要素ごとに指定値か継承元（[`Inherit`]）で持ち、元の符号（[`ColorCode`]）を残す。
//! 符号から RGB への表はプロファイル（#9）に置くので、ここでは引かない。レイヤからの継承は
//! [`Drawing2D::color_of`] などで解き、解けない継承は継承のまま返す。記号の定義の中の要素の継承は、
//! 配置の並びを渡して [`Drawing2D::color_placed`] などで解く（決定 0143）。
//!
//! 要素は、いまは線分・円・曲線・折れ線・文字・寸法・引出線・記号・ハッチング・表・下敷きだけ。寸法は、種類と定義点・保存された測定値・
//! 文字の上書きを別に持つ（[`Dimension`]）。引出線は経路を持ち、注記の要素を出所で指す（[`Leader`]）。
//! 寸法と引出線の端部記号は、図面の寸法スタイルの表（[`DimensionStyle`]）に置く。記号は、定義を名前空間付きの
//! キー（[`SymbolKey`]）で指し、配置と属性を持つ（[`Symbol`]）。定義の中身は、定義ごとの空間に置く（[`SymbolDefinition`]）。ハッチングは、境界の輪と、塗り・パターン・
//! グラデーションの区別を持つ（[`Hatch`]）。表は、行と列の大きさとセルの結合・内容を持つ（[`Table`]）。下敷きは、資源を参照名で指し、置き方と切り抜きを持つ（[`Underlay`]）。
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
    /// 線種。順序は元の形式の定義の順
    pub line_types: Vec<LineType>,
    /// 文書全体の線種の倍率。元の形式が決めていなければ `None`
    pub line_type_scale: Option<f64>,
    /// [`LineWeight::Default`] の線幅。元の形式が決めていなければ `None`
    pub default_line_weight: Option<PaperLength>,
    /// 寸法スタイル。順序は元の形式の定義の順
    pub dimension_styles: Vec<DimensionStyle>,
    /// 記号の定義。順序は元の形式の定義の順
    pub symbols: Vec<SymbolDefinition>,
    /// 形式固有の情報。順序は変換した順
    pub extensions: Vec<Extension>,
    /// 表題欄。順序は元の形式の順
    pub title_blocks: Vec<TitleBlock>,
}

impl Drawing2D {
    /// ID のレイヤ
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    /// ID の線種
    pub fn line_type(&self, id: LineTypeId) -> Option<&LineType> {
        self.line_types.iter().find(|line_type| line_type.id == id)
    }

    /// 名前の寸法スタイル。名前は大文字と小文字も含めて比べ、同じ名前があれば先のもの
    pub fn dimension_style(&self, name: &str) -> Option<&DimensionStyle> {
        self.dimension_styles
            .iter()
            .find(|style| style.name == name)
    }

    /// キーの記号の定義。名前は大文字と小文字も含めて比べ、同じキーがあれば先のもの
    pub fn symbol(&self, key: &SymbolKey) -> Option<&SymbolDefinition> {
        self.symbols.iter().find(|symbol| symbol.key == *key)
    }

    /// 付く先の、いまの値。図面と、実寸の空間・記号の定義の空間・用紙の要素を探す。なければ `None`
    pub fn basis(&self, target: &Target) -> Option<Basis> {
        match *target {
            Target::Document => Some(Basis::Document),
            Target::Model { space, index } => self
                .spaces
                .iter()
                .chain(self.symbols.iter().map(|symbol| &symbol.space))
                .find(|candidate| candidate.id == space)?
                .elements
                .get(index)
                .cloned()
                .map(Basis::Model),
            Target::Paper { sheet, index } => self
                .sheets
                .iter()
                .find(|candidate| candidate.id == sheet)?
                .elements
                .get(index)
                .cloned()
                .map(Basis::Paper),
        }
    }

    /// 付く先が読んだときのままの、形式固有の情報。元の系統へ書き戻せるのは、これだけ
    pub fn current_extensions(&self) -> impl Iterator<Item = &Extension> {
        self.extensions
            .iter()
            .filter(|extension| extension.is_current(self))
    }

    /// 付く先が変わったか、なくなった形式固有の情報。書き戻さず、更新するか、損失として診断する
    pub fn stale_extensions(&self) -> impl Iterator<Item = &Extension> {
        self.extensions
            .iter()
            .filter(|extension| !extension.is_current(self))
    }

    /// 要素の色。レイヤからの継承を解く。ブロックからの継承と、レイヤが値を持たないものは解かない
    pub fn color_of<P>(&self, element: &Element<P>) -> Inherit<Color> {
        self.through_layer(element.style.color, element.layer, |style| style.color)
    }

    /// 要素の線種。レイヤからの継承を解く。ブロックからの継承と、レイヤが値を持たないものは解かない
    pub fn line_type_of<P>(&self, element: &Element<P>) -> Inherit<LineTypeId> {
        self.through_layer(element.style.line_type, element.layer, |style| {
            style.line_type
        })
    }

    /// 要素の線幅。レイヤからの継承を解く。ブロックからの継承と、レイヤが値を持たないものは解かない。
    /// 元の形式から読めていなければ `None`
    pub fn line_weight_of<P>(&self, element: &Element<P>) -> Option<Inherit<LineWeight>> {
        let own = element.style.line_weight?;
        Some(self.through_layer(own, element.layer, |style| style.line_weight))
    }

    /// 記号の定義の中の要素の色。`chain` は要素と、それを置いた配置を内側から外側へ並べたもの。
    /// ブロックからの継承を配置で、レイヤからの継承をレイヤで解く（決定 0143）。並びが尽きても
    /// ブロックから継承するなら `Inherit::Block`、レイヤが値を持たなければ `Inherit::Layer` のまま
    pub fn color_placed(&self, chain: &[Inheritor<'_>]) -> Inherit<Color> {
        self.placed(chain, |style| Some(style.color), |style| style.color)
            .unwrap_or(Inherit::Block)
    }

    /// 記号の定義の中の要素の線種。解き方は [`Self::color_placed`] と同じ
    pub fn line_type_placed(&self, chain: &[Inheritor<'_>]) -> Inherit<LineTypeId> {
        self.placed(
            chain,
            |style| Some(style.line_type),
            |style| style.line_type,
        )
        .unwrap_or(Inherit::Block)
    }

    /// 記号の定義の中の要素の線幅。解き方は [`Self::color_placed`] と同じ。継承をたどる途中で、
    /// 元の形式から読めていない線幅に当たれば `None`
    pub fn line_weight_placed(&self, chain: &[Inheritor<'_>]) -> Option<Inherit<LineWeight>> {
        self.placed(chain, |style| style.line_weight, |style| style.line_weight)
    }

    fn placed<T>(
        &self,
        chain: &[Inheritor<'_>],
        own: impl Fn(&Style) -> Option<Inherit<T>>,
        get: impl Fn(&LayerStyle) -> Option<T> + Copy,
    ) -> Option<Inherit<T>> {
        for (at, placement) in chain.iter().enumerate() {
            match own(placement.style)? {
                Inherit::Block => continue,
                Inherit::Layer => {
                    // 配置のレイヤに従うレイヤなら、外側の配置のレイヤで解く
                    let follows = |p: &&Inheritor<'_>| {
                        self.layer(p.layer)
                            .is_some_and(|layer| layer.follows_placement)
                    };
                    let rest = chain.get(at..).unwrap_or_default();
                    let layer = rest
                        .iter()
                        .find(|p| !follows(p))
                        .or(rest.last())
                        .map_or(placement.layer, |p| p.layer);
                    return Some(self.through_layer(Inherit::Layer, layer, get));
                }
                own => return Some(own),
            }
        }
        Some(Inherit::Block)
    }

    fn through_layer<T>(
        &self,
        own: Inherit<T>,
        layer: LayerId,
        get: impl Fn(&LayerStyle) -> Option<T>,
    ) -> Inherit<T> {
        match own {
            Inherit::Layer => self
                .layer(layer)
                .and_then(|layer| layer.style.as_ref())
                .and_then(get)
                .map_or(Inherit::Layer, Inherit::Own),
            other => other,
        }
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
    /// 色・線種・線幅。ファイルに定義がなければ `None`
    pub style: Option<LayerStyle>,
    /// 記号の定義の中の要素がこのレイヤにあれば、レイヤからの継承を配置のレイヤで解く（決定 0143）
    pub follows_placement: bool,
    /// 定義の出所。定義がなければ既定の値
    pub source: Ref,
}

/// レイヤの表示・印刷・編集の可否。どれも、ビューごとの指定（[`View::hidden_layers`]）より前の値
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerState {
    /// 表示する。凍結した（`frozen`）レイヤは表示しない
    pub visible: bool,
    /// 印刷する
    pub printable: bool,
    /// 編集できる
    pub editable: bool,
    /// 凍結する。表示しないことに加え、このレイヤに置いた記号は、中の要素がほかのレイヤにあっても表示しない
    /// （DXF・DWG の凍結。決定 0141）。凍結を持たない形式は偽
    pub frozen: bool,
}

/// レイヤの色・線種・線幅。どれも、定義が値を持たなければ `None`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerStyle {
    /// 色
    pub color: Option<Color>,
    /// 線種
    pub line_type: Option<LineTypeId>,
    /// 線幅
    pub line_weight: Option<LineWeight>,
}

/// 要素の色・線種・線幅の指定
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// 色
    pub color: Inherit<Color>,
    /// 線種
    pub line_type: Inherit<LineTypeId>,
    /// 要素の線種の倍率。文書全体の倍率（[`Drawing2D::line_type_scale`]）と掛け合わせる
    pub line_type_scale: f64,
    /// 線幅。元の形式から読めていなければ `None`
    pub line_weight: Option<Inherit<LineWeight>>,
}

/// 継承を解くための、要素か配置の色・線種・線幅とレイヤ（[`Drawing2D::color_placed`]）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inheritor<'a> {
    /// 色・線種・線幅
    pub style: &'a Style,
    /// レイヤ
    pub layer: LayerId,
}

impl<'a, P> From<&'a Element<P>> for Inheritor<'a> {
    fn from(element: &'a Element<P>) -> Self {
        Inheritor {
            style: &element.style,
            layer: element.layer,
        }
    }
}

/// 指定値か、継承元
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inherit<T> {
    /// レイヤから継承する
    Layer,
    /// 要素を置いた部品の配置から継承する
    Block,
    /// 要素が持つ値
    Own(T),
}

/// 色。元の符号と RGB の少なくとも一方を持つ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    /// 元の符号。ファイルが RGB だけを持てば `None`
    pub code: Option<ColorCode>,
    /// ファイルが持つ RGB。符号表から引いた値は置かない
    pub rgb: Option<[u8; 3]>,
}

/// 色の元の符号。系統の違う符号を混ぜない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorCode {
    /// 系統
    pub system: ColorSystem,
    /// 系統の中の値
    pub value: u32,
}

/// 色の符号の系統
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSystem {
    /// AutoCAD の色の番号（ACI、DXF の 62）。1〜255
    Aci,
    /// SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの（決定 0121）
    Sxf,
    /// Jw_cad のペンの番号。1〜9 は図面ごとの基本色、100〜356 は SXF の拡張色（決定 0134）
    Jww,
}

/// 線種の ID。同じ図面の中で重ならない
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineTypeId(pub u32);

/// 線種。同じ形でも、名前と定義が別なら別の線種にする
#[derive(Clone, Debug, PartialEq)]
pub struct LineType {
    /// ID
    pub id: LineTypeId,
    /// 元の名前。翻訳しない
    pub name: String,
    /// 定義。要素が名前で指すだけで、ファイルに定義がなければ `None`
    pub definition: Option<LineTypeDefinition>,
    /// 定義の出所。定義がなければ既定の値
    pub source: Ref,
}

/// 線種の定義
#[derive(Clone, Debug, PartialEq)]
pub struct LineTypeDefinition {
    /// 元の説明
    pub description: String,
    /// 線と間の長さの列。正は線、負は間、0 は点
    pub pattern: Vec<f64>,
    /// 列の長さの空間
    pub basis: PatternBasis,
}

/// 線種の列の長さの空間
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatternBasis {
    /// 要素の空間の、文書の単位。要素と文書の倍率を掛ける（DXF の LTYPE）
    Element,
    /// 用紙の mm
    Paper,
}

/// 線幅
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineWeight {
    /// 用紙の上の幅
    Width(PaperLength),
    /// 文書の既定の線幅（[`Drawing2D::default_line_weight`]）
    Default,
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

/// 下敷きの切り抜き。頂点は、元の形式の下敷きの座標のまま持つ
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// 境界の頂点。2 つなら、向かい合う角の長方形
    pub vertices: Vec<[f64; 2]>,
    /// 境界の内側を隠す
    pub inverted: bool,
}

/// 表（決定 0113）。行と列の大きさとセルを持つ。表を線と文字で描いた記号の定義は、表示のための構成要素として
/// 指し、表の意味はセルが持つ
#[derive(Clone, Debug, PartialEq)]
pub struct Table<P> {
    /// 挿入点
    pub position: P,
    /// 行の向き（横の向きのベクトル）
    pub direction: [f64; 2],
    /// 行の高さ。上の行から
    pub row_heights: Vec<Length>,
    /// 列の幅。左の列から
    pub column_widths: Vec<Length>,
    /// セル。上の行から、行ごとに左の列から並べる。数は行の数と列の数の積
    pub cells: Vec<Cell>,
    /// 元の表のスタイル
    pub style: Option<Ref>,
    /// 表を線と文字で描いた記号の定義
    pub drawn: Option<SymbolKey>,
}

/// 表のセル
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    /// 内容
    pub content: CellContent,
    /// 結合して広げる列と行の数。結合しなければ `[1, 1]`
    pub span: [u32; 2],
    /// ほかのセルに結合されて覆われる
    pub covered: bool,
}

/// セルの内容
#[derive(Clone, Debug, PartialEq)]
pub enum CellContent {
    /// 空
    Empty,
    /// 書式の符号を含む元の文字列と、その区間
    Text {
        /// 元の内容
        content: String,
        /// 書式の区間
        spans: Vec<Span>,
    },
    /// 記号の定義
    Symbol(SymbolKey),
}

/// 引出線つきの注記。枝と注記を 1 つの要素に持つ。注記の記号は、バルーンなどの囲みを表す（決定 0110）
#[derive(Clone, Debug, PartialEq)]
pub struct Callout<P> {
    /// 枝。入力の順
    pub branches: Vec<Branch<P>>,
    /// 注記
    pub content: CalloutContent<P>,
    /// 線の引き方
    pub path: PathKind,
    /// 線の端部記号
    pub arrowhead: Arrowhead,
    /// 端部記号の大きさ
    pub arrow_size: Length,
    /// 線を注記に、水平の線（ランディング）でつなぐ
    pub landing: bool,
    /// 大きさに掛ける倍率
    pub scale: f64,
    /// 元のスタイルの出所。元の形式が指さなければ `None`
    pub style: Option<Ref>,
}

/// 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる
#[derive(Clone, Debug, PartialEq)]
pub struct Branch<P> {
    /// 線ごとの頂点。矢印の側から並べ、集まる点（`end`）は含めない
    pub lines: Vec<Vec<P>>,
    /// 線が集まる点。元の形式が持たなければ `None`
    pub end: Option<P>,
    /// 集まる点から注記への折れ曲がり。なければ `None`
    pub dogleg: Option<Dogleg>,
}

/// 枝の折れ曲がり
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dogleg {
    /// 向き。長さは意味を持たない
    pub direction: [f64; 2],
    /// 長さ
    pub length: Length,
}

/// 引出線の引き方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    /// 折れ線
    Straight,
    /// 頂点を通る曲線
    Spline,
    /// 線を描かない
    Hidden,
}

/// 引出線つきの注記の中身
#[derive(Clone, Debug, PartialEq)]
pub enum CalloutContent<P> {
    /// 注記を持たない
    None,
    /// 書式つきの文字
    Text(Box<FormattedText<P>>),
    /// 記号と、その属性の値
    Symbol {
        /// 記号
        symbol: Box<Symbol<P>>,
        /// 属性の値。入力の順
        values: Vec<AttributeValue>,
    },
}

/// 引出線つきの注記の記号の、属性の値。属性の定義は出所で指す
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributeValue {
    /// 属性の定義の出所
    pub definition: Ref,
    /// 値。元の内容のまま持つ
    pub text: String,
}

/// 寸法スタイルのうち、端部記号とその大きさ。寸法と引出線は名前で指す
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionStyle {
    /// 元の名前。翻訳しない
    pub name: String,
    /// 寸法線の 1 本目と 2 本目の補助線の側の端部記号
    pub ends: [Arrowhead; 2],
    /// 引出線の端部記号
    pub leader: Arrowhead,
    /// 端部記号の大きさ。要素の空間の文書の単位で、`scale` を掛ける前の値
    pub size: f64,
    /// 寸法線の端に矢印の代わりに描く斜線の大きさ。斜線を描かなければ `None`。単位は `size` と同じ
    pub tick: Option<f64>,
    /// 大きさに掛ける倍率。0 は、ビューの縮尺から決めることを表す（DXF の DIMSCALE）
    pub scale: f64,
    /// 定義の出所
    pub source: Ref,
}

/// 端部記号
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arrowhead {
    /// 元の形式の既定の矢印（DXF では塗りつぶした閉じた矢印）
    Default,
    /// 記号の定義。形式が名前で決めた矢印（DXF の `_DOT` など）も、この形で持つ
    Symbol(SymbolKey),
}

/// 記号の定義を指すキー。名前空間ごとに名前を比べる
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolKey {
    /// 名前空間（`dxf.block` など）
    pub namespace: &'static str,
    /// 名前空間の中の元の名前。翻訳しない
    pub name: String,
}

/// 記号の配置。記号の定義の点を、尺度・回転・移動の順に写す
#[derive(Clone, Debug, PartialEq)]
pub struct Symbol<P> {
    /// 定義
    pub key: SymbolKey,
    /// 定義の基点を置く点
    pub position: P,
    /// X と Y の尺度。負なら反転する
    pub scale: [f64; 2],
    /// 回転（度、反時計回り）
    pub rotation: f64,
    /// 格子に並べて置くなら、その列と行。1 つだけ置くなら `None`
    pub grid: Option<Grid>,
    /// 属性。順序は入力の順
    pub attributes: Vec<Attribute<P>>,
}

/// 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// 列の数
    pub columns: u32,
    /// 行の数
    pub rows: u32,
    /// 列の間隔
    pub column_spacing: Length,
    /// 行の間隔
    pub row_spacing: Length,
}

/// 記号の属性。値は文字として置く
#[derive(Clone, Debug, PartialEq)]
pub struct Attribute<P> {
    /// 元のタグ。翻訳しない
    pub tag: String,
    /// 値と、その配置と書式
    pub text: Text<P>,
    /// 複数行の属性の、書式つきの値。1 行の値（`text`）とは別に元の形式が持つ
    pub formatted: Option<FormattedText<P>>,
    /// 表示しない
    pub hidden: bool,
    /// レイヤ
    pub layer: LayerId,
    /// 色・線種・線幅
    pub style: Style,
    /// 出所
    pub source: Ref,
}

/// 記号の定義（決定 0112）。中身は、図面の実寸の空間とは別の、定義ごとの空間に置く。配置（[`Symbol`]）は、
/// この空間の点を、基点から配置の点へ写す
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolDefinition {
    /// 配置が指すキー
    pub key: SymbolKey,
    /// 中身を置く空間。ID は、図面の実寸の空間とも、ほかの定義とも重ならない
    pub space: ModelSpace,
    /// 配置の点に重ねる基点。元の形式から分からなければ `None`
    pub base: Option<ModelPoint>,
    /// 属性の定義。順序は入力の順
    pub attributes: Vec<AttributeDefinition>,
    /// 名前のない定義（寸法やハッチングが作ったものなど）
    pub anonymous: bool,
    /// 外部参照なら、元のパス。外部のファイルは読まない
    pub external: Option<String>,
    /// 出所
    pub source: Ref,
}

/// 記号の属性の定義。配置の属性の値は、ここから作る
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeDefinition {
    /// タグ・既定の値と、その配置と書式・表示しない・レイヤ・色・出所
    pub attribute: Attribute<ModelPoint>,
    /// 値を尋ねる文。翻訳しない
    pub prompt: String,
    /// 値が決まっていて、配置ごとに変えない
    pub constant: bool,
    /// 値を確かめる
    pub verify: bool,
    /// 尋ねずに既定の値を使う
    pub preset: bool,
}

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

/// パターンの 1 組の平行線。長さは、要素の空間の文書の単位（[`PatternBasis::Element`]）
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

/// 1 行の文字。書式の符号は解釈せず、内容のまま持つ
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

/// 書式つきの文字。基点と枠への位置合わせで置き、内容を書式の区間に分ける。元の内容も持ち、区間は
/// 解釈できた書式と、解釈できない符号を分ける（決定 0109）
#[derive(Clone, Debug, PartialEq)]
pub struct FormattedText<P> {
    /// 元の内容。書式の符号も、そのまま持つ
    pub content: String,
    /// 内容を解釈した区間。入力の順
    pub spans: Vec<Span>,
    /// 宣言された高さ
    pub height: Height,
    /// 基点
    pub point: P,
    /// 枠の水平の位置合わせ（左・中央・右）
    pub horizontal: Horizontal,
    /// 枠の垂直の位置合わせ（上・中央・下）
    pub vertical: Vertical,
    /// 向き
    pub rotation: Rotation,
    /// 枠の幅。なければ折り返さない
    pub width: Option<Length>,
    /// 宣言された枠の高さ。なければ内容に合わせる
    pub frame_height: Option<Length>,
    /// 書き進める向き
    pub flow: Flow,
    /// 行送り
    pub line_spacing: LineSpacing,
    /// 背景と枠の線。なければ描かない
    pub background: Option<Background>,
    /// 元の文字スタイルの名前
    pub style: String,
}

/// 向き。元の形式が持つ形のまま持つ
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rotation {
    /// 角度（度、反時計回り）
    Angle(f64),
    /// 文字の X 軸の向き。長さは意味を持たない
    Direction([f64; 2]),
}

/// 書き進める向き
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// 左から右へ（横書き）
    LeftToRight,
    /// 上から下へ（縦書き）
    TopToBottom,
    /// 文字スタイルに従う
    ByStyle,
}

/// 行送り。倍率は文字の高さから決まる既定の行送りへの倍率
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSpacing {
    /// 倍率
    pub factor: f64,
    /// 文字が大きい行も同じ行送りにする。偽なら、大きい行は広げる
    pub exact: bool,
}

/// 背景と枠の線
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Background {
    /// 塗り。なければ塗らない
    pub fill: Option<BackgroundFill>,
    /// 枠の線を描く
    pub frame: bool,
    /// 文字の範囲に対する、塗る範囲の倍率
    pub scale: f64,
    /// 元の透明度の値
    pub transparency: Option<u32>,
}

/// 背景の塗り
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BackgroundFill {
    /// 色で塗る
    Color(Color),
    /// 表示の背景の色で塗る
    Screen,
}

/// 書式つきの文字の区間
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    /// 中身
    pub piece: Piece,
    /// 区間に効いている書式。文字全体の値から変えたものだけを持つ
    pub format: Format,
}

/// 区間の中身
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    /// 文字列
    Text(String),
    /// 段落の区切り
    Paragraph,
    /// 段の区切り
    Column,
    /// 寸法値の文字で、測定値を書く所（DXF の `<>`）
    Measurement,
    /// 寸法値の文字で、ここから後を寸法線の下に書く（DXF の `\X`）
    Below,
    /// 上下に重ねた文字（分数・公差）
    Stack {
        /// 上の文字列
        upper: String,
        /// 下の文字列
        lower: String,
        /// 重ね方
        kind: StackKind,
    },
    /// 解釈できない書式の符号。元の符号のまま持つ
    Unparsed(String),
}

/// 上下の重ね方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackKind {
    /// 横線で分ける
    Horizontal,
    /// 斜線で分ける
    Diagonal,
    /// 線を引かずに重ねる（公差）
    Tolerance,
}

/// 区間の書式。`None` と偽は、文字全体の値のまま
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Format {
    /// 字の高さ
    pub height: Option<Scaled<Length>>,
    /// 字形の幅の倍率
    pub width_factor: Option<Scaled<f64>>,
    /// 字の間隔の倍率
    pub tracking: Option<Scaled<f64>>,
    /// 斜体の角度（度）
    pub oblique: Option<f64>,
    /// フォント
    pub font: Option<Font>,
    /// 色
    pub color: Option<Inherit<Color>>,
    /// 行の中の垂直の位置合わせ
    pub align: Option<Vertical>,
    /// 下線
    pub underline: bool,
    /// 上線
    pub overline: bool,
    /// 取り消し線
    pub strike: bool,
}

/// 値か、文字全体の値への倍率
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scaled<T> {
    /// 値
    Value(T),
    /// 文字全体の値への倍率
    Factor(f64),
}

/// フォントの指定。取得と字形の評価は別にする
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Font {
    /// 元の名前。翻訳しない
    pub name: String,
    /// 種類
    pub kind: FontKind,
}

/// フォントの種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontKind {
    /// 形のファイル（SHX など）
    Shape,
    /// 名前で指すフォント。値は元の指定
    Named {
        /// 太字
        bold: bool,
        /// 斜体
        italic: bool,
        /// 文字集合の番号
        charset: Option<u32>,
        /// ピッチと系統の番号
        pitch: Option<u32>,
    },
}

/// 表題欄（決定 0124）。元の形式が表題欄として持つ項目を、表示する要素と分けて持つ。罫線と文字から
/// 推定して作らない
#[derive(Clone, Debug, PartialEq)]
pub struct TitleBlock {
    /// 項目。順序は元の形式の順
    pub items: Vec<TitleItem>,
    /// 出所
    pub source: Ref,
}

/// 表題欄の項目
#[derive(Clone, Debug, PartialEq)]
pub struct TitleItem {
    /// 形式によらない項目のキー。共通の項目に当たらなければ `None`
    pub key: Option<&'static str>,
    /// 元の項目名。翻訳しない
    pub name: String,
    /// 値
    pub value: String,
    /// 値を表示する要素。元の形式が対応を持たなければ空
    pub display: Vec<Target>,
}

/// 形式固有の情報（決定 0022・0115）。共通の要素にできない残りを、系統・版・型付きの値・出所と、
/// 読んだときの付く先の値とともに持つ。付く先の値が変われば失効したものとし、古い値を書き戻さない
#[derive(Clone, Debug, PartialEq)]
pub struct Extension {
    /// 名前空間（`dxf.xdata` など）。値の型の契約を決める
    pub namespace: &'static str,
    /// 名前空間の中の元の名前（XDATA の登録アプリケーション名など）。翻訳しない
    pub name: String,
    /// 元の形式の版（DXF の `$ACADVER` など）。分からなければ `None`
    pub version: Option<String>,
    /// 名前空間の値の型の契約の版
    pub schema: u32,
    /// 付く先
    pub target: Target,
    /// 読んだときの付く先の値
    pub basis: Basis,
    /// 型付きの値。順序は元の形式の順
    pub fields: Vec<Field>,
    /// 元の形式のレコード
    pub source: Ref,
}

impl Extension {
    /// 付く先が読んだときのままか
    pub fn is_current(&self, drawing: &Drawing2D) -> bool {
        drawing.basis(&self.target).as_ref() == Some(&self.basis)
    }
}

/// 形式固有の情報が付く先
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// 図面全体
    Document,
    /// 実寸の空間か記号の定義の空間の、`index` 番目の要素
    Model {
        /// 空間
        space: SpaceId,
        /// 要素の添字
        index: usize,
    },
    /// 用紙の、`index` 番目の要素
    Paper {
        /// 用紙
        sheet: SheetId,
        /// 要素の添字
        index: usize,
    },
}

/// 形式固有の情報が依存する、付く先の値
#[derive(Clone, Debug, PartialEq)]
pub enum Basis {
    /// 図面全体。共通の欄には依存しない
    Document,
    /// 実寸の要素の、レイヤ・スタイル・形と意味・出所
    Model(Element<ModelPoint>),
    /// 用紙の要素の、レイヤ・スタイル・形と意味・出所
    Paper(Element<PaperPoint>),
}

/// 形式固有の情報の 1 つの値
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// 名前空間の中の値の名前
    pub key: &'static str,
    /// 値
    pub value: FieldValue,
}

/// 形式固有の情報の値の型
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// 文字列
    Text(String),
    /// 整数
    Integer(i64),
    /// 実数
    Real(f64),
    /// 3 次元の点やベクトル。座標の空間は名前空間が決める
    Point([f64; 3]),
    /// 元の形式のオブジェクトを指す値
    Handle(u64),
    /// バイト列
    Bytes(Vec<u8>),
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

    fn aci(value: u32) -> Color {
        Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value,
            }),
            rgb: None,
        }
    }

    /// ID が位置のレイヤ。`follows` なら配置のレイヤに従う
    fn layers(colors: &[(Option<u32>, bool)]) -> Drawing2D {
        let layers = colors
            .iter()
            .enumerate()
            .map(|(id, &(color, follows))| Layer {
                id: LayerId(id as u32),
                name: id.to_string(),
                state: None,
                style: Some(LayerStyle {
                    color: color.map(aci),
                    line_type: None,
                    line_weight: Some(LineWeight::Default),
                }),
                follows_placement: follows,
                source: Ref::default(),
            })
            .collect();
        Drawing2D {
            layers,
            ..Drawing2D::default()
        }
    }

    fn style(color: Inherit<Color>) -> Style {
        Style {
            color,
            line_type: Inherit::Block,
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Block),
        }
    }

    #[test]
    fn byblock_is_resolved_through_the_placements() {
        // レイヤ 0 は配置のレイヤに従う。レイヤ 1 は色 1、レイヤ 2 は色 2、レイヤ 3 は色を持たない
        let drawing = layers(&[
            (Some(7), true),
            (Some(1), false),
            (Some(2), false),
            (None, false),
        ]);
        let block = style(Inherit::Block);
        let layer = Style {
            line_type: Inherit::Layer,
            line_weight: Some(Inherit::Layer),
            ..style(Inherit::Layer)
        };
        let own = style(Inherit::Own(aci(5)));
        // 配置の色
        let chain = [placed(&block, 1), placed(&own, 2)];
        assert_eq!(drawing.color_placed(&chain), Inherit::Own(aci(5)));
        // 内側の配置もブロックから継承するなら、外側の配置まで進む。そこでレイヤから継承する
        let chain = [placed(&block, 1), placed(&block, 1), placed(&layer, 2)];
        assert_eq!(drawing.color_placed(&chain), Inherit::Own(aci(2)));
        // 要素が値を持てば、配置を見ない
        let chain = [placed(&own, 1), placed(&layer, 2)];
        assert_eq!(drawing.color_placed(&chain), Inherit::Own(aci(5)));
        // 並びが尽きれば、ブロックからの継承のまま
        assert_eq!(drawing.color_placed(&[placed(&block, 1)]), Inherit::Block);
        assert_eq!(drawing.color_placed(&[]), Inherit::Block);
        // 配置のレイヤに従うレイヤの要素は、配置のレイヤで解く。入れ子でも外へ進む
        let chain = [placed(&layer, 0), placed(&own, 0), placed(&own, 2)];
        assert_eq!(drawing.color_placed(&chain), Inherit::Own(aci(2)));
        // 一番外まで従うレイヤなら、そのレイヤで解く
        assert_eq!(
            drawing.color_placed(&[placed(&layer, 0)]),
            Inherit::Own(aci(7))
        );
        // 従わないレイヤは、そのレイヤで解く。値がなければレイヤからの継承のまま
        let chain = [placed(&layer, 1), placed(&own, 2)];
        assert_eq!(drawing.color_placed(&chain), Inherit::Own(aci(1)));
        let chain = [placed(&layer, 3), placed(&own, 2)];
        assert_eq!(drawing.color_placed(&chain), Inherit::Layer);
        // 線種と線幅も同じに解く。読めていない線幅に当たれば `None`
        let chain = [placed(&block, 1), placed(&layer, 2)];
        assert_eq!(drawing.line_type_placed(&chain), Inherit::Layer);
        assert_eq!(
            drawing.line_weight_placed(&chain),
            Some(Inherit::Own(LineWeight::Default))
        );
        let unknown = Style {
            line_weight: None,
            ..block
        };
        assert_eq!(
            drawing.line_weight_placed(&[placed(&block, 1), placed(&unknown, 2)]),
            None
        );
        // 要素から作る
        let element = Element {
            source: Ref::default(),
            layer: LayerId(2),
            style: own,
            shape: Shape::Line {
                start: at(MODEL, 0.0, 0.0),
                end: at(MODEL, 0.0, 0.0),
            },
        };
        assert_eq!(
            drawing.color_placed(&[(&element).into()]),
            Inherit::Own(aci(5))
        );
    }

    fn placed(style: &Style, layer: u32) -> Inheritor<'_> {
        Inheritor {
            style,
            layer: LayerId(layer),
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
    fn e2_hatch_spacing_follows_the_view() {
        // 実寸で 45 度の向きに 100√2 離れた平行線は、E1 のビューでは向きの縮尺で用紙に移る
        let line = PatternLine {
            angle: 45.0,
            base: [0.0, 0.0],
            offset: [-100.0, 100.0],
            dashes: Vec::new(),
        };
        let [x, y] = line.offset;
        let spacing = ModelLength {
            space: MODEL,
            value: norm(x, y),
        };
        let paper = e1().paper_length(spacing, [x, y]).unwrap();
        // X に 0.01 倍、Y に 0.02 倍なので、(-1, 2) mm の長さになる
        assert!((paper.0 - norm(1.0, 2.0)).abs() < 1e-12);
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

/// 図面モデルへの変更の一覧（決定 0139）。要素を出所で指し、並べた順に当てる
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    /// 変更
    pub edits: Vec<Edit>,
}

/// 図面モデルの要素 1 つへの変更
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// 同じ出所を持つ実寸の空間の要素を、この要素で置き換える
    Replace(Box<Element<ModelPoint>>),
    /// この出所の要素を除く
    Remove(Ref),
}
