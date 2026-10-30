//! 図面モデルの変更と、当てた結果。

use crate::{
    Affine, Color, DimensionStyle, Element, Hash, Inherit, Layer, LayerId, LineType, LineTypeId,
    LineWeight, ModelPoint, PaperPoint, Style, TextStyle,
};

/// 図面モデルへの変更の一覧（決定 0139）。要素を ID と読んだときのハッシュで指し（決定 0156）、並べた順に当てる
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    /// 変更
    pub edits: Vec<Edit>,
    /// 読んだときの文書のハッシュ。あれば、今の文書と同じときだけ当てる
    pub document: Option<Hash>,
}

/// 変更が指す、読んだときの要素。ID は `entities` が返す文字列（[`id`](crate::id)）、ハッシュは [`Hash::element`]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    /// 要素の ID
    pub id: String,
    /// 読んだときの要素のハッシュ
    pub hash: Hash,
}

/// 空間か用紙に置く要素
#[derive(Clone, Debug, PartialEq)]
pub enum Placed {
    /// 実寸の空間の要素
    Model(Element<ModelPoint>),
    /// 用紙の要素
    Paper(Element<PaperPoint>),
}

/// 図面モデルの要素 1 つへの変更
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// 指した要素の値（レイヤ・スタイル・形）を、この要素で置き換える。形の種類は変えない。要素の出所は使わない
    Replace {
        /// 置き換える要素
        target: Seen,
        /// 新しい値
        element: Box<Placed>,
    },
    /// 指した要素のレイヤ・色・線種・線種の倍率・線幅のうち、[`Restyle`] が持つものだけを変える。形は変えない
    Restyle {
        /// 変える要素
        target: Seen,
        /// 新しい値
        style: Restyle,
    },
    /// 指した要素を、この変換で動かす（[`transform::transform`](crate::transform::transform)）。AutoCAD の系統では、図形とともに動く XDATA と
    /// 記号の属性も同じ変換で直す
    Transform {
        /// 動かす要素
        target: Seen,
        /// 要素の空間（用紙）の点を写す変換
        transform: Affine,
    },
    /// 空間か用紙に、この要素を足す。要素の出所は使わない
    Add {
        /// 足す先の空間か用紙の ID（[`id::space`](crate::id::space)・[`id::sheet`](crate::id::sheet)）
        place: String,
        /// 足す要素。点は足す先の空間か用紙のもの
        element: Box<Placed>,
    },
    /// 指した要素を除く
    Remove {
        /// 除く要素
        target: Seen,
    },
    /// 指したレイヤの名前・状態・色・線種・線幅を、このレイヤのもので置き換える（決定 0162）。ID・出所・
    /// `follows_placement` は使わない。`target` の ID は [`id::layer`](crate::id::layer)、ハッシュは [`Hash::layer`]
    ReplaceLayer {
        /// 置き換えるレイヤ
        target: Seen,
        /// 新しい値。状態とスタイルを持つ
        layer: Box<Layer>,
    },
    /// レイヤを足す（決定 0162）。ID・出所・`follows_placement` は使わない。同じ変更の一覧の要素は、
    /// 足したレイヤを指せない
    AddLayer {
        /// 足すレイヤ。状態とスタイルを持つ
        layer: Box<Layer>,
    },
    /// 指したレイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤが指すレイヤと、0・Defpoints・
    /// 外部参照に依るレイヤは除けない
    RemoveLayer {
        /// 除くレイヤ
        target: Seen,
    },
    /// 指した文字スタイルの名前と定義を、この文字スタイルのもので置き換える（決定 0163）。出所は使わない。
    /// `target` の ID は [`id::text_style`](crate::id::text_style)、ハッシュは [`Hash::text_style`]
    ReplaceTextStyle {
        /// 置き換える文字スタイル
        target: Seen,
        /// 新しい値。定義を持つ
        style: Box<TextStyle>,
    },
    /// 文字スタイルを足す（決定 0163）。出所は使わない。同じ変更の一覧の要素は、足した文字スタイルを
    /// 指せない
    AddTextStyle {
        /// 足す文字スタイル。定義を持つ
        style: Box<TextStyle>,
    },
    /// 指した文字スタイルを除く（決定 0163）。要素・ほかのオブジェクト・今の文字スタイルが指すものと、
    /// Standard と外部参照に依るものは除けない
    RemoveTextStyle {
        /// 除く文字スタイル
        target: Seen,
    },
    /// 指した線種の名前と定義を、この線種のもので置き換える（決定 0164）。ID と出所は使わない。
    /// `target` の ID は [`id::line_type`](crate::id::line_type)、ハッシュは [`Hash::line_type`]
    ReplaceLineType {
        /// 置き換える線種
        target: Seen,
        /// 新しい値。定義を持つ
        line_type: Box<LineType>,
    },
    /// 線種を足す（決定 0164）。ID と出所は使わない。同じ変更の一覧の要素とレイヤは、足した線種を指せない
    AddLineType {
        /// 足す線種。定義を持つ
        line_type: Box<LineType>,
    },
    /// 指した線種を除く（決定 0164）。要素・レイヤ・ほかのオブジェクト・今の線種が指すものと、ByLayer・
    /// ByBlock・Continuous と外部参照に依るものは除けない
    RemoveLineType {
        /// 除く線種
        target: Seen,
    },
    /// 指した寸法スタイルの名前と定義（端部記号・その大きさ・斜線・倍率）を、この寸法スタイルのもので置き換える
    /// （決定 0165）。出所は使わない。`target` の ID は [`id::dimension_style`](crate::id::dimension_style)、ハッシュは
    /// [`Hash::dimension_style`]
    ReplaceDimensionStyle {
        /// 置き換える寸法スタイル
        target: Seen,
        /// 新しい値
        style: Box<DimensionStyle>,
    },
    /// 寸法スタイルを足す（決定 0165）。出所は使わない
    AddDimensionStyle {
        /// 足す寸法スタイル
        style: Box<DimensionStyle>,
    },
    /// 指した寸法スタイルを除く（決定 0165）。寸法・引出線・ほかのオブジェクト・今の寸法スタイルが指すものと、
    /// Standard と外部参照に依るものは除けない
    RemoveDimensionStyle {
        /// 除く寸法スタイル
        target: Seen,
    },
}

/// 要素のスタイルの変更（[`Edit::Restyle`]）。`None` の欄は変えない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Restyle {
    /// レイヤ
    pub layer: Option<LayerId>,
    /// 色
    pub color: Option<Inherit<Color>>,
    /// 線種
    pub line_type: Option<Inherit<LineTypeId>>,
    /// 要素の線種の倍率
    pub line_type_scale: Option<f64>,
    /// 線幅
    pub line_weight: Option<Inherit<LineWeight>>,
}

impl Edit {
    /// 変更の名前。診断の `edit` に書く
    pub fn name(&self) -> &'static str {
        match self {
            Self::Replace { .. } => "replace",
            Self::Restyle { .. } => "restyle",
            Self::Transform { .. } => "transform",
            Self::Add { .. } => "add",
            Self::Remove { .. } => "remove",
            Self::ReplaceLayer { .. } => "replace_layer",
            Self::AddLayer { .. } => "add_layer",
            Self::RemoveLayer { .. } => "remove_layer",
            Self::ReplaceTextStyle { .. } => "replace_text_style",
            Self::AddTextStyle { .. } => "add_text_style",
            Self::RemoveTextStyle { .. } => "remove_text_style",
            Self::ReplaceLineType { .. } => "replace_line_type",
            Self::AddLineType { .. } => "add_line_type",
            Self::RemoveLineType { .. } => "remove_line_type",
            Self::ReplaceDimensionStyle { .. } => "replace_dimension_style",
            Self::AddDimensionStyle { .. } => "add_dimension_style",
            Self::RemoveDimensionStyle { .. } => "remove_dimension_style",
        }
    }
}

impl Restyle {
    /// 元のレイヤとスタイルに、この変更を重ねたもの
    pub fn over(&self, layer: LayerId, style: &Style) -> (LayerId, Style) {
        let style = Style {
            color: self.color.unwrap_or(style.color),
            line_type: self.line_type.unwrap_or(style.line_type),
            line_type_scale: self.line_type_scale.unwrap_or(style.line_type_scale),
            line_weight: self.line_weight.or(style.line_weight),
            transparency: style.transparency,
        };
        (self.layer.unwrap_or(layer), style)
    }
}

/// 当てた変更 1 つの結果
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    /// 置き換えた要素か足した要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を
    /// 決められないときは `None`
    pub id: Option<String>,
    /// 書き戻した後の要素（レイヤ）のハッシュ。`id` と同じときにある
    pub hash: Option<Hash>,
}
