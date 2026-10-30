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
//! どの要素も、元の形式のレコードを指す出所（[`Ref`]）を持つ。要素の ID とハッシュは [`id`] と [`Hash`] で作る（決定 0156）。

use std::collections::BTreeSet;

use zumen_base::{Cx, Kind, Ref, Stop};

mod callout;
mod change;
mod dimension;
mod extension;
mod hash;
mod hatch;
pub mod id;
mod shape;
mod space;
mod style;
mod symbol;
mod table;
mod text;
pub mod transform;
mod underlay;
mod walk;

pub use callout::{AttributeValue, Branch, Callout, CalloutContent, Dogleg, PathKind};
pub use change::{Applied, Changes, Edit, Placed, Restyle, Seen};
pub use dimension::{Axis, Dimension, DimensionKind, DimensionText, Label, Leader, Measured};
pub use extension::{Basis, Extension, Field, FieldValue, Target};
pub use hash::{Canon, Encode, Hash};
pub use hatch::{
    Boundary, Fill, Gradient, Hatch, HatchLoop, Islands, Pattern, PatternLine, PatternOrigin,
};
pub use shape::{Edge, Element, KnotParameterization, Polyline, Shape, Spline, Vertex};
pub use space::{
    Affine, Length, ModelLength, ModelPoint, ModelSpace, PaperLength, PaperPoint, Sheet, SheetItem,
    Unit, View,
};
pub use style::{
    Arrowhead, Color, ColorCode, ColorSystem, DimensionStyle, Inherit, Inheritor, Layer, LayerId,
    LayerState, LayerStyle, LineType, LineTypeDefinition, LineTypeId, LineTypeMark, LineWeight,
    MarkContent, MarkRotation, PatternBasis, Style, TextStyle, TextStyleDefinition, Transparency,
};
pub use symbol::{Attribute, AttributeDefinition, Grid, Symbol, SymbolDefinition, SymbolKey};
pub use table::{
    Border, Borders, Cell, CellContent, CellData, CellFormat, CellValue, DateTime, Table,
    TableFormats, ValueUnit,
};
pub use text::{
    Background, BackgroundFill, Fit, Flow, Font, FontKind, Format, FormattedText, Height,
    HeightMeaning, Horizontal, LineSpacing, Piece, Placement, Rotation, Scaled, Span, StackKind,
    Text, Vertical,
};
pub use underlay::{Clip, Resource, Underlay, UnderlayFrame, UnderlayKind};
pub use walk::{Clipping, Item, Origin, Step, Visit};

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
    /// [`LineWeight::Default`] の線幅と、起点まで配置から継承する線幅。元の形式が決めていなければ `None`
    pub default_line_weight: Option<PaperLength>,
    /// 起点まで配置から継承する色（決定 0159）。元の形式が決めていなければ `None`
    pub default_color: Option<Color>,
    /// 起点まで配置から継承する線種（決定 0159）。元の形式が決めていなければ `None`
    pub default_line_type: Option<LineTypeId>,
    /// 寸法スタイル。順序は元の形式の定義の順
    pub dimension_styles: Vec<DimensionStyle>,
    /// 文字スタイル。順序は元の形式の定義の順。要素が名前で指すだけで、定義がないものは後ろに足す（決定 0158）
    pub text_styles: Vec<TextStyle>,
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

    /// 名前の文字スタイル。名前は大文字と小文字を区別せずに比べ、同じ名前があれば先のもの
    pub fn text_style(&self, name: &str) -> Option<&TextStyle> {
        self.text_styles
            .iter()
            .find(|style| style.name.eq_ignore_ascii_case(name))
    }

    /// 線種に添える文字の高さ（決定 0166）。文字スタイルが決まった高さを持てば、その高さに倍率を掛けた値、
    /// 持たないか、文字スタイルがなければ倍率そのもの。単位は線種の列と同じ。形か、有限でない高さなら `None`
    pub fn mark_text_height(&self, mark: &LineTypeMark) -> Option<f64> {
        let MarkContent::Text { style, .. } = &mark.content else {
            return None;
        };
        let height = style
            .as_deref()
            .and_then(|name| self.text_style(name))
            .and_then(|style| style.definition.as_ref())
            .and_then(|definition| definition.height)
            .filter(|height| *height > 0.0);
        let height = height.map_or(mark.scale, |height| height * mark.scale);
        height.is_finite().then_some(height)
    }

    /// 要素が名前で指すだけで、表にない文字スタイルを、定義のない項目として後ろに足す。
    /// 空の名前は足さない。作業量と中断を確かめ、途中で止まれば追加しない
    pub fn add_named_text_styles(&mut self, cx: &mut Cx<'_>) -> Result<(), (Stop, Ref)> {
        let mut known = BTreeSet::new();
        let mut names = Vec::new();
        // BTreeSet の比較は共通の先頭部分を繰り返し調べる。名前の長さと木の高さから作業量を数える
        let mut note = |name: &str, add: bool| -> Result<(), Stop> {
            if name.is_empty() {
                return cx.check(Kind::Work, 1);
            }
            let height = u64::from(known.len().max(1).ilog2()) + 1;
            let chunks = u64::try_from(name.len().div_ceil(1024))
                .unwrap_or(u64::MAX)
                .max(1);
            cx.check(Kind::Work, chunks.saturating_mul(height).saturating_mul(16))?;
            if !name.is_empty() && known.insert(name.to_ascii_lowercase()) && add {
                names.push(name.to_owned());
            }
            Ok(())
        };
        for style in &self.text_styles {
            note(&style.name, false).map_err(|stop| (stop, style.source.clone()))?;
        }
        for line_type in &self.line_types {
            let at = || line_type.source.clone();
            note("", true).map_err(|stop| (stop, at()))?;
            for mark in line_type.definition.iter().flat_map(|d| &d.marks) {
                let name = match &mark.content {
                    MarkContent::Text {
                        style: Some(style), ..
                    } => style.as_str(),
                    _ => "",
                };
                note(name, true).map_err(|stop| (stop, at()))?;
            }
        }
        for space in &self.spaces {
            note("", true).map_err(|stop| (stop, Ref::default()))?;
            for element in &space.elements {
                let at = || element.source.clone();
                note("", true).map_err(|stop| (stop, at()))?;
                shape_text_styles(&element.shape, &mut |name| note(name, true))
                    .map_err(|stop| (stop, at()))?;
            }
        }
        for sheet in &self.sheets {
            note("", true).map_err(|stop| (stop, sheet.source.clone()))?;
            for element in &sheet.elements {
                let at = || element.source.clone();
                note("", true).map_err(|stop| (stop, at()))?;
                shape_text_styles(&element.shape, &mut |name| note(name, true))
                    .map_err(|stop| (stop, at()))?;
            }
        }
        for symbol in &self.symbols {
            note("", true).map_err(|stop| (stop, symbol.source.clone()))?;
            for element in &symbol.space.elements {
                let at = || element.source.clone();
                note("", true).map_err(|stop| (stop, at()))?;
                shape_text_styles(&element.shape, &mut |name| note(name, true))
                    .map_err(|stop| (stop, at()))?;
            }
            for definition in &symbol.attributes {
                attribute_text_styles(&definition.attribute, &mut |name| note(name, true))
                    .map_err(|stop| (stop, definition.attribute.source.clone()))?;
            }
        }
        self.text_styles
            .extend(names.into_iter().map(|name| TextStyle {
                name,
                definition: None,
                source: Ref::default(),
            }));
        Ok(())
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

    /// 付く先の、いまの値。図面と、実寸の空間・記号の定義の空間・用紙の要素と、レイヤ・線種を探す。
    /// なければ `None`
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
            Target::Layer(id) => self.layer(id).cloned().map(Basis::Layer),
            Target::LineType(id) => self.line_type(id).cloned().map(Basis::LineType),
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

/// 形が名前で指す文字スタイルを `note` に渡す
fn shape_text_styles<P>(
    shape: &Shape<P>,
    note: &mut impl FnMut(&str) -> Result<(), Stop>,
) -> Result<(), Stop> {
    match shape {
        Shape::Text(text) => note(&text.style)?,
        Shape::Formatted(text) => note(&text.style)?,
        Shape::Symbol(symbol) => {
            for attribute in &symbol.attributes {
                attribute_text_styles(attribute, note)?;
            }
        }
        Shape::Callout(callout) => match &callout.content {
            CalloutContent::Text(text) => note(&text.style)?,
            CalloutContent::Symbol { symbol, .. } => {
                for attribute in &symbol.attributes {
                    attribute_text_styles(attribute, note)?;
                }
            }
            CalloutContent::None => {}
        },
        Shape::Table(table) => {
            let formats = table.formats.iter().flat_map(|formats| {
                [
                    &formats.title_format,
                    &formats.header_format,
                    &formats.data_format,
                ]
            });
            for format in table.cells.iter().map(|cell| &cell.format).chain(formats) {
                note(format.text_style.as_deref().unwrap_or(""))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn attribute_text_styles<P>(
    attribute: &Attribute<P>,
    note: &mut impl FnMut(&str) -> Result<(), Stop>,
) -> Result<(), Stop> {
    note(&attribute.text.style)?;
    if let Some(formatted) = &attribute.formatted {
        note(&formatted.style)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::norm;

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
                    transparency: None,
                }),
                follows_placement: follows,
                source: Ref::default(),
                description: None,
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
            transparency: Inherit::Layer,
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

#[cfg(test)]
mod text_style_tests {
    use super::*;

    #[test]
    fn named_text_styles_are_added_once_without_definitions() {
        let mark = |style: &str| LineTypeMark {
            index: 0,
            content: MarkContent::Text {
                content: "W".to_owned(),
                style: Some(style.to_owned()),
            },
            scale: 1.0,
            rotation: 0.0,
            rotation_basis: MarkRotation::Relative,
            offset: [0.0, 0.0],
        };
        let mut drawing = Drawing2D {
            line_types: vec![LineType {
                id: LineTypeId(0),
                name: "Water".to_owned(),
                definition: Some(LineTypeDefinition {
                    description: String::new(),
                    pattern: vec![1.0, -1.0],
                    basis: PatternBasis::Element,
                    marks: vec![mark("Standard"), mark("GOTHIC"), mark("gothic"), mark("")],
                }),
                source: Ref::default(),
            }],
            text_styles: vec![TextStyle {
                name: "standard".to_owned(),
                definition: None,
                source: Ref::at_record(3),
            }],
            ..Drawing2D::default()
        };
        let original = drawing.clone();
        let mut limited = zumen_base::Budget::new(
            zumen_base::Limits::default().with(Kind::Work, zumen_base::Limit::Finite(50)),
        );
        let mut diagnostics = zumen_base::Diagnostics::new();
        let cancel = zumen_base::CancelToken::new();
        assert!(
            drawing
                .add_named_text_styles(&mut Cx::new(&mut limited, &mut diagnostics, &cancel))
                .is_err()
        );
        assert_eq!(drawing, original);

        let mut budget = zumen_base::Budget::default();
        let mut diagnostics = zumen_base::Diagnostics::new();
        let cancel = zumen_base::CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        drawing.add_named_text_styles(&mut cx).unwrap();
        let names: Vec<&str> = drawing
            .text_styles
            .iter()
            .map(|style| style.name.as_str())
            .collect();
        assert_eq!(names, ["standard", "GOTHIC"]);
        assert_eq!(drawing.text_style("Gothic").unwrap().source, Ref::default());
    }

    #[test]
    fn mark_text_heights_scale_the_fixed_height_of_their_text_style() {
        let mark = |content: MarkContent| LineTypeMark {
            index: 0,
            content,
            scale: 0.5,
            rotation: 0.0,
            rotation_basis: MarkRotation::Upright,
            offset: [0.0, 0.0],
        };
        let text = |style: Option<&str>| {
            mark(MarkContent::Text {
                content: "GAS".to_owned(),
                style: style.map(str::to_owned),
            })
        };
        let style = |name: &str, height: Option<f64>| TextStyle {
            name: name.to_owned(),
            definition: Some(TextStyleDefinition {
                font: None,
                big_font: None,
                height,
                width: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                vertical: false,
            }),
            source: Ref::default(),
        };
        let drawing = Drawing2D {
            text_styles: vec![style("Fixed", Some(3.0)), style("Free", None)],
            ..Drawing2D::default()
        };
        // 決まった高さに倍率を掛ける。高さを決めない・ない文字スタイルなら、倍率を高さにする。形は高さを持たない
        assert_eq!(drawing.mark_text_height(&text(Some("fixed"))), Some(1.5));
        assert_eq!(drawing.mark_text_height(&text(Some("Free"))), Some(0.5));
        assert_eq!(drawing.mark_text_height(&text(Some("Missing"))), Some(0.5));
        assert_eq!(drawing.mark_text_height(&text(None)), Some(0.5));
        let huge = Drawing2D {
            text_styles: vec![style("Fixed", Some(1e200))],
            ..Drawing2D::default()
        };
        let mut oversized = text(Some("Fixed"));
        oversized.scale = 1e200;
        assert_eq!(huge.mark_text_height(&oversized), None);
        let shape = mark(MarkContent::Shape {
            font: None,
            number: 1,
        });
        assert_eq!(drawing.mark_text_height(&shape), None);
    }

    #[test]
    fn collecting_style_names_is_bounded_and_stops_without_changes() {
        use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

        let drawing_with = |count: usize| Drawing2D {
            text_styles: (0..count)
                .map(|i| TextStyle {
                    name: format!("long-common-prefix-{i:04}"),
                    definition: None,
                    source: Ref::default(),
                })
                .collect(),
            ..Drawing2D::default()
        };
        let used = |count| {
            let mut drawing = drawing_with(count);
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            drawing
                .add_named_text_styles(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
                .unwrap();
            budget.used(Kind::Work)
        };
        let small = used(128);
        let large = used(256);
        assert!(large < small * 3);

        let mut drawing = drawing_with(128);
        let original = drawing.clone();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(small - 1)));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        assert!(matches!(
            drawing.add_named_text_styles(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err((Stop::Exceeded(_), _))
        ));
        assert_eq!(drawing, original);
        cancel.cancel();
        assert_eq!(
            drawing.add_named_text_styles(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err((Stop::Cancelled, Ref::default()))
        );
        assert_eq!(drawing, original);
    }
}
