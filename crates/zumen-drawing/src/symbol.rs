//! 記号の定義と配置、属性。

use std::borrow::Cow;

use zumen_base::Ref;

use crate::{Clip, FormattedText, LayerId, Length, ModelPoint, ModelSpace, Style, Text};

/// 記号の定義を指すキー。名前空間ごとに名前を比べる
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SymbolKey {
    /// 名前空間（`dxf.block` など）
    pub namespace: Cow<'static, str>,
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
    /// 切り抜き（決定 0149）。頂点は、記号の定義の空間の座標で持つ。切り抜かなければ `None`
    pub clip: Option<Clip>,
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
