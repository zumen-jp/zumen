//! 寸法・引出線・引出線つきの注記・表を、図面モデルにある値から描く（決定 0184）。
//!
//! 描いた部品（DXF の名前のないブロック）を持つ寸法と表は、配置をたどるときに部品の中へ進むので、ここでは
//! 描かない。部品がなければ、寸法は定義点から補助線・寸法線と寸法値の文字を、表は行と列からセルの枠の線と
//! 文字を作る。引出線と注記は、経路と端部記号と注記の文字を描く。

use zumen_base::{Ref, Stop, codes};
use zumen_drawing::parts::{self, DimensionCurve, DimensionOffsets};
use zumen_drawing::transform::Point as Spot;
use zumen_drawing::{
    Arrowhead, Callout, CalloutContent, Dimension, Horizontal, Inherit, Label, Leader, PathKind,
    Piece, Scaled, Table, Vertical, VisitRef,
};
use zumen_geom2d::{self as geom, Xy};

use crate::layout::{entity_kind, text_flag};
use crate::{Bounds, Builder, TextItem, horizontal_code, length, map, turned, vertical_code};

impl Builder<'_, '_, '_> {
    /// 要素の空間の点・向き・高さで、文字を 1 つ置く
    #[allow(clippy::too_many_arguments)]
    fn label(
        &mut self,
        visit: &VisitRef<'_, '_>,
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
        if let Some([a, b, c]) =
            parts::arrow_triangle(tip, [tip[0] - from[0], tip[1] - from[1]], size)
        {
            self.face(vec![vec![a, b, c]], unit);
        }
    }

    /// 要素の空間の点を通る線。`curve` なら、点を通る 3 次のスプラインにし、点を求める手間を数える（#756）
    fn path(
        &mut self,
        points: Vec<Xy>,
        curve: bool,
        local: f64,
        source: &Ref,
    ) -> Result<Vec<Xy>, (Stop, Ref)> {
        if !curve || points.len() < 3 {
            return Ok(points);
        }
        let Some((knots, control)) =
            geom::interpolate(&points, None, None, geom::Spacing::Chord, false)
        else {
            return Ok(points);
        };
        let (found, work) = geom::spline_with_work(3, &knots, &control, &[], local);
        self.work(work, source)?;
        Ok(match found {
            Some(found) => {
                self.approximated = true;
                found
            }
            None => points,
        })
    }

    /// 変換の、面積の倍率の平方根（長さの平均の倍率）
    fn grow(visit: &VisitRef<'_, '_>) -> f64 {
        let [a, b, _, d, e, _] = visit.transform.m;
        (a * e - b * d).abs().sqrt()
    }

    /// 描いた部品のない寸法を、定義点から描く
    pub(crate) fn dimension<P: Spot>(
        &mut self,
        visit: &VisitRef<'_, '_>,
        dimension: &Dimension<P>,
        local: f64,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        let Some(drawn) = parts::dimension(
            dimension,
            |p| Some(p.xy()),
            |l| Some(length(*l)),
            DimensionOffsets::default(),
        ) else {
            self.report(codes::SCENE_DIMENSION_DROPPED, source);
            return Ok(());
        };
        let unit = self.unit(visit, entity_kind::DIMENSION, unit)?;
        let t = visit.transform;
        for [a, b] in drawn.extension_lines {
            self.stroke(visit, vec![map(&t, a), map(&t, b)], unit, source)?;
        }
        if let Some(curve) = drawn.curve {
            let points = match curve {
                DimensionCurve::Line([a, b]) => vec![map(&t, a), map(&t, b)],
                DimensionCurve::Arc {
                    center,
                    radius,
                    start,
                    sweep,
                } => {
                    let mut points = Vec::new();
                    geom::arc(center, radius, start, sweep, local, &mut points);
                    self.approximated = true;
                    points.into_iter().map(|p| map(&t, p)).collect()
                }
            };
            self.stroke(visit, points, unit, source)?;
        }
        if let Some(style) = self.drawing.dimension_style(&dimension.style) {
            let scale = if style.scale > 0.0 { style.scale } else { 1.0 };
            if let Some(tick) = style.tick.filter(|&v| v > 0.0) {
                for end in &drawn.ends {
                    if let Some([a, b]) = parts::tick_line(end.tip, end.toward_tip, tick * scale) {
                        self.stroke(visit, vec![map(&t, a), map(&t, b)], unit, source)?;
                    }
                }
            } else if style.size > 0.0 {
                let size = style.size * scale * Self::grow(visit);
                for end in &drawn.ends {
                    if style.ends.get(end.side) == Some(&Arrowhead::Default) {
                        let back = [
                            end.tip[0] - end.toward_tip[0],
                            end.tip[1] - end.toward_tip[1],
                        ];
                        self.arrow(map(&t, end.tip), map(&t, back), size, unit);
                    }
                }
            }
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
                    Piece::Measurement => {
                        content.push_str(&parts::format_measurement(drawn.display_value, 2));
                    }
                    Piece::Paragraph => content.push(' '),
                    _ => {}
                }
            }
            if let Some(height) = height {
                let at = dimension.text.position.map_or(drawn.middle, |p| p.xy());
                let degrees = dimension
                    .text
                    .rotation
                    .unwrap_or_else(|| parts::readable_angle(drawn.along));
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
        visit: &VisitRef<'_, '_>,
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
        let unit = self.unit(visit, entity_kind::LEADER, unit)?;
        let t = visit.transform;
        let line = self.path(points, leader.spline, local, source)?;
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
        visit: &VisitRef<'_, '_>,
        callout: &Callout<P>,
        local: f64,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        let unit = self.unit(visit, entity_kind::CALLOUT, unit)?;
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
                let drawn = self.path(points, callout.path == PathKind::Spline, local, source)?;
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
        visit: &VisitRef<'_, '_>,
        table: &Table<P>,
        source: &Ref,
        unit: &mut Option<u32>,
    ) -> Result<(), (Stop, Ref)> {
        let unit = self.unit(visit, entity_kind::TABLE, unit)?;
        let t = visit.transform;
        let (rows, columns) = (table.row_heights.len(), table.column_widths.len());
        if rows == 0 || columns == 0 {
            return Ok(());
        }
        self.segments(
            rows.saturating_mul(columns)
                .saturating_mul(4)
                .saturating_add(4),
            source,
        )?;
        let Some(drawn) = parts::table(table, table.position.xy(), |l| Some(length(*l)), None)
        else {
            return Ok(());
        };
        for item in drawn.texts {
            let cell_unit = match item.text_color {
                None | Some(Inherit::Block) => unit,
                Some(color) => {
                    let color = self.inherited(visit, color);
                    self.recolor(unit, color)
                }
            };
            let style = item
                .text_style
                .as_deref()
                .and_then(|name| self.drawing.text_style(name))
                .and_then(|s| s.definition.as_ref())
                .and_then(|s| s.font.as_ref())
                .map(|f| f.name.clone());
            let font = self.font(style.as_deref());
            self.label(
                visit,
                item.position,
                item.rotation,
                item.height,
                &item.plain,
                [
                    horizontal_code(item.alignment.0),
                    vertical_code(item.alignment.1),
                ],
                font,
                cell_unit,
            );
        }
        for [a, b] in drawn.borders {
            self.line(vec![map(&t, a), map(&t, b)], unit);
        }
        self.report(codes::SCENE_TABLE_APPROXIMATED, source);
        Ok(())
    }
}
