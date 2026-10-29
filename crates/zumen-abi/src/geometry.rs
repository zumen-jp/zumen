//! 形の値（`geometry`）と図面モデルの形の行き来（決定 0139）。
//!
//! 線分・円・円弧・幅を持たない折れ線・1 点に置く文字だけを扱う。点は要素の空間か用紙の座標、長さは同じ単位、
//! 角度は度で反時計回り。円弧は、反時計回りにたどる形で返す。

use zumen_drawing::transform::Point;
use zumen_drawing::{
    Drawing2D, Edge, Element, Length, ModelLength, ModelPoint, PaperLength, PaperPoint, Placed,
    Placement, Polyline, Shape, Text, Vertex, id,
};

use crate::bounds::Planar;
use crate::messages::{
    ArcGeometry, CircleGeometry, Geometry, LineGeometry, PolylineGeometry, TextGeometry,
};

/// 形の値と行き来できる点
pub(crate) trait Spot: Planar + Point {
    /// この点と同じ座標の系の、値が `value` の長さ
    fn length_of(self, value: f64) -> Length;
}

impl Spot for ModelPoint {
    fn length_of(self, value: f64) -> Length {
        Length::Model(ModelLength {
            space: self.space,
            value,
        })
    }
}

impl Spot for PaperPoint {
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
            }),
            Placement::Between { .. } => return None,
        },
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
        // 文字は、高さの意味・位置合わせ・文字スタイルを決める元の文字がなければ作らない
        (Geometry::Text(text), Some(Shape::Text(old))) => {
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
                ..old.clone()
            })
        }
        _ => return mismatch(),
    })
}

/// 形の空間か用紙を決める点
pub(crate) fn origin_of<P: Copy>(shape: &Shape<P>) -> Option<P> {
    match shape {
        Shape::Line { start, .. } => Some(*start),
        Shape::Circle { center, .. } | Shape::Curve(Edge::Arc { center, .. }) => Some(*center),
        Shape::Polyline(line) => line.vertices.first().map(|v| v.point),
        Shape::Text(text) => match text.placement {
            Placement::Anchor { point, .. } => Some(point),
            Placement::Between { first, .. } => Some(first),
        },
        _ => None,
    }
}

/// ID の要素。実寸の空間・記号の定義の空間・用紙の順に探す
pub(crate) fn find(drawing: &Drawing2D, wanted: &str) -> Option<Placed> {
    fn search<P: Clone>(place: &str, elements: &[Element<P>], wanted: &str) -> Option<Element<P>> {
        if !wanted
            .strip_prefix(place)
            .is_some_and(|rest| rest.starts_with('/'))
        {
            return None;
        }
        let mut namer = id::Namer::new(place);
        elements
            .iter()
            .enumerate()
            .find(|(index, element)| namer.next(*index, element) == wanted)
            .map(|(_, element)| element.clone())
    }
    for space in &drawing.spaces {
        if let Some(found) = search(&id::space(space.id), &space.elements, wanted) {
            return Some(Placed::Model(found));
        }
    }
    // 定義と用紙の ID は、apply の索引と同じく、重なれば 2 つ目から `/1` を足す
    let mut definitions = id::DefinitionNamer::default();
    for definition in &drawing.symbols {
        let place = definitions.next(definition);
        if let Some(found) = search(&place, &definition.space.elements, wanted) {
            return Some(Placed::Model(found));
        }
    }
    let mut sheets = id::SheetNamer::default();
    for sheet in &drawing.sheets {
        if let Some(found) = search(&sheets.next(sheet), &sheet.elements, wanted) {
            return Some(Placed::Paper(found));
        }
    }
    None
}
