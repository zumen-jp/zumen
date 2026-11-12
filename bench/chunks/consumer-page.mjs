// 本番の reader・表示部品を呼ぶ。測定用の metadata と Worker 往復はここだけのもの。
import { readScene, SCENE_NONE } from "/bindings/js/src/index.js";
import { View2D } from "/packages/view2d/src/index.js";
import { bundle } from "/packages/view2d/test/fixtures.mjs";

const width = 960, height = 640;
const check = (ok, message) => { if (!ok) throw new Error(message); };
const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const digest = async bytes => [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map(v => v.toString(16).padStart(2, "0")).join("");
const keys = refs => refs.map(ref => JSON.stringify([ref.id, ref.path.map(p => [p.id, p.column, p.row])])).sort();
let state;

function timed(repeats, action) {
  action();
  const times = [];
  for (let i = 0; i < repeats; i++) {
    const start = performance.now(); action(); times.push(performance.now() - start);
  }
  return median(times);
}

export async function setup(row, repeats) {
  const bytes = await (await fetch(`/data/${row.case}-${row.policy}.bin`)).arrayBuffer();
  check(await digest(bytes) === row.scene_sha256, "scene SHA-256");
  let scene;
  const reader_ms = timed(repeats, () => { scene = readScene(bytes); });
  const c = scene.columns, m = row.metrics;
  const counts = Object.fromEntries(["chunks", "lines", "faces", "texts", "points", "images", "vertices"].map(k => [k, m[k]]));
  counts.glyphs = c.glyph_shapes.length;
  const column_bytes = Object.values(c).reduce((n, a) => n + a.byteLength, 0);
  check(Object.values(c).every(a => a.length === 0 || a.buffer === bytes), "reader must share the aligned input buffer");
  const extent = { min_x: Infinity, min_y: Infinity, max_x: -Infinity, max_y: -Infinity };
  for (let i = 0; i < scene.chunks; i++) {
    const x = c.chunk_origins[2 * i], y = c.chunk_origins[2 * i + 1];
    extent.min_x = Math.min(extent.min_x, x + c.chunk_bounds[4 * i]);
    extent.min_y = Math.min(extent.min_y, y + c.chunk_bounds[4 * i + 1]);
    extent.max_x = Math.max(extent.max_x, x + c.chunk_bounds[4 * i + 2]);
    extent.max_y = Math.max(extent.max_y, y + c.chunk_bounds[4 * i + 3]);
  }
  // Rust の監査済みバイトから作る表示用のラッパー。API の応答・文書ハッシュを測ったとはしない。
  const input = { scene, document: { doc: "bench", format: row.case.endsWith("dwg") ? "dwg" : "dxf", hash: `scene:${row.scene_sha256}` },
    result: { from: "space/0", unit: "unknown", tolerance: m.tolerance, blob: 0, counts, extent } };
  const canvas = document.createElement("canvas"); document.body.append(canvas);
  const limits = { issues: Math.max(10_000, m.lines + m.faces + m.texts + m.points + m.images) };
  let default_limit_rejection = null;
  if (row.case === "synthetic-100k") {
    const probe = new View2D(canvas, { width, height, pixelRatio: 1 });
    try { probe.setScene(input); }
    catch (error) { check(error.message === "不正な値: limits.issues", "unexpected default rejection"); default_limit_rejection = "limits.issues"; }
    finally { probe.destroy(); }
  }
  const viewer = new View2D(canvas, { width, height, pixelRatio: 1, limits });
  // 同じカメラを保ち、setScene の検査・索引・報告・描画を一緒に測る。
  viewer.fitBounds(extent, 32);
  const set_scene_ms = timed(repeats, () => viewer.setScene(input));
  state = { row, bytes, scene, viewer, canvas, repeats };
  return { reader_ms, set_scene_ms, scene_bytes: bytes.byteLength, column_bytes,
    header_and_padding_bytes: bytes.byteLength - column_bytes,
    calculated_order_index_bytes: 4 * (m.lines + m.faces + m.texts + m.points + m.images),
    readback_rgba_bytes: width * height * 4, reader_copied_column_bytes: 0,
    limits, default_limit_rejection, view: viewer.getView(), extent };
}

// 元の f64 の数式と、butt の幅 1 px の線分の四角で独立に判定する。
function lineIntersects(a, b, rect, radius = .5) {
  const corners = [{ x: rect.min_x, y: rect.min_y }, { x: rect.max_x, y: rect.min_y }, { x: rect.max_x, y: rect.max_y }, { x: rect.min_x, y: rect.max_y }];
  const dx = b.x - a.x, dy = b.y - a.y, length = Math.hypot(dx, dy), nx = -dy / length * radius, ny = dx / length * radius;
  const polygon = [{ x: a.x + nx, y: a.y + ny }, { x: b.x + nx, y: b.y + ny }, { x: b.x - nx, y: b.y - ny }, { x: a.x - nx, y: a.y - ny }];
  return [[1, 0], [0, 1], [dx, dy], [-dy, dx]].every(([ax, ay]) => {
    const p = polygon.map(v => v.x * ax + v.y * ay), q = corners.map(v => v.x * ax + v.y * ay);
    return Math.max(...p) >= Math.min(...q) && Math.max(...q) >= Math.min(...p);
  });
}

function syntheticExpected(name, view, rect) {
  const n = name === "synthetic-10k" ? 10_000 : 100_000, scale = n === 10_000 ? .001 : 1;
  const project = (x, y) => ({ x: width / 2 + (x - view.center.x) * view.scale, y: height / 2 - (y - view.center.y) * view.scale });
  const expected = [];
  for (let i = 0; i < n; i++) {
    const x = 100_000 + (i % 1000) * 1.001234567 * scale, y = 50_000 + Math.floor(i / 1000) * 9.87654321 * scale;
    const a = project(x, y), b = project(x + .123456789 * scale, y + .234567891 * scale);
    if (lineIntersects(a, b, rect)) expected.push(JSON.stringify([`space/0/h${(i + 1).toString(16).toUpperCase()}`, []]));
  }
  return expected.sort();
}

// 量子化の境界差を分けるため、公開された f32 列にも同じ独立の幾何判定を当てる。
function encodedExpected(rect) {
  const { scene: { columns: c, chunks }, viewer } = state, view = viewer.getView(), decoder = new TextDecoder(), expected = [];
  for (let chunk = 0; chunk < chunks; chunk++) {
    const originX = c.chunk_origins[2 * chunk], originY = c.chunk_origins[2 * chunk + 1];
    const project = i => ({ x: width / 2 + ((originX - view.center.x) + c.line_xy[2 * i]) * view.scale,
      y: height / 2 - ((originY - view.center.y) + c.line_xy[2 * i + 1]) * view.scale });
    const finish = c.chunk_lines[chunk + 1] ?? c.line_units.length;
    for (let line = c.chunk_lines[chunk]; line < finish; line++) {
      const first = c.line_vertices[line], last = c.line_vertices[line + 1] ?? c.line_xy.length / 2;
      check(last - first === 2, "synthetic encoded segment");
      const unit = c.line_units[line], entity = c.unit_entities[unit];
      check(c.entity_paths[entity] === (c.entity_paths[entity + 1] ?? c.step_ids.length / 2), "synthetic empty placement path");
      if (lineIntersects(project(first), project(first + 1), rect, Math.max(1, c.unit_weights[unit] * 96 / 25.4) / 2)) {
        const id = decoder.decode(c.strings.subarray(c.entity_ids[2 * entity], c.entity_ids[2 * entity] + c.entity_ids[2 * entity + 1]));
        expected.push(JSON.stringify([id, []]));
      }
    }
  }
  return expected.sort();
}

function isolatedProbe(missing, rect) {
  const { scene: { columns: c }, viewer } = state, id = JSON.parse(missing)[0];
  const decoder = new TextDecoder();
  const entity = Array.from({ length: c.entity_ids.length / 2 }, (_, i) => i).find(i => decoder.decode(c.strings.subarray(c.entity_ids[2 * i], c.entity_ids[2 * i] + c.entity_ids[2 * i + 1])) === id);
  const line = c.line_units.findIndex(unit => c.unit_entities[unit] === entity);
  let chunk = 0;
  while (chunk + 1 < c.chunk_lines.length && c.chunk_lines[chunk + 1] <= line) chunk++;
  const start = c.line_vertices[line], finish = c.line_vertices[line + 1] ?? c.line_xy.length / 2;
  check(finish - start === 2, "isolated synthetic line");
  const xy = [...c.line_xy.slice(start * 2, finish * 2)], origin = [...c.chunk_origins.slice(chunk * 2, chunk * 2 + 2)];
  const strings = new TextEncoder().encode(id);
  const columns = { chunk_origins: origin, chunk_bounds: [Math.min(xy[0], xy[2]), Math.min(xy[1], xy[3]), Math.max(xy[0], xy[2]), Math.max(xy[1], xy[3])],
    chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    line_vertices: [0], line_xy: xy, line_units: [0], unit_entities: [0], unit_colors: [SCENE_NONE], unit_weights: [0], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, strings.length], entity_kinds: [c.entity_kinds[entity]], entity_paths: [0], strings };
  const canvas = document.createElement("canvas"), view = viewer.getView();
  const isolated = new View2D(canvas, { width, height, pixelRatio: 1, view });
  try {
    isolated.setScene(bundle(columns));
    const a = isolated.toScreen({ origin: { x: origin[0], y: origin[1] }, offset: { x: xy[0], y: xy[1] } });
    const b = isolated.toScreen({ origin: { x: origin[0], y: origin[1] }, offset: { x: xy[2], y: xy[3] } });
    const midpoint = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    const path = new Path2D(); path.moveTo(a.x, a.y); path.lineTo(b.x, b.y);
    const ctx = canvas.getContext("2d"); ctx.lineWidth = 1; ctx.lineCap = "butt";
    return { origin, xy, view, rect, a, b, midpoint,
      midpoint_inside_rect: midpoint.x > rect.min_x && midpoint.x < rect.max_x && midpoint.y > rect.min_y && midpoint.y < rect.max_y,
      canvas_stroke_contains_midpoint: ctx.isPointInStroke(path, midpoint.x, midpoint.y),
      isolated_rect_count: isolated.hitTestRect(rect).length, isolated_point_count: isolated.hitTest(midpoint, 0).length };
  } finally { isolated.destroy(); }
}

async function transfer(repeats, source, mode) {
  const worker = new Worker("/bench/chunks/consumer-echo.mjs", { type: "module" });
  try {
    await new Promise((resolve, reject) => { worker.onmessage = resolve; worker.onerror = reject; });
    const calls = [], roundtrips = [];
    for (let i = -1; i < repeats; i++) {
      const owned = source.slice(0); // 所有する送信元を用意するコピーは時間の外。
      const reply = new Promise((resolve, reject) => { worker.onmessage = event => resolve(event.data); worker.onerror = reject; });
      const start = performance.now();
      worker.postMessage({ mode, buffer: owned }, mode === "transfer" ? [owned] : []);
      const posted = performance.now();
      const result = await reply, end = performance.now();
      check(owned.byteLength === (mode === "transfer" ? 0 : source.byteLength), "sender ownership");
      check(result.byteLength === source.byteLength && await digest(result) === await digest(source), "worker bytes");
      if (i >= 0) { calls.push(posted - start); roundtrips.push(end - start); }
    }
    return { post_message_ms: median(calls), roundtrip_ms: median(roundtrips), payload_bytes: source.byteLength };
  } finally { worker.terminate(); }
}

export async function measure() {
  const { row, viewer, canvas, bytes, repeats } = state, ctx = canvas.getContext("2d");
  const render_submit_ms = timed(repeats, () => viewer.render());
  const rect = { min_x: width * .25, min_y: height * .25, max_x: width * .75, max_y: height * .75 };
  let selected;
  const select_rect_ms = timed(repeats, () => { selected = viewer.hitTestRect(rect); });
  const selected_keys = keys(selected);
  const again = keys(viewer.hitTestRect(rect));
  check(JSON.stringify(selected_keys) === JSON.stringify(again), "repeat selection");
  const full = keys(viewer.hitTestRect({ min_x: 0, min_y: 0, max_x: width, max_y: height }));
  let independent_selection = null;
  if (row.case.startsWith("synthetic")) {
    const formulaExpected = syntheticExpected(row.case, viewer.getView(), rect), expected = encodedExpected(rect);
    const formulaSet = new Set(formulaExpected), encodedSet = new Set(expected);
    const expectedSet = new Set(expected), selectedSet = new Set(selected_keys);
    const extra = selected_keys.filter(k => !expectedSet.has(k)), missing = expected.filter(k => !selectedSet.has(k));
    independent_selection = { expected_count: expected.length, actual_only_count: extra.length, missing_count: missing.length,
      exact: extra.length === 0 && missing.length === 0, expected_sha256: await digest(new TextEncoder().encode(JSON.stringify(expected))),
      source_formula_expected_count: formulaExpected.length,
      formula_vs_encoded_difference: formulaExpected.filter(k => !encodedSet.has(k)).length + expected.filter(k => !formulaSet.has(k)).length,
      full_expected_count: row.metrics.entities, full_exact: full.length === row.metrics.entities,
      first_missing_probe: missing.length ? isolatedProbe(missing[0], rect) : null };
  }
  viewer.select(selected);
  check(JSON.stringify(keys(viewer.getState().selection)) === JSON.stringify(selected_keys), "selection state");
  viewer.select([]);
  const frame = [];
  for (let i = -1; i < repeats; i++) {
    await new Promise(requestAnimationFrame);
    const start = performance.now(); viewer.render(); await new Promise(requestAnimationFrame);
    if (i >= 0) frame.push(performance.now() - start);
  }
  let pixels;
  const render_readback_ms = timed(repeats, () => { viewer.render(); pixels = ctx.getImageData(0, 0, width, height).data; });
  const pixel_sha256 = await digest(pixels);
  viewer.render();
  check(await digest(ctx.getImageData(0, 0, width, height).data) === pixel_sha256, "repeat pixels");
  let nonwhite_pixels = 0;
  for (let i = 0; i < pixels.length; i += 4) if (pixels[i] !== 255 || pixels[i + 1] !== 255 || pixels[i + 2] !== 255) nonwhite_pixels++;
  check(nonwhite_pixels > 0, "nonempty render");
  const report = viewer.getReport(), issue_counts = {};
  for (const issue of report.issues) issue_counts[`${issue.reason}:${issue.disposition}`] = (issue_counts[`${issue.reason}:${issue.disposition}`] ?? 0) + 1;
  for (const kind of ["line", "face", "text", "point", "image"]) {
    const omitted = report.issues.filter(i => i.primitive.kind === kind && i.disposition === "omitted").length;
    check(report.drawn[`${kind}s`] + omitted === row.metrics[`${kind}s`], "Rust/browser primitive counts");
  }
  return { render_submit_ms, select_rect_ms, render_to_next_raf_ms: median(frame), render_readback_ms,
    selected_count: selected.length, full_selected_count: full.length, selected_keys, full_keys: full,
    selection_json_utf8_bytes: new TextEncoder().encode(JSON.stringify(selected_keys)).byteLength,
    selection_sha256: await digest(new TextEncoder().encode(JSON.stringify(selected_keys))),
    full_selection_sha256: await digest(new TextEncoder().encode(JSON.stringify(full))),
    pixel_sha256, nonwhite_pixels, drawn: report.drawn, issue_counts, independent_selection,
    transfer: await transfer(repeats, bytes, "transfer"), clone: await transfer(repeats, bytes, "clone") };
}

export function dispose() { state.viewer.destroy(); state.canvas.remove(); state = null; }
