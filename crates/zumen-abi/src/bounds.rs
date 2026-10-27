//! entities の範囲（決定 0118）。形の定義から求め、文字の字形・線の太さ・記号の中身は含めない。
//! 三角関数と逆正接は、どの環境でも同じ値になる `libm` で求める（決定 0007）

use zumen_drawing::{
    Boundary, Callout, CalloutContent, DimensionKind, Edge, Length, ModelPoint, PaperPoint,
    Placement, Shape, Symbol, Table, Text, UnderlayFrame, Vertex,
};

use crate::messages::Extent;

/// 要素の座標の点
pub(crate) trait Planar: Copy {
    /// X と Y
    fn xy(self) -> (f64, f64);
    /// この点と同じ座標の系の長さなら、その値
    fn length(self, length: Length) -> Option<f64>;
}

impl Planar for ModelPoint {
    fn xy(self) -> (f64, f64) {
        (self.x, self.y)
    }

    fn length(self, length: Length) -> Option<f64> {
        match length {
            Length::Model(model) if model.space == self.space => Some(model.value),
            Length::Model(_) | Length::Paper(_) => None,
        }
    }
}

impl Planar for PaperPoint {
    fn xy(self) -> (f64, f64) {
        (self.x, self.y)
    }

    fn length(self, length: Length) -> Option<f64> {
        match length {
            Length::Paper(paper) => Some(paper.0),
            Length::Model(_) => None,
        }
    }
}

/// 範囲が長方形と重なるか接する
pub(crate) fn overlaps(extent: &Extent, region: &Extent) -> bool {
    extent.min_x <= region.max_x
        && extent.max_x >= region.min_x
        && extent.min_y <= region.max_y
        && extent.max_y >= region.min_y
}

/// 形の範囲。囲む点がなければ `None`
pub(crate) fn bbox<P: Planar>(shape: &Shape<P>) -> Option<Extent> {
    let mut b = Bounds(None);
    match shape {
        Shape::Line { start, end } => {
            b.point(*start);
            b.point(*end);
        }
        Shape::Circle { center, radius } => match center.length(*radius) {
            Some(r) => b.arc(center.xy(), r, 0.0, 360.0),
            None => b.point(*center),
        },
        Shape::Curve(edge) => b.edge(edge),
        Shape::Polyline(polyline) => b.vertices(&polyline.vertices, polyline.closed),
        Shape::Text(text) => b.text(text),
        Shape::Formatted(formatted) => b.point(formatted.point),
        Shape::Dimension(dimension) => {
            match &dimension.kind {
                DimensionKind::Linear {
                    first,
                    second,
                    line,
                    ..
                }
                | DimensionKind::Aligned {
                    first,
                    second,
                    line,
                    ..
                } => {
                    b.points([*first, *second, *line]);
                }
                DimensionKind::Angular {
                    vertex,
                    first,
                    second,
                    arc,
                } => {
                    b.points([*vertex, *first, *second, *arc]);
                }
                DimensionKind::AngularLines { first, second, arc } => {
                    b.points([first[0], first[1], second[0], second[1], *arc]);
                }
                DimensionKind::Radius { center, chord, .. } => b.points([*center, *chord]),
                DimensionKind::Diameter {
                    chord, opposite, ..
                } => b.points([*chord, *opposite]),
                DimensionKind::Ordinate {
                    origin,
                    feature,
                    leader_end,
                    ..
                } => {
                    b.points([*origin, *feature, *leader_end]);
                }
            }
            if let Some(position) = dimension.text.position {
                b.point(position);
            }
        }
        Shape::Leader(leader) => b.points(leader.path.iter().copied()),
        Shape::Callout(callout) => b.callout(callout),
        Shape::Table(table) => b.table(table),
        Shape::Underlay(underlay) => match &underlay.frame {
            UnderlayFrame::Pixels {
                origin, right, top, ..
            } => {
                b.points([*origin, *right, *top]);
                let [(ox, oy), (rx, ry), (tx, ty)] = [origin.xy(), right.xy(), top.xy()];
                b.add(rx + tx - ox, ry + ty - oy);
            }
            UnderlayFrame::Scaled { origin, .. } => b.point(*origin),
        },
        Shape::Symbol(symbol) => b.symbol(symbol),
        Shape::Hatch(hatch) => {
            for hatch_loop in &hatch.loops {
                match &hatch_loop.boundary {
                    Boundary::Polyline { vertices, closed } => b.vertices(vertices, *closed),
                    Boundary::Edges(edges) => edges.iter().for_each(|edge| b.edge(edge)),
                }
            }
        }
    }
    b.0
}

/// 囲む長方形を、点を足しながら広げる
struct Bounds(Option<Extent>);

impl Bounds {
    /// 点を足す。有限でない点は足さない
    fn add(&mut self, x: f64, y: f64) {
        if !(x.is_finite() && y.is_finite()) {
            return;
        }
        self.0 = Some(match self.0.take() {
            None => Extent {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            },
            Some(e) => Extent {
                min_x: e.min_x.min(x),
                min_y: e.min_y.min(y),
                max_x: e.max_x.max(x),
                max_y: e.max_y.max(y),
            },
        });
    }

    fn point<P: Planar>(&mut self, point: P) {
        let (x, y) = point.xy();
        self.add(x, y);
    }

    fn points<P: Planar>(&mut self, points: impl IntoIterator<Item = P>) {
        points.into_iter().for_each(|point| self.point(point));
    }

    /// 中心と半径の円を、角度 `from`（度）から反時計回りに `sweep` 度たどる間の、X と Y の極値を足す。
    /// 端点は足さない
    fn arc(&mut self, (cx, cy): (f64, f64), r: f64, from: f64, sweep: f64) {
        let extremes = [
            (0.0, cx + r, cy),
            (90.0, cx, cy + r),
            (180.0, cx - r, cy),
            (270.0, cx, cy - r),
        ];
        for (angle, x, y) in extremes {
            if (angle - from).rem_euclid(360.0) <= sweep {
                self.add(x, y);
            }
        }
    }

    fn edge<P: Planar>(&mut self, edge: &Edge<P>) {
        match edge {
            Edge::Line { start, end } => self.points([*start, *end]),
            Edge::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            } => {
                let Some(r) = center.length(*radius) else {
                    return self.point(*center);
                };
                let (cx, cy) = center.xy();
                for angle in [*start, *end] {
                    let t = angle.to_radians();
                    self.add(cx + r * libm::cos(t), cy + r * libm::sin(t));
                }
                let (from, sweep) = sweep(*start, *end, *counterclockwise);
                self.arc((cx, cy), r, from, sweep);
            }
            Edge::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            } => {
                let (cx, cy) = center.xy();
                let (ax, ay) = major.xy();
                let (mx, my) = (ax - cx, ay - cy);
                let (nx, ny) = (-my * ratio, mx * ratio);
                let mut at = |degrees: f64| {
                    let t = degrees.to_radians();
                    let (c, s) = (libm::cos(t), libm::sin(t));
                    self.add(cx + mx * c + nx * s, cy + my * c + ny * s);
                };
                at(*start);
                at(*end);
                // X と Y が極値になる角度は、それぞれ atan2 で求めた角度と、その反対
                let (from, sweep) = sweep(*start, *end, *counterclockwise);
                for (along, across) in [(mx, nx), (my, ny)] {
                    let extreme = libm::atan2(across, along).to_degrees();
                    for angle in [extreme, extreme + 180.0] {
                        if (angle - from).rem_euclid(360.0) <= sweep {
                            at(angle);
                        }
                    }
                }
            }
            Edge::Spline(spline) => {
                self.points(spline.control.iter().copied());
                self.points(spline.fit.iter().copied());
            }
        }
    }

    /// 頂点と、膨らみのある区間の極値を足す
    fn vertices<P: Planar>(&mut self, vertices: &[Vertex<P>], closed: bool) {
        self.points(vertices.iter().map(|vertex| vertex.point));
        let next = vertices
            .iter()
            .skip(1)
            .chain(vertices.first().filter(|_| closed));
        for (vertex, next) in vertices.iter().zip(next) {
            let bulge = vertex.bulge;
            if bulge == 0.0 || !bulge.is_finite() {
                continue;
            }
            let ((px, py), (qx, qy)) = (vertex.point.xy(), next.point.xy());
            let (dx, dy) = (qx - px, qy - py);
            if dx == 0.0 && dy == 0.0 {
                continue;
            }
            // 弦の中点から、弦に垂直に (1 - b²) / 4b だけ弦の長さに比べて動かした所が中心
            let f = (1.0 - bulge * bulge) / (4.0 * bulge);
            let (cx, cy) = ((px + qx) / 2.0 - f * dy, (py + qy) / 2.0 + f * dx);
            let r = ((px - cx) * (px - cx) + (py - cy) * (py - cy)).sqrt();
            // 反時計回りなら始点から、時計回りなら終点から反時計回りにたどる
            let (sx, sy) = if bulge > 0.0 { (px, py) } else { (qx, qy) };
            let from = libm::atan2(sy - cy, sx - cx).to_degrees();
            let sweep = 4.0 * libm::atan(bulge.abs()).to_degrees();
            self.arc((cx, cy), r, from, sweep);
        }
    }

    fn text<P: Planar>(&mut self, text: &Text<P>) {
        match text.placement {
            Placement::Anchor { point, .. } => self.point(point),
            Placement::Between { first, second, .. } => self.points([first, second]),
        }
    }

    fn callout<P: Planar>(&mut self, callout: &Callout<P>) {
        for branch in &callout.branches {
            self.points(branch.lines.iter().flatten().copied());
            self.points(branch.end);
        }
        match &callout.content {
            CalloutContent::Text(text) => self.point(text.point),
            CalloutContent::Symbol { symbol, .. } => self.symbol(symbol),
            CalloutContent::None => {}
        }
    }

    /// 挿入点から、行の向きに列の幅の和、それに垂直な下向きに行の高さの和で作る格子の角を足す
    fn table<P: Planar>(&mut self, table: &Table<P>) {
        self.point(table.position);
        let [dx, dy] = table.direction;
        let norm = (dx * dx + dy * dy).sqrt();
        let sum = |lengths: &[Length]| -> Option<f64> {
            lengths.iter().map(|l| table.position.length(*l)).sum()
        };
        let (Some(width), Some(height)) = (sum(&table.column_widths), sum(&table.row_heights))
        else {
            return;
        };
        if !(norm > 0.0 && norm.is_finite()) {
            return;
        }
        let (ux, uy) = (dx / norm, dy / norm);
        let (px, py) = table.position.xy();
        self.add(px + width * ux, py + width * uy);
        self.add(px + height * uy, py - height * ux);
        self.add(px + width * ux + height * uy, py + width * uy - height * ux);
    }

    fn symbol<P: Planar>(&mut self, symbol: &Symbol<P>) {
        self.point(symbol.position);
        for attribute in &symbol.attributes {
            self.text(&attribute.text);
            if let Some(formatted) = &attribute.formatted {
                self.point(formatted.point);
            }
        }
    }
}

/// 反時計回りにたどる始まりの角度と、たどる角度（0 以上 360 以下、度）。1 周以上の差は 1 周とする
fn sweep(start: f64, end: f64, counterclockwise: bool) -> (f64, f64) {
    let (from, to) = if counterclockwise {
        (start, end)
    } else {
        (end, start)
    };
    let difference = to - from;
    if difference.abs() >= 360.0 {
        (from, 360.0)
    } else {
        (from, difference.rem_euclid(360.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_drawing::{ModelLength, PaperLength, Polyline, SpaceId};

    fn at(x: f64, y: f64) -> ModelPoint {
        ModelPoint {
            space: SpaceId(0),
            x,
            y,
        }
    }

    fn model(value: f64) -> Length {
        Length::Model(ModelLength {
            space: SpaceId(0),
            value,
        })
    }

    fn rounded(extent: Option<Extent>) -> Option<[f64; 4]> {
        let round = |v: f64| (v * 1e9).round() / 1e9;
        extent.map(|e| {
            [
                round(e.min_x),
                round(e.min_y),
                round(e.max_x),
                round(e.max_y),
            ]
        })
    }

    fn arc(start: f64, end: f64, counterclockwise: bool) -> Shape<ModelPoint> {
        Shape::Curve(Edge::Arc {
            center: at(0.0, 0.0),
            radius: model(2.0),
            start,
            end,
            counterclockwise,
        })
    }

    #[test]
    fn circles_and_arcs_cover_the_extremes_they_pass() {
        let circle = Shape::Circle {
            center: at(1.0, 1.0),
            radius: model(2.0),
        };
        assert_eq!(rounded(bbox(&circle)), Some([-1.0, -1.0, 3.0, 3.0]));
        assert_eq!(
            rounded(bbox(&arc(0.0, 90.0, true))),
            Some([0.0, 0.0, 2.0, 2.0])
        );
        // 時計回りに 0 度から 90 度へは、180 度と 270 度を通る
        assert_eq!(
            rounded(bbox(&arc(0.0, 90.0, false))),
            Some([-2.0, -2.0, 2.0, 2.0])
        );
        assert_eq!(
            rounded(bbox(&arc(350.0, 10.0, true))).map(|e| e[2]),
            Some(2.0)
        );
        // 長さが用紙の系なら、中心だけを囲む
        let paper = Shape::Circle {
            center: at(1.0, 1.0),
            radius: Length::Paper(PaperLength(2.0)),
        };
        assert_eq!(rounded(bbox(&paper)), Some([1.0, 1.0, 1.0, 1.0]));
    }

    #[test]
    fn ellipses_cover_their_rotated_extremes() {
        let full = Shape::Curve(Edge::Elliptic {
            center: at(0.0, 0.0),
            major: at(0.0, 4.0),
            ratio: 0.5,
            start: 0.0,
            end: 360.0,
            counterclockwise: true,
        });
        assert_eq!(rounded(bbox(&full)), Some([-2.0, -4.0, 2.0, 4.0]));
    }

    #[test]
    fn bulges_cover_the_arc_between_vertices() {
        let vertex = |x: f64, bulge: f64| Vertex {
            point: at(x, 0.0),
            bulge,
        };
        let polyline = |bulge: f64| {
            Shape::Polyline(Polyline {
                vertices: vec![vertex(0.0, bulge), vertex(2.0, 0.0)],
                closed: false,
                width: None,
                vertex_widths: Vec::new(),
            })
        };
        // 膨らみ 1 は半円。正なら反時計回りで、弦の右（下）にふくらむ
        assert_eq!(rounded(bbox(&polyline(1.0))), Some([0.0, -1.0, 2.0, 0.0]));
        assert_eq!(rounded(bbox(&polyline(-1.0))), Some([0.0, 0.0, 2.0, 1.0]));
        assert_eq!(rounded(bbox(&polyline(0.0))), Some([0.0, 0.0, 2.0, 0.0]));
    }

    #[test]
    fn regions_include_touching_extents() {
        let extent = Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 1.0,
            max_y: 1.0,
        };
        let touching = Extent {
            min_x: 1.0,
            min_y: 1.0,
            max_x: 2.0,
            max_y: 2.0,
        };
        let apart = Extent {
            min_x: 1.5,
            min_y: 0.0,
            max_x: 2.0,
            max_y: 1.0,
        };
        assert!(overlaps(&extent, &touching));
        assert!(!overlaps(&extent, &apart));
    }
}
