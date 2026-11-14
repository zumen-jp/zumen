//! 足すレコードの値と、属性を変えたレコードの値。
//!
//! 値そのものは `zumen_acad::single`（`restyle_model`・`born_model`）の共通規則で作る。
//! 試作がまだ範囲の外とする条件（名前の表のレコード 1 つだけでは決まらない名前、Shift_JIS、属性の定義を持つ記号）だけを
//! 先に確かめ、当てはまれば `Decline::Outside` を返す。

use zumen_acad::Value;
use zumen_acad::single::{self, RestyleLineType};
use zumen_drawing::{
    Color, ColorCode, ColorSystem, Edge, Element, Fit, Grid, Height, HeightMeaning, Horizontal,
    Inherit, LayerId, Length, LineTypeId, LineWeight, ModelLength, ModelPoint, PaperLength,
    Placement, Polyline, Shape, SpaceId, Style, Symbol, SymbolKey, Text, Vertex, Vertical,
};

use crate::edit::{Decline, Outside};
use crate::lift::BLOCK;
use crate::store::{Charset, Document, table};

/// 色の指定
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSpec {
    /// 画層に従う
    ByLayer,
    /// ブロックに従う
    ByBlock,
    /// 色番号（1〜255）
    Aci(u8),
    /// RGB
    Rgb([u8; 3]),
    /// 色番号と RGB
    AciRgb(u8, [u8; 3]),
}

/// 線種の指定
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineTypeSpec {
    /// 画層に従う
    ByLayer,
    /// ブロックに従う
    ByBlock,
    /// 線種の名前
    Name(String),
}

/// 線の太さの指定
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WeightSpec {
    /// 画層に従う
    ByLayer,
    /// ブロックに従う
    ByBlock,
    /// 文書の既定
    Default,
    /// ミリメートル
    Mm(f64),
}

/// 属性を変える変更。`None` の項目は変えない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Restyle {
    /// 画層の名前
    pub layer: Option<String>,
    /// 色
    pub color: Option<ColorSpec>,
    /// 線種
    pub line_type: Option<LineTypeSpec>,
    /// 線種の尺度
    pub line_type_scale: Option<f64>,
    /// 線の太さ
    pub line_weight: Option<WeightSpec>,
}

/// 足す要素。モデル空間に足す
#[derive(Clone, Debug, PartialEq)]
pub struct Add {
    /// 画層の名前。`None` は画層 0
    pub layer: Option<String>,
    /// 色。`None` は画層に従う
    pub color: Option<ColorSpec>,
    /// 線種。`None` は画層に従う
    pub line_type: Option<LineTypeSpec>,
    /// 線の太さ。`None` は画層に従う
    pub line_weight: Option<WeightSpec>,
    /// 形
    pub shape: NewShape,
}

/// 足す要素の形
#[derive(Clone, Debug, PartialEq)]
pub enum NewShape {
    /// 線
    Line {
        /// 始まり
        start: [f64; 2],
        /// 終わり
        end: [f64; 2],
    },
    /// 円
    Circle {
        /// 中心
        center: [f64; 2],
        /// 半径
        radius: f64,
    },
    /// 反時計回りの円弧
    Arc {
        /// 中心
        center: [f64; 2],
        /// 半径
        radius: f64,
        /// 始まりの角（度）
        start: f64,
        /// 終わりの角（度）
        end: f64,
    },
    /// 折れ線
    Polyline {
        /// 頂点
        points: Vec<[f64; 2]>,
        /// 頂点ごとの膨らみ
        bulges: Option<Vec<f64>>,
        /// 閉じているか
        closed: bool,
    },
    /// 文字
    Text(NewText),
    /// 記号（ブロックの挿入）
    Symbol(NewSymbol),
}

/// 足す文字
#[derive(Clone, Debug, PartialEq)]
pub struct NewText {
    /// 中身
    pub content: String,
    /// 置く点
    pub position: [f64; 2],
    /// 2 点の間に置くときの終わりの点と、幅も合わせるか
    pub end: Option<([f64; 2], bool)>,
    /// 横の位置合わせ（72 の値。0・1・2・4）
    pub horizontal: Option<u8>,
    /// 縦の位置合わせ（73 の値。0〜3）
    pub vertical: Option<u8>,
    /// 高さ
    pub height: f64,
    /// 回転（度）
    pub rotation: f64,
    /// 幅の係数
    pub width_factor: Option<f64>,
    /// 傾き（度）
    pub oblique: Option<f64>,
    /// 左右を反転するか
    pub backward: bool,
    /// 上下を反転するか
    pub upside_down: bool,
    /// 文字スタイルの名前。`None` は Standard
    pub style: Option<String>,
}

/// 足す記号
#[derive(Clone, Debug, PartialEq)]
pub struct NewSymbol {
    /// 定義の名前
    pub name: String,
    /// 置く点
    pub position: [f64; 2],
    /// 尺度
    pub scale: [f64; 2],
    /// 回転（度）
    pub rotation: f64,
    /// 格子
    pub grid: Option<NewGrid>,
}

/// 記号の格子
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewGrid {
    /// 列の数
    pub columns: u32,
    /// 行の数
    pub rows: u32,
    /// 列の間隔
    pub column_spacing: f64,
    /// 行の間隔
    pub row_spacing: f64,
}

const UNSUPPORTED: Decline = Decline::Unsupported;
const MODEL: SpaceId = SpaceId(0);

fn model_point([x, y]: [f64; 2]) -> ModelPoint {
    ModelPoint { space: MODEL, x, y }
}

fn model_length(value: f64) -> Length {
    Length::Model(ModelLength {
        space: MODEL,
        value,
    })
}

fn color_from(spec: ColorSpec) -> Result<Inherit<Color>, Decline> {
    Ok(match spec {
        ColorSpec::ByLayer => Inherit::Layer,
        ColorSpec::ByBlock => Inherit::Block,
        ColorSpec::Aci(0) | ColorSpec::AciRgb(0, _) => return Err(UNSUPPORTED),
        ColorSpec::Aci(aci) => Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value: u32::from(aci),
            }),
            rgb: None,
        }),
        ColorSpec::Rgb(rgb) => Inherit::Own(Color {
            code: None,
            rgb: Some(rgb),
        }),
        ColorSpec::AciRgb(aci, rgb) => Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value: u32::from(aci),
            }),
            rgb: Some(rgb),
        }),
    })
}

fn weight_from(spec: WeightSpec) -> Result<Inherit<LineWeight>, Decline> {
    Ok(match spec {
        WeightSpec::ByLayer => Inherit::Layer,
        WeightSpec::ByBlock => Inherit::Block,
        WeightSpec::Default => Inherit::Own(LineWeight::Default),
        WeightSpec::Mm(mm) if mm.is_finite() && mm >= 0.0 => {
            Inherit::Own(LineWeight::Width(PaperLength(mm)))
        }
        WeightSpec::Mm(_) => return Err(UNSUPPORTED),
    })
}

impl Document {
    /// 文字列を、この図面の符号化で書けるか確かめる。試作は、UTF-8 の図面のほかでは ASCII だけを書く
    fn check_encodable(&self, value: &str) -> Result<(), Decline> {
        if value.is_ascii() || self.charset == Charset::Utf8 {
            Ok(())
        } else {
            Err(Decline::Outside(Outside::Charset))
        }
    }

    /// 画層か線種の名前が、開くときに読んだ表の 1 レコードだけで同じ綴りに決まるか確かめる
    fn check_defined(&self, table: usize, name: &str) -> Result<(), Decline> {
        let outside = Decline::Outside(Outside::Names);
        if !self.names_exact {
            return Err(outside);
        }
        let spelling = self
            .names
            .get(table)
            .and_then(Option::as_ref)
            .and_then(|names| names.get(&name.to_lowercase()))
            .ok_or(outside)?;
        if spelling.count == 1 && spelling.tables == 1 && *spelling.name == *name.as_bytes() {
            Ok(())
        } else {
            Err(outside)
        }
    }

    /// 文字スタイルの名前を確かめる。表があれば表にあること、なければ Standard で符号化できること
    fn check_text_style(&self, name: &str) -> Result<(), Decline> {
        if !self.names_exact {
            return Err(Decline::Outside(Outside::Names));
        }
        match self.names.get(table::STYLE).and_then(Option::as_ref) {
            Some(names) => {
                if names.contains_key(&name.to_lowercase()) {
                    Ok(())
                } else {
                    Err(UNSUPPORTED)
                }
            }
            None if name.eq_ignore_ascii_case("Standard") => self.check_encodable(name),
            None => Err(UNSUPPORTED),
        }
    }

    /// 記号の定義の名前を確かめる。名前で 1 つに決まり、属性の定義を持たない定義だけを受け付ける
    fn check_block_name(&self, name: &str) -> Result<(), Decline> {
        let lower = name.to_lowercase();
        if lower.starts_with("*model_space") || lower.starts_with("*paper_space") {
            return Err(UNSUPPORTED);
        }
        if !self.names_exact {
            return Err(Decline::Outside(Outside::Names));
        }
        let definition = self
            .usable_definitions
            .get(&lower)
            .and_then(|at| self.definitions.get(*at as usize))
            .ok_or(Decline::Outside(Outside::Definition))?;
        if let Some(names) = self.names.get(table::BLOCK_RECORD).and_then(Option::as_ref) {
            let key = self
                .decode(&definition.name)
                .ok_or(Decline::Outside(Outside::Names))?
                .to_lowercase();
            if !names.contains_key(&key) {
                return Err(UNSUPPORTED);
            }
        }
        Ok(())
    }

    /// 線種の指定を共通関数向けの形にする
    fn restyle_line_type<'a>(
        &self,
        spec: &'a LineTypeSpec,
    ) -> Result<RestyleLineType<'a>, Decline> {
        Ok(match spec {
            LineTypeSpec::ByLayer => RestyleLineType::Layer,
            LineTypeSpec::ByBlock => RestyleLineType::Block,
            LineTypeSpec::Name(name)
                if name.eq_ignore_ascii_case("ByLayer") || name.eq_ignore_ascii_case("ByBlock") =>
            {
                return Err(Decline::Outside(Outside::Names));
            }
            LineTypeSpec::Name(name) => {
                self.check_defined(table::LTYPE, name)?;
                RestyleLineType::Named(name.as_str())
            }
        })
    }

    /// 属性を変えた後のレコードの値（`zumen_acad::single::restyle_model`）。`layer` は、今の画層の名前。
    /// 何も変わらなければ、元の値のまま
    pub(crate) fn restyled(
        &self,
        kind: &[u8],
        layer: &[u8],
        old: &[Value],
        change: &Restyle,
    ) -> Result<Vec<Value>, Decline> {
        let current_layer = self
            .decode(layer)
            .ok_or(Decline::Outside(Outside::Charset))?;
        let new_layer = match &change.layer {
            Some(name) => {
                self.check_defined(table::LAYER, name)?;
                Some(name.as_str())
            }
            None => None,
        };
        let new_line_type = match &change.line_type {
            Some(spec) => Some(self.restyle_line_type(spec)?),
            None => None,
        };
        let color = change.color.map(color_from).transpose()?;
        let line_weight = change.line_weight.map(weight_from).transpose()?;
        if let Some(scale) = change.line_type_scale
            && !(scale.is_finite() && scale > 0.0)
        {
            return Err(UNSUPPORTED);
        }
        single::restyle_model(
            &self.single_tables,
            kind,
            old,
            current_layer,
            new_layer,
            new_line_type,
            color,
            change.line_type_scale,
            line_weight,
        )
        .ok_or(UNSUPPORTED)
    }

    /// 足す要素のレコードの種類と値（`zumen_acad::single::born_model`）。値は、ハンドル（5）から並ぶ
    pub(crate) fn born(
        &self,
        add: &Add,
        handle: u64,
    ) -> Result<(&'static [u8], Vec<Value>), Decline> {
        let layer_name = add.layer.as_deref().unwrap_or("0");
        self.check_defined(table::LAYER, layer_name)?;
        let color = match add.color {
            Some(spec) => color_from(spec)?,
            None => Inherit::Layer,
        };
        let (line_type, line_type_name) = match &add.line_type {
            None | Some(LineTypeSpec::ByLayer) => (Inherit::Layer, None),
            Some(LineTypeSpec::ByBlock) => (Inherit::Block, None),
            Some(LineTypeSpec::Name(name))
                if name.eq_ignore_ascii_case("ByLayer") || name.eq_ignore_ascii_case("ByBlock") =>
            {
                return Err(Decline::Outside(Outside::Names));
            }
            Some(LineTypeSpec::Name(name)) => {
                self.check_defined(table::LTYPE, name)?;
                (Inherit::Own(LineTypeId(0)), Some(name.as_str()))
            }
        };
        let line_weight = match add.line_weight {
            Some(spec) => Some(weight_from(spec)?),
            None => Some(Inherit::Layer),
        };
        let shape = self.new_shape(&add.shape)?;
        let element = Element {
            layer: LayerId(0),
            style: Style {
                color,
                line_type,
                line_type_scale: 1.0,
                line_weight,
                transparency: Inherit::Layer,
            },
            shape,
            source: Default::default(),
        };
        let mut records = single::born_model(
            &self.single_tables,
            &element,
            layer_name,
            line_type_name,
            &[],
            &[],
            handle,
        )
        .ok_or(UNSUPPORTED)?;
        if records.len() != 1 {
            return Err(Decline::Outside(Outside::Definition));
        }
        records.pop().ok_or(UNSUPPORTED)
    }

    fn new_shape(&self, shape: &NewShape) -> Result<Shape<ModelPoint>, Decline> {
        Ok(match shape {
            NewShape::Line { start, end } => Shape::Line {
                start: model_point(*start),
                end: model_point(*end),
            },
            NewShape::Circle { center, radius } => Shape::Circle {
                center: model_point(*center),
                radius: model_length(*radius),
            },
            NewShape::Arc {
                center,
                radius,
                start,
                end,
            } => Shape::Curve(Edge::Arc {
                center: model_point(*center),
                radius: model_length(*radius),
                start: *start,
                end: *end,
                counterclockwise: true,
            }),
            NewShape::Polyline {
                points,
                bulges,
                closed,
            } => {
                if points.len() < 2
                    || bulges.as_ref().is_some_and(|bulges| {
                        bulges.len() != points.len() || bulges.iter().any(|b| !b.is_finite())
                    })
                {
                    return Err(UNSUPPORTED);
                }
                let vertices = points
                    .iter()
                    .enumerate()
                    .map(|(index, point)| Vertex {
                        point: model_point(*point),
                        bulge: bulges
                            .as_ref()
                            .and_then(|bulges| bulges.get(index))
                            .copied()
                            .unwrap_or(0.0),
                    })
                    .collect();
                Shape::Polyline(Polyline {
                    vertices,
                    closed: *closed,
                    width: None,
                    vertex_widths: Vec::new(),
                    line_type_generation: false,
                })
            }
            NewShape::Text(new) => Shape::Text(self.new_text(new)?),
            NewShape::Symbol(new) => Shape::Symbol(self.new_symbol(new)?),
        })
    }

    fn new_text(&self, new: &NewText) -> Result<Text<ModelPoint>, Decline> {
        if new.content.contains(['\n', '\r']) {
            return Err(UNSUPPORTED);
        }
        let style_name = new.style.as_deref().unwrap_or("Standard");
        self.check_text_style(style_name)?;
        self.check_encodable(&new.content)?;
        let placement = match new.end {
            Some((end, fit)) => {
                if new.horizontal.is_some() || new.vertical.is_some() || end == new.position {
                    return Err(UNSUPPORTED);
                }
                let angle =
                    libm::atan2(end[1] - new.position[1], end[0] - new.position[0]).to_degrees();
                let turn = (new.rotation - angle).rem_euclid(360.0);
                if turn.min(360.0 - turn) > 1e-6 {
                    return Err(UNSUPPORTED);
                }
                Placement::Between {
                    first: model_point(new.position),
                    second: model_point(end),
                    fit: if fit { Fit::Fit } else { Fit::Aligned },
                }
            }
            None => {
                let horizontal = match new.horizontal.unwrap_or(0) {
                    0 => Horizontal::Left,
                    1 => Horizontal::Center,
                    2 => Horizontal::Right,
                    4 => Horizontal::Middle,
                    _ => return Err(UNSUPPORTED),
                };
                let vertical = match new.vertical.unwrap_or(0) {
                    0 => Vertical::Baseline,
                    1 => Vertical::Bottom,
                    2 => Vertical::Middle,
                    3 => Vertical::Top,
                    _ => return Err(UNSUPPORTED),
                };
                Placement::Anchor {
                    point: model_point(new.position),
                    horizontal,
                    vertical,
                }
            }
        };
        Ok(Text {
            content: new.content.clone(),
            height: Height {
                value: model_length(new.height),
                meaning: HeightMeaning::Declared("dxf.TEXT.40".into()),
            },
            placement,
            rotation: new.rotation,
            width_factor: new.width_factor.unwrap_or(1.0),
            oblique: new.oblique.unwrap_or(0.0),
            backward: new.backward,
            upside_down: new.upside_down,
            style: style_name.to_owned(),
        })
    }

    fn new_symbol(&self, new: &NewSymbol) -> Result<Symbol<ModelPoint>, Decline> {
        self.check_block_name(&new.name)?;
        let grid = match &new.grid {
            None => None,
            Some(grid) => {
                if grid.columns == 0 || grid.rows == 0 || (grid.columns, grid.rows) == (1, 1) {
                    return Err(UNSUPPORTED);
                }
                Some(Grid {
                    columns: grid.columns,
                    rows: grid.rows,
                    column_spacing: model_length(grid.column_spacing),
                    row_spacing: model_length(grid.row_spacing),
                })
            }
        };
        Ok(Symbol {
            key: SymbolKey {
                namespace: BLOCK.into(),
                name: new.name.clone(),
            },
            position: model_point(new.position),
            scale: new.scale,
            rotation: new.rotation,
            grid,
            attributes: Vec::new(),
            clip: None,
        })
    }
}
