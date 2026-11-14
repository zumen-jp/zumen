//! 図形の追加（`Edit::Add`）とスタイル変更（`Edit::Restyle`）の入力とレコード生成。
//!
//! 公開操作の入力（`Add`・`Restyle`）を、`Today` の `zumen-abi`（`changes.rs`・`geometry.rs`）と
//! 同じ検証・変換規則で `Element<ModelPoint>` に組み、`zumen-acad` の `single::born_model` で
//! DXF のレコード列にする。

use std::borrow::Cow;

use zumen_acad::Value;
use zumen_acad::single::{self, BornRecords, RestyleLineType};
use zumen_base::Ref;
use zumen_drawing::{
    Affine, Attribute, Color, ColorCode, ColorSystem, Edge, Element, Fit, Grid, Height,
    HeightMeaning, Horizontal, Inherit, LayerId, Length, LineTypeId, LineWeight, ModelLength,
    ModelPoint, PaperLength, Placement, Polyline, Shape, SpaceId, Style, Symbol, SymbolKey, Text,
    Vertex, Vertical, transform::transform as transform_shape,
};

use crate::lift::BLOCK;
use crate::store::{Document, OTag};
use crate::value::value_bytes;

const MODEL: SpaceId = SpaceId(0);

/// 色の指定
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSpec {
    /// 画層に従う
    ByLayer,
    /// ブロックに従う
    ByBlock,
    /// AutoCAD 色番号
    Aci(u8),
    /// RGB
    Rgb([u8; 3]),
    /// AutoCAD 色番号と RGB の組
    AciRgb(u8, [u8; 3]),
}

impl ColorSpec {
    pub(crate) fn to_inherit(self) -> Inherit<Color> {
        match self {
            Self::ByLayer => Inherit::Layer,
            Self::ByBlock => Inherit::Block,
            Self::Aci(aci) => Inherit::Own(Color {
                code: Some(ColorCode {
                    system: ColorSystem::Aci,
                    value: u32::from(aci),
                }),
                rgb: None,
            }),
            Self::Rgb(rgb) => Inherit::Own(Color {
                code: None,
                rgb: Some(rgb),
            }),
            Self::AciRgb(aci, rgb) => Inherit::Own(Color {
                code: Some(ColorCode {
                    system: ColorSystem::Aci,
                    value: u32::from(aci),
                }),
                rgb: Some(rgb),
            }),
        }
    }
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
    /// 既定の太さ
    Default,
    /// mm 単位の太さ
    Mm(f64),
}

impl WeightSpec {
    pub(crate) fn to_inherit(self) -> Option<Inherit<LineWeight>> {
        match self {
            Self::ByLayer => Some(Inherit::Layer),
            Self::ByBlock => Some(Inherit::Block),
            Self::Default => Some(Inherit::Own(LineWeight::Default)),
            Self::Mm(mm) if mm.is_finite() && mm >= 0.0 => {
                Some(Inherit::Own(LineWeight::Width(PaperLength(mm))))
            }
            Self::Mm(_) => None,
        }
    }
}

/// 属性を変える変更の入力
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Restyle {
    /// 新しい画層の名前
    pub layer: Option<String>,
    /// 新しい色
    pub color: Option<ColorSpec>,
    /// 新しい線種
    pub line_type: Option<LineTypeSpec>,
    /// 新しい線種の尺度
    pub line_type_scale: Option<f64>,
    /// 新しい線の太さ
    pub line_weight: Option<WeightSpec>,
}

/// 記号の格子配置の指定
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NewGrid {
    /// 列数
    pub columns: u32,
    /// 行数
    pub rows: u32,
    /// 列の間隔
    pub column_spacing: f64,
    /// 行の間隔
    pub row_spacing: f64,
}

/// 1 行の文字を足す入力
#[derive(Clone, Debug, PartialEq)]
pub struct NewText {
    /// 本文
    pub content: String,
    /// 1 点目の位置
    pub position: [f64; 2],
    /// 2 点目の位置と `fit`（`true` なら `Fit`、`false` なら `Aligned`）
    pub end: Option<([f64; 2], bool)>,
    /// 横の揃え（`0: Left, 1: Center, 2: Right, 4: Middle`）
    pub horizontal: Option<u8>,
    /// 縦の揃え（`0: Baseline, 1: Bottom, 2: Middle, 3: Top`）
    pub vertical: Option<u8>,
    /// 高さ
    pub height: f64,
    /// 回転角（度）
    pub rotation: f64,
    /// 幅の倍率
    pub width_factor: Option<f64>,
    /// 傾きの角（度）
    pub oblique: Option<f64>,
    /// 左右反転
    pub backward: bool,
    /// 上下反転
    pub upside_down: bool,
    /// 文字スタイルの名前
    pub style: Option<String>,
}

/// 記号の配置を足す入力
#[derive(Clone, Debug, PartialEq)]
pub struct NewSymbol {
    /// ブロック定義の名前
    pub name: String,
    /// 挿入点
    pub position: [f64; 2],
    /// X・Y の尺度
    pub scale: [f64; 2],
    /// 回転角（度）
    pub rotation: f64,
    /// 格子配置
    pub grid: Option<NewGrid>,
}

/// 足す図形の形
#[derive(Clone, Debug, PartialEq)]
pub enum NewShape {
    /// 線分
    Line {
        /// 始点
        start: [f64; 2],
        /// 終点
        end: [f64; 2],
    },
    /// 円
    Circle {
        /// 中心
        center: [f64; 2],
        /// 半径
        radius: f64,
    },
    /// 円弧
    Arc {
        /// 中心
        center: [f64; 2],
        /// 半径
        radius: f64,
        /// 始角（度）
        start: f64,
        /// 終角（度）
        end: f64,
    },
    /// 折れ線
    Polyline {
        /// 頂点列
        points: Vec<[f64; 2]>,
        /// 膨らみ列
        bulges: Option<Vec<f64>>,
        /// 閉じているか
        closed: bool,
    },
    /// 1 行の文字
    Text(NewText),
    /// 記号の配置
    Symbol(NewSymbol),
}

/// 図形を 1 つ足す入力
#[derive(Clone, Debug, PartialEq)]
pub struct Add {
    /// 画層の名前（`None` なら `"0"`）
    pub layer: Option<String>,
    /// 色
    pub color: Option<ColorSpec>,
    /// 線種
    pub line_type: Option<LineTypeSpec>,
    /// 線の太さ
    pub line_weight: Option<WeightSpec>,
    /// 形
    pub shape: NewShape,
}

impl LineTypeSpec {
    pub(crate) fn as_restyle<'a>(&'a self, doc: &'a Document) -> Option<RestyleLineType<'a>> {
        match self {
            Self::ByLayer => Some(RestyleLineType::Layer),
            Self::ByBlock => Some(RestyleLineType::Block),
            Self::Name(name) => {
                let id = doc.lookup_live_line_type(name)?;
                let exact = doc.line_types.get(id.0 as usize)?.name.as_str();
                Some(RestyleLineType::Named(exact))
            }
        }
    }
}

fn model_pt([x, y]: [f64; 2]) -> Option<ModelPoint> {
    (x.is_finite() && y.is_finite()).then_some(ModelPoint { space: MODEL, x, y })
}

fn model_len(v: f64) -> Option<Length> {
    v.is_finite().then_some(Length::Model(ModelLength {
        space: MODEL,
        value: v,
    }))
}

fn positive_len(v: f64) -> Option<Length> {
    (v.is_finite() && v > 0.0).then_some(Length::Model(ModelLength {
        space: MODEL,
        value: v,
    }))
}

fn horizontal_of(code: u8) -> Horizontal {
    match code {
        0 => Horizontal::Left,
        1 => Horizontal::Center,
        2 => Horizontal::Right,
        _ => Horizontal::Middle,
    }
}

fn vertical_of(code: u8) -> Vertical {
    match code {
        0 => Vertical::Baseline,
        1 => Vertical::Bottom,
        2 => Vertical::Middle,
        _ => Vertical::Top,
    }
}

/// 符号化した追加レコード（種類名・タグ列・値列）
pub(crate) type EncodedBornRecord = (Vec<u8>, Vec<OTag>, Vec<Value>);

/// `BornRecords` を `OTag` のレコード列に符号化する
pub(crate) fn encode_born_records(records: BornRecords) -> Option<Vec<EncodedBornRecord>> {
    let mut out = Vec::with_capacity(records.len());
    for (kind, values) in records {
        let mut tags = Vec::with_capacity(1 + values.len());
        tags.push(OTag::added(0, kind.to_vec()));
        for val in &values {
            tags.push(OTag::added(val.code, value_bytes(&val.data)?));
        }
        out.push((kind.to_vec(), tags, values));
    }
    Some(out)
}

impl Document {
    /// `Add` を検証して `BornRecords` を作る。今の実装が断る入力は `None` を返す
    pub(crate) fn build_born_records(&self, add: &Add, handle: u64) -> Option<BornRecords> {
        self.entities_end?;
        let target_layer_name = add.layer.as_deref().unwrap_or("0");
        let layer_id = self.lookup_live_layer(target_layer_name)?;
        let resolved_layer_name = self.layers.get(layer_id.0 as usize)?.name.as_str();

        let (line_type, lt_name): (Inherit<LineTypeId>, Option<&str>) = match &add.line_type {
            None | Some(LineTypeSpec::ByLayer) => (Inherit::Layer, None),
            Some(LineTypeSpec::ByBlock) => (Inherit::Block, None),
            Some(LineTypeSpec::Name(name)) => {
                let id = self.lookup_live_line_type(name)?;
                let exact = self.line_types.get(id.0 as usize)?.name.as_str();
                (Inherit::Own(id), Some(exact))
            }
        };

        let line_weight = match add.line_weight {
            Some(spec) => Some(spec.to_inherit()?),
            None => Some(Inherit::Layer),
        };
        let color = add.color.map_or(Inherit::Layer, ColorSpec::to_inherit);

        let mut attrib_layers: Vec<(LayerId, String)> = Vec::new();
        let mut attrib_line_types: Vec<(LineTypeId, String)> = Vec::new();

        let shape: Shape<ModelPoint> = match &add.shape {
            NewShape::Line { start, end } => Shape::Line {
                start: model_pt(*start)?,
                end: model_pt(*end)?,
            },
            NewShape::Circle { center, radius } => Shape::Circle {
                center: model_pt(*center)?,
                radius: positive_len(*radius)?,
            },
            NewShape::Arc {
                center,
                radius,
                start,
                end,
            } => {
                if !start.is_finite() || !end.is_finite() {
                    return None;
                }
                Shape::Curve(Edge::Arc {
                    center: model_pt(*center)?,
                    radius: positive_len(*radius)?,
                    start: *start,
                    end: *end,
                    counterclockwise: true,
                })
            }
            NewShape::Polyline {
                points,
                bulges,
                closed,
            } => {
                if points.len() < 2 {
                    return None;
                }
                let bulges_slice: Vec<f64> = match bulges {
                    None => vec![0.0; points.len()],
                    Some(b) if b.len() == points.len() => b.clone(),
                    Some(_) => return None,
                };
                let mut vertices = Vec::with_capacity(points.len());
                for (&pt, &bulge) in points.iter().zip(&bulges_slice) {
                    if !bulge.is_finite() {
                        return None;
                    }
                    vertices.push(Vertex {
                        point: model_pt(pt)?,
                        bulge,
                    });
                }
                Shape::Polyline(Polyline {
                    vertices,
                    closed: *closed,
                    width: None,
                    vertex_widths: Vec::new(),
                    line_type_generation: false,
                })
            }
            NewShape::Text(t) => {
                if t.content.contains(['\n', '\r']) || !t.rotation.is_finite() {
                    return None;
                }
                let width_factor = match t.width_factor {
                    None => 1.0,
                    Some(wf) if wf.is_finite() && wf > 0.0 => wf,
                    Some(_) => return None,
                };
                let oblique = match t.oblique {
                    None => 0.0,
                    Some(ob) if ob.is_finite() => ob,
                    Some(_) => return None,
                };
                let placement = match t.end {
                    Some((end_xy, is_fit)) => {
                        if t.horizontal.is_some() || t.vertical.is_some() {
                            return None;
                        }
                        let first = model_pt(t.position)?;
                        let second = model_pt(end_xy)?;
                        if first.x == second.x && first.y == second.y {
                            return None;
                        }
                        let turn = (t.rotation
                            - libm::atan2(second.y - first.y, second.x - first.x).to_degrees())
                        .rem_euclid(360.0);
                        if turn.min(360.0 - turn) > 1e-6 {
                            return None;
                        }
                        Placement::Between {
                            first,
                            second,
                            fit: if is_fit { Fit::Fit } else { Fit::Aligned },
                        }
                    }
                    None => Placement::Anchor {
                        point: model_pt(t.position)?,
                        horizontal: t.horizontal.map_or(Horizontal::Left, horizontal_of),
                        vertical: t.vertical.map_or(Vertical::Baseline, vertical_of),
                    },
                };
                Shape::Text(Text {
                    content: t.content.clone(),
                    placement,
                    height: Height {
                        value: positive_len(t.height)?,
                        meaning: HeightMeaning::Declared("dxf.TEXT.40".into()),
                    },
                    rotation: t.rotation,
                    width_factor,
                    oblique,
                    backward: t.backward,
                    upside_down: t.upside_down,
                    style: t.style.clone().unwrap_or_else(|| "Standard".to_owned()),
                })
            }
            NewShape::Symbol(s) => {
                if !s.scale[0].is_finite()
                    || !s.scale[1].is_finite()
                    || s.scale[0] == 0.0
                    || s.scale[1] == 0.0
                    || !s.rotation.is_finite()
                {
                    return None;
                }
                let grid = match s.grid {
                    None => None,
                    Some(g) => {
                        if g.columns == 0 || g.rows == 0 || (g.columns, g.rows) == (1, 1) {
                            return None;
                        }
                        Some(Grid {
                            columns: g.columns,
                            rows: g.rows,
                            column_spacing: model_len(g.column_spacing)?,
                            row_spacing: model_len(g.row_spacing)?,
                        })
                    }
                };
                let position = model_pt(s.position)?;
                let def_opt = self.symbols.iter().find(|def| {
                    def.key.namespace == BLOCK && def.key.name.eq_ignore_ascii_case(s.name.as_str())
                });
                let mut attributes: Vec<Attribute<ModelPoint>> = Vec::new();
                if let Some(def) = def_opt {
                    let non_const: Vec<&Attribute<ModelPoint>> = def
                        .attributes
                        .iter()
                        .filter(|ad| !ad.constant)
                        .map(|ad| &ad.attribute)
                        .collect();
                    if !non_const.is_empty() {
                        if s.scale[0].abs() != s.scale[1].abs() {
                            return None;
                        }
                        let [bx, by] = def.base.map_or([0.0, 0.0], |b| [b.x, b.y]);
                        let radians = s.rotation.to_radians();
                        let (sin, cos) = (libm::sin(radians), libm::cos(radians));
                        let [sx, sy] = s.scale;
                        let [a, b, d, e] = [cos * sx, -sin * sy, sin * sx, cos * sy];
                        let affine = Affine {
                            m: [
                                a,
                                b,
                                position.x - (a * bx + b * by),
                                d,
                                e,
                                position.y - (d * bx + e * by),
                            ],
                        };
                        for attr_def in non_const {
                            if attr_def.formatted.is_some() {
                                return None;
                            }
                            let lifted_def_text = lift_def_text_to_model(&attr_def.text)?;
                            let Shape::Text(placed_text) =
                                transform_shape(&Shape::Text(lifted_def_text), &affine)?
                            else {
                                return None;
                            };
                            let a_layer_name =
                                self.layers.get(attr_def.layer.0 as usize)?.name.clone();
                            attrib_layers.push((attr_def.layer, a_layer_name));
                            if let Inherit::Own(lt_id) = attr_def.style.line_type {
                                let a_lt_name = self.line_types.get(lt_id.0 as usize)?.name.clone();
                                attrib_line_types.push((lt_id, a_lt_name));
                            }
                            attributes.push(Attribute {
                                tag: attr_def.tag.clone(),
                                text: placed_text,
                                formatted: None,
                                hidden: attr_def.hidden,
                                layer: attr_def.layer,
                                style: attr_def.style,
                                source: attr_def.source.clone(),
                            });
                        }
                    }
                }

                Shape::Symbol(Symbol {
                    key: SymbolKey {
                        namespace: Cow::Borrowed(BLOCK),
                        name: s.name.clone(),
                    },
                    position,
                    scale: s.scale,
                    rotation: s.rotation,
                    grid,
                    attributes,
                    clip: None,
                })
            }
        };

        let element = Element {
            source: Ref::default(),
            layer: layer_id,
            style: Style {
                color,
                line_type,
                line_type_scale: 1.0,
                line_weight,
                transparency: Inherit::Layer,
            },
            shape,
        };

        let mut tables = self.tables_cache.clone();
        tables.markers = self.live_markers();
        single::born_model(
            &tables,
            &element,
            resolved_layer_name,
            lt_name,
            &attrib_layers,
            &attrib_line_types,
            handle,
        )
    }
}

fn lift_def_text_to_model(text: &Text<ModelPoint>) -> Option<Text<ModelPoint>> {
    let pt = |p: ModelPoint| ModelPoint {
        space: MODEL,
        x: p.x,
        y: p.y,
    };
    let Length::Model(h) = text.height.value else {
        return None;
    };
    let height_val = positive_len(h.value)?;
    Some(Text {
        content: text.content.clone(),
        placement: match text.placement {
            Placement::Anchor {
                point,
                horizontal,
                vertical,
            } => Placement::Anchor {
                point: pt(point),
                horizontal,
                vertical,
            },
            Placement::Between { first, second, fit } => Placement::Between {
                first: pt(first),
                second: pt(second),
                fit,
            },
        },
        height: Height {
            value: height_val,
            meaning: text.height.meaning.clone(),
        },
        rotation: text.rotation,
        width_factor: text.width_factor,
        oblique: text.oblique,
        backward: text.backward,
        upside_down: text.upside_down,
        style: text.style.clone(),
    })
}
