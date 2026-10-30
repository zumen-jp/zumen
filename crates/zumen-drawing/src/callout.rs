//! 引出線つきの注記。

use zumen_base::Ref;

use crate::{Arrowhead, Color, FormattedText, Inherit, Length, Style, Symbol};

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
    /// 線（枝の線と折れ曲がり）の色・線種・線幅。`Inherit::Block` は、この注記の要素の値に従う（要素を
    /// 線の配置とみなす）。`Inherit::Layer` は、この注記の要素のレイヤの値に従う。線種の倍率は、要素の
    /// 線種の倍率と掛け合わせる
    pub line_style: Style,
    /// 注記の文字か記号の色。`Inherit::Block` は、この注記の要素の色に従う。記号のときは、記号の中の要素が
    /// 配置から継承する色を、この色で解く
    pub content_color: Inherit<Color>,
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
