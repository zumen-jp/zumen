//! 表示のバッファー [`Display`] の要素に対する、決定的な点選択の当たり判定。
//!
//! 平方根・逆正接・正弦・余弦・床関数はすべて `libm` を通し、ネイティブと wasm で同じ結果を返す。

use std::collections::BTreeMap;
use std::sync::Arc;

use zumen_base::{Cx, Kind, Stop};

use crate::display::Display;

/// 恒等の 2D アフィン変換 `[1, 0, 0, 0, 1, 0]`
pub const IDENTITY_XFORM: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

/// 記号の定義の再帰展開の上限
const MAX_HIT_DEPTH: u32 = 16;

/// 非等方拡大された円弧（楕円弧）を折れ線で判定するときの分割数
const ELLIPSE_ARC_STEPS: u32 = 32;

#[inline]
fn apply_xform(m: &[f64; 6], [x, y]: [f64; 2]) -> [f64; 2] {
    [m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5]]
}

#[inline]
fn compose_xform(a: &[f64; 6], b: &[f64; 6]) -> [f64; 6] {
    [
        a[0] * b[0] + a[1] * b[3],
        a[0] * b[1] + a[1] * b[4],
        a[0] * b[2] + a[1] * b[5] + a[2],
        a[3] * b[0] + a[4] * b[3],
        a[3] * b[1] + a[4] * b[4],
        a[3] * b[2] + a[4] * b[5] + a[5],
    ]
}

#[inline]
fn pair_f64(high: f32, low: f32) -> f64 {
    f64::from(high) + f64::from(low)
}

#[inline]
fn rem_tau(angle: f64) -> f64 {
    let tau = core::f64::consts::TAU;
    let q = libm::floor(angle / tau);
    let r = angle - q * tau;
    if r < 0.0 {
        (r + tau).min(tau)
    } else if r >= tau {
        r - tau
    } else {
        r
    }
}

/// 軸平行な箱 `bounds = [min_x, min_y, max_x, max_y]` から点 `point` までの距離が `tolerance` 以内か
#[inline]
pub fn box_hits_point(bounds: [f64; 4], point: [f64; 2], tolerance: f64) -> bool {
    let [x0, y0, x1, y1] = bounds;
    if !(x0.is_finite()
        && y0.is_finite()
        && x1.is_finite()
        && y1.is_finite()
        && x0 <= x1
        && y0 <= y1)
    {
        return false;
    }
    let dx = if point[0] < x0 {
        x0 - point[0]
    } else if point[0] > x1 {
        point[0] - x1
    } else {
        0.0
    };
    let dy = if point[1] < y0 {
        y0 - point[1]
    } else if point[1] > y1 {
        point[1] - y1
    } else {
        0.0
    };
    let dist = libm::hypot(dx, dy);
    dist.is_finite() && dist <= tolerance
}

/// 線分 `p0..p1` から点 `point` までの距離が `tolerance` 以内か
#[inline]
pub fn segment_hits_point(p0: [f64; 2], p1: [f64; 2], point: [f64; 2], tolerance: f64) -> bool {
    if !(p0[0].is_finite() && p0[1].is_finite() && p1[0].is_finite() && p1[1].is_finite()) {
        return false;
    }
    let vx = p1[0] - p0[0];
    let vy = p1[1] - p0[1];
    let len = libm::hypot(vx, vy);
    if !len.is_finite() || len == 0.0 {
        let dist = libm::hypot(point[0] - p0[0], point[1] - p0[1]);
        return dist.is_finite() && dist <= tolerance;
    }
    let ux = vx / len;
    let uy = vy / len;
    let wx = point[0] - p0[0];
    let wy = point[1] - p0[1];
    let proj = (wx * ux + wy * uy).clamp(0.0, len);
    let closest = if proj <= 0.0 {
        p0
    } else if proj >= len {
        p1
    } else {
        [p0[0] + ux * proj, p0[1] + uy * proj]
    };
    let dist = libm::hypot(point[0] - closest[0], point[1] - closest[1]);
    dist.is_finite() && dist <= tolerance
}

/// 円周または円弧から点 `point` までの距離が `tolerance` 以内か
pub fn arc_hits_point(
    center: [f64; 2],
    radius: f64,
    start_rad: f64,
    sweep_rad: f64,
    point: [f64; 2],
    tolerance: f64,
) -> bool {
    if !(center[0].is_finite()
        && center[1].is_finite()
        && radius.is_finite()
        && start_rad.is_finite()
        && sweep_rad.is_finite())
    {
        return false;
    }
    let radius = radius.abs();
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    let r_pt = libm::hypot(dx, dy);
    if !r_pt.is_finite() {
        return false;
    }
    if radius == 0.0 {
        return r_pt <= tolerance;
    }
    if sweep_rad == 0.0 {
        let sx = center[0] + radius * libm::cos(start_rad);
        let sy = center[1] + radius * libm::sin(start_rad);
        let dist = libm::hypot(point[0] - sx, point[1] - sy);
        return dist.is_finite() && dist <= tolerance;
    }
    let tau = core::f64::consts::TAU;
    if sweep_rad.abs() >= tau - 1e-5 {
        return (r_pt - radius).abs() <= tolerance;
    }
    if r_pt == 0.0 {
        return radius <= tolerance;
    }
    let theta = libm::atan2(dy, dx);
    let in_span = if sweep_rad > 0.0 {
        rem_tau(theta - start_rad) <= sweep_rad
    } else {
        rem_tau(start_rad - theta) <= -sweep_rad
    };
    let dist = if in_span {
        (r_pt - radius).abs()
    } else {
        let end = start_rad + sweep_rad;
        let sx = center[0] + radius * libm::cos(start_rad);
        let sy = center[1] + radius * libm::sin(start_rad);
        let ex = center[0] + radius * libm::cos(end);
        let ey = center[1] + radius * libm::sin(end);
        libm::hypot(point[0] - sx, point[1] - sy).min(libm::hypot(point[0] - ex, point[1] - ey))
    };
    dist.is_finite() && dist <= tolerance
}

fn transformed_arc_hits_point(
    center: [f64; 2],
    radius: f64,
    start_rad: f64,
    sweep_rad: f64,
    xform: &[f64; 6],
    point: [f64; 2],
    tolerance: f64,
) -> bool {
    if *xform == IDENTITY_XFORM {
        return arc_hits_point(center, radius, start_rad, sweep_rad, point, tolerance);
    }
    if !(center[0].is_finite()
        && center[1].is_finite()
        && radius.is_finite()
        && start_rad.is_finite()
        && sweep_rad.is_finite())
    {
        return false;
    }
    let radius = radius.abs();
    if radius == 0.0 {
        let p = apply_xform(xform, center);
        return segment_hits_point(p, p, point, tolerance);
    }
    let sx = libm::hypot(xform[0], xform[3]);
    let sy = libm::hypot(xform[1], xform[4]);
    let dot = xform[0] * xform[1] + xform[3] * xform[4];
    if sx > 0.0 && (sx - sy).abs() <= sx * 1e-5 && dot.abs() <= sx * sy * 1e-5 {
        let det = xform[0] * xform[4] - xform[1] * xform[3];
        let phi = libm::atan2(xform[3], xform[0]);
        let (new_start, new_sweep) = if det >= 0.0 {
            (phi + start_rad, sweep_rad)
        } else {
            (phi - start_rad, -sweep_rad)
        };
        return arc_hits_point(
            apply_xform(xform, center),
            radius * sx,
            new_start,
            new_sweep,
            point,
            tolerance,
        );
    }
    // 非等方の拡大・せん断で楕円弧になるときは弦に分割して判定する
    let steps = ELLIPSE_ARC_STEPS;
    let mut prev = apply_xform(
        xform,
        [
            center[0] + radius * libm::cos(start_rad),
            center[1] + radius * libm::sin(start_rad),
        ],
    );
    for step in 1..=steps {
        let t = f64::from(step) / f64::from(steps);
        let angle = start_rad + sweep_rad * t;
        let next = apply_xform(
            xform,
            [
                center[0] + radius * libm::cos(angle),
                center[1] + radius * libm::sin(angle),
            ],
        );
        if segment_hits_point(prev, next, point, tolerance) {
            return true;
        }
        prev = next;
    }
    false
}

fn quad_hits_point(corners: [[f64; 2]; 4], point: [f64; 2], tolerance: f64) -> bool {
    if corners
        .iter()
        .any(|c| !(c[0].is_finite() && c[1].is_finite()))
    {
        return false;
    }
    let mut non_neg = true;
    let mut non_pos = true;
    let [c0, c1, c2, c3] = corners;
    for (a, b) in [(c0, c1), (c1, c2), (c2, c3), (c3, c0)] {
        if segment_hits_point(a, b, point, tolerance) {
            return true;
        }
        let cross = (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0]);
        if cross < 0.0 {
            non_neg = false;
        }
        if cross > 0.0 {
            non_pos = false;
        }
    }
    non_neg || non_pos
}

fn text_primitive_hits_point(
    text: [f32; 8],
    xform: &[f64; 6],
    point: [f64; 2],
    tolerance: f64,
) -> bool {
    let origin = [pair_f64(text[0], text[1]), pair_f64(text[2], text[3])];
    let h = f64::from(text[4]).abs();
    let rot = f64::from(text[5]);
    let wf = f64::from(text[6]).abs().max(0.5);
    if !(origin[0].is_finite() && origin[1].is_finite() && h.is_finite() && rot.is_finite()) {
        return false;
    }
    if h == 0.0 {
        let p = apply_xform(xform, origin);
        return segment_hits_point(p, p, point, tolerance);
    }
    let w = (h * wf * 3.0).max(h);
    let (s, c) = (libm::sin(rot), libm::cos(rot));
    let ux = [c, s];
    let vy = [-s, c];
    let local_corner = |du: f64, dv: f64| -> [f64; 2] {
        apply_xform(
            xform,
            [
                origin[0] + ux[0] * du + vy[0] * dv,
                origin[1] + ux[1] * du + vy[1] * dv,
            ],
        )
    };
    quad_hits_point(
        [
            local_corner(-0.5 * w, -0.25 * h),
            local_corner(w, -0.25 * h),
            local_corner(w, 1.25 * h),
            local_corner(-0.5 * w, 1.25 * h),
        ],
        point,
        tolerance,
    )
}

fn instance_cell_xform(
    origin: [f64; 2],
    scale: [f64; 2],
    rot: f64,
    col: u32,
    row: u32,
    spacing: [f64; 2],
) -> [f64; 6] {
    let (s, c) = (libm::sin(rot), libm::cos(rot));
    let dx = f64::from(col) * spacing[0];
    let dy = f64::from(row) * spacing[1];
    let tx = origin[0] + dx * c - dy * s;
    let ty = origin[1] + dx * s + dy * c;
    [
        scale[0] * c,
        -scale[1] * s,
        tx,
        scale[0] * s,
        scale[1] * c,
        ty,
    ]
}

impl Display {
    /// 行 `row` の表示要素が、変換 `xform` のもとで点 `point` から許容距離 `tolerance` 以内にあるかを判定する。
    ///
    /// 最上位（`depth == 0`）で `bounds` が与えられたときは、まず囲む箱との距離で絞り込み、
    /// 文字のみの行はその囲む箱で判定する。挿入（記号）は `symbol_hit` で定義の中身を再帰的にたどる。
    #[expect(clippy::too_many_arguments)]
    pub fn row_hits_point_with_cx<F>(
        &self,
        row: u32,
        bounds: Option<[f64; 4]>,
        point: [f64; 2],
        tolerance: f64,
        xform: &[f64; 6],
        depth: u32,
        symbol_hit: &mut F,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Stop>
    where
        F: FnMut(u32, [f64; 2], f64, &[f64; 6], u32, &mut Cx<'_>) -> Result<bool, Stop>,
    {
        if !(point[0].is_finite()
            && point[1].is_finite()
            && tolerance.is_finite()
            && tolerance >= 0.0)
        {
            return Ok(false);
        }
        if depth == 0
            && let Some(bx) = bounds
            && !box_hits_point(bx, point, tolerance)
        {
            return Ok(false);
        }
        let Some(&slot) = self.slots.get(row as usize) else {
            return Ok(false);
        };
        let (firsts, counts) = self.slot_ranges(slot);
        let prim_total = u64::from(counts[0])
            + u64::from(counts[1])
            + u64::from(counts[2])
            + u64::from(counts[3]);
        cx.check(Kind::Work, prim_total.max(1))?;

        let seg_start = firsts[0] as usize;
        for at in seg_start..seg_start + counts[0] as usize {
            if let Some(seg) = self.segments.get(at) {
                let p0 = apply_xform(xform, [pair_f64(seg[0], seg[1]), pair_f64(seg[2], seg[3])]);
                let p1 = apply_xform(xform, [pair_f64(seg[4], seg[5]), pair_f64(seg[6], seg[7])]);
                if segment_hits_point(p0, p1, point, tolerance) {
                    return Ok(true);
                }
            }
        }

        let arc_start = firsts[1] as usize;
        for at in arc_start..arc_start + counts[1] as usize {
            if let Some(arc) = self.arcs.get(at) {
                let center = [pair_f64(arc[0], arc[1]), pair_f64(arc[2], arc[3])];
                if transformed_arc_hits_point(
                    center,
                    f64::from(arc[4]),
                    f64::from(arc[5]),
                    f64::from(arc[6]),
                    xform,
                    point,
                    tolerance,
                ) {
                    return Ok(true);
                }
            }
        }

        if counts[2] > 0 {
            if depth == 0
                && counts[0] == 0
                && counts[1] == 0
                && counts[3] == 0
                && let Some(bx) = bounds
                && box_hits_point(bx, point, tolerance)
            {
                return Ok(true);
            }
            let text_start = firsts[2] as usize;
            for at in text_start..text_start + counts[2] as usize {
                if let Some(&text) = self.texts.get(at)
                    && text_primitive_hits_point(text, xform, point, tolerance)
                {
                    return Ok(true);
                }
            }
        }

        let inst_start = firsts[3] as usize;
        for at in inst_start..inst_start + counts[3] as usize {
            let (Some(&inst), Some(&iref)) = (self.instances.get(at), self.instance_ref.get(at))
            else {
                continue;
            };
            let origin = [pair_f64(inst[0], inst[1]), pair_f64(inst[2], inst[3])];
            let scale = [f64::from(inst[4]), f64::from(inst[5])];
            let rot = f64::from(inst[6]);
            let spacing = [f64::from(inst[7]), f64::from(inst[8])];
            if !(origin[0].is_finite()
                && origin[1].is_finite()
                && scale[0].is_finite()
                && scale[1].is_finite()
                && rot.is_finite()
                && spacing[0].is_finite()
                && spacing[1].is_finite())
            {
                continue;
            }
            let def_idx = iref[0];
            if def_idx == u32::MAX {
                let anchor = apply_xform(xform, origin);
                if segment_hits_point(anchor, anchor, point, tolerance) {
                    return Ok(true);
                }
                continue;
            }
            if depth >= MAX_HIT_DEPTH {
                continue;
            }
            cx.enter()?;
            let cols = iref[1].max(1);
            let rows = iref[2].max(1);
            let cell_res = (|| -> Result<bool, Stop> {
                for r in 0..rows {
                    for c in 0..cols {
                        cx.check(Kind::Work, 1)?;
                        let local_cell = instance_cell_xform(origin, scale, rot, c, r, spacing);
                        let cell_xform = compose_xform(xform, &local_cell);
                        if symbol_hit(def_idx, point, tolerance, &cell_xform, depth + 1, cx)? {
                            return Ok(true);
                        }
                    }
                }
                Ok(false)
            })();
            cx.leave();
            if cell_res? {
                return Ok(true);
            }
        }

        // プリミティブを持たない生存行（空の挿入や点のみ等）は囲む箱が許容距離以内なら一致とする
        if prim_total == 0
            && depth == 0
            && let Some(bx) = bounds
            && box_hits_point(bx, point, tolerance)
        {
            return Ok(true);
        }

        Ok(false)
    }
}

/// 記号の定義の表示を必要になったものだけ 1 回ずつ組み立てて保持し、再帰的な挿入の当たり判定を行うキャッシュ
#[derive(Default)]
pub struct SymbolHitCache {
    entries: BTreeMap<u32, Option<Arc<Display>>>,
}

impl SymbolHitCache {
    /// 新しい空のキャッシュを作る
    pub fn new() -> Self {
        Self::default()
    }

    /// 最上位の行 `row` が点 `point` から許容距離 `tolerance` 以内にあるかを判定する
    #[expect(clippy::too_many_arguments)]
    pub fn row_hits_with_cx<L>(
        &mut self,
        display: &Display,
        row: u32,
        bounds: Option<[f64; 4]>,
        point: [f64; 2],
        tolerance: f64,
        load_symbol: &mut L,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Stop>
    where
        L: FnMut(u32, &mut Display, &mut Cx<'_>) -> Result<Option<u32>, Stop>,
    {
        display.row_hits_point_with_cx(
            row,
            bounds,
            point,
            tolerance,
            &IDENTITY_XFORM,
            0,
            &mut |def_idx, pt, tol, xform, depth, inner_cx| {
                self.symbol_hits_with_cx(def_idx, pt, tol, xform, depth, load_symbol, inner_cx)
            },
            cx,
        )
    }

    #[expect(clippy::too_many_arguments)]
    fn symbol_hits_with_cx<L>(
        &mut self,
        def_idx: u32,
        point: [f64; 2],
        tolerance: f64,
        xform: &[f64; 6],
        depth: u32,
        load_symbol: &mut L,
        cx: &mut Cx<'_>,
    ) -> Result<bool, Stop>
    where
        L: FnMut(u32, &mut Display, &mut Cx<'_>) -> Result<Option<u32>, Stop>,
    {
        let cached = if let Some(existing) = self.entries.get(&def_idx) {
            existing.clone()
        } else {
            let mut sym = Display::with_rows(0);
            let loaded = match load_symbol(def_idx, &mut sym, cx)? {
                Some(rows) if rows > 0 => Some(Arc::new(sym)),
                _ => None,
            };
            cx.check(Kind::DecodedBytes, 64)?;
            self.entries.insert(def_idx, loaded.clone());
            loaded
        };
        let Some(sym) = cached else {
            let anchor = [xform[2], xform[5]];
            return Ok(segment_hits_point(anchor, anchor, point, tolerance));
        };
        let rows = u32::try_from(sym.slots.len()).unwrap_or(u32::MAX);
        for sym_row in 0..rows {
            if sym.row_hits_point_with_cx(
                sym_row,
                None,
                point,
                tolerance,
                xform,
                depth,
                &mut |nested_idx, pt, tol, nested_xform, nested_depth, inner_cx| {
                    self.symbol_hits_with_cx(
                        nested_idx,
                        pt,
                        tol,
                        nested_xform,
                        nested_depth,
                        load_symbol,
                        inner_cx,
                    )
                },
                cx,
            )? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics};

    use super::*;

    #[test]
    fn segment_and_arc_hit_testing_is_exact_and_excludes_interior() {
        let mut display = Display::with_rows(2);
        // 行 0: (0,0)-(10,0) の線分
        display.push_with([1, 0, 0, 0], [0, 0], |w| {
            w.push_segment([0.0, 0.0], [10.0, 0.0]);
        });
        // 行 1: 中心 (0,0)・半径 10 の上半円弧 (0..180 度)
        display.push_with([0, 1, 0, 0], [0, 0], |w| {
            w.push_arc_deg([0.0, 0.0], 10.0, 0.0, 180.0);
        });
        // 行 2: 中心 (50,0)・半径 10 の全周円
        display.push_with([0, 1, 0, 0], [0, 0], |w| {
            w.push_circle([50.0, 0.0], 10.0);
        });

        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let mut no_sym =
            |_: u32, _: [f64; 2], _: f64, _: &[f64; 6], _: u32, _: &mut Cx<'_>| Ok(false);

        assert!(
            display
                .row_hits_point_with_cx(
                    0,
                    Some([0.0, 0.0, 10.0, 0.0]),
                    [5.0, 0.4],
                    0.5,
                    &IDENTITY_XFORM,
                    0,
                    &mut no_sym,
                    &mut cx,
                )
                .unwrap()
        );
        assert!(
            !display
                .row_hits_point_with_cx(
                    0,
                    Some([0.0, 0.0, 10.0, 0.0]),
                    [5.0, 0.6],
                    0.5,
                    &IDENTITY_XFORM,
                    0,
                    &mut no_sym,
                    &mut cx,
                )
                .unwrap()
        );

        // 上半円弧: 頂点 (0, 10) は当たるが、中心 (0, 0) や下側 (0, -10) は外れる
        assert!(
            display
                .row_hits_point_with_cx(
                    1,
                    Some([-10.0, 0.0, 10.0, 10.0]),
                    [0.0, 9.7],
                    0.5,
                    &IDENTITY_XFORM,
                    0,
                    &mut no_sym,
                    &mut cx,
                )
                .unwrap()
        );
        assert!(
            !display
                .row_hits_point_with_cx(
                    1,
                    Some([-10.0, 0.0, 10.0, 10.0]),
                    [0.0, 0.0],
                    0.5,
                    &IDENTITY_XFORM,
                    0,
                    &mut no_sym,
                    &mut cx,
                )
                .unwrap()
        );
        assert!(
            !display
                .row_hits_point_with_cx(
                    1,
                    Some([-10.0, -10.0, 10.0, 10.0]),
                    [0.0, -9.9],
                    0.5,
                    &IDENTITY_XFORM,
                    0,
                    &mut no_sym,
                    &mut cx,
                )
                .unwrap()
        );

        // 全周円: 円周上は当たり、円の中心は外れる
        assert!(
            display
                .row_hits_point_with_cx(
                    2,
                    Some([40.0, -10.0, 60.0, 10.0]),
                    [60.2, 0.0],
                    0.5,
                    &IDENTITY_XFORM,
                    0,
                    &mut no_sym,
                    &mut cx,
                )
                .unwrap()
        );
        assert!(
            !display
                .row_hits_point_with_cx(
                    2,
                    Some([40.0, -10.0, 60.0, 10.0]),
                    [50.0, 0.0],
                    0.5,
                    &IDENTITY_XFORM,
                    0,
                    &mut no_sym,
                    &mut cx,
                )
                .unwrap()
        );
    }
}
