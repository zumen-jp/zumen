//! 形式間の書き出しで、基本図形の点・長さと継承した見え方を移す部品。
//! 書き手が対応範囲と忠実度を決め、対応しない形は None で返す。

use crate::{
    Color, Drawing2D, Edge, Element, Height, Inherit, Length, LineWeight, Palette, PatternBasis,
    Placement, Point, Polyline, Shape, Text, Vertex,
};
use zumen_base::{Cx, Error, Kind};
/// 指定の描画順を一度ずつたどり、未列挙の要素を入力順で末尾に足す。
/// 範囲外・重複・未列挙の有無も返し、書き手が診断する。
pub fn order(count: usize, order: &[usize], cx: &mut Cx<'_>) -> Result<(Vec<usize>, bool), Error> {
    cx.check(
        Kind::Work,
        (count as u64).saturating_add(order.len() as u64),
    )?;
    cx.check(Kind::DecodedBytes, (count as u64).saturating_mul(16))?;
    let mut seen = vec![false; count];
    let mut out = Vec::with_capacity(count);
    let mut changed = false;
    for &index in order {
        if let Some(slot) = seen.get_mut(index)
            && !*slot
        {
            *slot = true;
            out.push(index);
        } else {
            changed = true;
        }
    }
    for (index, seen) in seen.iter().enumerate() {
        if !seen {
            out.push(index);
            changed |= !order.is_empty();
        }
    }
    Ok((out, changed))
}

/// 形の種類。変換の集計と診断で使う。
pub fn kind<P>(shape: &Shape<P>) -> &'static str {
    match shape {
        Shape::Line { .. } => "line",
        Shape::Circle { .. } => "circle",
        Shape::Curve(Edge::Line { .. }) => "line",
        Shape::Curve(Edge::Arc { .. }) => "arc",
        Shape::Curve(Edge::Elliptic { .. }) => "ellipse",
        Shape::Curve(Edge::Spline(_)) => "spline",
        Shape::Curve(Edge::Clothoid { .. }) => "clothoid",
        Shape::Polyline(_) => "polyline",
        Shape::Text(_) => "text",
        Shape::Formatted(_) => "formatted",
        Shape::Dimension(_) => "dimension",
        Shape::Leader(_) => "leader",
        Shape::Callout(_) => "callout",
        Shape::Table(_) => "table",
        Shape::Underlay(_) => "underlay",
        Shape::Symbol(_) => "symbol",
        Shape::Hatch(_) => "hatch",
        Shape::Point(_) => "point",
    }
}

/// 基本図形の点と長さを写す。複製の確保・走査を先に数える。
/// 複合図形・スプラインなどはこの部品の対象にせず、呼び出し元が診断する。
pub fn basic<P: Copy, Q>(
    shape: &Shape<P>,
    point: &impl Fn(P) -> Option<Q>,
    length: &impl Fn(Length) -> Option<Length>,
    cx: &mut Cx<'_>,
) -> Result<Option<Shape<Q>>, Error> {
    let size = match shape {
        Shape::Text(text) => (text.content.len() as u64)
            .saturating_add(text.style.len() as u64)
            .saturating_add(match &text.height.meaning {
                crate::HeightMeaning::Declared(key) => key.len() as u64,
                _ => 0,
            })
            .saturating_mul(4),
        Shape::Point(dot) => match &dot.look {
            crate::PointLook::Marker { key, .. } => (key.namespace.len() as u64)
                .saturating_add(key.code.len() as u64)
                .saturating_mul(4),
            crate::PointLook::Document => 0,
        },
        Shape::Polyline(poly) => (poly.vertices.len() as u64)
            .saturating_mul(64)
            .saturating_add((poly.vertex_widths.len() as u64).saturating_mul(32)),
        Shape::Line { .. }
        | Shape::Circle { .. }
        | Shape::Curve(Edge::Line { .. } | Edge::Arc { .. } | Edge::Elliptic { .. }) => 256,
        _ => return Ok(None),
    }
    .saturating_add(512);
    cx.check(Kind::Work, size)?;
    cx.check(Kind::DecodedBytes, size)?;
    Ok((|| {
        Some(match shape {
            Shape::Line { start, end } | Shape::Curve(Edge::Line { start, end }) => Shape::Line {
                start: point(*start)?,
                end: point(*end)?,
            },
            Shape::Circle { center, radius } => Shape::Circle {
                center: point(*center)?,
                radius: length(*radius)?,
            },
            Shape::Curve(Edge::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            }) => Shape::Curve(Edge::Arc {
                center: point(*center)?,
                radius: length(*radius)?,
                start: *start,
                end: *end,
                counterclockwise: *counterclockwise,
            }),
            Shape::Curve(Edge::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            }) => Shape::Curve(Edge::Elliptic {
                center: point(*center)?,
                major: point(*major)?,
                ratio: *ratio,
                start: *start,
                end: *end,
                counterclockwise: *counterclockwise,
            }),
            Shape::Polyline(poly) => Shape::Polyline(Polyline {
                vertices: poly
                    .vertices
                    .iter()
                    .map(|v| {
                        Some(Vertex {
                            point: point(v.point)?,
                            bulge: v.bulge,
                        })
                    })
                    .collect::<Option<_>>()?,
                closed: poly.closed,
                width: match poly.width {
                    Some(v) => Some(length(v)?),
                    None => None,
                },
                vertex_widths: poly
                    .vertex_widths
                    .iter()
                    .map(|v| Some([length(v[0])?, length(v[1])?]))
                    .collect::<Option<_>>()?,
                line_type_generation: poly.line_type_generation,
            }),
            Shape::Text(text) => Shape::Text(Text {
                content: text.content.clone(),
                height: Height {
                    value: length(text.height.value)?,
                    meaning: text.height.meaning.clone(),
                },
                placement: match text.placement {
                    Placement::Anchor {
                        point: p,
                        horizontal,
                        vertical,
                    } => Placement::Anchor {
                        point: point(p)?,
                        horizontal,
                        vertical,
                    },
                    Placement::Between { first, second, fit } => Placement::Between {
                        first: point(first)?,
                        second: point(second)?,
                        fit,
                    },
                },
                rotation: text.rotation,
                width_factor: text.width_factor,
                oblique: text.oblique,
                backward: text.backward,
                upside_down: text.upside_down,
                style: text.style.clone(),
            }),
            Shape::Point(dot) => Shape::Point(Point {
                position: point(dot.position)?,
                rotation: dot.rotation,
                look: dot.look.clone(),
                construction: dot.construction,
            }),
            _ => return None,
        })
    })())
}

/// 継承を解いた色・線幅・単純な線種。代替した値のキーも返す。
pub struct Appearance {
    /// 図面の RGB を優先し、符号は CAD 色表 v1 で解く。
    pub rgb: [u8; 3],
    /// 用紙の線幅（mm）。
    pub width: f64,
    /// 線と間を正負で交互に持つ。空なら実線。
    pub dashes: Vec<f64>,
    /// 代替値にした項目。
    pub reasons: Vec<&'static str>,
}

/// 色は黒、未定義の幅は 0.25 mm、解けない線種は実線で代替する。
/// 値を黙って変えないため、呼び出し元は reasons の診断と忠実度を必ず記録する。
pub fn appearance<P>(
    drawing: &Drawing2D,
    element: &Element<P>,
    model_scale: f64,
    cx: &mut Cx<'_>,
) -> Result<Appearance, Error> {
    cx.check(
        Kind::Work,
        1 + drawing.layers.len() as u64 * 4 + drawing.line_types.len() as u64,
    )?;
    cx.check(Kind::DecodedBytes, 512)?;
    let mut reasons = Vec::new();
    if !matches!(element.style.color, Inherit::Own(_))
        || !matches!(element.style.line_type, Inherit::Own(_))
        || matches!(
            element.style.line_weight,
            Some(Inherit::Layer | Inherit::Block | Inherit::Own(LineWeight::Default))
        )
    {
        reasons.push("style_inheritance");
    }
    let own_color = match drawing.color_of(element) {
        Inherit::Own(value) => Some(value),
        _ => drawing.default_color,
    };
    let rgb = own_color
        .and_then(|Color { rgb, code }| {
            rgb.or_else(|| code.and_then(|code| Palette::cad().get(code.system, code.value)))
        })
        .unwrap_or_else(|| {
            reasons.push("color_default");
            [0; 3]
        });
    let width = match drawing.line_weight_of(element) {
        Some(Inherit::Own(LineWeight::Width(value))) => Some(value.0),
        _ => drawing.default_line_weight.map(|v| v.0),
    }
    .filter(|v| v.is_finite() && *v > 0.0)
    .unwrap_or_else(|| {
        reasons.push("line_weight_default");
        0.25
    });
    let line = match drawing.line_type_of(element) {
        Inherit::Own(id) => Some(id),
        _ => drawing.default_line_type,
    }
    .and_then(|id| drawing.line_type(id));
    let mut dashes = Vec::new();
    match line.and_then(|line| line.definition.as_ref()) {
        Some(def) if def.marks.is_empty() => {
            cx.check(Kind::Work, def.pattern.len() as u64)?;
            cx.check(
                Kind::DecodedBytes,
                (def.pattern.len() as u64).saturating_mul(16),
            )?;
            let factor = match def.basis {
                PatternBasis::Paper => 1.0,
                PatternBasis::Element => {
                    model_scale
                        * element.style.line_type_scale
                        * drawing.line_type_scale.unwrap_or(1.0)
                }
            };
            let valid = factor.is_finite()
                && factor > 0.0
                && def.pattern.len() % 2 == 0
                && def.pattern.iter().enumerate().all(|(i, v)| {
                    let v = *v * factor;
                    v.is_finite() && if i % 2 == 0 { v >= 0.0 } else { v < 0.0 }
                });
            if valid {
                dashes.extend(def.pattern.iter().map(|v| *v * factor));
            } else {
                reasons.push("line_type");
            }
        }
        None if line.is_some_and(|line| {
            line.name.eq_ignore_ascii_case("continuous") || line.name == "1"
        }) => {}
        _ => reasons.push("line_type"),
    }
    let opacity = match element.style.transparency {
        Inherit::Own(value) => Some(value.alpha),
        Inherit::Layer => drawing.layer(element.layer).and_then(|layer| {
            layer
                .style
                .and_then(|style| style.transparency.map(|v| v.alpha))
        }),
        Inherit::Block => {
            reasons.push("transparency");
            None
        }
    };
    if opacity.is_some_and(|alpha| alpha != 255) {
        reasons.push("transparency");
    }
    Ok(Appearance {
        rgb,
        width,
        dashes,
        reasons,
    })
}
