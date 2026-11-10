//! 横と縦の尺度の違うビューを、用紙の要素に焼き込む（決定 0238）。
//!
//! DXF・DWG のビューポートは相似の変換しか表せない。縦断図のように横と縦の尺度の違うビューは、実寸の空間の
//! 要素をビューの変換で用紙に写した要素にして書く。線分と曲がらない折れ線は頂点を写す。円・円弧・楕円弧・
//! スプライン・クロソイドと膨らみを持つ折れ線は、用紙の上で [`TOLERANCE`] に収まる折れ線にする。文字は基点を
//! 写し、字の枠を写した形から高さ・幅の比・傾き・回転を決める。ほかの要素は焼き込まない。

use zumen_drawing::{
    Edge, Element, Height, Length, ModelPoint, ModelSpace, PaperLength, PaperPoint, Placement,
    Polyline, Shape, Text, Vertex, View,
};
use zumen_geom2d as geom;

/// 用紙の上で、折れ線が元の曲線から離れてよい距離（mm）
const TOLERANCE: f64 = 0.02;

/// 焼き込むビューか。相似でなく、向きを潰さず、切り抜きを持たないビューだけを焼き込む
pub(crate) fn bakes(view: &View) -> bool {
    view.transform.similarity().is_none() && view.transform.invertible() && view.clip.is_empty()
}

/// ビュー `view` の空間 `space` の要素を、描く順に用紙の要素にする。焼き込めない要素は `Err` で返す
pub(crate) fn bake<'a>(
    space: &'a ModelSpace,
    view: &'a View,
) -> impl Iterator<Item = Result<Element<PaperPoint>, &'a Element<ModelPoint>>> + 'a {
    let order: Vec<usize> = if space.order.len() == space.elements.len() {
        space.order.clone()
    } else {
        (0..space.elements.len()).collect()
    };
    order
        .into_iter()
        .filter_map(|index| space.elements.get(index))
        .filter(|element| !view.hidden_layers.contains(&element.layer))
        .map(|element| {
            shape(&element.shape, view)
                .map(|shape| Element {
                    source: element.source.clone(),
                    layer: element.layer,
                    style: element.style,
                    shape,
                })
                .ok_or(element)
        })
}

fn shape(shape: &Shape<ModelPoint>, view: &View) -> Option<Shape<PaperPoint>> {
    let m = view.transform.m;
    let tolerance = TOLERANCE / stretch(m);
    let paper = |[x, y]: geom::Xy| {
        let (x, y) = view.transform.apply(x, y);
        PaperPoint {
            sheet: view.sheet,
            x,
            y,
        }
    };
    let xy = |point: &ModelPoint| [point.x, point.y];
    let lines = |points: Vec<geom::Xy>, closed: bool| line(points.into_iter().map(paper), closed);
    Some(match shape {
        Shape::Line { start, end } | Shape::Curve(Edge::Line { start, end }) => Shape::Line {
            start: paper(xy(start)),
            end: paper(xy(end)),
        },
        Shape::Polyline(polyline) => {
            if polyline.width.is_some() || !polyline.vertex_widths.is_empty() {
                return None;
            }
            if polyline.vertices.iter().all(|v| v.bulge == 0.0) {
                let mut out = line(
                    polyline.vertices.iter().map(|v| paper(xy(&v.point))),
                    polyline.closed,
                )?;
                if let Shape::Polyline(out) = &mut out {
                    out.line_type_generation = polyline.line_type_generation;
                }
                return Some(out);
            }
            let vertices: Vec<(geom::Xy, f64)> = polyline
                .vertices
                .iter()
                .map(|v| (xy(&v.point), v.bulge))
                .collect();
            let mut points = geom::polyline(&vertices, polyline.closed, tolerance);
            if polyline.closed {
                points.pop();
            }
            lines(points, polyline.closed)?
        }
        Shape::Circle { center, radius } => {
            let mut points = Vec::new();
            geom::arc(
                xy(center),
                model(*radius)?,
                0.0,
                360.0,
                tolerance,
                &mut points,
            );
            points.pop();
            lines(points, true)?
        }
        Shape::Curve(Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        }) => {
            let mut points = Vec::new();
            let sweep = geom::sweep(*start, *end, *counterclockwise);
            geom::arc(
                xy(center),
                model(*radius)?,
                *start,
                sweep,
                tolerance,
                &mut points,
            );
            lines(points, false)?
        }
        Shape::Curve(Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        }) => {
            let mut points = Vec::new();
            let sweep = geom::sweep(*start, *end, *counterclockwise);
            geom::ellipse(
                xy(center),
                xy(major),
                *ratio,
                *start,
                sweep,
                tolerance,
                &mut points,
            );
            lines(points, false)?
        }
        Shape::Curve(Edge::Spline(spline)) => {
            let control: Vec<geom::Xy> = spline.control.iter().map(xy).collect();
            let points = geom::spline(
                spline.degree,
                &spline.knots,
                &control,
                &spline.weights,
                tolerance,
            )?;
            lines(points, false)?
        }
        Shape::Curve(Edge::Clothoid {
            origin,
            parameter,
            angle,
            counterclockwise,
            start,
            end,
        }) => lines(
            geom::clothoid(
                xy(origin),
                model(*parameter)?,
                *angle,
                *counterclockwise,
                model(*start)?,
                model(*end)?,
                tolerance,
            ),
            false,
        )?,
        Shape::Text(text) => Shape::Text(lettering(text, view)?),
        _ => return None,
    })
}

/// 点を結んだ折れ線。点が 2 つに満たないか、座標が有限でなければ `None`
fn line(points: impl Iterator<Item = PaperPoint>, closed: bool) -> Option<Shape<PaperPoint>> {
    let vertices: Vec<Vertex<PaperPoint>> =
        points.map(|point| Vertex { point, bulge: 0.0 }).collect();
    if vertices.len() < 2
        || !vertices
            .iter()
            .all(|v| v.point.x.is_finite() && v.point.y.is_finite())
    {
        return None;
    }
    Some(Shape::Polyline(Polyline {
        vertices,
        closed,
        width: None,
        vertex_widths: Vec::new(),
        line_type_generation: false,
    }))
}

/// 実寸の空間の長さの値。用紙の長さなら `None`
fn model(length: Length) -> Option<f64> {
    match length {
        Length::Model(length) if length.value.is_finite() => Some(length.value),
        _ => None,
    }
}

/// 変換がいちばん引き延ばす向きの倍率（行列の最大の特異値）
fn stretch(m: [f64; 6]) -> f64 {
    let [a, b, _, d, e, _] = m;
    let sum = a * a + b * b + d * d + e * e;
    let det = a * e - b * d;
    let root = (sum * sum - 4.0 * det * det).max(0.0).sqrt();
    ((sum + root) / 2.0).sqrt()
}

/// 文字を写す。字の枠の横の向き（幅）と縦の向き（傾きを含む高さ）を写し、写した横の向きを回転、横に直交する
/// 縦の成分を高さ、横に沿う縦の成分を傾きにする。鏡映になる変換は、文字の向きを表せないので写さない
fn lettering(text: &Text<ModelPoint>, view: &View) -> Option<Text<PaperPoint>> {
    let [a, b, _, d, e, _] = view.transform.m;
    let vector = |[x, y]: [f64; 2]| [a * x + b * y, d * x + e * y];
    let height = model(text.height.value)?;
    let rotation = text.rotation.to_radians();
    let (cos, sin) = (libm::cos(rotation), libm::sin(rotation));
    let slant = libm::tan(text.oblique.to_radians());
    let across = vector([text.width_factor * cos, text.width_factor * sin]);
    let up = vector([-sin + slant * cos, cos + slant * sin]);
    let wide = (across[0] * across[0] + across[1] * across[1]).sqrt();
    let cross = across[0] * up[1] - across[1] * up[0];
    if !(wide.is_finite() && wide > 0.0 && cross.is_finite() && cross > 0.0) {
        return None;
    }
    // 高さ 1 の字の、写した高さと、横に沿う縦の成分
    let tall = cross / wide;
    let lean = (across[0] * up[0] + across[1] * up[1]) / wide;
    let place = |point: ModelPoint| {
        let (x, y) = view.transform.apply(point.x, point.y);
        PaperPoint {
            sheet: view.sheet,
            x,
            y,
        }
    };
    let placement = match &text.placement {
        Placement::Anchor {
            point,
            horizontal,
            vertical,
        } => Placement::Anchor {
            point: place(*point),
            horizontal: *horizontal,
            vertical: *vertical,
        },
        Placement::Between { first, second, fit } => Placement::Between {
            first: place(*first),
            second: place(*second),
            fit: *fit,
        },
    };
    let turned = libm::atan2(across[1], across[0]).to_degrees();
    Some(Text {
        content: text.content.clone(),
        height: Height {
            value: Length::Paper(PaperLength(height * tall)),
            meaning: text.height.meaning.clone(),
        },
        placement,
        rotation: turned.rem_euclid(360.0),
        width_factor: wide / tall,
        oblique: libm::atan(lean / tall).to_degrees(),
        backward: text.backward,
        upside_down: text.upside_down,
        style: text.style.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::Ref;
    use zumen_drawing::{
        Affine, HeightMeaning, Horizontal, Inherit, LayerId, ModelLength, SheetId, SpaceId, Style,
        Vertical,
    };

    const SPACE: SpaceId = SpaceId(0);

    /// 横 1/1000・縦 1/100 のビュー
    fn view() -> View {
        View {
            space: SPACE,
            sheet: SheetId(0),
            transform: Affine {
                m: [1.0, 0.0, 10.0, 0.0, 10.0, 20.0],
            },
            clip: Vec::new(),
            hidden_layers: Vec::new(),
            source: Ref::default(),
        }
    }

    fn at(x: f64, y: f64) -> ModelPoint {
        ModelPoint { space: SPACE, x, y }
    }

    fn real(value: f64) -> Length {
        Length::Model(ModelLength {
            space: SPACE,
            value,
        })
    }

    #[test]
    fn an_arc_becomes_an_ellipse_within_the_tolerance() {
        let view = view();
        assert!(bakes(&view));
        let arc = Shape::Curve(Edge::Arc {
            center: at(0.0, 0.0),
            radius: real(5.0),
            start: 0.0,
            end: 180.0,
            counterclockwise: true,
        });
        let Some(Shape::Polyline(polyline)) = shape(&arc, &view) else {
            panic!("折れ線にならない");
        };
        assert!(polyline.vertices.len() > 10);
        for pair in polyline.vertices.windows(2) {
            let [p, q] = [pair[0].point, pair[1].point];
            // 用紙の楕円 ((x-10)/5)² + ((y-20)/50)² = 1 の上の点で、弦の中点も許容差の内
            let on = |x: f64, y: f64| {
                let (u, v) = ((x - 10.0) / 5.0, (y - 20.0) / 50.0);
                (u * u + v * v).sqrt()
            };
            assert!((on(p.x, p.y) - 1.0).abs() < 1e-9);
            let mid = on((p.x + q.x) / 2.0, (p.y + q.y) / 2.0);
            assert!((1.0 - mid) * 5.0 < TOLERANCE, "{mid}");
        }
    }

    #[test]
    fn text_keeps_its_shape_on_paper() {
        let view = view();
        let text = |rotation: f64| {
            Shape::Text(Text {
                content: "BP".to_string(),
                height: Height {
                    value: real(0.25),
                    meaning: HeightMeaning::Frame,
                },
                placement: Placement::Anchor {
                    point: at(1.0, 2.0),
                    horizontal: Horizontal::Left,
                    vertical: Vertical::Baseline,
                },
                rotation,
                width_factor: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                style: String::new(),
            })
        };
        let Some(Shape::Text(out)) = shape(&text(0.0), &view) else {
            panic!("文字にならない");
        };
        assert_eq!(out.height.value, Length::Paper(PaperLength(2.5)));
        assert!((out.width_factor - 0.1).abs() < 1e-12);
        assert!(out.oblique.abs() < 1e-12 && out.rotation.abs() < 1e-12);
        assert!(matches!(
            out.placement,
            Placement::Anchor { point, .. } if point.x == 11.0 && point.y == 40.0
        ));
        // 縦に立てた文字は、横の尺度で高さが決まる
        let Some(Shape::Text(out)) = shape(&text(90.0), &view) else {
            panic!("文字にならない");
        };
        let Length::Paper(PaperLength(height)) = out.height.value else {
            panic!("用紙の長さでない");
        };
        assert!((height - 0.25).abs() < 1e-12);
        assert!((out.width_factor - 10.0).abs() < 1e-9);
        assert!((out.rotation - 90.0).abs() < 1e-9);
    }

    #[test]
    fn hidden_layers_and_unknown_shapes_are_left_out() {
        let mut view = view();
        view.hidden_layers.push(LayerId(1));
        let element = |layer, shape| Element {
            source: Ref::default(),
            layer: LayerId(layer),
            style: Style {
                color: Inherit::Layer,
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
                transparency: Inherit::Layer,
            },
            shape,
        };
        let line = Shape::Line {
            start: at(0.0, 0.0),
            end: at(1.0, 1.0),
        };
        let space = ModelSpace {
            id: SPACE,
            unit: zumen_drawing::Unit::Meter,
            axes: zumen_drawing::Axes::default(),
            elements: vec![
                element(0, line.clone()),
                element(1, line),
                element(
                    0,
                    Shape::Circle {
                        center: at(0.0, 0.0),
                        radius: Length::Paper(PaperLength(1.0)),
                    },
                ),
            ],
            order: Vec::new(),
        };
        let out: Vec<_> = bake(&space, &view).collect();
        assert_eq!(out.len(), 2);
        assert!(matches!(
            &out[0],
            Ok(Element { shape: Shape::Line { end, .. }, .. }) if end.x == 11.0 && end.y == 30.0
        ));
        assert!(out[1].is_err());
    }
}
