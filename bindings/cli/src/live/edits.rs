//! 画層・線種の表変更と、折れ線・文字・記号挿入・記号定義の追加、定義内要素の移動。

use std::borrow::Cow;

use zumen_base::{Cx, Diagnostic, Error, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Axes, Color, ColorCode, ColorSystem, Edit, Element, Hash, Height, HeightMeaning,
    Horizontal, Inherit, Layer, LayerId, LayerState, LayerStyle, Length, LineType,
    LineTypeDefinition, LineTypeId, LineWeight, ModelLength, ModelPoint, ModelSpace, PaperLength,
    PaperPoint, PatternBasis, Placed, Placement, Polyline, Seen, Shape, SpaceId, Style, Symbol,
    SymbolDefinition, SymbolKey, Text, Unit, Vertex, Vertical, id,
};
use zumen_live::RowId;

use super::document::{DrawShape, Problem, invert, model_shape};
use super::wire::{self, Words};

/// 表項目の名前・説明の上限（4 KiB）
pub(super) const MAX_NAME_BYTES: usize = 4096;
/// 線種パターンの要素数の上限
pub(super) const MAX_PATTERN_LEN: usize = 256;
/// 記号定義に一度に足す要素数の上限
pub(super) const MAX_SYMBOL_ELEMENTS: usize = 4096;

#[derive(Clone, Debug)]
pub(super) enum LayerOp<'a> {
    Update { layer: u32, patch: LayerPatch<'a> },
    Add { patch: LayerPatch<'a> },
    Remove(u32),
}

#[derive(Clone, Debug)]
pub(super) struct LayerPatch<'a> {
    pub state: Option<LayerState>,
    pub color: Option<Option<Color>>,
    pub line_weight: Option<Option<LineWeight>>,
    pub line_type: Option<Option<u32>>,
    pub name: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub(super) enum LineTypeOp<'a> {
    Update {
        line_type: u32,
        spec: LineTypeSpec<'a>,
    },
    Add(LineTypeSpec<'a>),
    Remove(u32),
}

#[derive(Clone, Debug)]
pub(super) struct LineTypeSpec<'a> {
    pub basis: PatternBasis,
    pub name: &'a str,
    pub description: &'a str,
    pub pattern: Vec<f64>,
}

#[derive(Clone, Debug)]
pub(super) enum AddedShape<'a> {
    Polyline {
        closed: bool,
        vertices: Vec<[f64; 3]>,
    },
    Text {
        point: [f64; 2],
        height: f64,
        rotation: f64,
        width_factor: f64,
        content: &'a str,
    },
    Symbol {
        symbol_index: u32,
        position: [f64; 2],
        scale: [f64; 2],
        rotation: f64,
    },
}

#[derive(Clone, Debug)]
pub(super) struct NewSymbol<'a> {
    pub template: RowId,
    pub name: &'a str,
    pub base: [f64; 2],
    pub elements: Vec<DrawShape>,
}

fn read_padded_str<'a>(input: &mut Words<'a>, length: usize, max: usize) -> Option<&'a str> {
    if length > max {
        return None;
    }
    let padded = length.checked_next_multiple_of(4)?;
    if input.0.len() < padded {
        return None;
    }
    let (raw, rest) = input.0.split_at(padded);
    input.0 = rest;
    let (text, pad) = raw.split_at(length);
    if pad.iter().any(|&b| b != 0) {
        return None;
    }
    std::str::from_utf8(text).ok()
}

fn parse_own_color(flags: u32, code: u32, rgb: u32) -> Option<Option<Color>> {
    if flags == 0 && code == 0 && rgb == 0 {
        return Some(None);
    }
    if !(1..=3).contains(&flags) {
        return None;
    }
    let code_val = if flags & 1 != 0 {
        let system = match code >> 24 {
            1 => ColorSystem::Aci,
            2 => ColorSystem::Sxf,
            3 => ColorSystem::Jww,
            _ => return None,
        };
        Some(ColorCode {
            system,
            value: code & 0x00ff_ffff,
        })
    } else if code == 0 {
        None
    } else {
        return None;
    };
    let rgb_val = if flags & 2 != 0 {
        if rgb > 0x00ff_ffff {
            return None;
        }
        let [_, r, g, b] = rgb.to_be_bytes();
        Some([r, g, b])
    } else if rgb == 0 {
        None
    } else {
        return None;
    };
    Some(Some(Color {
        code: code_val,
        rgb: rgb_val,
    }))
}

pub(super) fn parse_layer_op<'a>(input: &mut Words<'a>) -> Option<LayerOp<'a>> {
    let op = input.u32()?;
    let layer = input.u32()?;
    if op == 3 {
        return input.0.is_empty().then_some(LayerOp::Remove(layer));
    }
    if op != 1 && op != 2 {
        return None;
    }
    if op == 2 && layer != 0 {
        return None;
    }
    let mask = input.u32()?;
    let state_flags = input.u32()?;
    let (color_flags, code, rgb) = (input.u32()?, input.u32()?, input.u32()?);
    let weight_kind = input.u32()?;
    let width = input.f64()?;
    let (lt_kind, lt_value) = (input.u32()?, input.u32()?);
    let name_len = input.u32()? as usize;
    if mask == 0 || mask > 0b1_1111 || (op == 2 && mask & 16 == 0) {
        return None;
    }
    let state = if mask & 1 != 0 {
        if state_flags > 0b1111 {
            return None;
        }
        Some(LayerState {
            visible: state_flags & 1 != 0,
            printable: state_flags & 2 != 0,
            editable: state_flags & 4 != 0,
            frozen: state_flags & 8 != 0,
        })
    } else if state_flags == 0 {
        None
    } else {
        return None;
    };
    let color = if mask & 2 != 0 {
        Some(parse_own_color(color_flags, code, rgb)?)
    } else if color_flags == 0 && code == 0 && rgb == 0 {
        None
    } else {
        return None;
    };
    let line_weight = if mask & 4 != 0 {
        Some(match weight_kind {
            0 if width.to_bits() == 0 => None,
            2 if width.is_finite() && width >= 0.0 => Some(LineWeight::Width(PaperLength(width))),
            3 if width.to_bits() == 0 => Some(LineWeight::Default),
            _ => return None,
        })
    } else if weight_kind == 0 && width.to_bits() == 0 {
        None
    } else {
        return None;
    };
    let line_type = if mask & 8 != 0 {
        Some(match lt_kind {
            0 if lt_value == 0 => None,
            2 => Some(lt_value),
            _ => return None,
        })
    } else if lt_kind == 0 && lt_value == 0 {
        None
    } else {
        return None;
    };
    let name = if mask & 16 != 0 {
        if name_len == 0 {
            return None;
        }
        Some(read_padded_str(input, name_len, MAX_NAME_BYTES)?)
    } else if name_len == 0 {
        None
    } else {
        return None;
    };
    if !input.0.is_empty() {
        return None;
    }
    let patch = LayerPatch {
        state,
        color,
        line_weight,
        line_type,
        name,
    };
    Some(if op == 1 {
        LayerOp::Update { layer, patch }
    } else {
        LayerOp::Add { patch }
    })
}

pub(super) fn parse_line_type_op<'a>(input: &mut Words<'a>) -> Option<LineTypeOp<'a>> {
    let op = input.u32()?;
    let line_type = input.u32()?;
    if op == 3 {
        return input.0.is_empty().then_some(LineTypeOp::Remove(line_type));
    }
    if (op != 1 && op != 2) || (op == 2 && line_type != 0) {
        return None;
    }
    let basis = match input.u32()? {
        0 => PatternBasis::Element,
        1 => PatternBasis::Paper,
        _ => return None,
    };
    let name_len = input.u32()? as usize;
    let desc_len = input.u32()? as usize;
    let pattern_len = input.u32()? as usize;
    if name_len == 0 || pattern_len > MAX_PATTERN_LEN {
        return None;
    }
    let name = read_padded_str(input, name_len, MAX_NAME_BYTES)?;
    let description = read_padded_str(input, desc_len, MAX_NAME_BYTES)?;
    if input.0.len() != pattern_len.checked_mul(8)? {
        return None;
    }
    let mut pattern = Vec::with_capacity(pattern_len);
    for _ in 0..pattern_len {
        let val = input.f64()?;
        if !val.is_finite() {
            return None;
        }
        pattern.push(val);
    }
    let spec = LineTypeSpec {
        basis,
        name,
        description,
        pattern,
    };
    Some(if op == 1 {
        LineTypeOp::Update { line_type, spec }
    } else {
        LineTypeOp::Add(spec)
    })
}

pub(super) fn parse_add_element<'a>(input: &mut Words<'a>) -> Option<(RowId, AddedShape<'a>)> {
    let template = RowId(input.u32()?);
    let kind = input.u32()?;
    match kind {
        1 => {
            let closed = match input.u32()? {
                0 => false,
                1 => true,
                _ => return None,
            };
            let count = input.u32()? as usize;
            if !(2..=wire::MAX_ROWS).contains(&count) || input.0.len() != count.checked_mul(24)? {
                return None;
            }
            let mut vertices = Vec::with_capacity(count);
            for _ in 0..count {
                let (x, y, bulge) = (input.f64()?, input.f64()?, input.f64()?);
                if !(x.is_finite() && y.is_finite() && bulge.is_finite()) {
                    return None;
                }
                vertices.push([x, y, bulge]);
            }
            Some((template, AddedShape::Polyline { closed, vertices }))
        }
        2 => {
            let len = input.u32()? as usize;
            if input.u32()? != 0 {
                return None;
            }
            let (x, y, height, rotation, width_factor) = (
                input.f64()?,
                input.f64()?,
                input.f64()?,
                input.f64()?,
                input.f64()?,
            );
            if !(x.is_finite()
                && y.is_finite()
                && height.is_finite()
                && height > 0.0
                && rotation.is_finite()
                && width_factor.is_finite()
                && width_factor > 0.0)
            {
                return None;
            }
            let content = read_padded_str(input, len, wire::MAX_TEXT_BYTES)?;
            if !input.0.is_empty() || content.contains(['\n', '\r']) {
                return None;
            }
            Some((
                template,
                AddedShape::Text {
                    point: [x, y],
                    height,
                    rotation,
                    width_factor,
                    content,
                },
            ))
        }
        3 => {
            let symbol_index = input.u32()?;
            if input.u32()? != 0 || symbol_index == u32::MAX {
                return None;
            }
            let (x, y, sx, sy, rotation) = (
                input.f64()?,
                input.f64()?,
                input.f64()?,
                input.f64()?,
                input.f64()?,
            );
            if !input.0.is_empty()
                || !(x.is_finite()
                    && y.is_finite()
                    && sx.is_finite()
                    && sx != 0.0
                    && sy.is_finite()
                    && sy != 0.0
                    && rotation.is_finite())
            {
                return None;
            }
            Some((
                template,
                AddedShape::Symbol {
                    symbol_index,
                    position: [x, y],
                    scale: [sx, sy],
                    rotation,
                },
            ))
        }
        _ => None,
    }
}

pub(super) fn parse_add_symbol<'a>(input: &mut Words<'a>) -> Option<NewSymbol<'a>> {
    let template = RowId(input.u32()?);
    let name_len = input.u32()? as usize;
    let elem_count = input.u32()? as usize;
    if input.u32()? != 0 || name_len == 0 || elem_count > MAX_SYMBOL_ELEMENTS {
        return None;
    }
    let (bx, by) = (input.f64()?, input.f64()?);
    if !(bx.is_finite() && by.is_finite()) {
        return None;
    }
    let name = read_padded_str(input, name_len, MAX_NAME_BYTES)?;
    if input.0.len() != elem_count.checked_mul(48)? {
        return None;
    }
    let mut elements = Vec::with_capacity(elem_count);
    for _ in 0..elem_count {
        let kind = input.u32()?;
        if input.u32()? != 0 {
            return None;
        }
        let mut v = [0_f64; 5];
        for item in &mut v {
            *item = input.f64()?;
        }
        if v.iter().any(|val| !val.is_finite()) {
            return None;
        }
        let shape = match kind {
            1 if v[4] == 0.0 => DrawShape::Line([v[0], v[1], v[2], v[3]]),
            2 if v[3] == 0.0 && v[4] == 0.0 && v[2] > 0.0 => DrawShape::Circle([v[0], v[1], v[2]]),
            3 if v[2] > 0.0 => DrawShape::Arc(v),
            _ => return None,
        };
        elements.push(shape);
    }
    Some(NewSymbol {
        template,
        name,
        base: [bx, by],
        elements,
    })
}

pub(super) fn build_layer_edit(
    op: LayerOp<'_>,
    layers: &[(u32, Layer)],
    line_types: &[LineType],
    cx: &mut Cx<'_>,
) -> Result<Edit, Problem> {
    let resolve_lt = |lt: Option<Option<u32>>| -> Result<Option<Option<LineTypeId>>, Problem> {
        Ok(match lt {
            None => None,
            Some(None) => Some(None),
            Some(Some(val)) => Some(Some(
                line_types
                    .iter()
                    .find(|item| item.id.0 == val)
                    .ok_or(Problem::Core(Error::Invalid))?
                    .id,
            )),
        })
    };
    match op {
        LayerOp::Remove(value) => {
            let current = &layers
                .iter()
                .find(|(slot, _)| *slot == value)
                .ok_or(Problem::Missing(RowId(value)))?
                .1;
            let hash = Hash::layer_checked(current, cx).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            })?;
            Ok(Edit::RemoveLayer {
                target: Seen {
                    id: id::layer(current),
                    hash,
                },
            })
        }
        LayerOp::Update { layer, patch } => {
            let current = &layers
                .iter()
                .find(|(slot, _)| *slot == layer)
                .ok_or(Problem::Missing(RowId(layer)))?
                .1;
            let resolved_lt = resolve_lt(patch.line_type)?;
            let hash = Hash::layer_checked(current, cx).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            })?;
            let mut next = current.clone();
            if let Some(name) = patch.name {
                cx.check(Kind::DecodedBytes, name.len() as u64)
                    .map_err(|stop| {
                        cx.report(stop.to_diagnostic());
                        Problem::Core(Error::Stopped(stop))
                    })?;
                name.clone_into(&mut next.name);
            }
            if let Some(state) = patch.state {
                next.state = Some(state);
            } else if next.state.is_none() {
                next.state = Some(LayerState {
                    visible: true,
                    printable: true,
                    editable: true,
                    frozen: false,
                });
            }
            let mut style = next.style.unwrap_or(LayerStyle {
                color: None,
                line_type: None,
                line_weight: None,
                transparency: None,
            });
            if let Some(mut color) = patch.color {
                if let Some(c) = &mut color
                    && c.code.is_none()
                {
                    c.code = style.color.and_then(|old| old.code);
                }
                style.color = color;
            }
            if let Some(weight) = patch.line_weight {
                style.line_weight = weight;
            }
            if let Some(lt) = resolved_lt {
                style.line_type = lt;
            }
            next.style = Some(style);
            Ok(Edit::ReplaceLayer {
                target: Seen {
                    id: id::layer(current),
                    hash,
                },
                layer: Box::new(next),
            })
        }
        LayerOp::Add { patch } => {
            let resolved_lt = resolve_lt(patch.line_type)?;
            let name = patch.name.ok_or(Problem::Core(Error::Invalid))?;
            cx.check(Kind::DecodedBytes, name.len() as u64 + 128)
                .map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Problem::Core(Error::Stopped(stop))
                })?;
            let base_layer = layers.first().map(|(_, layer)| layer);
            let base_style = base_layer.and_then(|layer| layer.style);
            let state = patch
                .state
                .or_else(|| base_layer.and_then(|layer| layer.state))
                .unwrap_or(LayerState {
                    visible: true,
                    printable: true,
                    editable: true,
                    frozen: false,
                });
            let color = match patch.color {
                Some(Some(mut c)) => {
                    if c.code.is_none() {
                        c.code = base_style.and_then(|s| s.color).and_then(|old| old.code);
                    }
                    Some(c)
                }
                Some(None) => None,
                None => base_style.and_then(|s| s.color),
            };
            let style = LayerStyle {
                color,
                line_type: resolved_lt.unwrap_or_else(|| base_style.and_then(|s| s.line_type)),
                line_weight: patch
                    .line_weight
                    .unwrap_or_else(|| base_style.and_then(|s| s.line_weight)),
                transparency: None,
            };
            Ok(Edit::AddLayer {
                layer: Box::new(Layer {
                    id: LayerId(0),
                    name: name.to_owned(),
                    state: Some(state),
                    style: Some(style),
                    follows_placement: false,
                    description: None,
                    source: Ref::default(),
                }),
            })
        }
    }
}

pub(super) fn build_line_type_edit(
    op: LineTypeOp<'_>,
    line_types: &[LineType],
    cx: &mut Cx<'_>,
) -> Result<Edit, Problem> {
    match op {
        LineTypeOp::Remove(value) => {
            let current = line_types
                .iter()
                .find(|lt| lt.id.0 == value)
                .ok_or(Problem::Missing(RowId(value)))?;
            let hash = Hash::line_type_checked(current, cx).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            })?;
            Ok(Edit::RemoveLineType {
                target: Seen {
                    id: id::line_type(current),
                    hash,
                },
            })
        }
        LineTypeOp::Update { line_type, spec } => {
            let current = line_types
                .iter()
                .find(|lt| lt.id.0 == line_type)
                .ok_or(Problem::Missing(RowId(line_type)))?;
            let hash = Hash::line_type_checked(current, cx).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            })?;
            let alloc = (spec.name.len() + spec.description.len()) as u64
                + (spec.pattern.len() as u64).saturating_mul(8);
            cx.check(Kind::DecodedBytes, alloc).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            })?;
            let marks = current
                .definition
                .as_ref()
                .map_or_else(Vec::new, |def| def.marks.clone());
            Ok(Edit::ReplaceLineType {
                target: Seen {
                    id: id::line_type(current),
                    hash,
                },
                line_type: Box::new(LineType {
                    id: current.id,
                    name: spec.name.to_owned(),
                    definition: Some(LineTypeDefinition {
                        description: spec.description.to_owned(),
                        pattern: spec.pattern,
                        basis: spec.basis,
                        marks,
                    }),
                    source: current.source.clone(),
                }),
            })
        }
        LineTypeOp::Add(spec) => {
            let alloc = (spec.name.len() + spec.description.len()) as u64
                + (spec.pattern.len() as u64).saturating_mul(8)
                + 128;
            cx.check(Kind::DecodedBytes, alloc).map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            })?;
            Ok(Edit::AddLineType {
                line_type: Box::new(LineType {
                    id: LineTypeId(0),
                    name: spec.name.to_owned(),
                    definition: Some(LineTypeDefinition {
                        description: spec.description.to_owned(),
                        pattern: spec.pattern,
                        basis: spec.basis,
                        marks: Vec::new(),
                    }),
                    source: Ref::default(),
                }),
            })
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FormatFlavor {
    Acad,
    Jww,
    Sxf,
}

pub(super) fn build_added_placed(
    template: Placed,
    sheet: Affine,
    shape: AddedShape<'_>,
    symbol_key: Option<SymbolKey>,
    flavor: FormatFlavor,
    cx: &mut Cx<'_>,
) -> Result<Option<Placed>, Problem> {
    let Some(inverse) = invert(sheet) else {
        return Ok(None);
    };
    let [ia, ib, ic, id_m, ie, iff] = inverse.m;
    let map_pt = |[x, y]: [f64; 2]| -> Option<(f64, f64)> {
        let (px, py) = (ia * x + ib * y + ic, id_m * x + ie * y + iff);
        (px.is_finite() && py.is_finite()).then_some((px, py))
    };
    let [sa, sb, _, sd, se, _] = sheet.m;
    let uniform_positive = sb == 0.0 && sd == 0.0 && sa == se && sa > 0.0;

    match shape {
        AddedShape::Polyline { closed, vertices } => {
            let has_bulge = vertices.iter().any(|v| v[2] != 0.0);
            if has_bulge && !uniform_positive {
                return Ok(None);
            }
            cx.check(
                Kind::DecodedBytes,
                (vertices.len() as u64)
                    .saturating_mul(32)
                    .saturating_add(64),
            )
            .and_then(|()| cx.check(Kind::Work, vertices.len() as u64))
            .map_err(|stop| {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            })?;
            Ok(Some(match template {
                Placed::Model(el) => {
                    let Shape::Line { start, .. } = el.shape else {
                        return Ok(None);
                    };
                    let space = start.space;
                    let mut out = Vec::with_capacity(vertices.len());
                    for [x, y, bulge] in vertices {
                        let Some((px, py)) = map_pt([x, y]) else {
                            return Ok(None);
                        };
                        out.push(Vertex {
                            point: ModelPoint {
                                space,
                                x: px,
                                y: py,
                            },
                            bulge,
                        });
                    }
                    Placed::Model(Element {
                        shape: Shape::Polyline(Polyline {
                            vertices: out,
                            closed,
                            width: None,
                            vertex_widths: Vec::new(),
                            line_type_generation: false,
                        }),
                        ..el
                    })
                }
                Placed::Paper(el) => {
                    let Shape::Line { start, .. } = el.shape else {
                        return Ok(None);
                    };
                    let sheet_id = start.sheet;
                    let mut out = Vec::with_capacity(vertices.len());
                    for [x, y, bulge] in vertices {
                        let Some((px, py)) = map_pt([x, y]) else {
                            return Ok(None);
                        };
                        out.push(Vertex {
                            point: PaperPoint {
                                sheet: sheet_id,
                                x: px,
                                y: py,
                            },
                            bulge,
                        });
                    }
                    Placed::Paper(Element {
                        shape: Shape::Polyline(Polyline {
                            vertices: out,
                            closed,
                            width: None,
                            vertex_widths: Vec::new(),
                            line_type_generation: false,
                        }),
                        ..el
                    })
                }
            }))
        }
        AddedShape::Text {
            point,
            height,
            rotation,
            width_factor,
            content,
        } => {
            if !uniform_positive {
                return Ok(None);
            }
            let Some((px, py)) = map_pt(point) else {
                return Ok(None);
            };
            let mapped_h = height * ia;
            if !(mapped_h.is_finite() && mapped_h > 0.0) {
                return Ok(None);
            }
            cx.check(Kind::DecodedBytes, content.len() as u64 + 128)
                .map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Problem::Core(Error::Stopped(stop))
                })?;
            let (meaning, vertical, style_name) = match flavor {
                FormatFlavor::Acad => (
                    HeightMeaning::Declared(Cow::Borrowed("dxf.TEXT.40")),
                    Vertical::Baseline,
                    "Standard",
                ),
                FormatFlavor::Jww => (
                    HeightMeaning::Declared(Cow::Borrowed("jww.CDataMoji.height")),
                    Vertical::Bottom,
                    "ＭＳ ゴシック",
                ),
                FormatFlavor::Sxf => (HeightMeaning::Frame, Vertical::Bottom, "ＭＳ ゴシック"),
            };
            let adjust_style = |mut st: Style| -> Style {
                if flavor == FormatFlavor::Sxf {
                    st.line_weight = None;
                    st.line_type = Inherit::Layer;
                }
                st
            };
            Ok(Some(match template {
                Placed::Model(el) => {
                    let Shape::Line { start, .. } = el.shape else {
                        return Ok(None);
                    };
                    let space = start.space;
                    Placed::Model(Element {
                        style: adjust_style(el.style),
                        shape: Shape::Text(Text {
                            content: content.to_owned(),
                            height: Height {
                                value: Length::Model(ModelLength {
                                    space,
                                    value: mapped_h,
                                }),
                                meaning,
                            },
                            placement: Placement::Anchor {
                                point: ModelPoint {
                                    space,
                                    x: px,
                                    y: py,
                                },
                                horizontal: Horizontal::Left,
                                vertical,
                            },
                            rotation,
                            width_factor,
                            oblique: 0.0,
                            backward: false,
                            upside_down: false,
                            style: style_name.to_owned(),
                        }),
                        ..el
                    })
                }
                Placed::Paper(el) => {
                    let Shape::Line { start, .. } = el.shape else {
                        return Ok(None);
                    };
                    let sheet_id = start.sheet;
                    Placed::Paper(Element {
                        style: adjust_style(el.style),
                        shape: Shape::Text(Text {
                            content: content.to_owned(),
                            height: Height {
                                value: Length::Paper(PaperLength(mapped_h)),
                                meaning,
                            },
                            placement: Placement::Anchor {
                                point: PaperPoint {
                                    sheet: sheet_id,
                                    x: px,
                                    y: py,
                                },
                                horizontal: Horizontal::Left,
                                vertical,
                            },
                            rotation,
                            width_factor,
                            oblique: 0.0,
                            backward: false,
                            upside_down: false,
                            style: style_name.to_owned(),
                        }),
                        ..el
                    })
                }
            }))
        }
        AddedShape::Symbol {
            position,
            scale,
            rotation,
            ..
        } => {
            if !uniform_positive {
                return Ok(None);
            }
            let Some(key) = symbol_key else {
                return Err(Problem::Core(Error::Invalid));
            };
            let Some((px, py)) = map_pt(position) else {
                return Ok(None);
            };
            let mapped_scale = match flavor {
                // JWW の定義の座標は用紙 mm 基準で、モデルの記号尺度は空間の縮尺 (ia) を掛けた値を持つ
                FormatFlavor::Jww => [scale[0] * ia, scale[1] * ia],
                // Acad・SXF は挿入先の空間（または用紙）の尺度をそのまま持つ
                FormatFlavor::Acad | FormatFlavor::Sxf => scale,
            };
            if !(mapped_scale[0].is_finite()
                && mapped_scale[0] != 0.0
                && mapped_scale[1].is_finite()
                && mapped_scale[1] != 0.0)
            {
                return Ok(None);
            }
            let adjust_sym_style = |mut st: Style| -> Style {
                if flavor == FormatFlavor::Jww {
                    st.color = Inherit::Block;
                    st.line_type = Inherit::Block;
                    st.line_weight = None;
                } else if flavor == FormatFlavor::Sxf {
                    st.line_weight = None;
                }
                st
            };
            cx.check(Kind::DecodedBytes, key.name.len() as u64 + 128)
                .map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Problem::Core(Error::Stopped(stop))
                })?;
            Ok(Some(match template {
                Placed::Model(el) => {
                    let Shape::Line { start, .. } = el.shape else {
                        return Ok(None);
                    };
                    let space = start.space;
                    Placed::Model(Element {
                        style: adjust_sym_style(el.style),
                        shape: Shape::Symbol(Symbol {
                            key,
                            position: ModelPoint {
                                space,
                                x: px,
                                y: py,
                            },
                            scale: mapped_scale,
                            rotation,
                            grid: None,
                            attributes: Vec::new(),
                            clip: None,
                        }),
                        ..el
                    })
                }
                Placed::Paper(el) => {
                    let Shape::Line { start, .. } = el.shape else {
                        return Ok(None);
                    };
                    let sheet_id = start.sheet;
                    Placed::Paper(Element {
                        style: adjust_sym_style(el.style),
                        shape: Shape::Symbol(Symbol {
                            key,
                            position: PaperPoint {
                                sheet: sheet_id,
                                x: px,
                                y: py,
                            },
                            scale: mapped_scale,
                            rotation,
                            grid: None,
                            attributes: Vec::new(),
                            clip: None,
                        }),
                        ..el
                    })
                }
            }))
        }
    }
}

pub(super) fn build_symbol_definition(
    template: Placed,
    symbol_space_id: u32,
    spec: NewSymbol<'_>,
    cx: &mut Cx<'_>,
) -> Result<Edit, Problem> {
    let (layer, style) = match template {
        Placed::Model(el) => (el.layer, el.style),
        Placed::Paper(el) => (el.layer, el.style),
    };
    let alloc = spec.name.len() as u64 + (spec.elements.len() as u64).saturating_mul(160) + 256;
    cx.check(Kind::DecodedBytes, alloc)
        .and_then(|()| cx.check(Kind::Work, spec.elements.len() as u64 + 1))
        .map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Problem::Core(Error::Stopped(stop))
        })?;
    let space = SpaceId(symbol_space_id);
    let mut elements = Vec::with_capacity(spec.elements.len());
    for item in spec.elements {
        elements.push(Element {
            source: Ref::default(),
            layer,
            style,
            shape: model_shape(
                item,
                |x, y| ModelPoint { space, x, y },
                |value| Length::Model(ModelLength { space, value }),
            ),
        });
    }
    let count = elements.len();
    Ok(Edit::AddSymbol {
        symbol: Box::new(SymbolDefinition {
            key: SymbolKey {
                namespace: Cow::Borrowed("dxf.block"),
                name: spec.name.to_owned(),
            },
            space: ModelSpace {
                id: space,
                unit: Unit::Unknown,
                axes: Axes::EastNorth,
                elements,
                order: (0..count).collect(),
            },
            base: Some(ModelPoint {
                space,
                x: spec.base[0],
                y: spec.base[1],
            }),
            attributes: Vec::new(),
            anonymous: false,
            external: None,
            source: Ref::default(),
        }),
    })
}

pub(super) fn reject_add(kind: &'static str, format: &'static str, cx: &mut Cx<'_>) -> Problem {
    cx.report(
        Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
            .with("edit", "add")
            .with("kind", kind)
            .with("format", format),
    );
    Problem::Core(Error::Rejected)
}
