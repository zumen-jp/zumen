//! tables の op の項目: 図面モデルの表を、境界の項目にする（決定 0162・0165）。
//!
//! 要素が名前で指す文字スタイルと寸法スタイルの ID も、ここで引く（決定 0158）。

use std::collections::BTreeMap;

use zumen_base::codes;
use zumen_base::{Cx, Diagnostic, Kind};
use zumen_drawing::{
    Arrowhead, ColorSystem, DimensionStyle, Drawing2D, Hash, Layer, LineType, LineTypeId,
    LineWeight, MarkContent, PatternBasis, id,
};

use crate::messages::{
    self, DimensionStyleItem, Font, FontKind, LayerItem, LineTypeItem, LineTypeMarkKind,
    TextStyleItem,
};
use crate::session::{Failed, stopped};

/// 文字スタイルの表の項目。定義がなければ、ID・名前・ハッシュだけを返す
/// レイヤの表の項目（決定 0162）。線種は `line_types` の ID で指す
pub(crate) fn layer_item(
    id: String,
    layer: &Layer,
    hash: Hash,
    line_types: &BTreeMap<LineTypeId, String>,
) -> LayerItem {
    let style = layer.style.as_ref();
    LayerItem {
        id,
        name: layer.name.clone(),
        hash: hash.to_string(),
        defined: layer.state.is_some() || layer.style.is_some(),
        visible: layer.state.map(|state| state.visible),
        printable: layer.state.map(|state| state.printable),
        editable: layer.state.map(|state| state.editable),
        frozen: layer.state.map(|state| state.frozen),
        color: style
            .and_then(|style| style.color)
            .map(|color| messages::ColorChange {
                inherit: None,
                aci: color
                    .code
                    .filter(|code| code.system == ColorSystem::Aci)
                    .map(|code| u64::from(code.value)),
                sxf: color
                    .code
                    .filter(|code| code.system == ColorSystem::Sxf)
                    .map(|code| u64::from(code.value)),
                jww: color
                    .code
                    .filter(|code| code.system == ColorSystem::Jww)
                    .map(|code| u64::from(code.value)),
                rgb: color
                    .rgb
                    .map(|rgb| rgb.iter().map(|&value| u64::from(value)).collect()),
            }),
        line_type_id: style
            .and_then(|style| style.line_type)
            .and_then(|line_type| line_types.get(&line_type).cloned()),
        line_weight: style
            .and_then(|style| style.line_weight)
            .map(|weight| match weight {
                LineWeight::Width(width) => messages::LineWeightChange {
                    inherit: None,
                    mm: Some(width.0),
                    default: None,
                },
                LineWeight::Default => messages::LineWeightChange {
                    inherit: None,
                    mm: None,
                    default: Some(true),
                },
            }),
    }
}

pub(crate) fn dimension_style_item(
    id: String,
    style: &DimensionStyle,
    hash: Hash,
) -> DimensionStyleItem {
    DimensionStyleItem {
        id,
        name: style.name.clone(),
        hash: hash.to_string(),
        ends: Some(style.ends.iter().map(arrowhead_name).collect()),
        leader: Some(arrowhead_name(&style.leader)),
        size: Some(style.size),
        tick: Some(style.tick.unwrap_or(0.0)),
        scale: Some(style.scale),
    }
}

/// 端部記号の、ブロックの名前。既定の矢印は空の名前（決定 0165）
fn arrowhead_name(arrow: &Arrowhead) -> String {
    match arrow {
        Arrowhead::Default => String::new(),
        Arrowhead::Symbol(key) => key.name.clone(),
    }
}

pub(crate) fn line_type_item(
    id: String,
    line_type: &LineType,
    hash: Hash,
    style_ids: &StyleIds,
    cx: &mut Cx<'_>,
) -> Result<LineTypeItem, Failed> {
    let definition = line_type.definition.as_ref();
    let mut mark = |mark: &zumen_drawing::LineTypeMark| {
        // 項目の構築と、添える文字・フォントの名前の複製を先に数える。
        let bytes = match &mark.content {
            MarkContent::Shape { font, .. } => font.as_ref().map_or(0, |font| font.name.len()),
            MarkContent::Text { content, style } => content
                .len()
                .saturating_add(style.as_ref().map_or(0, String::len)),
        };
        cx.check(Kind::Work, 1 + bytes as u64 / 1024)
            .map_err(|stop| stopped(cx, stop))?;
        let (kind, font, number, text, style) = match &mark.content {
            MarkContent::Shape { font, number } => (
                LineTypeMarkKind::Shape,
                font.as_ref().map(font_of),
                Some(u64::from(*number)),
                None,
                None,
            ),
            MarkContent::Text { content, style } => (
                LineTypeMarkKind::Text,
                None,
                None,
                Some(content.clone()),
                style.clone(),
            ),
        };
        let (style_id, height) = match &mark.content {
            MarkContent::Text { style, .. } => {
                let (id, fixed) = if let Some(name) = style {
                    // 同じ名前を持つ最初の項目を索引から引く。比較と小文字への変換も数える。
                    let depth = u64::from(style_ids.text.len().max(1).ilog2()) + 1;
                    let length = u64::try_from(name.len())
                        .unwrap_or(u64::MAX)
                        .saturating_add(1);
                    cx.check(Kind::Work, length.saturating_mul(depth).saturating_mul(16))
                        .map_err(|stop| stopped(cx, stop))?;
                    let key = name.to_ascii_lowercase();
                    (
                        style_ids.text.get(&key).cloned(),
                        style_ids.text_heights.get(&key).copied().flatten(),
                    )
                } else {
                    (None, None)
                };
                let height = fixed
                    .filter(|height| *height > 0.0)
                    .map_or(mark.scale, |height| height * mark.scale);
                let height = if height.is_finite() {
                    Some(height)
                } else {
                    cx.report(
                        Diagnostic::new(codes::DRAWING_MARK_HEIGHT_DROPPED)
                            .with("name", line_type.name.clone())
                            .at(line_type.source.clone()),
                    );
                    None
                };
                (id, height)
            }
            MarkContent::Shape { .. } => (None, None),
        };
        Ok(messages::LineTypeMark {
            index: mark.index as u64,
            kind,
            font,
            number,
            text,
            style_id,
            style,
            height,
            scale: mark.scale,
            rotation: mark.rotation,
            rotation_basis: match mark.rotation_basis {
                zumen_drawing::MarkRotation::Relative => messages::MarkRotation::Relative,
                zumen_drawing::MarkRotation::Absolute => messages::MarkRotation::Absolute,
                zumen_drawing::MarkRotation::Upright => messages::MarkRotation::Upright,
            },
            offset: mark.offset.to_vec(),
        })
    };
    Ok(LineTypeItem {
        id,
        name: line_type.name.clone(),
        hash: hash.to_string(),
        defined: definition.is_some(),
        description: definition.map(|d| d.description.clone()),
        pattern: definition.map(|d| d.pattern.clone()),
        paper: definition.map(|d| d.basis == PatternBasis::Paper),
        marked: definition.map(|d| !d.marks.is_empty()),
        marks: definition
            .map(|d| d.marks.iter().map(&mut mark).collect())
            .transpose()?,
    })
}

pub(crate) fn text_style_item(
    id: String,
    style: &zumen_drawing::TextStyle,
    hash: Hash,
) -> TextStyleItem {
    let definition = style.definition.as_ref();
    TextStyleItem {
        id,
        name: style.name.clone(),
        hash: hash.to_string(),
        defined: definition.is_some(),
        font: definition.and_then(|d| d.font.as_ref()).map(font_of),
        big_font: definition.and_then(|d| d.big_font.as_ref()).map(font_of),
        height: definition.and_then(|d| d.height),
        width: definition.map(|d| d.width),
        oblique: definition.map(|d| d.oblique),
        backward: definition.map(|d| d.backward),
        upside_down: definition.map(|d| d.upside_down),
        vertical: definition.map(|d| d.vertical),
    }
}

fn font_of(font: &zumen_drawing::Font) -> Font {
    match font.kind {
        zumen_drawing::FontKind::Shape => Font {
            name: font.name.clone(),
            kind: FontKind::Shape,
            bold: None,
            italic: None,
            charset: None,
            pitch: None,
        },
        zumen_drawing::FontKind::Named {
            bold,
            italic,
            charset,
            pitch,
        } => Font {
            name: font.name.clone(),
            kind: FontKind::Named,
            bold: Some(bold),
            italic: Some(italic),
            charset: charset.map(u64::from),
            pitch: pitch.map(u64::from),
        },
    }
}

/// 同じ表の ID が重なるときは、2 つ目から番号を足す
pub(crate) fn table_id(base: String, seen: &mut BTreeMap<String, u64>) -> String {
    let count = seen.entry(base.clone()).or_default();
    let result = if *count == 0 {
        base
    } else {
        format!("{base}/{count}")
    };
    *count = count.saturating_add(1);
    result
}

/// 要素が名前で指す文字スタイルと寸法スタイルの ID（tables が返すもの）。名前の比較は図面モデルと同じにし、
/// 同じ名前があれば先の項目にする（決定 0158）
#[derive(Default)]
pub(crate) struct StyleIds {
    text: BTreeMap<String, String>,
    text_heights: BTreeMap<String, Option<f64>>,
    dimension: BTreeMap<String, String>,
}

impl StyleIds {
    pub(crate) fn new(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut result = Self::default();
        let mut seen = BTreeMap::new();
        for (index, style) in drawing.dimension_styles.iter().enumerate() {
            let work = style.name.len().saturating_add(1);
            cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                .map_err(|stop| stopped(cx, stop))?;
            let id = table_id(id::dimension_style(style, index), &mut seen);
            result.dimension.entry(style.name.clone()).or_insert(id);
        }
        seen.clear();
        for (index, style) in drawing.text_styles.iter().enumerate() {
            // 2 つの索引は、名前の共通部分を木の高さに応じて繰り返し比べる。
            let depth = u64::from(result.text.len().max(1).ilog2()) + 1;
            let length = u64::try_from(style.name.len())
                .unwrap_or(u64::MAX)
                .saturating_add(1);
            let work = length.saturating_mul(depth).saturating_mul(32);
            cx.check(Kind::Work, work)
                .map_err(|stop| stopped(cx, stop))?;
            let id = table_id(id::text_style(style, index), &mut seen);
            let key = style.name.to_ascii_lowercase();
            result.text_heights.entry(key.clone()).or_insert_with(|| {
                style
                    .definition
                    .as_ref()
                    .and_then(|definition| definition.height)
            });
            result.text.entry(key).or_insert(id);
        }
        Ok(result)
    }

    /// 形の値に、名前で引いたスタイルの ID を書く
    pub(crate) fn fill(&self, geometry: &mut messages::Geometry) {
        let find = |items: &BTreeMap<String, String>, name: &str| {
            items.get(&name.to_ascii_lowercase()).cloned()
        };
        match geometry {
            messages::Geometry::Text(text) => {
                text.style_id = text
                    .style
                    .as_deref()
                    .and_then(|name| find(&self.text, name));
            }
            messages::Geometry::FormattedText(text) => {
                text.style_id = text
                    .style
                    .as_deref()
                    .and_then(|name| find(&self.text, name));
            }
            messages::Geometry::Dimension(dimension) => {
                dimension.style_id = self.dimension.get(&dimension.style).cloned();
            }
            messages::Geometry::Leader(leader) => {
                leader.style_id = self.dimension.get(&leader.style).cloned();
            }
            _ => {}
        }
    }
}
