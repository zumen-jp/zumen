//! 寸法・引出線・引出線つきの注記・表を、図面モデルにある値から描く（決定 0184）。
//!
//! 描いた部品（DXF の名前のないブロック）を持つ寸法と表は、配置をたどるときに部品の中へ進むので、ここでは
//! 描かない。部品がなければ、寸法は定義点から補助線・寸法線と寸法値の文字を、表は行と列からセルの枠の線と
//! 文字を作る。引出線と注記は、経路と端部記号と注記の文字を描く。

use zumen_base::{Ref, Stop, codes};
use zumen_drawing::transform::Point as Spot;
use zumen_drawing::{
    Arrowhead, Callout, CalloutContent, CellContent, CellFormat, Dimension, DimensionKind,
    Horizontal, Inherit, Label, Leader, PathKind, Piece, Scaled, Table, Vertical, Visit,
};
use zumen_geom2d::{self as geom, Xy};

use crate::layout::{entity_kind, text_flag};
use crate::{Bounds, Builder, TextItem, horizontal_code, length, map, turned, vertical_code};

/// 2 点の距離
fn distance(a: Xy, b: Xy) -> f64 {
    libm::hypot(b[0] - a[0], b[1] - a[1])
}

/// a から b への向き（度）
fn angle(a: Xy, b: Xy) -> f64 {
    libm::atan2(b[1] - a[1], b[0] - a[0]).to_degrees()
}

/// 読める向きにした文字の回転（度）。右から左へ向くなら半回転する
fn readable(degrees: f64) -> f64 {
    let turn = degrees.rem_euclid(360.0);
    if turn > 90.0 && turn <= 270.0 {
        degrees + 180.0
    } else {
        degrees
    }
}

/// 寸法値の数。小数第 2 位までにし、後ろの 0 を落とす
fn number(value: f64) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// 2 本の線（2 点ずつ）の交点
fn intersection([a, b]: [Xy; 2], [c, d]: [Xy; 2]) -> Option<Xy> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let denominator = r[0] * s[1] - r[1] * s[0];
    if denominator == 0.0 || !denominator.is_finite() {
        return None;
    }
    let f = ((c[0] - a[0]) * s[1] - (c[1] - a[1]) * s[0]) / denominator;
    Some([a[0] + r[0] * f, a[1] + r[1] * f])
}

/// 定義点から作った寸法の形（要素の空間）
struct Drawn {
    lines: Vec<Vec<Xy>>,
    /// 寸法値の文字の既定の位置
    middle: Xy,
    /// 寸法値の文字の既定の向き（度）
    along: f64,
    /// 測った値（長さか度）
    value: f64,
}

/// 寸法の線と、`[x の向き, y の向き]` で測った長さ
fn linear(first: Xy, second: Xy, line: Xy, degrees: f64) -> Drawn {
    let r = degrees.to_radians();
    let d = [libm::cos(r), libm::sin(r)];
    let n = [-d[1], d[0]];
    let foot = |p: Xy| {
        let k = n[0] * (line[0] - p[0]) + n[1] * (line[1] - p[1]);
        [p[0] + n[0] * k, p[1] + n[1] * k]
    };
    let (f1, f2) = (foot(first), foot(second));
    Drawn {
        lines: vec![vec![first, f1], vec![second, f2], vec![f1, f2]],
        middle: [(f1[0] + f2[0]) / 2.0, (f1[1] + f2[1]) / 2.0],
        along: degrees,
        value: (d[0] * (second[0] - first[0]) + d[1] * (second[1] - first[1])).abs(),
    }
}

impl Builder<'_, '_, '_> {
    /// 要素の空間の点・向き・高さで、文字を 1 つ置く
    #[allow(clippy::too_many_arguments)]
    fn label(
        &mut self,
        visit: &Visit<'_>,
        at: Xy,
        degrees: f64,
        height: f64,
        content: &str,
        align: [u8; 2],
        font: u32,
        unit: u32,
    ) {
        if content.is_empty() || !(height.is_finite() && height > 0.0) {
            return;
        }
        let t = visit.transform;
        let (rotation, along) = turned(&t, degrees);
        let (_, up) = turned(&t, degrees + 90.0);
        let [a, b, _, d, e, _] = t.m;
        let string = self.strings.add(content);
        self.put_text(TextItem {
            string,
            at: map(&t, at),
            second: None,
            height: height * up,
            width: if up > 0.0 { along / up } else { 1.0 },
            rotation,
            oblique: 0.0,
            box_width: 0.0,
            align,
            flags: if a * e - b * d < 0.0 {
                text_flag::UPSIDE_DOWN
            } else {
                0
            },
            line: 0,
            font,
            unit,
            bounds: Bounds::default(),
            glyphs: Vec::new(),
        });
    }

    /// 起点の座標の、`from` から `tip` へ向く塗った閉じた矢印。幅は長さの 1/3
    fn arrow(&mut self, tip: Xy, from: Xy, size: f64, unit: u32) {
        let length = distance(from, tip);
        if !(length > 0.0 && size > 0.0 && size.is_finite()) {
            return;
        }
        let d = [(tip[0] - from[0]) / length, (tip[1] - from[1]) / length];
        let back = [tip[0] - d[0] * size, tip[1] - d[1] * size];
        let half = size / 6.0;
        let n = [-d[1] * half, d[0] * half];
        self.face(
            vec![vec![
                tip,
                [back[0] + n[0], back[1] + n[1]],
                [back[0] - n[0], back[1] - n[1]],
            ]],
            unit,
        );
    }

    /// 要素の空間の点を通る線。`curve` なら、点を通る 3 次のスプラインにする
    fn path(&mut self, points: Vec<Xy>, curve: bool, local: f64) -> Vec<Xy> {
        if !curve || points.len() < 3 {
            return points;
        }
        match geom::interpolate(&points, None, None, geom::Spacing::Chord, false)
            .and_then(|(knots, control)| geom::spline(3, &knots, &control, &[], local))
        {
            Some(found) => {
                self.approximated = true;
                found
            }
            None => points,
        }
    }

    /// 変換の、面積の倍率の平方根（長さの平均の倍率）
    fn grow(visit: &Visit<'_>) -> f64 {
        let [a, b, _, d, e, _] = visit.transform.m;
        (a * e - b * d).abs().sqrt()
    }

    /// 描いた部品のない寸法を、定義点から描く
    pub(crate) fn dimension<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        dimension: &Dimension<P>,
        local: f64,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        let angular = |builder: &mut Self, vertex: Xy, a: Xy, b: Xy, through: Xy| {
            let radius = distance(vertex, through);
            let (a1, a2, am) = (angle(vertex, a), angle(vertex, b), angle(vertex, through));
            let forward = geom::sweep(a1, a2, true);
            let (start, sweep) = if (am - a1).rem_euclid(360.0) <= forward {
                (a1, forward)
            } else {
                (a2, geom::sweep(a2, a1, true))
            };
            let mut points = Vec::new();
            geom::arc(vertex, radius, start, sweep, local, &mut points);
            builder.approximated = true;
            let half = (start + sweep / 2.0).to_radians();
            Drawn {
                lines: vec![points],
                middle: [
                    vertex[0] + radius * libm::cos(half),
                    vertex[1] + radius * libm::sin(half),
                ],
                along: (start + sweep / 2.0) - 90.0,
                value: sweep,
            }
        };
        let drawn = match dimension.kind {
            DimensionKind::Linear {
                first,
                second,
                line,
                rotation,
                ..
            } => Some(linear(first.xy(), second.xy(), line.xy(), rotation)),
            DimensionKind::Aligned {
                first,
                second,
                line,
                ..
            } => {
                let (a, b) = (first.xy(), second.xy());
                Some(linear(a, b, line.xy(), angle(a, b)))
            }
            DimensionKind::Radius { center, chord, .. } => {
                let (c, p) = (center.xy(), chord.xy());
                Some(Drawn {
                    lines: vec![vec![c, p]],
                    middle: p,
                    along: angle(c, p),
                    value: distance(c, p),
                })
            }
            DimensionKind::Diameter {
                chord, opposite, ..
            } => {
                let (a, b) = (chord.xy(), opposite.xy());
                Some(Drawn {
                    lines: vec![vec![a, b]],
                    middle: [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0],
                    along: angle(a, b),
                    value: distance(a, b),
                })
            }
            DimensionKind::Ordinate {
                origin,
                feature,
                leader_end,
                axis,
            } => {
                let (o, f, e) = (origin.xy(), feature.xy(), leader_end.xy());
                Some(Drawn {
                    lines: vec![vec![f, e]],
                    middle: e,
                    along: angle(f, e),
                    value: match axis {
                        zumen_drawing::Axis::X => f[0] - o[0],
                        zumen_drawing::Axis::Y => f[1] - o[1],
                    },
                })
            }
            DimensionKind::Angular {
                vertex,
                first,
                second,
                arc,
            } => Some(angular(
                self,
                vertex.xy(),
                first.xy(),
                second.xy(),
                arc.xy(),
            )),
            DimensionKind::AngularLines { first, second, arc } => {
                let a = [first[0].xy(), first[1].xy()];
                let b = [second[0].xy(), second[1].xy()];
                intersection(a, b).map(|vertex| {
                    let far = |[p, q]: [Xy; 2]| {
                        if distance(vertex, p) >= distance(vertex, q) {
                            p
                        } else {
                            q
                        }
                    };
                    angular(self, vertex, far(a), far(b), arc.xy())
                })
            }
            DimensionKind::ArcLength {
                center,
                first,
                second,
                arc,
            } => {
                let (c, a, b) = (center.xy(), first.xy(), second.xy());
                let mut drawn = angular(self, c, a, b, arc.xy());
                drawn.value = distance(c, a) * drawn.value.to_radians();
                Some(drawn)
            }
        };
        let Some(drawn) = drawn else {
            self.report(codes::SCENE_DIMENSION_DROPPED, source);
            return Ok(());
        };
        let unit = self.unit(visit, entity_kind::DIMENSION, unit);
        let t = visit.transform;
        for line in drawn.lines {
            let points = line.into_iter().map(|p| map(&t, p)).collect();
            self.stroke(visit, points, unit, source)?;
        }
        // 寸法値は、書式の高さを持つ区間があるときだけ描く。高さは最初の区間の値
        if let Label::Override { spans, .. } = &dimension.label {
            let height = spans.iter().find_map(|span| match span.format.height {
                Some(Scaled::Value(value)) => Some(length(value)),
                _ => None,
            });
            let font = spans
                .iter()
                .find_map(|span| span.format.font.as_ref())
                .map(|f| f.name.clone());
            let mut content = String::new();
            for span in spans {
                match &span.piece {
                    Piece::Text(text) => content.push_str(text),
                    Piece::Measurement => content.push_str(&number(drawn.value)),
                    Piece::Paragraph => content.push(' '),
                    _ => {}
                }
            }
            if let Some(height) = height {
                let at = dimension.text.position.map_or(drawn.middle, |p| p.xy());
                let degrees = dimension
                    .text
                    .rotation
                    .unwrap_or_else(|| readable(drawn.along));
                let (h, v) = dimension
                    .text
                    .attachment
                    .unwrap_or((Horizontal::Center, Vertical::Middle));
                let font = self.font(font.as_deref());
                self.label(
                    visit,
                    at,
                    degrees,
                    height,
                    &content,
                    [horizontal_code(h), vertical_code(v)],
                    font,
                    unit,
                );
            }
        }
        self.report(codes::SCENE_DIMENSION_APPROXIMATED, source);
        Ok(())
    }

    /// 引出線の経路と、寸法スタイルの端部記号
    pub(crate) fn leader<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        leader: &Leader<P>,
        local: f64,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        let points: Vec<Xy> = leader.path.iter().map(|p| p.xy()).collect();
        let (Some(&tip), Some(&next)) = (points.first(), points.get(1)) else {
            self.report(codes::SCENE_LEADER_DROPPED, source);
            return Ok(());
        };
        let unit = self.unit(visit, entity_kind::LEADER, unit);
        let t = visit.transform;
        let line = self.path(points, leader.spline, local);
        self.segments(line.len(), source)?;
        let line = line.into_iter().map(|p| map(&t, p)).collect();
        self.stroke(visit, line, unit, source)?;
        if leader.arrow == Some(false) {
            return Ok(());
        }
        let style = self.drawing.dimension_style(&leader.style);
        match style {
            Some(style) if style.leader == Arrowhead::Default && style.size > 0.0 => {
                let scale = if style.scale > 0.0 { style.scale } else { 1.0 };
                let size = style.size * scale * Self::grow(visit);
                self.arrow(map(&t, tip), map(&t, next), size, unit);
            }
            _ => self.report(codes::SCENE_ARROWHEAD_DROPPED, source),
        }
        Ok(())
    }

    /// 引出線つきの注記の枝と折れ曲がりと端部記号と、注記の文字
    pub(crate) fn callout<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        callout: &Callout<P>,
        local: f64,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        let unit = self.unit(visit, entity_kind::CALLOUT, unit);
        let t = visit.transform;
        let line_unit = match callout.line_style.color {
            Inherit::Block => unit,
            color => {
                let color = self.inherited(visit, color);
                self.recolor(unit, color)
            }
        };
        let scale = if callout.scale > 0.0 {
            callout.scale
        } else {
            1.0
        };
        let size = length(callout.arrow_size) * scale * Self::grow(visit);
        let mut dropped = false;
        for branch in &callout.branches {
            for line in &branch.lines {
                let mut points: Vec<Xy> = line.iter().map(|p| p.xy()).collect();
                points.extend(branch.end.map(|p| p.xy()));
                if callout.path == PathKind::Hidden || points.len() < 2 {
                    continue;
                }
                let (Some(&tip), Some(&next)) = (points.first(), points.get(1)) else {
                    continue;
                };
                let drawn = self.path(points, callout.path == PathKind::Spline, local);
                self.segments(drawn.len(), source)?;
                let drawn = drawn.into_iter().map(|p| map(&t, p)).collect();
                self.stroke(visit, drawn, line_unit, source)?;
                match callout.arrowhead {
                    Arrowhead::Default => self.arrow(map(&t, tip), map(&t, next), size, line_unit),
                    Arrowhead::Symbol(_) => dropped = true,
                }
            }
            if callout.landing
                && callout.path != PathKind::Hidden
                && let (Some(end), Some(dogleg)) = (branch.end, branch.dogleg)
            {
                let [dx, dy] = dogleg.direction;
                let norm = libm::hypot(dx, dy);
                if norm > 0.0 {
                    let e = end.xy();
                    let reach = length(dogleg.length) * scale / norm;
                    let points = vec![map(&t, e), map(&t, [e[0] + dx * reach, e[1] + dy * reach])];
                    self.stroke(visit, points, line_unit, source)?;
                }
            }
        }
        if dropped {
            self.report(codes::SCENE_ARROWHEAD_DROPPED, source);
        }
        match &callout.content {
            CalloutContent::None => {}
            CalloutContent::Text(text) => {
                let content_unit = match callout.content_color {
                    Inherit::Block => unit,
                    color => {
                        let color = self.inherited(visit, color);
                        self.recolor(unit, color)
                    }
                };
                self.formatted(visit, text, content_unit)?;
            }
            // 記号の中身は、部品としてたどった要素として描く
            CalloutContent::Symbol { values, .. } => {
                if !values.is_empty() {
                    self.report(codes::SCENE_CALLOUT_VALUE_DROPPED, source);
                }
            }
        }
        Ok(())
    }

    /// 描いた部品のない表を、行と列から描く
    pub(crate) fn table<P: Spot>(
        &mut self,
        visit: &Visit<'_>,
        table: &Table<P>,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        let unit = self.unit(visit, entity_kind::TABLE, unit);
        let t = visit.transform;
        let heights: Vec<f64> = table.row_heights.iter().map(|h| length(*h)).collect();
        let widths: Vec<f64> = table.column_widths.iter().map(|w| length(*w)).collect();
        let (rows, columns) = (heights.len(), widths.len());
        if rows == 0 || columns == 0 {
            return Ok(());
        }
        let ys: Vec<f64> = std::iter::once(0.0)
            .chain(heights.iter().scan(0.0, |sum, h| {
                *sum += h;
                Some(*sum)
            }))
            .collect();
        let xs: Vec<f64> = std::iter::once(0.0)
            .chain(widths.iter().scan(0.0, |sum, w| {
                *sum += w;
                Some(*sum)
            }))
            .collect();
        let [dx, dy] = table.direction;
        let norm = libm::hypot(dx, dy);
        let d = if norm > 0.0 {
            [dx / norm, dy / norm]
        } else {
            [1.0, 0.0]
        };
        let degrees = angle([0.0, 0.0], d);
        let o = table.position.xy();
        // 行は下へ進む。y は上の辺からの距離
        let at = |x: f64, y: f64| [o[0] + d[0] * x + d[1] * y, o[1] + d[1] * x - d[0] * y];
        let (width, height) = (
            xs.last().copied().unwrap_or(0.0),
            ys.last().copied().unwrap_or(0.0),
        );
        self.segments(rows.saturating_mul(columns).saturating_mul(4) + 4, source)?;
        let mut edges = vec![
            [at(width, 0.0), at(width, height)],
            [at(0.0, height), at(width, height)],
        ];
        let kind = |row: usize| -> Option<&CellFormat> {
            let formats = table.formats.as_deref()?;
            let title = usize::from(formats.title);
            Some(if formats.title && row == 0 {
                &formats.title_format
            } else if formats.header && row == title {
                &formats.header_format
            } else {
                &formats.data_format
            })
        };
        for row in 0..rows {
            for column in 0..columns {
                let Some(cell) = table.cells.get(row * columns + column) else {
                    continue;
                };
                if cell.covered {
                    continue;
                }
                let [across, down] = cell.span;
                let end_column = (column + across.max(1) as usize).min(columns);
                let end_row = (row + down.max(1) as usize).min(rows);
                let (Some(&x0), Some(&x1), Some(&y0), Some(&y1)) = (
                    xs.get(column),
                    xs.get(end_column),
                    ys.get(row),
                    ys.get(end_row),
                ) else {
                    continue;
                };
                edges.push([at(x0, y0), at(x1, y0)]);
                edges.push([at(x0, y0), at(x0, y1)]);
                let CellContent::Text { spans, .. } = &cell.content else {
                    continue;
                };
                let row_format = kind(row);
                let Some(text_height) = cell
                    .format
                    .text_height
                    .or_else(|| row_format.and_then(|f| f.text_height))
                    .map(length)
                else {
                    continue;
                };
                let (h, v) = cell
                    .format
                    .alignment
                    .or_else(|| row_format.and_then(|f| f.alignment))
                    .unwrap_or((Horizontal::Center, Vertical::Middle));
                let margin = text_height / 2.0;
                let x = match h {
                    Horizontal::Left => x0 + margin,
                    Horizontal::Right => x1 - margin,
                    Horizontal::Center | Horizontal::Middle => (x0 + x1) / 2.0,
                };
                let y = match v {
                    Vertical::Top => y0 + margin,
                    Vertical::Bottom | Vertical::Baseline => y1 - margin,
                    Vertical::Middle => (y0 + y1) / 2.0,
                };
                let mut content = String::new();
                for span in spans {
                    match &span.piece {
                        Piece::Text(text) => content.push_str(text),
                        Piece::Paragraph => content.push(' '),
                        _ => {}
                    }
                }
                let cell_unit = match cell
                    .format
                    .text_color
                    .or_else(|| row_format.and_then(|f| f.text_color))
                {
                    None | Some(Inherit::Block) => unit,
                    Some(color) => {
                        let color = self.inherited(visit, color);
                        self.recolor(unit, color)
                    }
                };
                let style = cell
                    .format
                    .text_style
                    .as_deref()
                    .or_else(|| row_format.and_then(|f| f.text_style.as_deref()))
                    .and_then(|name| self.drawing.text_style(name))
                    .and_then(|s| s.definition.as_ref())
                    .and_then(|s| s.font.as_ref())
                    .map(|f| f.name.clone());
                let font = self.font(style.as_deref());
                self.label(
                    visit,
                    at(x, y),
                    degrees + cell.rotation,
                    text_height,
                    &content,
                    [horizontal_code(h), vertical_code(v)],
                    font,
                    cell_unit,
                );
            }
        }
        for [a, b] in edges {
            self.line(vec![map(&t, a), map(&t, b)], unit);
        }
        self.report(codes::SCENE_TABLE_APPROXIMATED, source);
        Ok(())
    }
}
