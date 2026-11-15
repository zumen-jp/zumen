//! 差分で更新する表示のバッファーと、実 GPU（WebGPU）での転送・描画時間の測定。
//!
//! 開発用ポート（24100〜24199）で一時的なローカル HTTP サーバーを立て、ヘッドレスの Chromium 系ブラウザー
//! （Edge または Chrome）で WebGPU を動かして次の項目を測る。
//! - 初回の表示バッファー全体の GPU 転送（`queue.writeBuffer` ＋ `onSubmittedWorkDone`）
//! - 視点の移動・拡大縮小（カメラ uniform だけの更新 ＋ 全要素のインスタンス描画）
//! - 1 件を動かす・100 件を動かす・1 件の色を変える・1 件を除く変更での、64 要素ブロック単位の差分転送 ＋ 描画
//! - 比較用の、変更のたびに表示バッファー全体を再転送して描画する時間

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zumen_bench_live::born::{ColorSpec, Restyle};
use zumen_bench_live::display::{DIRTY_BLOCK, DirtyColumn, Display};
use zumen_bench_live::edit::Edit;
use zumen_bench_live::paged::Paged;
use zumen_bench_live::store::Document;
use zumen_drawing::Shape;

use crate::random::Random;

/// GPU 測定の設定
pub struct Options {
    /// 各ケースの計測フレーム数
    pub frames: usize,
    /// 待ち受けるポート（省略時は 24150〜24199 の空きを使う）
    pub port: Option<u16>,
    /// 乱数の種
    pub seed: u64,
}

#[allow(clippy::disallowed_methods)]
fn now() -> Instant {
    Instant::now()
}

fn us(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e6
}

fn median(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(f64::NAN)
}

fn p99(values: &mut [f64]) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(f64::total_cmp);
    let idx = ((values.len() as f64 * 0.99) as usize).min(values.len() - 1);
    values.get(idx).copied().unwrap_or(f64::NAN)
}

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_paged_f32<const N: usize>(out: &mut Vec<u8>, col: &Paged<[f32; N]>) {
    for item in col.iter() {
        for &v in item {
            push_f32(out, v);
        }
    }
}

fn push_paged_u32(out: &mut Vec<u8>, col: &Paged<u32>) {
    for &v in col.iter() {
        push_u32(out, v);
    }
}

fn push_paged_u32_arr<const N: usize>(out: &mut Vec<u8>, col: &Paged<[u32; N]>) {
    for item in col.iter() {
        for &v in item {
            push_u32(out, v);
        }
    }
}

fn aci_rgb(aci: u8) -> u32 {
    match aci {
        1 => 0x00FF_3B30,
        2 => 0x00FF_CC00,
        3 => 0x0034_C759,
        4 => 0x0032_ADE6,
        5 => 0x0000_7AFF,
        6 => 0x00AF_52DE,
        7 => 0x00F2_F2F7,
        _ => {
            let r = u32::from(aci.wrapping_mul(73) | 0x40);
            let g = u32::from(aci.wrapping_mul(151) | 0x40);
            let b = u32::from(aci.wrapping_mul(211) | 0x40);
            (r << 16) | (g << 8) | b
        }
    }
}

/// ブロック定義の中の図形をローカル座標の線分列 `[x0, y0, x1, y1]` に展開する（1 定義あたり最大 64 本）
fn build_symbol_segments(doc: &Document) -> (Vec<[u32; 2]>, Vec<[f32; 4]>, u32) {
    let mut ranges = Vec::with_capacity(doc.symbols.len().max(1));
    let mut segs: Vec<[f32; 4]> = Vec::new();
    let mut max_count = 0u32;
    if doc.symbols.is_empty() {
        ranges.push([0, 0]);
        segs.push([0.0; 4]);
        return (ranges, segs, 1);
    }
    for sym in &doc.symbols {
        let first = segs.len() as u32;
        let mut count = 0u32;
        for el in &sym.space.elements {
            if count >= 64 {
                break;
            }
            match &el.shape {
                Shape::Line { start, end } => {
                    segs.push([start.x as f32, start.y as f32, end.x as f32, end.y as f32]);
                    count += 1;
                }
                Shape::Polyline(poly) => {
                    let n = poly.vertices.len();
                    let last = if poly.closed && n > 1 {
                        n
                    } else {
                        n.saturating_sub(1)
                    };
                    for i in 0..last {
                        if count >= 64 {
                            break;
                        }
                        if let (Some(a), Some(b)) =
                            (poly.vertices.get(i), poly.vertices.get((i + 1) % n))
                        {
                            segs.push([
                                a.point.x as f32,
                                a.point.y as f32,
                                b.point.x as f32,
                                b.point.y as f32,
                            ]);
                            count += 1;
                        }
                    }
                }
                Shape::Circle { center, radius } => {
                    let r = match radius {
                        zumen_drawing::Length::Paper(p) => p.0 as f32,
                        zumen_drawing::Length::Model(m) => m.value as f32,
                    };
                    let (cx, cy) = (center.x as f32, center.y as f32);
                    let pts = [[cx + r, cy], [cx, cy + r], [cx - r, cy], [cx, cy - r]];
                    for i in 0..4 {
                        if count >= 64 {
                            break;
                        }
                        segs.push([
                            pts[i][0],
                            pts[i][1],
                            pts[(i + 1) % 4][0],
                            pts[(i + 1) % 4][1],
                        ]);
                        count += 1;
                    }
                }
                _ => {}
            }
        }
        if count == 0 {
            // 空の定義や対応外の図形だけの定義でも位置が分かるよう短い十字を置く
            segs.push([-0.5, 0.0, 0.5, 0.0]);
            segs.push([0.0, -0.5, 0.0, 0.5]);
            count = 2;
        }
        max_count = max_count.max(count);
        ranges.push([first, count]);
    }
    if segs.is_empty() {
        segs.push([0.0; 4]);
    }
    (ranges, segs, max_count.max(1))
}

fn compute_extent(doc: &Document) -> [f64; 4] {
    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for item in doc.boxes.iter() {
        if item[0] <= item[2] && item[1] <= item[3] && item.iter().all(|v| v.is_finite()) {
            extent = [
                extent[0].min(item[0]),
                extent[1].min(item[1]),
                extent[2].max(item[2]),
                extent[3].max(item[3]),
            ];
        }
    }
    if extent.iter().any(|v| !v.is_finite()) {
        extent = [0.0, 0.0, 100.0, 100.0];
    }
    extent
}

fn build_init_payload(doc: &Document, extent: [f64; 4]) -> Vec<u8> {
    let d = &doc.display;
    let (def_ranges, def_segs, max_def_segs) = build_symbol_segments(doc);
    let layer_count = doc.layers.len().max(1);
    let cx = (extent[0] + extent[2]) * 0.5;
    let cy = (extent[1] + extent[3]) * 0.5;
    let [eye_xh, eye_xl] = zumen_bench_live::display::split_f64(cx);
    let [eye_yh, eye_yl] = zumen_bench_live::display::split_f64(cy);
    let half_w = (((extent[2] - extent[0]) * 0.55).max(1.0)) as f32;
    let half_h = (((extent[3] - extent[1]) * 0.55).max(1.0)) as f32;

    let mut out = Vec::with_capacity(d.bytes() + 4096);
    push_u32(&mut out, 0x5A47_5055);
    push_u32(&mut out, d.segments.len() as u32);
    push_u32(&mut out, d.arcs.len() as u32);
    push_u32(&mut out, d.texts.len() as u32);
    push_u32(&mut out, d.instances.len() as u32);
    push_u32(&mut out, layer_count as u32);
    push_u32(&mut out, def_ranges.len() as u32);
    push_u32(&mut out, def_segs.len() as u32);
    push_u32(&mut out, max_def_segs);
    push_f32(&mut out, eye_xh);
    push_f32(&mut out, eye_xl);
    push_f32(&mut out, eye_yh);
    push_f32(&mut out, eye_yl);
    push_f32(&mut out, half_w);
    push_f32(&mut out, half_h);
    push_u32(&mut out, doc.counts.elements);

    if doc.layers.is_empty() {
        push_u32(&mut out, 1);
        push_u32(&mut out, 0x00F2_F2F7);
    } else {
        for layer in &doc.layers {
            let vis = u32::from(layer.state.is_none_or(|st| st.visible && !st.frozen));
            let rgb = layer
                .style
                .as_ref()
                .and_then(|s| match &s.color {
                    Some(c) => c
                        .rgb
                        .map(|[r, g, b]| (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
                        .or_else(|| c.code.and_then(|v| u8::try_from(v.value).ok()).map(aci_rgb)),
                    None => None,
                })
                .unwrap_or(0x00F2_F2F7);
            push_u32(&mut out, vis);
            push_u32(&mut out, rgb);
        }
    }

    for [first, count] in def_ranges {
        push_u32(&mut out, first);
        push_u32(&mut out, count);
    }
    for [x0, y0, x1, y1] in def_segs {
        push_f32(&mut out, x0);
        push_f32(&mut out, y0);
        push_f32(&mut out, x1);
        push_f32(&mut out, y1);
    }

    push_paged_f32(&mut out, &d.segments);
    push_paged_u32(&mut out, &d.segment_owner);
    push_paged_u32_arr(&mut out, &d.segment_style);

    push_paged_f32(&mut out, &d.arcs);
    push_paged_u32(&mut out, &d.arc_owner);
    push_paged_u32_arr(&mut out, &d.arc_style);

    push_paged_f32(&mut out, &d.texts);
    push_paged_u32(&mut out, &d.text_owner);
    push_paged_u32_arr(&mut out, &d.text_style);

    push_paged_f32(&mut out, &d.instances);
    push_paged_u32_arr(&mut out, &d.instance_ref);
    push_paged_u32(&mut out, &d.instance_owner);
    push_paged_u32_arr(&mut out, &d.instance_style);

    out
}

struct CpuPatchStats {
    name: &'static str,
    cpu_median_us: f64,
    cpu_p99_us: f64,
    dirty_median_bytes: f64,
    dirty_calls_median: f64,
}

fn append_dirty_writes(display: &mut Display, out: &mut Vec<u8>) -> (usize, usize) {
    let mut writes: Vec<(u32, u32, Vec<u8>)> = Vec::new();

    fn collect_f32<const N: usize>(
        col_id: u32,
        col: &Paged<[f32; N]>,
        dirty: &DirtyColumn,
        writes: &mut Vec<(u32, u32, Vec<u8>)>,
    ) {
        let stride = N * 4;
        for span in dirty.spans(col.len()) {
            if let Some(slice) = col.page_slice(span.start, span.end) {
                let mut buf = Vec::with_capacity(slice.len() * stride);
                for item in slice {
                    for &v in item {
                        buf.extend_from_slice(&v.to_le_bytes());
                    }
                }
                writes.push((col_id, (span.start * stride) as u32, buf));
            }
        }
    }

    fn collect_u32(
        col_id: u32,
        col: &Paged<u32>,
        dirty: &DirtyColumn,
        writes: &mut Vec<(u32, u32, Vec<u8>)>,
    ) {
        for span in dirty.spans(col.len()) {
            if let Some(slice) = col.page_slice(span.start, span.end) {
                let mut buf = Vec::with_capacity(slice.len() * 4);
                for &v in slice {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
                writes.push((col_id, (span.start * 4) as u32, buf));
            }
        }
    }

    fn collect_u32_arr<const N: usize>(
        col_id: u32,
        col: &Paged<[u32; N]>,
        dirty: &DirtyColumn,
        writes: &mut Vec<(u32, u32, Vec<u8>)>,
    ) {
        let stride = N * 4;
        for span in dirty.spans(col.len()) {
            if let Some(slice) = col.page_slice(span.start, span.end) {
                let mut buf = Vec::with_capacity(slice.len() * stride);
                for item in slice {
                    for &v in item {
                        buf.extend_from_slice(&v.to_le_bytes());
                    }
                }
                writes.push((col_id, (span.start * stride) as u32, buf));
            }
        }
    }

    collect_f32(0, &display.segments, &display.dirty.segments, &mut writes);
    collect_u32(
        1,
        &display.segment_owner,
        &display.dirty.segment_owner,
        &mut writes,
    );
    collect_u32_arr(
        2,
        &display.segment_style,
        &display.dirty.segment_style,
        &mut writes,
    );

    collect_f32(3, &display.arcs, &display.dirty.arcs, &mut writes);
    collect_u32(4, &display.arc_owner, &display.dirty.arc_owner, &mut writes);
    collect_u32_arr(5, &display.arc_style, &display.dirty.arc_style, &mut writes);

    collect_f32(6, &display.texts, &display.dirty.texts, &mut writes);
    collect_u32(
        7,
        &display.text_owner,
        &display.dirty.text_owner,
        &mut writes,
    );
    collect_u32_arr(
        8,
        &display.text_style,
        &display.dirty.text_style,
        &mut writes,
    );

    collect_f32(9, &display.instances, &display.dirty.instances, &mut writes);
    collect_u32_arr(
        10,
        &display.instance_ref,
        &display.dirty.instance_ref,
        &mut writes,
    );
    collect_u32(
        11,
        &display.instance_owner,
        &display.dirty.instance_owner,
        &mut writes,
    );
    collect_u32_arr(
        12,
        &display.instance_style,
        &display.dirty.instance_style,
        &mut writes,
    );

    push_u32(out, display.segments.len() as u32);
    push_u32(out, display.arcs.len() as u32);
    push_u32(out, display.texts.len() as u32);
    push_u32(out, display.instances.len() as u32);
    push_u32(out, writes.len() as u32);
    let mut total_bytes = 0usize;
    let calls = writes.len();
    for (col_id, dst_off, bytes) in writes {
        total_bytes += bytes.len();
        push_u32(out, col_id);
        push_u32(out, dst_off);
        push_u32(out, bytes.len() as u32);
        out.extend_from_slice(&bytes);
    }
    display.clear_dirty();
    (total_bytes, calls)
}

fn build_patches_payload(
    doc: &mut Document,
    extent: [f64; 4],
    frames: usize,
    seed: u64,
) -> (Vec<u8>, Vec<CpuPatchStats>) {
    let handles: Vec<u64> = doc
        .rows
        .iter()
        .filter(|r| r.alive && !doc.duplicate_handles.contains(&r.handle().unwrap_or(0)))
        .filter_map(|r| r.handle())
        .collect();
    let span = (extent[2] - extent[0]).max(extent[3] - extent[1]).max(1.0);
    let mut rng = Random(seed);
    let mut out = Vec::new();
    let mut stats = Vec::new();
    push_u32(&mut out, 4); // 4 種類の変更列

    // 1. 1 件を動かす
    {
        let mut cpu_times = Vec::with_capacity(frames);
        let mut d_bytes = Vec::with_capacity(frames);
        let mut d_calls = Vec::with_capacity(frames);
        let count_pos = out.len();
        push_u32(&mut out, 0);
        let mut recorded = 0u32;
        for _ in 0..frames {
            if handles.is_empty() {
                break;
            }
            let h = handles[rng.below(handles.len())];
            let dx = rng.between(-span, span) / 128.0;
            let dy = rng.between(-span, span) / 128.0;
            let edit = [Edit::Transform {
                handle: h,
                matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
            }];
            let t0 = now();
            let ok = doc.apply(&edit).is_ok();
            let elapsed = us(t0);
            if ok {
                let (b, c) = append_dirty_writes(&mut doc.display, &mut out);
                cpu_times.push(elapsed);
                d_bytes.push(b as f64);
                d_calls.push(c as f64);
                recorded += 1;
            }
        }
        out[count_pos..count_pos + 4].copy_from_slice(&recorded.to_le_bytes());
        stats.push(CpuPatchStats {
            name: "1 件を動かす",
            cpu_median_us: median(&mut cpu_times),
            cpu_p99_us: p99(&mut cpu_times),
            dirty_median_bytes: median(&mut d_bytes),
            dirty_calls_median: median(&mut d_calls),
        });
    }

    // 2. 続いた 100 件をまとめて動かす
    {
        let group = handles.len().min(100);
        let mut cpu_times = Vec::with_capacity(frames);
        let mut d_bytes = Vec::with_capacity(frames);
        let mut d_calls = Vec::with_capacity(frames);
        let count_pos = out.len();
        push_u32(&mut out, 0);
        let mut recorded = 0u32;
        for _ in 0..frames {
            if group == 0 {
                break;
            }
            let first = rng.below(handles.len() - group + 1);
            let dx = rng.between(-span, span) / 128.0;
            let dy = rng.between(-span, span) / 128.0;
            let edits: Vec<Edit> = handles[first..first + group]
                .iter()
                .map(|&h| Edit::Transform {
                    handle: h,
                    matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
                })
                .collect();
            let t0 = now();
            let ok = doc.apply(&edits).is_ok();
            let elapsed = us(t0);
            if ok {
                let (b, c) = append_dirty_writes(&mut doc.display, &mut out);
                cpu_times.push(elapsed);
                d_bytes.push(b as f64);
                d_calls.push(c as f64);
                recorded += 1;
            }
        }
        out[count_pos..count_pos + 4].copy_from_slice(&recorded.to_le_bytes());
        stats.push(CpuPatchStats {
            name: "100 件を 1 回で動かす",
            cpu_median_us: median(&mut cpu_times),
            cpu_p99_us: p99(&mut cpu_times),
            dirty_median_bytes: median(&mut d_bytes),
            dirty_calls_median: median(&mut d_calls),
        });
    }

    // 3. 1 件の色を変える
    {
        let mut cpu_times = Vec::with_capacity(frames);
        let mut d_bytes = Vec::with_capacity(frames);
        let mut d_calls = Vec::with_capacity(frames);
        let count_pos = out.len();
        push_u32(&mut out, 0);
        let mut recorded = 0u32;
        for i in 0..frames {
            if handles.is_empty() {
                break;
            }
            let h = handles[rng.below(handles.len())];
            let edit = [Edit::Restyle {
                handle: h,
                style: Restyle {
                    color: Some(ColorSpec::Aci(1 + (i % 6) as u8)),
                    ..Restyle::default()
                },
            }];
            let t0 = now();
            let ok = doc.apply(&edit).is_ok();
            let elapsed = us(t0);
            if ok {
                let (b, c) = append_dirty_writes(&mut doc.display, &mut out);
                cpu_times.push(elapsed);
                d_bytes.push(b as f64);
                d_calls.push(c as f64);
                recorded += 1;
            }
        }
        out[count_pos..count_pos + 4].copy_from_slice(&recorded.to_le_bytes());
        stats.push(CpuPatchStats {
            name: "1 件の色を変える",
            cpu_median_us: median(&mut cpu_times),
            cpu_p99_us: p99(&mut cpu_times),
            dirty_median_bytes: median(&mut d_bytes),
            dirty_calls_median: median(&mut d_calls),
        });
    }

    // 4. 1 件を除いてすぐ取り消す（可視フラグ配列だけの更新）
    {
        let mut cpu_times = Vec::with_capacity(frames);
        let mut d_bytes = Vec::with_capacity(frames);
        let mut d_calls = Vec::with_capacity(frames);
        let count_pos = out.len();
        push_u32(&mut out, 0);
        let mut recorded = 0u32;
        for step in 0..frames {
            if handles.is_empty() {
                break;
            }
            if step % 2 == 0 {
                let h = handles[rng.below(handles.len())];
                let edit = [Edit::Remove { handle: h }];
                let t0 = now();
                let ok = doc.apply(&edit).is_ok();
                let elapsed = us(t0);
                if ok {
                    let (b, c) = append_dirty_writes(&mut doc.display, &mut out);
                    cpu_times.push(elapsed);
                    d_bytes.push(b as f64);
                    d_calls.push(c as f64);
                    recorded += 1;
                }
            } else {
                let t0 = now();
                let ok = doc.undo();
                let elapsed = us(t0);
                if ok {
                    let (b, c) = append_dirty_writes(&mut doc.display, &mut out);
                    cpu_times.push(elapsed);
                    d_bytes.push(b as f64);
                    d_calls.push(c as f64);
                    recorded += 1;
                }
            }
        }
        out[count_pos..count_pos + 4].copy_from_slice(&recorded.to_le_bytes());
        stats.push(CpuPatchStats {
            name: "1 件を除く・取り消す",
            cpu_median_us: median(&mut cpu_times),
            cpu_p99_us: p99(&mut cpu_times),
            dirty_median_bytes: median(&mut d_bytes),
            dirty_calls_median: median(&mut d_calls),
        });
    }

    (out, stats)
}

const HTML_PAGE: &str = r#"<!doctype html>
<meta charset="utf-8">
<title>zumen live WebGPU benchmark</title>
<canvas id="cv" width="1920" height="1080"></canvas>
<script>
const COMMON_WGSL = `
struct Camera {
  eye_hi: vec2<f32>,
  eye_lo: vec2<f32>,
  inv_half: vec2<f32>,
  viewport: vec2<f32>,
};

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var<storage, read> layers: array<vec2<u32>>;

fn resolve_color(style: vec2<u32>) -> vec4<f32> {
  let n = arrayLength(&layers);
  let lid = min(style.x, n - 1u);
  let linfo = layers[lid];
  if (linfo.x == 0u) {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
  }
  var c = linfo.y;
  if (style.y != 0u) {
    c = style.y * 2654435761u;
  }
  let r = max(f32((c >> 16u) & 255u) / 255.0, 0.2);
  let g = max(f32((c >> 8u) & 255u) / 255.0, 0.2);
  let b = max(f32(c & 255u) / 255.0, 0.2);
  return vec4<f32>(r, g, b, 1.0);
}

fn expand_line_quad(p0: vec2<f32>, p1: vec2<f32>, viewport: vec2<f32>, corner: u32) -> vec2<f32> {
  let d_px = (p1 - p0) * 0.5 * viewport;
  let len_px = length(d_px);
  var dir = vec2<f32>(1.0, 0.0);
  if (len_px > 1e-4) {
    dir = d_px / len_px;
  }
  let nrm = vec2<f32>(-dir.y, dir.x);
  let half_w = 0.85;
  var a_px = p0 * 0.5 * viewport;
  var b_px = p1 * 0.5 * viewport;
  if (len_px < 1.2) {
    let mid = 0.5 * (a_px + b_px);
    a_px = mid - dir * 0.6;
    b_px = mid + dir * 0.6;
  }
  var pos_px = a_px;
  switch (corner) {
    case 0u: { pos_px = a_px - nrm * half_w; }
    case 1u: { pos_px = b_px - nrm * half_w; }
    case 2u: { pos_px = a_px + nrm * half_w; }
    case 3u: { pos_px = a_px + nrm * half_w; }
    case 4u: { pos_px = b_px - nrm * half_w; }
    default: { pos_px = b_px + nrm * half_w; }
  }
  return (pos_px / viewport) * 2.0;
}
`;

const SEG_WGSL = COMMON_WGSL + `
@group(0) @binding(2) var<storage, read> segs: array<f32>;
@group(0) @binding(3) var<storage, read> owners: array<u32>;
@group(0) @binding(4) var<storage, read> styles: array<vec2<u32>>;

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
  var out: VsOut;
  if (owners[ii] == 0xffffffffu) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let col = resolve_color(styles[ii]);
  if (col.a == 0.0) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let b = ii * 8u;
  let p0 = vec2<f32>(
    (segs[b] - cam.eye_hi.x) + (segs[b + 1u] - cam.eye_lo.x),
    (segs[b + 2u] - cam.eye_hi.y) + (segs[b + 3u] - cam.eye_lo.y)
  ) * cam.inv_half;
  let p1 = vec2<f32>(
    (segs[b + 4u] - cam.eye_hi.x) + (segs[b + 5u] - cam.eye_lo.x),
    (segs[b + 6u] - cam.eye_hi.y) + (segs[b + 7u] - cam.eye_lo.y)
  ) * cam.inv_half;
  let ndc = expand_line_quad(p0, p1, cam.viewport, vi);
  out.pos = vec4<f32>(ndc, 0.0, 1.0);
  out.color = col;
  return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
  return in.color;
}
`;

const ARC_WGSL = COMMON_WGSL + `
@group(0) @binding(2) var<storage, read> arcs: array<f32>;
@group(0) @binding(3) var<storage, read> owners: array<u32>;
@group(0) @binding(4) var<storage, read> styles: array<vec2<u32>>;

struct ArcOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
  @location(1) local_px: vec2<f32>,
  @location(2) params: vec3<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> ArcOut {
  var out: ArcOut;
  if (owners[ii] == 0xffffffffu) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let col = resolve_color(styles[ii]);
  if (col.a == 0.0) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let b = ii * 7u;
  let c_ndc = vec2<f32>(
    (arcs[b] - cam.eye_hi.x) + (arcs[b + 1u] - cam.eye_lo.x),
    (arcs[b + 2u] - cam.eye_hi.y) + (arcs[b + 3u] - cam.eye_lo.y)
  ) * cam.inv_half;
  let r_px = max(arcs[b + 4u] * cam.inv_half.x * 0.5 * cam.viewport.x, 0.8);
  let pad_px = r_px + 1.5;
  var corner = vec2<f32>(-1.0, -1.0);
  switch (vi) {
    case 0u: { corner = vec2<f32>(-1.0, -1.0); }
    case 1u: { corner = vec2<f32>( 1.0, -1.0); }
    case 2u: { corner = vec2<f32>(-1.0,  1.0); }
    case 3u: { corner = vec2<f32>(-1.0,  1.0); }
    case 4u: { corner = vec2<f32>( 1.0, -1.0); }
    default: { corner = vec2<f32>( 1.0,  1.0); }
  }
  let off_px = corner * pad_px;
  let ndc = c_ndc + (off_px / cam.viewport) * 2.0;
  out.pos = vec4<f32>(ndc, 0.0, 1.0);
  out.color = col;
  out.local_px = off_px;
  out.params = vec3<f32>(r_px, arcs[b + 5u], arcs[b + 6u]);
  return out;
}

@fragment
fn fs(in: ArcOut) -> @location(0) vec4<f32> {
  let d = length(in.local_px);
  if (abs(d - in.params.x) > 1.1) {
    discard;
  }
  let sweep = in.params.z;
  if (abs(sweep) < 6.282) {
    let ang = atan2(in.local_px.y, in.local_px.x);
    let rel = fract((ang - in.params.y) / 6.2831853);
    let need = abs(sweep) / 6.2831853;
    if (rel > need) {
      discard;
    }
  }
  return in.color;
}
`;

const TXT_WGSL = COMMON_WGSL + `
@group(0) @binding(2) var<storage, read> texts: array<f32>;
@group(0) @binding(3) var<storage, read> owners: array<u32>;
@group(0) @binding(4) var<storage, read> styles: array<vec2<u32>>;

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
  var out: VsOut;
  if (owners[ii] == 0xffffffffu) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let col = resolve_color(styles[ii]);
  if (col.a == 0.0) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let b = ii * 8u;
  let base_w = vec2<f32>(
    (texts[b] - cam.eye_hi.x) + (texts[b + 1u] - cam.eye_lo.x),
    (texts[b + 2u] - cam.eye_hi.y) + (texts[b + 3u] - cam.eye_lo.y)
  );
  let h = texts[b + 4u];
  let rot = texts[b + 5u];
  let wf = max(texts[b + 6u], 0.5);
  let dir = vec2<f32>(cos(rot), sin(rot)) * (h * 2.5 * wf);
  let p0 = base_w * cam.inv_half;
  let p1 = (base_w + dir) * cam.inv_half;
  let ndc = expand_line_quad(p0, p1, cam.viewport, vi);
  out.pos = vec4<f32>(ndc, 0.0, 1.0);
  out.color = col;
  return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
  return in.color;
}
`;

const INST_WGSL = COMMON_WGSL + `
@group(0) @binding(2) var<storage, read> insts: array<f32>;
@group(0) @binding(3) var<storage, read> irefs: array<u32>;
@group(0) @binding(4) var<storage, read> owners: array<u32>;
@group(0) @binding(5) var<storage, read> styles: array<vec2<u32>>;
@group(0) @binding(6) var<storage, read> def_ranges: array<vec2<u32>>;
@group(0) @binding(7) var<storage, read> def_segs: array<vec4<f32>>;

struct VsOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) color: vec4<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
  var out: VsOut;
  if (owners[ii] == 0xffffffffu) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let rbase = ii * 3u;
  let def_id = irefs[rbase];
  if (def_id >= arrayLength(&def_ranges)) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let dr = def_ranges[def_id];
  let local_seg = vi / 6u;
  let corner = vi % 6u;
  if (local_seg >= dr.y) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let col = resolve_color(styles[ii]);
  if (col.a == 0.0) {
    out.pos = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    return out;
  }
  let ib = ii * 9u;
  let origin = vec2<f32>(
    (insts[ib] - cam.eye_hi.x) + (insts[ib + 1u] - cam.eye_lo.x),
    (insts[ib + 2u] - cam.eye_hi.y) + (insts[ib + 3u] - cam.eye_lo.y)
  );
  let sx = insts[ib + 4u];
  let sy = insts[ib + 5u];
  let rot = insts[ib + 6u];
  let cs = cos(rot);
  let sn = sin(rot);
  let seg = def_segs[dr.x + local_seg];
  let l0 = vec2<f32>(seg.x * sx, seg.y * sy);
  let l1 = vec2<f32>(seg.z * sx, seg.w * sy);
  let w0 = origin + vec2<f32>(l0.x * cs - l0.y * sn, l0.x * sn + l0.y * cs);
  let w1 = origin + vec2<f32>(l1.x * cs - l1.y * sn, l1.x * sn + l1.y * cs);
  let ndc = expand_line_quad(w0 * cam.inv_half, w1 * cam.inv_half, cam.viewport, corner);
  out.pos = vec4<f32>(ndc, 0.0, 1.0);
  out.color = col;
  return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
  return in.color;
}
`;

function statsOf(arr) {
  if (!arr.length) return { median: 0, min: 0, p99: 0 };
  const s = [...arr].sort((a, b) => a - b);
  return {
    median: s[Math.floor(s.length / 2)],
    min: s[0],
    p99: s[Math.min(Math.floor(s.length * 0.99), s.length - 1)],
  };
}

async function main() {
  try {
    if (!navigator.gpu) {
      throw new Error("navigator.gpu が無効");
    }
    const adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
    if (!adapter) {
      throw new Error("WebGPU adapter を取得できない");
    }
    const info = adapter.info || {};
    const adapterDesc = [info.vendor, info.architecture, info.device, info.description]
      .filter(Boolean)
      .join(" / ") || "WebGPU Adapter";
    const hasTimestamp = adapter.features.has("timestamp-query");
    const device = await adapter.requestDevice({
      requiredFeatures: hasTimestamp ? ["timestamp-query"] : [],
      requiredLimits: {
        maxStorageBufferBindingSize: Math.min(adapter.limits.maxStorageBufferBindingSize, 512 * 1024 * 1024),
        maxBufferSize: Math.min(adapter.limits.maxBufferSize, 512 * 1024 * 1024),
      },
    });

    const [initBuf, patchBuf] = await Promise.all([
      fetch("/init.bin").then((r) => r.arrayBuffer()),
      fetch("/patches.bin").then((r) => r.arrayBuffer()),
    ]);

    const dv = new DataView(initBuf);
    let off = 0;
    const magic = dv.getUint32(off, true); off += 4;
    if (magic !== 0x5A475055) throw new Error("init magic mismatch");
    let segLen = dv.getUint32(off, true); off += 4;
    let arcLen = dv.getUint32(off, true); off += 4;
    let txtLen = dv.getUint32(off, true); off += 4;
    let instLen = dv.getUint32(off, true); off += 4;
    const layerLen = dv.getUint32(off, true); off += 4;
    const defLen = dv.getUint32(off, true); off += 4;
    const defSegLen = dv.getUint32(off, true); off += 4;
    const maxDefSegs = dv.getUint32(off, true); off += 4;
    const eyeXh = dv.getFloat32(off, true); off += 4;
    const eyeXl = dv.getFloat32(off, true); off += 4;
    const eyeYh = dv.getFloat32(off, true); off += 4;
    const eyeYl = dv.getFloat32(off, true); off += 4;
    const halfW = dv.getFloat32(off, true); off += 4;
    const halfH = dv.getFloat32(off, true); off += 4;
    const elemCount = dv.getUint32(off, true); off += 4;

    const sliceSpec = (byteLen) => {
      const s = { offset: off, byteLen };
      off += byteLen;
      return s;
    };

    const layerSpec = sliceSpec(layerLen * 8);
    const defRangeSpec = sliceSpec(defLen * 8);
    const defSegSpec = sliceSpec(defSegLen * 16);

    const colSpecs = [
      sliceSpec(segLen * 32),
      sliceSpec(segLen * 4),
      sliceSpec(segLen * 8),
      sliceSpec(arcLen * 28),
      sliceSpec(arcLen * 4),
      sliceSpec(arcLen * 8),
      sliceSpec(txtLen * 32),
      sliceSpec(txtLen * 4),
      sliceSpec(txtLen * 8),
      sliceSpec(instLen * 36),
      sliceSpec(instLen * 12),
      sliceSpec(instLen * 4),
      sliceSpec(instLen * 8),
    ];

    const totalGpuBytes = colSpecs.reduce((a, c) => a + c.byteLen, 0) + layerSpec.byteLen + defRangeSpec.byteLen + defSegSpec.byteLen;

    const makeStorage = (spec, extraSlack) => {
      const cap = Math.max(spec.byteLen + extraSlack, 64);
      return device.createBuffer({
        size: (cap + 15) & ~15,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
    };

    // 初回転送を測定（複数回測って中央値を取る）
    const layerBuf = makeStorage(layerSpec, 0);
    const defRangeBuf = makeStorage(defRangeSpec, 0);
    const defSegBuf = makeStorage(defSegSpec, 0);
    const colBufs = colSpecs.map((s) => makeStorage(s, 65536));

    const uploadAll = () => {
      device.queue.writeBuffer(layerBuf, 0, initBuf, layerSpec.offset, layerSpec.byteLen);
      device.queue.writeBuffer(defRangeBuf, 0, initBuf, defRangeSpec.offset, defRangeSpec.byteLen);
      device.queue.writeBuffer(defSegBuf, 0, initBuf, defSegSpec.offset, defSegSpec.byteLen);
      for (let i = 0; i < 13; i++) {
        if (colSpecs[i].byteLen > 0) {
          device.queue.writeBuffer(colBufs[i], 0, initBuf, colSpecs[i].offset, colSpecs[i].byteLen);
        }
      }
    };

    uploadAll();
    await device.queue.onSubmittedWorkDone();

    const initUploadTimes = [];
    for (let r = 0; r < 7; r++) {
      const t0 = performance.now();
      uploadAll();
      await device.queue.onSubmittedWorkDone();
      initUploadTimes.push(performance.now() - t0);
    }

    const camBuf = device.createBuffer({
      size: 32,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const setCamera = (panFrac, zoom) => {
      const arr = new Float32Array([
        eyeXh + halfW * panFrac,
        eyeXl,
        eyeYh + halfH * panFrac,
        eyeYl,
        zoom / halfW,
        zoom / halfH,
        1920.0,
        1080.0,
      ]);
      device.queue.writeBuffer(camBuf, 0, arr);
    };
    setCamera(0.0, 1.0);

    const makePipe = (code) => {
      const mod = device.createShaderModule({ code });
      return device.createRenderPipeline({
        layout: "auto",
        vertex: { module: mod, entryPoint: "vs" },
        fragment: {
          module: mod,
          entryPoint: "fs",
          targets: [{ format: "rgba8unorm" }],
        },
        primitive: { topology: "triangle-list" },
      });
    };

    const segPipe = makePipe(SEG_WGSL);
    const arcPipe = makePipe(ARC_WGSL);
    const txtPipe = makePipe(TXT_WGSL);
    const instPipe = makePipe(INST_WGSL);

    const bgEntries = (pipe, indices) =>
      device.createBindGroup({
        layout: pipe.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: camBuf } },
          { binding: 1, resource: { buffer: layerBuf } },
          ...indices.map((buf, idx) => ({ binding: idx + 2, resource: { buffer: buf } })),
        ],
      });

    const segBG = bgEntries(segPipe, [colBufs[0], colBufs[1], colBufs[2]]);
    const arcBG = bgEntries(arcPipe, [colBufs[3], colBufs[4], colBufs[5]]);
    const txtBG = bgEntries(txtPipe, [colBufs[6], colBufs[7], colBufs[8]]);
    const instBG = bgEntries(instPipe, [colBufs[9], colBufs[10], colBufs[11], colBufs[12], defRangeBuf, defSegBuf]);

    const targetTex = device.createTexture({
      size: [1920, 1080],
      format: "rgba8unorm",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    });
    const targetView = targetTex.createView();

    let querySet = null;
    let queryResolveBuf = null;
    let queryReadBuf = null;
    if (hasTimestamp) {
      querySet = device.createQuerySet({ type: "timestamp", count: 2 });
      queryResolveBuf = device.createBuffer({
        size: 16,
        usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC,
      });
      queryReadBuf = device.createBuffer({
        size: 16,
        usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
      });
    }

    const encodeDraw = (enc, recordTimestamp) => {
      const desc = {
        colorAttachments: [{
          view: targetView,
          clearValue: { r: 0.06, g: 0.07, b: 0.09, a: 1.0 },
          loadOp: "clear",
          storeOp: "store",
        }],
      };
      if (recordTimestamp && querySet) {
        desc.timestampWrites = {
          querySet,
          beginningOfPassWriteIndex: 0,
          endOfPassWriteIndex: 1,
        };
      }
      const pass = enc.beginRenderPass(desc);
      if (segLen > 0) {
        pass.setPipeline(segPipe);
        pass.setBindGroup(0, segBG);
        pass.draw(6, segLen, 0, 0);
      }
      if (arcLen > 0) {
        pass.setPipeline(arcPipe);
        pass.setBindGroup(0, arcBG);
        pass.draw(6, arcLen, 0, 0);
      }
      if (txtLen > 0) {
        pass.setPipeline(txtPipe);
        pass.setBindGroup(0, txtBG);
        pass.draw(6, txtLen, 0, 0);
      }
      if (instLen > 0) {
        pass.setPipeline(instPipe);
        pass.setBindGroup(0, instBG);
        pass.draw(6 * maxDefSegs, instLen, 0, 0);
      }
      pass.end();
      if (recordTimestamp && querySet) {
        enc.resolveQuerySet(querySet, 0, 2, queryResolveBuf, 0);
        enc.copyBufferToBuffer(queryResolveBuf, 0, queryReadBuf, 0, 16);
      }
    };

    const readPassMs = async () => {
      if (!queryReadBuf) return null;
      await queryReadBuf.mapAsync(GPUMapMode.READ);
      const u64 = new BigUint64Array(queryReadBuf.getMappedRange());
      const ns = Number(u64[1] - u64[0]);
      queryReadBuf.unmap();
      return ns / 1e6;
    };

    // ウォームアップと描画ピクセル検証
    for (let w = 0; w < 8; w++) {
      const enc = device.createCommandEncoder();
      encodeDraw(enc, false);
      device.queue.submit([enc.finish()]);
      await device.queue.onSubmittedWorkDone();
    }

    // 中央 256x256 ピクセルを読み戻し、背景色以外のピクセルが実際に描かれたことを検証する
    const probeBuf = device.createBuffer({
      size: 256 * 256 * 4,
      usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
    });
    {
      const enc = device.createCommandEncoder();
      encodeDraw(enc, false);
      enc.copyTextureToBuffer(
        { texture: targetTex, origin: [832, 412, 0] },
        { buffer: probeBuf, bytesPerRow: 256 * 4, rowsPerImage: 256 },
        [256, 256, 1]
      );
      device.queue.submit([enc.finish()]);
      await probeBuf.mapAsync(GPUMapMode.READ);
      const px = new Uint8Array(probeBuf.getMappedRange());
      var litPixels = 0;
      for (let i = 0; i < px.length; i += 4) {
        if (px[i] > 30 || px[i + 1] > 30 || px[i + 2] > 35) litPixels++;
      }
      probeBuf.unmap();
    }

    // 視点移動・拡大縮小フレームの測定
    const panWall = [];
    const panGpu = [];
    for (let f = 0; f < 40; f++) {
      const pan = Math.sin(f * 0.2) * 0.15;
      const zoom = 1.0 + Math.cos(f * 0.15) * 0.25;
      const t0 = performance.now();
      setCamera(pan, zoom);
      const enc = device.createCommandEncoder();
      encodeDraw(enc, hasTimestamp);
      device.queue.submit([enc.finish()]);
      await device.queue.onSubmittedWorkDone();
      panWall.push(performance.now() - t0);
      if (hasTimestamp) {
        const ms = await readPassMs();
        if (ms !== null && ms >= 0) panGpu.push(ms);
      }
    }
    setCamera(0.0, 1.0);

    // 差分パッチ列を順に適用して測定
    const pdv = new DataView(patchBuf);
    let poff = 0;
    const workloadCount = pdv.getUint32(poff, true); poff += 4;
    const workloads = [];
    for (let w = 0; w < workloadCount; w++) {
      const stepCount = pdv.getUint32(poff, true); poff += 4;
      const wallTimes = [];
      const gpuPassTimes = [];
      const uploadedBytes = [];
      for (let s = 0; s < stepCount; s++) {
        const nSeg = pdv.getUint32(poff, true); poff += 4;
        const nArc = pdv.getUint32(poff, true); poff += 4;
        const nTxt = pdv.getUint32(poff, true); poff += 4;
        const nInst = pdv.getUint32(poff, true); poff += 4;
        const writeCount = pdv.getUint32(poff, true); poff += 4;
        const writes = [];
        let stepBytes = 0;
        for (let k = 0; k < writeCount; k++) {
          const colId = pdv.getUint32(poff, true); poff += 4;
          const dstOff = pdv.getUint32(poff, true); poff += 4;
          const bLen = pdv.getUint32(poff, true); poff += 4;
          writes.push({ colId, dstOff, srcOff: poff, bLen });
          poff += bLen;
          stepBytes += bLen;
        }
        const t0 = performance.now();
        segLen = nSeg; arcLen = nArc; txtLen = nTxt; instLen = nInst;
        for (let k = 0; k < writes.length; k++) {
          const wr = writes[k];
          if (wr.bLen > 0) {
            device.queue.writeBuffer(colBufs[wr.colId], wr.dstOff, patchBuf, wr.srcOff, wr.bLen);
          }
        }
        const enc = device.createCommandEncoder();
        encodeDraw(enc, hasTimestamp);
        device.queue.submit([enc.finish()]);
        await device.queue.onSubmittedWorkDone();
        wallTimes.push(performance.now() - t0);
        uploadedBytes.push(stepBytes);
        if (hasTimestamp) {
          const ms = await readPassMs();
          if (ms !== null && ms >= 0) gpuPassTimes.push(ms);
        }
      }
      workloads.push({
        wall: statsOf(wallTimes),
        gpu: statsOf(gpuPassTimes),
        bytesMedian: statsOf(uploadedBytes).median,
      });
    }

    // 比較用: バッファー全体を再転送して 1 フレーム描く時間
    const fullReuploadWall = [];
    for (let r = 0; r < 15; r++) {
      const t0 = performance.now();
      uploadAll();
      const enc = device.createCommandEncoder();
      encodeDraw(enc, false);
      device.queue.submit([enc.finish()]);
      await device.queue.onSubmittedWorkDone();
      fullReuploadWall.push(performance.now() - t0);
    }

    const report = {
      ok: true,
      adapter: adapterDesc,
      hasTimestamp,
      elemCount,
      totalGpuBytes,
      litPixels,
      initUpload: statsOf(initUploadTimes),
      panDraw: { wall: statsOf(panWall), gpu: statsOf(panGpu) },
      fullReuploadDraw: statsOf(fullReuploadWall),
      workloads,
    };
    await fetch("/report", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(report),
    });
  } catch (err) {
    await fetch("/report", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ ok: false, error: String(err && err.stack || err) }),
    });
  }
}
main();
</script>
"#;

fn find_browser() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn bind_dev_port(preferred: Option<u16>) -> Result<(TcpListener, u16), String> {
    if let Some(p) = preferred {
        if !(24100..=24199).contains(&p) {
            return Err(format!("ポート番号は 24100〜24199 の範囲で指定する: {p}"));
        }
        let listener = TcpListener::bind(("127.0.0.1", p))
            .map_err(|e| format!("127.0.0.1:{p} を開けない: {e}"))?;
        return Ok((listener, p));
    }
    for p in 24150..=24199 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", p)) {
            return Ok((listener, p));
        }
    }
    Err("24150〜24199 に空いているポートがない".to_owned())
}

fn handle_http(
    mut stream: TcpStream,
    init_bin: &[u8],
    patches_bin: &[u8],
    report_slot: &Mutex<Option<String>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut buf = vec![0u8; 65536];
    let mut read_total = 0usize;
    let mut header_end = None;
    let mut content_len = 0usize;

    loop {
        if read_total == buf.len() {
            buf.resize(buf.len() * 2, 0);
        }
        let Ok(n) = stream.read(&mut buf[read_total..]) else {
            break;
        };
        if n == 0 {
            break;
        }
        read_total += n;
        if header_end.is_none()
            && let Some(pos) = buf[..read_total].windows(4).position(|w| w == b"\r\n\r\n")
        {
            let hend = pos + 4;
            header_end = Some(hend);
            let head_str = String::from_utf8_lossy(&buf[..hend]);
            for line in head_str.lines() {
                if let Some((k, v)) = line.split_once(':')
                    && k.trim().eq_ignore_ascii_case("content-length")
                {
                    content_len = v.trim().parse().unwrap_or(0);
                }
            }
        }
        if let Some(hend) = header_end
            && read_total >= hend + content_len
        {
            break;
        }
    }

    let Some(hend) = header_end else {
        return;
    };
    let head_str = String::from_utf8_lossy(&buf[..hend]);
    let first_line = head_str.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    let send = |stream: &mut TcpStream, ctype: &str, body: &[u8]| {
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(body);
        let _ = stream.flush();
    };

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            send(
                &mut stream,
                "text/html; charset=utf-8",
                HTML_PAGE.as_bytes(),
            );
        }
        ("GET", "/init.bin") => {
            send(&mut stream, "application/octet-stream", init_bin);
        }
        ("GET", "/patches.bin") => {
            send(&mut stream, "application/octet-stream", patches_bin);
        }
        ("POST", "/report") => {
            let body = String::from_utf8_lossy(&buf[hend..hend + content_len]).into_owned();
            if let Ok(mut guard) = report_slot.lock() {
                *guard = Some(body);
            }
            send(&mut stream, "text/plain", b"ok");
        }
        _ => {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        }
    }
}

fn run_one(path: &str, options: &Options, browser: &Path) -> Result<(), String> {
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    let mut doc = Document::open_path(std::path::Path::new(path), threads)?;
    let extent = compute_extent(&doc);
    let init_bin = Arc::new(build_init_payload(&doc, extent));
    let (patches_raw, cpu_stats) =
        build_patches_payload(&mut doc, extent, options.frames, options.seed);
    let patches_bin = Arc::new(patches_raw);

    let (listener, port) = bind_dev_port(options.port)?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("nonblocking 設定失敗: {e}"))?;

    let profile_dir = std::env::temp_dir().join(format!("zumen-live-gpu-{port}"));
    let _ = std::fs::remove_dir_all(&profile_dir);
    let _ = std::fs::create_dir_all(&profile_dir);

    let url = format!("http://127.0.0.1:{port}/");
    let mut child = Command::new(browser)
        .arg("--headless=new")
        .arg("--enable-unsafe-webgpu")
        .arg("--enable-dawn-features=allow_unsafe_apis")
        .arg("--disable-gpu-vsync")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-extensions")
        .arg(format!("--user-data-dir={}", profile_dir.display()))
        .arg(&url)
        .spawn()
        .map_err(|e| format!("ブラウザーを起動できない: {e}"))?;

    let report_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let deadline = now() + Duration::from_secs(90);
    while now() < deadline {
        if report_slot.lock().ok().is_some_and(|g| g.is_some()) {
            break;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let init_c = Arc::clone(&init_bin);
                let patch_c = Arc::clone(&patches_bin);
                let slot_c = Arc::clone(&report_slot);
                std::thread::spawn(move || {
                    handle_http(stream, &init_c, &patch_c, &slot_c);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile_dir);

    let raw_json = report_slot
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .ok_or_else(|| "時間内に GPU 測定の応答が届かなかった".to_owned())?;

    let parsed: zumen::json::Value = zumen::json::parse(raw_json.as_bytes())
        .map_err(|_| format!("JSON を読めない: {raw_json}"))?;
    if parsed.pointer("/ok") != Some(&zumen::json::Value::Bool(true)) {
        let err_msg = parsed
            .pointer("/error")
            .map(|v| format!("{v:?}"))
            .unwrap_or(raw_json);
        return Err(format!("WebGPU 実行エラー: {err_msg}"));
    }

    let num = |ptr: &str| -> f64 {
        match parsed.pointer(ptr) {
            Some(zumen::json::Value::Number(n)) => n.to_f64().unwrap_or(f64::NAN),
            _ => f64::NAN,
        }
    };
    let adapter = match parsed.pointer("/adapter") {
        Some(zumen::json::Value::String(s)) => s.as_str(),
        _ => "不明",
    };
    let has_ts = parsed.pointer("/hasTimestamp") == Some(&zumen::json::Value::Bool(true));
    let gpu_mib = num("/totalGpuBytes") / (1024.0 * 1024.0);
    let lit_px = num("/litPixels") as u64;

    println!(
        "== {path}（要素 {}、差分ブロック {} 要素、GPU バッファー {:.2} MiB）",
        doc.counts.elements, DIRTY_BLOCK, gpu_mib
    );
    println!(
        "  GPU: {adapter}（1920×1080、中央 256×256 の描画ピクセル {lit_px}、ハードウェア timestamp {}）",
        if has_ts { "あり" } else { "なし" }
    );
    println!(
        "  初回 GPU 全体転送: 中央値 {:.2} ms（最小 {:.2} ms、99% {:.2} ms）",
        num("/initUpload/median"),
        num("/initUpload/min"),
        num("/initUpload/p99")
    );
    if has_ts {
        println!(
            "  視点移動・拡大縮小で 1 フレーム描く: GPU 完了待ち 中央値 {:.3} ms（99% {:.3} ms）、レンダーパス実行 中央値 {:.3} ms（最小 {:.3} ms）",
            num("/panDraw/wall/median"),
            num("/panDraw/wall/p99"),
            num("/panDraw/gpu/median"),
            num("/panDraw/gpu/min"),
        );
    } else {
        println!(
            "  視点移動・拡大縮小で 1 フレーム描く: GPU 完了待ち 中央値 {:.3} ms（最小 {:.3} ms、99% {:.3} ms）",
            num("/panDraw/wall/median"),
            num("/panDraw/wall/min"),
            num("/panDraw/wall/p99"),
        );
    }
    println!(
        "  （比較）変更ごとに全体を再転送して 1 フレーム描く: 中央値 {:.2} ms（99% {:.2} ms）",
        num("/fullReuploadDraw/median"),
        num("/fullReuploadDraw/p99"),
    );

    for (idx, st) in cpu_stats.iter().enumerate() {
        let base = format!("/workloads/{idx}");
        let wall_med = num(&format!("{base}/wall/median"));
        let wall_p99 = num(&format!("{base}/wall/p99"));
        let gpu_med = num(&format!("{base}/gpu/median"));
        let pass_str = if has_ts && gpu_med.is_finite() {
            format!("、レンダーパス {:.3} ms", gpu_med)
        } else {
            String::new()
        };
        println!(
            "  {}: CPU エンジン {:.1} µs（99% {:.1} µs）＋ 差分転送 {:.0} B（{:.0} 回）＋ 描画完了 中央値 {:.3} ms（99% {:.3} ms{}）",
            st.name,
            st.cpu_median_us,
            st.cpu_p99_us,
            st.dirty_median_bytes,
            st.dirty_calls_median,
            wall_med,
            wall_p99,
            pass_str
        );
    }
    Ok(())
}

/// 指定した図面列で GPU の転送と描画を測る
pub fn run(paths: &[String], options: &Options) -> Result<(), String> {
    let browser = find_browser().ok_or_else(|| "Chrome または Edge が見つからない".to_owned())?;
    for path in paths {
        run_one(path, options, &browser)?;
    }
    Ok(())
}
