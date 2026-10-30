//! レイヤ・線種・文字スタイル・寸法スタイルの表の項目と、要素の色・線種・線幅・透過性の指定と継承。

use zumen_base::Ref;

use crate::{Element, Font, PaperLength, SymbolKey};

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
    /// 説明。ファイルが持たないか空なら `None`（決定 0157）
    pub description: Option<String>,
    /// 定義の出所。定義がなければ既定の値
    pub source: Ref,
}

/// レイヤの表示・印刷・編集の可否。どれも、ビューごとの指定（[`View::hidden_layers`](crate::View::hidden_layers)）より前の値
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

/// レイヤの色・線種・線幅・透過性。どれも、定義が値を持たなければ `None`
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerStyle {
    /// 色
    pub color: Option<Color>,
    /// 線種
    pub line_type: Option<LineTypeId>,
    /// 線幅
    pub line_weight: Option<LineWeight>,
    /// 透過性（決定 0157）。`None` は不透明
    pub transparency: Option<Transparency>,
}

/// 要素の色・線種・線幅の指定
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// 色
    pub color: Inherit<Color>,
    /// 線種
    pub line_type: Inherit<LineTypeId>,
    /// 要素の線種の倍率。文書全体の倍率（[`Drawing2D::line_type_scale`](crate::Drawing2D::line_type_scale)）と掛け合わせる
    pub line_type_scale: f64,
    /// 線幅。元の形式から読めていなければ `None`
    pub line_weight: Option<Inherit<LineWeight>>,
    /// 透過性（決定 0157）。透過性を持たない形式は、レイヤから継承する
    pub transparency: Inherit<Transparency>,
}

/// 継承を解くための、要素か配置の色・線種・線幅とレイヤ（[`Drawing2D::color_placed`](crate::Drawing2D::color_placed)）
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

/// 透過性。AutoCAD の不透明度の値（DXF の 440 の下位のバイト）で持ち、0 は透明、255 は不透明
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transparency {
    /// 不透明度
    pub alpha: u8,
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
    /// 列の要素に添える形と文字（決定 0150）。順序は元の形式の順
    pub marks: Vec<LineTypeMark>,
}

/// 線種の列の要素に添える形か文字（DXF の複合線種）。長さとずれは、列と同じ空間の長さ
#[derive(Clone, Debug, PartialEq)]
pub struct LineTypeMark {
    /// 添える列の要素の番号（[`LineTypeDefinition::pattern`] の添字）。その要素の始まりに置く
    pub index: usize,
    /// 形か文字
    pub content: MarkContent,
    /// 倍率。文字なら、文字スタイルの高さへの倍率（高さが 0 の文字スタイルなら、高さそのもの）
    pub scale: f64,
    /// 回転（度、反時計回り）
    pub rotation: f64,
    /// 回転の基準
    pub rotation_basis: MarkRotation,
    /// 置く位置のずれ。線の向きを x とする
    pub offset: [f64; 2],
}

/// 線種に添える中身
#[derive(Clone, Debug, PartialEq)]
pub enum MarkContent {
    /// 形のファイルの形
    Shape {
        /// 形のファイル。元の形式が指す文字スタイルがなければ `None`
        font: Option<Font>,
        /// 形の番号
        number: u16,
    },
    /// 文字
    Text {
        /// 文字列
        content: String,
        /// 元の文字スタイルの名前。元の形式が指す文字スタイルがなければ `None`
        style: Option<String>,
    },
}

/// 線種に添える形と文字の回転の基準
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkRotation {
    /// 線の向きから測る
    Relative,
    /// 文書の x 軸から測る
    Absolute,
    /// 線の向きから測り、文字が逆さに見えるなら半回転する
    Upright,
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
    /// 文書の既定の線幅（[`Drawing2D::default_line_weight`](crate::Drawing2D::default_line_weight)）
    Default,
}

impl Style {
    /// 色・線種・線幅・透過性のすべてを、要素を置いた配置から継承する指定。線種の倍率は 1
    pub const BLOCK: Self = Self {
        color: Inherit::Block,
        line_type: Inherit::Block,
        line_type_scale: 1.0,
        line_weight: Some(Inherit::Block),
        transparency: Inherit::Block,
    };
}

/// 文字スタイル（決定 0158）。文字・書式つきの文字・表のセルの書式・線種の文字は、今は名前で指す。
/// フォントと縦書きは文字スタイルから引き、高さ・幅の倍率・傾き・裏返し・上下反転は要素の値を使う
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    /// 元の名前。翻訳しない
    pub name: String,
    /// 定義。要素が名前で指すだけで、ファイルに定義がなければ `None`
    pub definition: Option<TextStyleDefinition>,
    /// 定義の出所。定義がなければ既定の値
    pub source: Ref,
}

/// 文字スタイルの定義。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値
#[derive(Clone, Debug, PartialEq)]
pub struct TextStyleDefinition {
    /// 主のフォント。ファイルが決めていなければ `None`
    pub font: Option<Font>,
    /// 大きいフォント（漢字などの形のファイル）。なければ `None`
    pub big_font: Option<Font>,
    /// 決まった高さ。文書の単位。決めなければ `None`
    pub height: Option<f64>,
    /// 幅の倍率
    pub width: f64,
    /// 傾き（度）
    pub oblique: f64,
    /// 左右を裏返す
    pub backward: bool,
    /// 上下を反転する
    pub upside_down: bool,
    /// 縦書き
    pub vertical: bool,
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
