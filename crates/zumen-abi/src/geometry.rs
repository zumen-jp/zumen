//! 形の値（`geometry`）と図面モデルの形の行き来（決定 0139）。
//!
//! 線分・円・円弧・楕円・幅を持たない折れ線・1 点に置く文字・書式つきの文字・ブロックの記号・制御点で決まるスプラインだけを扱う。点は要素の空間か用紙の座標、長さは同じ
//! 単位、角度は度で反時計回り。円弧と楕円は、反時計回りにたどる形で返す。

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use zumen_drawing::transform::{Point, transform};
use zumen_drawing::{
    Affine, Attribute, Edge, Fit, Flow, FormattedText, Grid, HeightMeaning, Horizontal, Length,
    LineSpacing, ModelLength, ModelPoint, PaperLength, PaperPoint, Placement, Polyline, Rotation,
    Shape, Spline, Symbol, SymbolKey, Text, Vertex, Vertical,
};

use crate::bounds::Planar;
use crate::messages::{
    ArcGeometry, Attachment, CircleGeometry, EllipseGeometry, FormattedTextGeometry, Geometry,
    LineGeometry, PolylineGeometry, SplineGeometry, SymbolAttribute, SymbolGeometry, SymbolGrid,
    TextFit, TextGeometry,
};

/// 境界の apply が書き戻す DXF と DWG の、ブロックの記号の名前空間
const BLOCK: &str = "dxf.block";

/// 形の値と行き来できる点
pub(crate) trait Spot: Planar + Point + PartialEq {
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
        Shape::Curve(Edge::Spline(spline))
            if !spline.control.is_empty()
                && spline.fit.is_empty()
                && spline.start_tangent.is_none()
                && spline.end_tangent.is_none() =>
        {
            Geometry::Spline(SplineGeometry {
                degree: spline.degree.into(),
                knots: spline.knots.clone(),
                control: spline.control.iter().map(|point| xy(*point)).collect(),
                weights: (!spline.weights.is_empty()).then(|| spline.weights.clone()),
                fit: None,
                start_tangent: None,
                end_tangent: None,
                closed: spline.closed,
                periodic: spline.periodic,
            })
        }
        // 制御点や節点も持つものは、それらが当てはめる点から再現できるか分からないので返さない。
        Shape::Curve(Edge::Spline(spline))
            if spline.fit.len() >= 2
                && spline.control.is_empty()
                && spline.knots.is_empty()
                && spline.weights.is_empty() =>
        {
            Geometry::Spline(SplineGeometry {
                degree: spline.degree.into(),
                knots: Vec::new(),
                control: Vec::new(),
                weights: None,
                fit: Some(spline.fit.iter().map(|point| xy(*point)).collect()),
                start_tangent: spline.start_tangent.map(Vec::from),
                end_tangent: spline.end_tangent.map(Vec::from),
                closed: spline.closed,
                periodic: spline.periodic,
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
        Shape::Text(text) => {
            let (point, end, fit, rotation) = match text.placement {
                Placement::Anchor { point, .. } => (point, None, None, text.rotation),
                Placement::Between { first, second, fit } => {
                    let ([x0, y0], [x1, y1]) = (Point::xy(first), Point::xy(second));
                    if x0 == x1 && y0 == y1 {
                        return None;
                    }
                    let rotation = libm::atan2(y1 - y0, x1 - x0).to_degrees();
                    (
                        first,
                        Some(xy(second)),
                        Some(match fit {
                            Fit::Aligned => TextFit::Aligned,
                            Fit::Fit => TextFit::Fit,
                        }),
                        rotation,
                    )
                }
            };
            Geometry::Text(TextGeometry {
                content: text.content.clone(),
                position: xy(point),
                end,
                fit,
                height: point.length(text.height.value)?,
                rotation,
                style: Some(text.style.clone()),
            })
        }
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
        Shape::Symbol(symbol) if symbol.key.namespace == BLOCK => {
            Geometry::Symbol(SymbolGeometry {
                name: symbol.key.name.clone(),
                position: xy(symbol.position),
                scale: symbol.scale.to_vec(),
                rotation: symbol.rotation,
                grid: match &symbol.grid {
                    Some(grid) => Some(SymbolGrid {
                        columns: grid.columns.into(),
                        rows: grid.rows.into(),
                        column_spacing: symbol.position.length(grid.column_spacing)?,
                        row_spacing: symbol.position.length(grid.row_spacing)?,
                    }),
                    None => None,
                },
                attributes: (!symbol.attributes.is_empty()).then(|| {
                    symbol
                        .attributes
                        .iter()
                        .map(|attribute| SymbolAttribute {
                            tag: attribute.tag.clone(),
                            value: attribute.text.content.clone(),
                        })
                        .collect()
                }),
            })
        }
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
    // 2 点の間に合わせる文字の 2 点と合わせ方。AutoCAD は回転を 2 点から決め直すので、回転は 1 点目から
    // 2 点目への向きと同じでなければならない
    let between = |text: &TextGeometry| -> Result<Option<(P, P, Fit)>, String> {
        let (end, fit) = match (&text.end, text.fit) {
            (None, None) => return Ok(None),
            (Some(_), None) => return Err("geometry/fit".to_string()),
            (None, Some(_)) => return Err("geometry/end".to_string()),
            (Some(end), Some(fit)) => (end, fit),
        };
        let first = point(&text.position, "position")?;
        let second = point(end, "end")?;
        let ([x0, y0], [x1, y1]) = (Point::xy(first), Point::xy(second));
        if x0 == x1 && y0 == y1 {
            return Err("geometry/end".to_string());
        }
        let turn = (finite(text.rotation, "rotation")?
            - libm::atan2(y1 - y0, x1 - x0).to_degrees())
        .rem_euclid(360.0);
        if turn.min(360.0 - turn) > 1e-6 {
            return Err("geometry/rotation".to_string());
        }
        let fit = match fit {
            TextFit::Aligned => Fit::Aligned,
            TextFit::Fit => Fit::Fit,
        };
        Ok(Some((first, second, fit)))
    };
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
        (Geometry::Spline(spline), was @ (None | Some(Shape::Curve(Edge::Spline(_))))) => {
            let degree = u32::try_from(spline.degree)
                .ok()
                .filter(|degree| *degree >= 1)
                .ok_or_else(|| "geometry/degree".to_string())?;
            // 置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない。DWG の場合（scenario）が
            // 変わるため
            let fitting = spline.fit.is_some();
            if let Some(Shape::Curve(Edge::Spline(old))) = was
                && !old.fit.is_empty()
                && (!old.control.is_empty() || !old.knots.is_empty() || !old.weights.is_empty())
            {
                return mismatch();
            }
            if let Some(Shape::Curve(Edge::Spline(old))) = was
                && old.fit.is_empty() == fitting
            {
                return Err(if fitting {
                    "geometry/fit"
                } else {
                    "geometry/control"
                }
                .to_string());
            }
            if let Some(fit) = &spline.fit {
                if !spline.knots.is_empty() {
                    return Err("geometry/knots".to_string());
                }
                if !spline.control.is_empty() {
                    return Err("geometry/control".to_string());
                }
                if spline.weights.is_some() {
                    return Err("geometry/weights".to_string());
                }
                if fit.len() < 2 {
                    return Err("geometry/fit".to_string());
                }
                let mut points = Vec::with_capacity(fit.len());
                for (index, xy) in fit.iter().enumerate() {
                    points.push(point(xy, &format!("fit/{index}"))?);
                }
                // 0 の接線は、DWG では接線なしと同じに書かれるので受けない
                let tangent = |given: &Option<Vec<f64>>, field: &str| match given.as_deref() {
                    None => Ok(None),
                    Some(&[x, y]) if x.is_finite() && y.is_finite() && (x != 0.0 || y != 0.0) => {
                        Ok(Some([x, y]))
                    }
                    Some(_) => Err(format!("geometry/{field}")),
                };
                return Ok(Shape::Curve(Edge::Spline(Spline {
                    degree,
                    knots: Vec::new(),
                    control: Vec::new(),
                    weights: Vec::new(),
                    fit: points,
                    start_tangent: tangent(&spline.start_tangent, "start_tangent")?,
                    end_tangent: tangent(&spline.end_tangent, "end_tangent")?,
                    closed: spline.closed,
                    periodic: spline.periodic,
                })));
            }
            if spline.start_tangent.is_some() {
                return Err("geometry/start_tangent".to_string());
            }
            if spline.end_tangent.is_some() {
                return Err("geometry/end_tangent".to_string());
            }
            let mut control = Vec::with_capacity(spline.control.len());
            for (index, xy) in spline.control.iter().enumerate() {
                control.push(point(xy, &format!("control/{index}"))?);
            }
            if control.len() <= degree as usize {
                return Err("geometry/control".to_string());
            }
            for (index, knot) in spline.knots.iter().enumerate() {
                finite(*knot, &format!("knots/{index}"))?;
            }
            // 節点は減らない並びで、周期的でなければ制御点の数と次数と 1 の和の数
            if spline
                .knots
                .windows(2)
                .any(|pair| matches!(*pair, [a, b] if b < a))
                || (!spline.periodic && spline.knots.len() != control.len() + degree as usize + 1)
            {
                return Err("geometry/knots".to_string());
            }
            let weights = match &spline.weights {
                None => Vec::new(),
                Some(weights)
                    if weights.len() == control.len()
                        && weights.iter().all(|w| w.is_finite() && *w > 0.0) =>
                {
                    weights.clone()
                }
                Some(_) => return Err("geometry/weights".to_string()),
            };
            Shape::Curve(Edge::Spline(Spline {
                degree,
                knots: spline.knots.clone(),
                control,
                weights,
                fit: Vec::new(),
                start_tangent: None,
                end_tangent: None,
                closed: spline.closed,
                periodic: spline.periodic,
            }))
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
                placement: match between(text)? {
                    Some((first, second, fit)) => Placement::Between { first, second, fit },
                    None => Placement::Anchor {
                        point: point(&text.position, "position")?,
                        horizontal: Horizontal::Left,
                        vertical: Vertical::Baseline,
                    },
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
            // 1 点に置く文字と 2 点の間に合わせる文字は、入れ替えない
            let placement = match (old.placement, between(text)?) {
                (
                    Placement::Anchor {
                        horizontal,
                        vertical,
                        ..
                    },
                    None,
                ) => Placement::Anchor {
                    point: point(&text.position, "position")?,
                    horizontal,
                    vertical,
                },
                (Placement::Between { fit: was, .. }, Some((first, second, fit))) => {
                    if fit != was {
                        return Err("geometry/fit".to_string());
                    }
                    Placement::Between { first, second, fit }
                }
                (Placement::Anchor { .. }, Some(_)) | (Placement::Between { .. }, None) => {
                    return Err("geometry/end".to_string());
                }
            };
            if text.content.contains(['\n', '\r']) {
                return Err("geometry/content".to_string());
            }
            Shape::Text(Text {
                content: text.content.clone(),
                placement,
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
        (Geometry::Symbol(symbol), was) => {
            let was = match was {
                None => None,
                Some(Shape::Symbol(old)) => Some(old),
                Some(_) => return mismatch(),
            };
            let position = point(&symbol.position, "position")?;
            let scale = match *symbol.scale.as_slice() {
                [x, y] if [x, y].iter().all(|v| v.is_finite() && *v != 0.0) => [x, y],
                _ => return Err("geometry/scale".to_string()),
            };
            let rotation = finite(symbol.rotation, "rotation")?;
            let grid = match &symbol.grid {
                None => None,
                Some(grid) => {
                    let count = |value: u64| u32::try_from(value).ok().filter(|n| *n >= 1);
                    let (Some(columns), Some(rows)) = (count(grid.columns), count(grid.rows))
                    else {
                        return Err("geometry/grid".to_string());
                    };
                    // 読み直すと、1×1 の格子は格子なしになる
                    if (columns, rows) == (1, 1) {
                        return Err("geometry/grid".to_string());
                    }
                    Some(Grid {
                        columns,
                        rows,
                        column_spacing: origin
                            .length_of(finite(grid.column_spacing, "grid/column_spacing")?),
                        row_spacing: origin
                            .length_of(finite(grid.row_spacing, "grid/row_spacing")?),
                    })
                }
            };
            let counts = |grid: &Option<Grid>| grid.as_ref().map(|g| (g.columns, g.rows));
            Shape::Symbol(match was {
                // 置き換えは、定義・格子の列と行の数・属性・切り抜きを変えない。属性は置く点などに合わせて
                // 動かせないので、属性を持つ記号は置く点・尺度・回転も変えない
                Some(old) => {
                    if symbol.name != old.key.name {
                        return Err("geometry/name".to_string());
                    }
                    if counts(&grid) != counts(&old.grid) {
                        return Err("geometry/grid".to_string());
                    }
                    // 属性の値は、置き換えでは変えられない
                    if let Some(attributes) = &symbol.attributes
                        && (attributes.len() != old.attributes.len()
                            || attributes.iter().zip(&old.attributes).any(|(new, was)| {
                                new.tag != was.tag || new.value != was.text.content
                            }))
                    {
                        return Err("geometry/attributes".to_string());
                    }
                    if !old.attributes.is_empty()
                        && (position != old.position
                            || scale != old.scale
                            || rotation != old.rotation)
                    {
                        return mismatch();
                    }
                    Symbol {
                        position,
                        scale,
                        rotation,
                        grid,
                        ..old.clone()
                    }
                }
                None => Symbol {
                    key: SymbolKey {
                        namespace: BLOCK,
                        name: symbol.name.clone(),
                    },
                    position,
                    scale,
                    rotation,
                    grid,
                    attributes: Vec::new(),
                    clip: None,
                },
            })
        }
        _ => return mismatch(),
    })
}

/// 足す記号の属性。AutoCAD が挿入するときと同じく、定義の属性の定義のうち値が決まっていないものを、記号の
/// 置く点・尺度・回転で写して作り、`given` のタグの値で既定の値を替える。実寸の空間に、X と Y の尺度の大きさが
/// 同じで置く記号だけに作る。用紙では、定義の長さを mm に直す倍率を書き戻す側しか知らず、尺度の大きさが違えば
/// 文字の形を保って写せないため。必要な属性を作れないときは失敗する
pub(crate) fn attributes_of<P: Spot>(
    definition: Option<&zumen_drawing::SymbolDefinition>,
    symbol: &Symbol<P>,
    given: Option<&[SymbolAttribute]>,
) -> Result<Vec<Attribute<P>>, String> {
    let given = given.unwrap_or_default();
    let mut supplied = BTreeMap::new();
    for (index, attribute) in given.iter().enumerate() {
        if attribute.value.contains(['\n', '\r']) {
            return Err(format!("geometry/attributes/{index}/value"));
        }
        if supplied
            .insert(attribute.tag.to_ascii_lowercase(), attribute)
            .is_some()
        {
            return Err(format!("geometry/attributes/{index}/tag"));
        }
    }
    let Some(definition) = definition else {
        return Ok(Vec::new());
    };
    let defined: Vec<_> = definition
        .attributes
        .iter()
        .filter(|definition| !definition.constant)
        .map(|definition| &definition.attribute)
        .collect();
    let tags: BTreeSet<_> = defined
        .iter()
        .map(|item| item.tag.to_ascii_lowercase())
        .collect();
    for (index, attribute) in given.iter().enumerate() {
        if !tags.contains(&attribute.tag.to_ascii_lowercase()) {
            return Err(format!("geometry/attributes/{index}/tag"));
        }
    }
    let [sx, sy] = symbol.scale;
    let made = (!P::PAPER && sx.abs() == sy.abs())
        .then(|| {
            let [bx, by] = definition.base.map_or([0.0, 0.0], |base| [base.x, base.y]);
            let [px, py] = Point::xy(symbol.position);
            let radians = symbol.rotation.to_radians();
            let (sin, cos) = (libm::sin(radians), libm::cos(radians));
            let [a, b, d, e] = [cos * sx, -sin * sy, sin * sx, cos * sy];
            let affine = Affine {
                m: [a, b, px - (a * bx + b * by), d, e, py - (d * bx + e * by)],
            };
            defined
                .iter()
                .map(|defined| {
                    if defined.formatted.is_some() {
                        return None;
                    }
                    let Shape::Text(mut text) = transform(
                        &Shape::Text(lifted(&defined.text, symbol.position)?),
                        &affine,
                    )?
                    else {
                        return None;
                    };
                    if let Some(value) = supplied.get(&defined.tag.to_ascii_lowercase()) {
                        text.content = value.value.clone();
                    }
                    Some(Attribute {
                        tag: defined.tag.clone(),
                        text,
                        formatted: None,
                        hidden: defined.hidden,
                        layer: defined.layer,
                        style: defined.style,
                        source: defined.source.clone(),
                    })
                })
                .collect::<Option<Vec<_>>>()
        })
        .flatten();
    match made {
        Some(attributes) => Ok(attributes),
        None if defined.is_empty() && given.is_empty() => Ok(Vec::new()),
        None => Err("geometry/attributes".to_string()),
    }
}

/// 定義の空間の文字を、座標を変えずに `origin` の空間に移す
fn lifted<P: Spot>(text: &Text<ModelPoint>, origin: P) -> Option<Text<P>> {
    let at = |point: ModelPoint| origin.at([point.x, point.y]);
    let Length::Model(height) = text.height.value else {
        return None;
    };
    Some(Text {
        content: text.content.clone(),
        placement: match text.placement {
            Placement::Anchor {
                point,
                horizontal,
                vertical,
            } => Placement::Anchor {
                point: at(point),
                horizontal,
                vertical,
            },
            Placement::Between { first, second, fit } => Placement::Between {
                first: at(first),
                second: at(second),
                fit,
            },
        },
        height: zumen_drawing::Height {
            value: origin.length_of(height.value),
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
        Shape::Symbol(symbol) => Some(symbol.position),
        Shape::Curve(Edge::Spline(spline)) => {
            spline.control.first().or(spline.fit.first()).copied()
        }
        _ => None,
    }
}
