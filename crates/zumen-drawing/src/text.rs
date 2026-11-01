//! 文字と書式付きの文字、配置・書体。

use std::borrow::Cow;

use crate::{Color, Inherit, Length};

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
    Declared(Cow<'static, str>),
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
