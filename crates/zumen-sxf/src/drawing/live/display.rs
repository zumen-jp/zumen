//! SXF 要素の外接矩形計算と差分表示バッファー同期。

use core::f64::consts::TAU;
use std::collections::BTreeMap;

use zumen_drawing::transform::Point as _;
use zumen_drawing::{
    Affine, Boundary, CalloutContent, Color, DimensionKind, Edge, FormattedText, Hatch, Inherit,
    Length, ModelPoint, PaperLength, PaperPoint, Placed, Placement, Rotation, Shape, SpaceId,
    SymbolKey, UnderlayFrame, transform,
};
use zumen_live::display::{Display, DisplayWriter, radians_from_deg};
use zumen_live::{CommittedTxn, EMPTY_BOUNDS, ExistingRowDiff, PatchPayload};

use super::{SxfContainer, SxfRow};

const DISPLAY_TOLERANCE: f64 = 0.5;

#[inline]
pub(super) fn pack_sxf_color(color: &Inherit<Color>) -> u32 {
    match color {
        Inherit::Own(c) => {
            if let Some([r, g, b]) = c.rgb {
                0x0100_0000 | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
            } else if let Some(code) = c.code {
                0x0200_0000 | (code.value & 0x00FF_FFFF)
            } else {
                0
            }
        }
        Inherit::Layer | Inherit::Block => 0,
    }
}

#[inline]
fn len_val(len: &Length) -> f64 {
    match len {
        Length::Paper(PaperLength(v)) => *v,
        Length::Model(m) => m.value,
    }
}

#[inline]
fn apply_affine([a, b, c, d, e, f]: [f64; 6], [x, y]: [f64; 2]) -> [f64; 2] {
    [a * x + b * y + c, d * x + e * y + f]
}

#[inline]
fn include_pt(acc: &mut Option<[f64; 4]>, [x, y]: [f64; 2]) {
    if !x.is_finite() || !y.is_finite() {
        return;
    }
    match acc {
        Some([x0, y0, x1, y1]) => {
            *x0 = x0.min(x);
            *y0 = y0.min(y);
            *x1 = x1.max(x);
            *y1 = y1.max(y);
        }
        None => *acc = Some([x, y, x, y]),
    }
}

#[inline]
fn union_bounds(acc: &mut Option<[f64; 4]>, [bx0, by0, bx1, by1]: [f64; 4]) {
    if bx0 > bx1 || by0 > by1 {
        return;
    }
    include_pt(acc, [bx0, by0]);
    include_pt(acc, [bx1, by1]);
}

pub(super) fn transform_bounds(bounds: [f64; 4], affine: &Affine) -> [f64; 4] {
    let [x0, y0, x1, y1] = bounds;
    if x0 > x1 || y0 > y1 {
        return EMPTY_BOUNDS;
    }
    let mut out = None;
    for corner in [[x0, y0], [x1, y0], [x1, y1], [x0, y1]] {
        include_pt(&mut out, apply_affine(affine.m, corner));
    }
    out.unwrap_or(EMPTY_BOUNDS)
}

pub(super) fn raw_shape_bounds<P: transform::Point>(shape: &Shape<P>) -> [f64; 4] {
    let mut acc: Option<[f64; 4]> = None;
    match shape {
        Shape::Line { start, end } => {
            include_pt(&mut acc, start.xy());
            include_pt(&mut acc, end.xy());
        }
        Shape::Point(dot) => {
            include_pt(&mut acc, dot.position.xy());
        }
        Shape::Circle { center, radius } => {
            let [cx, cy] = center.xy();
            let r = len_val(radius).abs();
            include_pt(&mut acc, [cx - r, cy - r]);
            include_pt(&mut acc, [cx + r, cy + r]);
        }
        Shape::Polyline(poly) => {
            for v in &poly.vertices {
                include_pt(&mut acc, v.point.xy());
            }
        }
        Shape::Curve(edge) => include_edge_bounds(&mut acc, edge),
        Shape::Text(text) => {
            let [px, py] = match text.placement {
                Placement::Anchor { point, .. } => point.xy(),
                Placement::Between { first, .. } => first.xy(),
            };
            let h = len_val(&text.height.value).abs();
            include_pt(&mut acc, [px - h, py - h]);
            include_pt(&mut acc, [px + h, py + h]);
        }
        Shape::Formatted(ft) => include_formatted_bounds(&mut acc, ft),
        Shape::Symbol(sym) => {
            include_pt(&mut acc, sym.position.xy());
        }
        Shape::Underlay(u) => match &u.frame {
            UnderlayFrame::Corners { origin, right, top }
            | UnderlayFrame::Pixels {
                origin, right, top, ..
            } => {
                let [ox, oy] = origin.xy();
                let [rx, ry] = right.xy();
                let [tx, ty] = top.xy();
                include_pt(&mut acc, [ox, oy]);
                include_pt(&mut acc, [rx, ry]);
                include_pt(&mut acc, [tx, ty]);
                include_pt(&mut acc, [rx + tx - ox, ry + ty - oy]);
            }
            UnderlayFrame::Scaled { origin, .. } => {
                include_pt(&mut acc, origin.xy());
            }
        },
        Shape::Hatch(hatch) => {
            for lp in &hatch.loops {
                match &lp.boundary {
                    Boundary::Polyline { vertices, .. } => {
                        for v in vertices {
                            include_pt(&mut acc, v.point.xy());
                        }
                    }
                    Boundary::Edges(edges) => {
                        for edge in edges {
                            include_edge_bounds(&mut acc, edge);
                        }
                    }
                }
            }
        }
        Shape::Dimension(dim) => {
            match &dim.kind {
                DimensionKind::Linear {
                    first,
                    second,
                    line,
                    ..
                } => {
                    include_pt(&mut acc, first.xy());
                    include_pt(&mut acc, second.xy());
                    include_pt(&mut acc, line.xy());
                }
                DimensionKind::Aligned {
                    first,
                    second,
                    line,
                    ..
                } => {
                    include_pt(&mut acc, first.xy());
                    include_pt(&mut acc, second.xy());
                    include_pt(&mut acc, line.xy());
                }
                DimensionKind::Angular {
                    vertex,
                    first,
                    second,
                    arc,
                } => {
                    include_pt(&mut acc, vertex.xy());
                    include_pt(&mut acc, first.xy());
                    include_pt(&mut acc, second.xy());
                    include_pt(&mut acc, arc.xy());
                }
                DimensionKind::AngularLines {
                    first: [fl0, fl1],
                    second: [sl0, sl1],
                    arc,
                } => {
                    include_pt(&mut acc, fl0.xy());
                    include_pt(&mut acc, fl1.xy());
                    include_pt(&mut acc, sl0.xy());
                    include_pt(&mut acc, sl1.xy());
                    include_pt(&mut acc, arc.xy());
                }
                DimensionKind::Radius { center, chord, .. } => {
                    include_pt(&mut acc, center.xy());
                    include_pt(&mut acc, chord.xy());
                }
                DimensionKind::Diameter {
                    chord, opposite, ..
                } => {
                    include_pt(&mut acc, chord.xy());
                    include_pt(&mut acc, opposite.xy());
                }
                DimensionKind::ArcLength {
                    center,
                    first,
                    second,
                    arc,
                } => {
                    include_pt(&mut acc, center.xy());
                    include_pt(&mut acc, first.xy());
                    include_pt(&mut acc, second.xy());
                    include_pt(&mut acc, arc.xy());
                }
                DimensionKind::Ordinate {
                    origin,
                    feature,
                    leader_end,
                    ..
                } => {
                    include_pt(&mut acc, origin.xy());
                    include_pt(&mut acc, feature.xy());
                    include_pt(&mut acc, leader_end.xy());
                }
            }
            if let Some(pos) = &dim.text.position {
                include_pt(&mut acc, pos.xy());
            }
        }
        Shape::Callout(callout) => {
            for branch in &callout.branches {
                for line in &branch.lines {
                    for p in line {
                        include_pt(&mut acc, p.xy());
                    }
                }
            }
            if let CalloutContent::Text(ft) = &callout.content {
                include_formatted_bounds(&mut acc, ft);
            }
        }
        _ => {}
    }
    acc.unwrap_or(EMPTY_BOUNDS)
}

fn include_formatted_bounds<P: transform::Point>(
    acc: &mut Option<[f64; 4]>,
    ft: &FormattedText<P>,
) {
    let [px, py] = ft.point.xy();
    let h = len_val(&ft.height.value).abs();
    let w = h * (ft.content.chars().count().max(1) as f64);
    include_pt(acc, [px - w, py - h]);
    include_pt(acc, [px + w, py + h]);
}

fn include_edge_bounds<P: transform::Point>(acc: &mut Option<[f64; 4]>, edge: &Edge<P>) {
    match edge {
        Edge::Line { start, end } => {
            include_pt(acc, start.xy());
            include_pt(acc, end.xy());
        }
        Edge::Arc { center, radius, .. } => {
            let [cx, cy] = center.xy();
            let r = len_val(radius).abs();
            include_pt(acc, [cx - r, cy - r]);
            include_pt(acc, [cx + r, cy + r]);
        }
        Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        } => {
            let mut pts = Vec::new();
            let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            zumen_geom2d::ellipse(
                center.xy(),
                major.xy(),
                *ratio,
                *start,
                sweep,
                DISPLAY_TOLERANCE,
                &mut pts,
            );
            for pt in pts {
                include_pt(acc, pt);
            }
        }
        Edge::Spline(sp) => {
            for pt in &sp.control {
                include_pt(acc, pt.xy());
            }
            for pt in &sp.fit {
                include_pt(acc, pt.xy());
            }
        }
        Edge::Clothoid {
            origin,
            parameter,
            counterclockwise,
            angle,
            start,
            end,
        } => {
            let pts = zumen_geom2d::clothoid(
                origin.xy(),
                len_val(parameter),
                *angle,
                *counterclockwise,
                len_val(start),
                len_val(end),
                DISPLAY_TOLERANCE,
            );
            if pts.is_empty() {
                include_pt(acc, origin.xy());
            } else {
                for pt in pts {
                    include_pt(acc, pt);
                }
            }
        }
    }
}

/// 部分図空間・部分図定義の要素について、子部分図の外接矩形も展開した局所外接矩形を求める
pub(super) fn model_shape_bounds(
    shape: &Shape<ModelPoint>,
    symbol_bounds: &BTreeMap<SymbolKey, [f64; 4]>,
) -> [f64; 4] {
    if let Shape::Symbol(sym) = shape
        && let Some(&sb) = symbol_bounds.get(&sym.key)
        && sb != EMPTY_BOUNDS
    {
        let rad = sym.rotation.to_radians();
        let (cos, sin) = (libm::cos(rad), libm::sin(rad));
        let [sx, sy] = sym.scale;
        let affine = Affine {
            m: [
                sx * cos,
                -sy * sin,
                sym.position.x,
                sx * sin,
                sy * cos,
                sym.position.y,
            ],
        };
        return transform_bounds(sb, &affine);
    }
    raw_shape_bounds(shape)
}

/// シート上の要素（またはビュー経由で配置される部分図要素）のシート座標での外接矩形
pub(super) fn compute_sheet_bounds(
    container: SxfContainer,
    placed: &Placed,
    space_views: &BTreeMap<SpaceId, Vec<Affine>>,
    symbol_bounds: &BTreeMap<SymbolKey, [f64; 4]>,
) -> [f64; 4] {
    match (container, placed) {
        (SxfContainer::Sheet(_), Placed::Paper(elem)) => {
            if let Shape::Symbol(sym) = &elem.shape
                && let Some(&sb) = symbol_bounds.get(&sym.key)
                && sb != EMPTY_BOUNDS
            {
                let rad = sym.rotation.to_radians();
                let (cos, sin) = (libm::cos(rad), libm::sin(rad));
                let [sx, sy] = sym.scale;
                let affine = Affine {
                    m: [
                        sx * cos,
                        -sy * sin,
                        sym.position.x,
                        sx * sin,
                        sy * cos,
                        sym.position.y,
                    ],
                };
                return transform_bounds(sb, &affine);
            }
            raw_shape_bounds(&elem.shape)
        }
        (SxfContainer::Space(space), Placed::Model(elem)) => {
            let local = model_shape_bounds(&elem.shape, symbol_bounds);
            if local == EMPTY_BOUNDS {
                return EMPTY_BOUNDS;
            }
            let Some(views) = space_views.get(&space) else {
                return EMPTY_BOUNDS;
            };
            let mut acc = None;
            for v in views {
                union_bounds(&mut acc, transform_bounds(local, v));
            }
            acc.unwrap_or(EMPTY_BOUNDS)
        }
        _ => EMPTY_BOUNDS,
    }
}

pub(super) struct DisplayContext<'a> {
    pub(super) space_views: &'a BTreeMap<SpaceId, Vec<Affine>>,
    pub(super) symbol_indices: &'a BTreeMap<SymbolKey, u32>,
}

pub(super) fn display_counts_and_style(
    row: &SxfRow,
    ctx: &DisplayContext<'_>,
) -> ([u32; 4], [u32; 2]) {
    let (layer, color) = match &row.placed {
        Placed::Paper(e) => (e.layer.0, pack_sxf_color(&e.style.color)),
        Placed::Model(e) => (e.layer.0, pack_sxf_color(&e.style.color)),
    };
    let style = [layer, color];
    match (row.container, &row.placed) {
        (SxfContainer::Sheet(_), Placed::Paper(elem)) => {
            (count_paper_shape(&elem.shape, ctx.symbol_indices), style)
        }
        (SxfContainer::Space(space), Placed::Model(elem)) => {
            let Some(views) = ctx.space_views.get(&space) else {
                return ([0, 0, 0, 0], style);
            };
            if views.is_empty() {
                return ([0, 0, 0, 0], style);
            }
            let mut total = [0u32; 4];
            for view in views {
                let [c0, c1, c2, c3] =
                    count_model_shape_for_view(&elem.shape, view, ctx.symbol_indices);
                let [t0, t1, t2, t3] = total;
                total = [
                    t0.saturating_add(c0),
                    t1.saturating_add(c1),
                    t2.saturating_add(c2),
                    t3.saturating_add(c3),
                ];
            }
            (total, style)
        }
        _ => ([0, 0, 0, 0], style),
    }
}

fn count_paper_shape(
    shape: &Shape<PaperPoint>,
    symbol_indices: &BTreeMap<SymbolKey, u32>,
) -> [u32; 4] {
    match shape {
        Shape::Line { .. } | Shape::Point(_) => [1, 0, 0, 0],
        Shape::Circle { .. } | Shape::Curve(Edge::Arc { .. }) => [0, 1, 0, 0],
        Shape::Polyline(poly) => [poly.vertices.len().saturating_sub(1) as u32, 0, 0, 0],
        Shape::Curve(edge) => count_edge(edge, true),
        Shape::Text(_) | Shape::Formatted(_) => [0, 0, 1, 0],
        Shape::Symbol(sym) => [0, 0, 0, u32::from(symbol_indices.contains_key(&sym.key))],
        Shape::Underlay(u) => match u.frame {
            UnderlayFrame::Corners { .. } | UnderlayFrame::Pixels { .. } => [4, 0, 0, 0],
            UnderlayFrame::Scaled { .. } => [1, 0, 0, 0],
        },
        Shape::Hatch(hatch) => count_hatch(hatch, true),
        Shape::Dimension(dim) => [1, 0, u32::from(dim.text.position.is_some()), 0],
        Shape::Callout(c) => {
            let mut segs = 0u32;
            for b in &c.branches {
                for l in &b.lines {
                    segs = segs.saturating_add(l.len().saturating_sub(1) as u32);
                }
            }
            let texts = u32::from(matches!(c.content, CalloutContent::Text(_)));
            [segs, 0, texts, 0]
        }
        _ => [0, 0, 0, 0],
    }
}

fn is_conformal_similarity(m: [f64; 6]) -> Option<(f64, f64, bool)> {
    let [a, b, _, d, e, _] = m;
    let sx = libm::hypot(a, d);
    let sy = libm::hypot(b, e);
    if sx <= 0.0 || !sx.is_finite() || (sx - sy).abs() > 1e-9 * sx.max(1.0) {
        return None;
    }
    let dot = a * b + d * e;
    if dot.abs() > 1e-9 * sx * sy {
        return None;
    }
    let det = a * e - b * d;
    Some((sx, libm::atan2(d, a), det < 0.0))
}

pub(super) fn count_model_shape_for_view(
    shape: &Shape<ModelPoint>,
    view: &Affine,
    symbol_indices: &BTreeMap<SymbolKey, u32>,
) -> [u32; 4] {
    let sim = is_conformal_similarity(view.m).is_some();
    match shape {
        Shape::Line { .. } | Shape::Point(_) => [1, 0, 0, 0],
        Shape::Circle { center, radius } => {
            if sim {
                [0, 1, 0, 0]
            } else {
                let r = len_val(radius).abs();
                let mut pts = Vec::new();
                zumen_geom2d::ellipse(
                    center.xy(),
                    [center.x + r, center.y],
                    1.0,
                    0.0,
                    360.0,
                    DISPLAY_TOLERANCE,
                    &mut pts,
                );
                [pts.len().saturating_sub(1) as u32, 0, 0, 0]
            }
        }
        Shape::Curve(edge) => count_edge(edge, sim),
        Shape::Polyline(poly) => [poly.vertices.len().saturating_sub(1) as u32, 0, 0, 0],
        Shape::Text(_) | Shape::Formatted(_) => [0, 0, 1, 0],
        Shape::Symbol(sym) => [0, 0, 0, u32::from(symbol_indices.contains_key(&sym.key))],
        Shape::Underlay(u) => match u.frame {
            UnderlayFrame::Corners { .. } | UnderlayFrame::Pixels { .. } => [4, 0, 0, 0],
            UnderlayFrame::Scaled { .. } => [1, 0, 0, 0],
        },
        Shape::Hatch(hatch) => count_hatch(hatch, sim),
        Shape::Dimension(dim) => [1, 0, u32::from(dim.text.position.is_some()), 0],
        Shape::Callout(c) => {
            let mut segs = 0u32;
            for b in &c.branches {
                for l in &b.lines {
                    segs = segs.saturating_add(l.len().saturating_sub(1) as u32);
                }
            }
            let texts = u32::from(matches!(c.content, CalloutContent::Text(_)));
            [segs, 0, texts, 0]
        }
        _ => [0, 0, 0, 0],
    }
}

fn count_edge<P: transform::Point>(edge: &Edge<P>, allow_arc: bool) -> [u32; 4] {
    match edge {
        Edge::Line { .. } => [1, 0, 0, 0],
        Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        } => {
            if allow_arc {
                [0, 1, 0, 0]
            } else {
                let r = len_val(radius).abs();
                let [cx, cy] = center.xy();
                let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
                let mut pts = Vec::new();
                zumen_geom2d::ellipse(
                    [cx, cy],
                    [cx + r, cy],
                    1.0,
                    *start,
                    sweep,
                    DISPLAY_TOLERANCE,
                    &mut pts,
                );
                [pts.len().saturating_sub(1) as u32, 0, 0, 0]
            }
        }
        Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        } => {
            let mut pts = Vec::new();
            let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            zumen_geom2d::ellipse(
                center.xy(),
                major.xy(),
                *ratio,
                *start,
                sweep,
                DISPLAY_TOLERANCE,
                &mut pts,
            );
            [pts.len().saturating_sub(1) as u32, 0, 0, 0]
        }
        Edge::Spline(sp) => [sp.control.len().saturating_sub(1) as u32, 0, 0, 0],
        Edge::Clothoid {
            origin,
            parameter,
            counterclockwise,
            angle,
            start,
            end,
        } => {
            let pts = zumen_geom2d::clothoid(
                origin.xy(),
                len_val(parameter),
                *angle,
                *counterclockwise,
                len_val(start),
                len_val(end),
                DISPLAY_TOLERANCE,
            );
            [pts.len().saturating_sub(1) as u32, 0, 0, 0]
        }
    }
}

fn count_hatch<P: transform::Point>(hatch: &Hatch<P>, allow_arc: bool) -> [u32; 4] {
    let mut segs = 0u32;
    let mut arcs = 0u32;
    for lp in &hatch.loops {
        match &lp.boundary {
            Boundary::Polyline { vertices, closed } => {
                if vertices.len() >= 2 {
                    segs = segs.saturating_add((vertices.len() - 1 + usize::from(*closed)) as u32);
                }
            }
            Boundary::Edges(edges) => {
                for edge in edges {
                    let [c0, c1, _, _] = count_edge(edge, allow_arc);
                    segs = segs.saturating_add(c0);
                    arcs = arcs.saturating_add(c1);
                }
            }
        }
    }
    [segs, arcs, 0, 0]
}

pub(super) fn write_display_row(row: &SxfRow, ctx: &DisplayContext<'_>, w: &mut DisplayWriter<'_>) {
    match (row.container, &row.placed) {
        (SxfContainer::Sheet(_), Placed::Paper(elem)) => {
            let identity = Affine {
                m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            };
            write_shape(&elem.shape, &identity, ctx.symbol_indices, w);
        }
        (SxfContainer::Space(space), Placed::Model(elem)) => {
            if let Some(views) = ctx.space_views.get(&space) {
                for view in views {
                    write_shape(&elem.shape, view, ctx.symbol_indices, w);
                }
            }
        }
        _ => {}
    }
}

pub(super) fn write_shape<P: transform::Point>(
    shape: &Shape<P>,
    view: &Affine,
    symbol_indices: &BTreeMap<SymbolKey, u32>,
    w: &mut DisplayWriter<'_>,
) {
    let pt = |p: &P| apply_affine(view.m, p.xy());
    let sim = is_conformal_similarity(view.m);
    let [m0, _, _, m3, _, _] = view.m;
    let view_rot = libm::atan2(m3, m0);
    let scale_approx = libm::hypot(m0, m3).max(1e-12);

    match shape {
        Shape::Line { start, end } => {
            w.push_segment(pt(start), pt(end));
        }
        Shape::Point(dot) => {
            let p = pt(&dot.position);
            w.push_segment(p, p);
        }
        Shape::Circle { center, radius } => {
            let r = len_val(radius).abs();
            if let Some((s, _, _)) = sim {
                w.push_arc_rad(pt(center), r * s, 0.0, TAU);
            } else {
                let [cx, cy] = center.xy();
                let mut pts = Vec::new();
                zumen_geom2d::ellipse(
                    [cx, cy],
                    [cx + r, cy],
                    1.0,
                    0.0,
                    360.0,
                    DISPLAY_TOLERANCE,
                    &mut pts,
                );
                push_polyline_pts(&pts, view, w);
            }
        }
        Shape::Polyline(poly) => {
            for pair in poly.vertices.windows(2) {
                if let Some([a, b]) = pair.first_chunk::<2>() {
                    w.push_segment(pt(&a.point), pt(&b.point));
                }
            }
        }
        Shape::Curve(edge) => write_edge(edge, view, sim, w),
        Shape::Text(text) => {
            let pos = match &text.placement {
                Placement::Anchor { point, .. } => pt(point),
                Placement::Between { first, .. } => pt(first),
            };
            let h = len_val(&text.height.value).abs() * scale_approx;
            let rot_rad = radians_from_deg(text.rotation) + view_rot;
            w.push_text(
                pos,
                h,
                rot_rad,
                text.width_factor,
                radians_from_deg(text.oblique),
            );
        }
        Shape::Formatted(ft) => {
            let pos = pt(&ft.point);
            let h = len_val(&ft.height.value).abs() * scale_approx;
            let deg = match ft.rotation {
                Rotation::Angle(a) => a,
                _ => 0.0,
            };
            let rot_rad = radians_from_deg(deg) + view_rot;
            w.push_text(pos, h, rot_rad, 1.0, 0.0);
        }
        Shape::Symbol(sym) => {
            if let Some(&def_idx) = symbol_indices.get(&sym.key) {
                let [sx, sy] = sym.scale;
                let rot_rad = radians_from_deg(sym.rotation) + view_rot;
                w.push_instance(
                    pt(&sym.position),
                    [sx * scale_approx, sy * scale_approx],
                    rot_rad,
                    [0.0, 0.0],
                    def_idx,
                    [1, 1],
                );
            }
        }
        Shape::Underlay(u) => match &u.frame {
            UnderlayFrame::Corners { origin, right, top }
            | UnderlayFrame::Pixels {
                origin, right, top, ..
            } => {
                let [o0, o1] = pt(origin);
                let [r0, r1] = pt(right);
                let [t0, t1] = pt(top);
                let opp = [r0 + t0 - o0, r1 + t1 - o1];
                w.push_segment([o0, o1], [r0, r1]);
                w.push_segment([r0, r1], opp);
                w.push_segment(opp, [t0, t1]);
                w.push_segment([t0, t1], [o0, o1]);
            }
            UnderlayFrame::Scaled { origin, .. } => {
                let p = pt(origin);
                w.push_segment(p, p);
            }
        },
        Shape::Hatch(hatch) => {
            for lp in &hatch.loops {
                match &lp.boundary {
                    Boundary::Polyline { vertices, closed } => {
                        if vertices.len() >= 2 {
                            for pair in vertices.windows(2) {
                                if let Some([a, b]) = pair.first_chunk::<2>() {
                                    w.push_segment(pt(&a.point), pt(&b.point));
                                }
                            }
                            if *closed
                                && let (Some(last), Some(first)) =
                                    (vertices.last(), vertices.first())
                            {
                                w.push_segment(pt(&last.point), pt(&first.point));
                            }
                        }
                    }
                    Boundary::Edges(edges) => {
                        for edge in edges {
                            write_edge(edge, view, sim, w);
                        }
                    }
                }
            }
        }
        Shape::Dimension(dim) => {
            let p_a = match &dim.kind {
                DimensionKind::Linear { first, second, .. }
                | DimensionKind::Aligned { first, second, .. }
                | DimensionKind::Angular { first, second, .. }
                | DimensionKind::ArcLength { first, second, .. } => (pt(first), pt(second)),
                DimensionKind::AngularLines {
                    first: [fl0, _],
                    second: [sl0, _],
                    ..
                } => (pt(fl0), pt(sl0)),
                DimensionKind::Radius { center, chord, .. } => (pt(center), pt(chord)),
                DimensionKind::Diameter {
                    chord, opposite, ..
                } => (pt(chord), pt(opposite)),
                DimensionKind::Ordinate {
                    feature,
                    leader_end,
                    ..
                } => (pt(feature), pt(leader_end)),
            };
            w.push_segment(p_a.0, p_a.1);
            if let Some(tpos) = &dim.text.position {
                let rot = dim.text.rotation.unwrap_or(0.0);
                w.push_text(
                    pt(tpos),
                    2.5 * scale_approx,
                    radians_from_deg(rot) + view_rot,
                    1.0,
                    0.0,
                );
            }
        }
        Shape::Callout(callout) => {
            for b in &callout.branches {
                for l in &b.lines {
                    for pair in l.windows(2) {
                        if let Some([a, b_pt]) = pair.first_chunk::<2>() {
                            w.push_segment(pt(a), pt(b_pt));
                        }
                    }
                }
            }
            if let CalloutContent::Text(ft) = &callout.content {
                let pos = pt(&ft.point);
                let h = len_val(&ft.height.value).abs() * scale_approx;
                let deg = match ft.rotation {
                    Rotation::Angle(a) => a,
                    _ => 0.0,
                };
                w.push_text(pos, h, radians_from_deg(deg) + view_rot, 1.0, 0.0);
            }
        }
        _ => {}
    }
}

fn write_edge<P: transform::Point>(
    edge: &Edge<P>,
    view: &Affine,
    sim: Option<(f64, f64, bool)>,
    w: &mut DisplayWriter<'_>,
) {
    let pt = |p: &P| apply_affine(view.m, p.xy());
    match edge {
        Edge::Line { start, end } => {
            w.push_segment(pt(start), pt(end));
        }
        Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        } => {
            let r = len_val(radius).abs();
            let sweep_deg = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            if let Some((s, rot_rad, flipped)) = sim {
                let start_rad = if flipped {
                    rot_rad - radians_from_deg(*start)
                } else {
                    rot_rad + radians_from_deg(*start)
                };
                let sweep_rad = if flipped {
                    -radians_from_deg(sweep_deg)
                } else {
                    radians_from_deg(sweep_deg)
                };
                w.push_arc_rad(pt(center), r * s, start_rad, sweep_rad);
            } else {
                let [cx, cy] = center.xy();
                let mut pts = Vec::new();
                zumen_geom2d::ellipse(
                    [cx, cy],
                    [cx + r, cy],
                    1.0,
                    *start,
                    sweep_deg,
                    DISPLAY_TOLERANCE,
                    &mut pts,
                );
                push_polyline_pts(&pts, view, w);
            }
        }
        Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            counterclockwise,
        } => {
            let mut pts = Vec::new();
            let sweep = zumen_geom2d::sweep(*start, *end, *counterclockwise);
            zumen_geom2d::ellipse(
                center.xy(),
                major.xy(),
                *ratio,
                *start,
                sweep,
                DISPLAY_TOLERANCE,
                &mut pts,
            );
            push_polyline_pts(&pts, view, w);
        }
        Edge::Spline(sp) => {
            for pair in sp.control.windows(2) {
                if let Some([a, b]) = pair.first_chunk::<2>() {
                    w.push_segment(pt(a), pt(b));
                }
            }
        }
        Edge::Clothoid {
            origin,
            parameter,
            counterclockwise,
            angle,
            start,
            end,
        } => {
            let pts = zumen_geom2d::clothoid(
                origin.xy(),
                len_val(parameter),
                *angle,
                *counterclockwise,
                len_val(start),
                len_val(end),
                DISPLAY_TOLERANCE,
            );
            push_polyline_pts(&pts, view, w);
        }
    }
}

fn push_polyline_pts(pts: &[[f64; 2]], view: &Affine, w: &mut DisplayWriter<'_>) {
    for pair in pts.windows(2) {
        if let Some(&[a, b]) = pair.first_chunk::<2>() {
            w.push_segment(apply_affine(view.m, a), apply_affine(view.m, b));
        }
    }
}

pub(super) fn apply_display_commit<P: PatchPayload>(
    display: &mut Display,
    ctx: &DisplayContext<'_>,
    committed: &CommittedTxn<SxfRow, P>,
) {
    for diff in committed.existing() {
        match diff {
            ExistingRowDiff::Updated { row, after_row, .. } => {
                let (counts, style) = display_counts_and_style(after_row, ctx);
                display.update_with(row.0, counts, style, |w| {
                    write_display_row(after_row, ctx, w);
                });
            }
            ExistingRowDiff::Removed { row, .. } => {
                display.hide(row.0);
            }
        }
    }
    for born in committed.born() {
        let (counts, style) = display_counts_and_style(&born.after_row, ctx);
        display.push_with(counts, style, |w| {
            write_display_row(&born.after_row, ctx, w);
        });
    }
}

pub(super) fn revert_display_commit<P: PatchPayload>(
    display: &mut Display,
    ctx: &DisplayContext<'_>,
    committed: &CommittedTxn<SxfRow, P>,
) {
    for _ in 0..committed.born().len() {
        display.pop();
    }
    for diff in committed.existing() {
        match diff {
            ExistingRowDiff::Removed { row, .. } => {
                display.show(row.0);
            }
            ExistingRowDiff::Updated {
                row, before_row, ..
            } => {
                let (counts, style) = display_counts_and_style(before_row, ctx);
                display.update_with(row.0, counts, style, |w| {
                    write_display_row(before_row, ctx, w);
                });
            }
        }
    }
}
