//! 表と、そのセル・書式・値。

use zumen_base::Ref;

use crate::{Color, Horizontal, Inherit, Length, LineWeight, Span, SymbolKey, Vertical};

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
    /// 表のスタイルの、行の種類ごとの書式（決定 0151）。元の形式が表のスタイルを持たないか、読めなければ `None`
    pub formats: Option<Box<TableFormats>>,
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
    /// 型のある値と、その単位と表示の書式。元の形式が値を持たなければ `None`。表示する文字は `content` が持つ
    pub value: Option<CellValue>,
    /// 文字の回転（度、反時計回り）
    pub rotation: f64,
    /// セルが上書きする書式（決定 0151）。`None` の欄は、行の種類の書式（[`TableFormats`]）に従う
    pub format: CellFormat,
}

/// 表のスタイルの、行の種類ごとの書式（決定 0151）
#[derive(Clone, Debug, PartialEq)]
pub struct TableFormats {
    /// 先頭の行がタイトルの行か
    pub title: bool,
    /// タイトルの行の次（タイトルの行がなければ先頭）の行が見出しの行か
    pub header: bool,
    /// タイトルの行の書式
    pub title_format: CellFormat,
    /// 見出しの行の書式
    pub header_format: CellFormat,
    /// データの行の書式
    pub data_format: CellFormat,
}

/// セルの書式。`None` の欄は、元の形式が値を持たない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CellFormat {
    /// 元の文字スタイルの名前
    pub text_style: Option<String>,
    /// 文字の高さ
    pub text_height: Option<Length>,
    /// 文字の水平と垂直の位置合わせ
    pub alignment: Option<(Horizontal, Vertical)>,
    /// 文字の色
    pub text_color: Option<Inherit<Color>>,
    /// 背景を塗るか
    pub fill: Option<bool>,
    /// 背景の色
    pub fill_color: Option<Inherit<Color>>,
    /// 枠
    pub borders: Borders,
}

/// セルか行の枠。内側の枠は、表のスタイルの行の種類の書式だけが持つ
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Borders {
    /// 上
    pub top: Border,
    /// 右
    pub right: Border,
    /// 下
    pub bottom: Border,
    /// 左
    pub left: Border,
    /// 内側の横の枠
    pub horizontal: Border,
    /// 内側の縦の枠
    pub vertical: Border,
}

/// 枠の 1 辺。`None` の欄は、元の形式が値を持たない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Border {
    /// 色
    pub color: Option<Inherit<Color>>,
    /// 線幅
    pub weight: Option<Inherit<LineWeight>>,
    /// 表示するか
    pub visible: Option<bool>,
}

/// セルの型のある値（決定 0147）
#[derive(Clone, Debug, PartialEq)]
pub struct CellValue {
    /// 値
    pub data: CellData,
    /// 値の単位の種類
    pub unit: ValueUnit,
    /// 元の形式の表示の書式。翻訳しない。なければ `None`
    pub format: Option<String>,
}

/// セルの値
#[derive(Clone, Debug, PartialEq)]
pub enum CellData {
    /// 文字列。書式の符号を含む
    Text(String),
    /// 整数
    Integer(i64),
    /// 実数。長さ・面積・体積は、図面の単位のまま
    Real(f64),
    /// 日時
    Date(DateTime),
    /// 点。2 次元の点は Z を 0 とする
    Point([f64; 3]),
}

/// 日時。時差を持たない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    /// 年
    pub year: u16,
    /// 月（1〜12）
    pub month: u8,
    /// 日（1〜31）
    pub day: u8,
    /// 時（0〜23）
    pub hour: u8,
    /// 分
    pub minute: u8,
    /// 秒
    pub second: u8,
    /// ミリ秒
    pub millisecond: u16,
}

/// 値の単位の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueUnit {
    /// 単位なし
    None,
    /// 長さ
    Distance,
    /// 角度
    Angle,
    /// 面積
    Area,
    /// 体積
    Volume,
    /// 通貨
    Currency,
    /// 百分率
    Percentage,
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
