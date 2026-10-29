//! 形の値（`geometry`）と図面モデルの形の行き来（決定 0139）。
//!
//! 線分・円・円弧・楕円・幅を持たない折れ線・1 点に置く文字・書式つきの文字だけを扱う。点は要素の空間か用紙の座標、長さは同じ
//! 単位、角度は度で反時計回り。円弧と楕円は、反時計回りにたどる形で返す。

use std::cell::Cell;

use zumen_drawing::transform::Point;
use zumen_drawing::{
    Edge, Flow, FormattedText, HeightMeaning, Horizontal, Length, LineSpacing, ModelLength,
    ModelPoint, PaperLength, PaperPoint, Placement, Polyline, Rotation, Shape, Text, Vertex,
    Vertical,
};

use crate::bounds::Planar;
use crate::messages::{
    ArcGeometry, Attachment, CircleGeometry, EllipseGeometry, FormattedTextGeometry, Geometry,
    LineGeometry, PolylineGeometry, TextGeometry,
};

/// 形の値と行き来できる点
pub(crate) trait Spot: Planar + Point {
    /// 用紙の点。用紙の長さは図面の単位から mm に直したもので、直す倍率は書き戻す側だけが知る
    const PAPER: bool;

    /// この点と同じ座標の系の、値が `value` の長さ
    fn length_of(self, value: f64) -> Length;
}

impl Spot for ModelPoint {
    const PAPER: bool = false;

    fn length_of(self, value: f64) -> Length {
        Length::Model(ModelLength {
            space: self.space,
            value,
        })
    }
}

impl Spot for PaperPoint {
    const PAPER: bool = true;

    fn length_of(self, value: f64) -> Length {
        Length::Paper(PaperLength(value))
    }
}

/// 形の値。扱わない形と、点と違う座標の系の長さを持つ形は `None`
pub(crate) fn geometry_of<P: Spot>(shape: &Shape<P>) -> Option<Geometry> {
    let xy = |point: P| Point::xy(point).to_vec();
    Some(match shape {
        Shape::Line { start, end } => Geometry::Line(LineGeometry {
            start: xy(*start),
            end: xy(*end),
        }),
        Shape::Circle { center, radius } => Geometry::Circle(CircleGeometry {
            center: xy(*center),
            radius: center.length(*radius)?,
        }),
        Shape::Curve(Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        }) => {
            let (start, end) = if *counterclockwise {
                (*start, *end)
            } else {
                (*end, *start)
            };
            Geometry::Arc(ArcGeometry {
                center: xy(*center),
                radius: center.length(*radius)?,
                start_angle: start,
                end_angle: end,
            })
        }
        Shape::Curve(Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        }) => {
            // 時計回りの弧は、同じ点をたどる反時計回りの弧にする
            let (start, end) = if *counterclockwise {
                (*start, *end)
            } else {
                (*end, *start)
            };
            Geometry::Ellipse(EllipseGeometry {
                center: xy(*center),
                major: xy(*major),
                ratio: *ratio,
                start_angle: start,
                end_angle: end,
            })
        }
        Shape::Polyline(line) if line.width.is_none() && line.vertex_widths.is_empty() => {
            let curved = line.vertices.iter().any(|v| v.bulge != 0.0);
            Geometry::Polyline(PolylineGeometry {
                points: line.vertices.iter().map(|v| xy(v.point)).collect(),
                bulges: curved.then(|| line.vertices.iter().map(|v| v.bulge).collect()),
                closed: line.closed,
            })
        }
        Shape::Text(text) => match text.placement {
            Placement::Anchor { point, .. } => Geometry::Text(TextGeometry {
                content: text.content.clone(),
                position: xy(point),
                height: point.length(text.height.value)?,
                rotation: text.rotation,
                style: Some(text.style.clone()),
            }),
            Placement::Between { .. } => return None,
        },
        Shape::Formatted(text) => Geometry::FormattedText(FormattedTextGeometry {
            content: text.content.clone(),
            position: xy(text.point),
            height: text.point.length(text.height.value)?,
            rotation: degrees(text.rotation),
            attachment: Some(attachment_of(text.horizontal, text.vertical)?),
            width: match text.width {
                Some(width) => Some(text.point.length(width)?),
                None => None,
            },
            style: Some(text.style.clone()),
        }),
        _ => return None,
    })
}

/// 形の値で作った形。`was` は置き換える前の形で、足すときは `None`。`origin` は、空間か用紙を決める点。
/// 合わない値は、`geometry` からの欄の場所を返す
pub(crate) fn shape_of<P: Spot>(
    geometry: &Geometry,
    was: Option<&Shape<P>>,
    origin: P,
) -> Result<Shape<P>, String> {
    let point = |xy: &[f64], field: &str| match *xy {
        [x, y] if x.is_finite() && y.is_finite() => Ok(origin.at([x, y])),
        _ => Err(format!("geometry/{field}")),
    };
    let positive = |value: f64, field: &str| {
        if value.is_finite() && value > 0.0 {
            Ok(origin.length_of(value))
        } else {
            Err(format!("geometry/{field}"))
        }
    };
    let finite = |value: f64, field: &str| {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(format!("geometry/{field}"))
        }
    };
    let mismatch = || Err("geometry".to_string());
    Ok(match (geometry, was) {
        (Geometry::Line(line), None | Some(Shape::Line { .. })) => Shape::Line {
            start: point(&line.start, "start")?,
            end: point(&line.end, "end")?,
        },
        (Geometry::Circle(circle), None | Some(Shape::Circle { .. })) => Shape::Circle {
            center: point(&circle.center, "center")?,
            radius: positive(circle.radius, "radius")?,
        },
        (Geometry::Arc(arc), None | Some(Shape::Curve(Edge::Arc { .. }))) => {
            Shape::Curve(Edge::Arc {
                center: point(&arc.center, "center")?,
                radius: positive(arc.radius, "radius")?,
                start: finite(arc.start_angle, "start_angle")?,
                end: finite(arc.end_angle, "end_angle")?,
                counterclockwise: true,
            })
        }
        (Geometry::Ellipse(ellipse), None | Some(Shape::Curve(Edge::Elliptic { .. }))) => {
            let center = point(&ellipse.center, "center")?;
            let major = point(&ellipse.major, "major")?;
            if Point::xy(major) == Point::xy(center) {
                return Err("geometry/major".to_string());
            }
            if !(ellipse.ratio > 0.0 && ellipse.ratio <= 1.0) {
                return Err("geometry/ratio".to_string());
            }
            Shape::Curve(Edge::Elliptic {
                center,
                major,
                ratio: ellipse.ratio,
                start: finite(ellipse.start_angle, "start_angle")?,
                end: finite(ellipse.end_angle, "end_angle")?,
                counterclockwise: true,
            })
        }
        (Geometry::Polyline(line), was) => {
            // 幅は置き換える前の値のまま。頂点ごとの幅は頂点の数で決まるので、持つものは置き換えない
            let width = match was {
                None => None,
                Some(Shape::Polyline(old)) if old.vertex_widths.is_empty() => old.width,
                Some(_) => return mismatch(),
            };
            if line.points.len() < 2 {
                return Err("geometry/points".to_string());
            }
            let bulges = match &line.bulges {
                None => vec![0.0; line.points.len()],
                Some(bulges) if bulges.len() == line.points.len() => bulges.clone(),
                Some(_) => return Err("geometry/bulges".to_string()),
            };
            let mut vertices = Vec::with_capacity(line.points.len());
            for (index, (xy, bulge)) in line.points.iter().zip(bulges).enumerate() {
                vertices.push(Vertex {
                    point: point(xy, &format!("points/{index}"))?,
                    bulge: finite(bulge, &format!("bulges/{index}"))?,
                });
            }
            Shape::Polyline(Polyline {
                vertices,
                closed: line.closed,
                width,
                vertex_widths: Vec::new(),
            })
        }
        // 足す文字は、AutoCAD の系統の TEXT の高さ（40）として書く。境界の apply が書けるのは DXF と DWG だけ
        (Geometry::Text(text), None) => {
            if text.content.contains(['\n', '\r']) {
                return Err("geometry/content".to_string());
            }
            Shape::Text(Text {
                content: text.content.clone(),
                placement: Placement::Anchor {
                    point: point(&text.position, "position")?,
                    horizontal: Horizontal::Left,
                    vertical: Vertical::Baseline,
                },
                height: zumen_drawing::Height {
                    value: positive(text.height, "height")?,
                    meaning: HeightMeaning::Declared("dxf.TEXT.40"),
                },
                rotation: finite(text.rotation, "rotation")?,
                width_factor: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                style: text.style.clone().unwrap_or_else(|| "Standard".to_string()),
            })
        }
        // 置き換える文字は、高さの意味・位置合わせ・文字スタイルを元の文字から受け継ぐ
        (Geometry::Text(text), Some(Shape::Text(old))) => {
            if text.style.as_ref().is_some_and(|style| *style != old.style) {
                return Err("geometry/style".to_string());
            }
            let Placement::Anchor {
                horizontal,
                vertical,
                ..
            } = old.placement
            else {
                return mismatch();
            };
            if text.content.contains(['\n', '\r']) {
                return Err("geometry/content".to_string());
            }
            Shape::Text(Text {
                content: text.content.clone(),
                placement: Placement::Anchor {
                    point: point(&text.position, "position")?,
                    horizontal,
                    vertical,
                },
                height: zumen_drawing::Height {
                    value: positive(text.height, "height")?,
                    meaning: old.height.meaning.clone(),
                },
                rotation: finite(text.rotation, "rotation")?,
                width_factor: old.width_factor,
                oblique: old.oblique,
                backward: old.backward,
                upside_down: old.upside_down,
                style: old.style.clone(),
            })
        }
        (Geometry::FormattedText(text), was) => {
            let was = match was {
                None => None,
                Some(Shape::Formatted(old)) => Some(old),
                Some(_) => return mismatch(),
            };
            if let (Some(style), Some(old)) = (&text.style, was)
                && *style != old.style
            {
                return Err("geometry/style".to_string());
            }
            // MTEXT の改行は \P で書く。生の改行は、書き戻すファイルの行を壊す
            if text.content.contains(['\n', '\r']) {
                return Err("geometry/content".to_string());
            }
            // 区間は、書き戻す側が内容を解釈し直した区間と合わなければならない。用紙では、内容の中の絶対の
            // 高さなどを mm に直す倍率が分からないので、そうした書式を持つ内容は受けない
            let absolute = Cell::new(false);
            let spans = zumen_acad::mtext_spans(&text.content, &|value| {
                absolute.set(true);
                origin.length_of(value)
            });
            if P::PAPER && absolute.get() {
                return Err("geometry/content".to_string());
            }
            let rotation = finite(text.rotation, "rotation")?;
            let direction = || {
                let radians = rotation.to_radians();
                let (sin, cos) = (libm::sin(radians), libm::cos(radians));
                Rotation::Direction([cos, sin])
            };
            // 置き換えるときは元と同じ形（角度か向き）で持つ。DWG に足す MTEXT は向きでしか書けないので、
            // 0 でない角度は向きにして足す
            let rotation = match was.map(|old| old.rotation) {
                Some(old) if degrees(old) == rotation => old,
                Some(Rotation::Angle(_)) => Rotation::Angle(rotation),
                Some(Rotation::Direction(_)) => direction(),
                None if rotation == 0.0 => Rotation::Angle(0.0),
                None => direction(),
            };
            let (horizontal, vertical) = match (text.attachment, was) {
                (Some(attachment), _) => placement_of(attachment),
                (None, Some(old)) => (old.horizontal, old.vertical),
                (None, None) => (Horizontal::Left, Vertical::Top),
            };
            let width = match text.width {
                Some(width) => Some(positive(width, "width")?),
                None => None,
            };
            let point = point(&text.position, "position")?;
            let height = positive(text.height, "height")?;
            Shape::Formatted(match was {
                Some(old) => FormattedText {
                    content: text.content.clone(),
                    spans,
                    height: zumen_drawing::Height {
                        value: height,
                        meaning: old.height.meaning.clone(),
                    },
                    point,
                    horizontal,
                    vertical,
                    rotation,
                    width,
                    frame_height: old.frame_height,
                    flow: old.flow,
                    line_spacing: old.line_spacing,
                    background: old.background,
                    style: old.style.clone(),
                },
                None => FormattedText {
                    content: text.content.clone(),
                    spans,
                    height: zumen_drawing::Height {
                        value: height,
                        meaning: HeightMeaning::Declared("dxf.MTEXT.40"),
                    },
                    point,
                    horizontal,
                    vertical,
                    rotation,
                    width,
                    frame_height: None,
                    flow: Flow::LeftToRight,
                    line_spacing: LineSpacing {
                        factor: 1.0,
                        exact: false,
                    },
                    background: None,
                    style: text.style.clone().unwrap_or_else(|| "Standard".to_string()),
                },
            })
        }
        _ => return mismatch(),
    })
}

/// 向きの角度（度）
fn degrees(rotation: Rotation) -> f64 {
    match rotation {
        Rotation::Angle(angle) => angle,
        Rotation::Direction([x, y]) => libm::atan2(y, x).to_degrees(),
    }
}

/// 枠の位置合わせの値。MTEXT の 9 つの位置のほかは `None`
fn attachment_of(horizontal: Horizontal, vertical: Vertical) -> Option<Attachment> {
    Some(match (vertical, horizontal) {
        (Vertical::Top, Horizontal::Left) => Attachment::TopLeft,
        (Vertical::Top, Horizontal::Center) => Attachment::TopCenter,
        (Vertical::Top, Horizontal::Right) => Attachment::TopRight,
        (Vertical::Middle, Horizontal::Left) => Attachment::MiddleLeft,
        (Vertical::Middle, Horizontal::Center) => Attachment::MiddleCenter,
        (Vertical::Middle, Horizontal::Right) => Attachment::MiddleRight,
        (Vertical::Bottom, Horizontal::Left) => Attachment::BottomLeft,
        (Vertical::Bottom, Horizontal::Center) => Attachment::BottomCenter,
        (Vertical::Bottom, Horizontal::Right) => Attachment::BottomRight,
        _ => return None,
    })
}

/// 位置合わせの値の、水平と垂直の位置合わせ
fn placement_of(attachment: Attachment) -> (Horizontal, Vertical) {
    match attachment {
        Attachment::TopLeft => (Horizontal::Left, Vertical::Top),
        Attachment::TopCenter => (Horizontal::Center, Vertical::Top),
        Attachment::TopRight => (Horizontal::Right, Vertical::Top),
        Attachment::MiddleLeft => (Horizontal::Left, Vertical::Middle),
        Attachment::MiddleCenter => (Horizontal::Center, Vertical::Middle),
        Attachment::MiddleRight => (Horizontal::Right, Vertical::Middle),
        Attachment::BottomLeft => (Horizontal::Left, Vertical::Bottom),
        Attachment::BottomCenter => (Horizontal::Center, Vertical::Bottom),
        Attachment::BottomRight => (Horizontal::Right, Vertical::Bottom),
    }
}

/// 形の空間か用紙を決める点
pub(crate) fn origin_of<P: Copy>(shape: &Shape<P>) -> Option<P> {
    match shape {
        Shape::Line { start, .. } => Some(*start),
        Shape::Circle { center, .. }
        | Shape::Curve(Edge::Arc { center, .. } | Edge::Elliptic { center, .. }) => Some(*center),
        Shape::Polyline(line) => line.vertices.first().map(|v| v.point),
        Shape::Text(text) => match text.placement {
            Placement::Anchor { point, .. } => Some(point),
            Placement::Between { first, .. } => Some(first),
        },
        Shape::Formatted(text) => Some(text.point),
        _ => None,
    }
}
