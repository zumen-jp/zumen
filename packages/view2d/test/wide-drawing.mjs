import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";

const ID = "space/0/h828draw", SIZE = 200;
const bits = new DataView(new ArrayBuffer(8)), UNIT = 1n << 400n;
// この合成の値はすべて 2^-200 の倍数。製品の幾何・平方根を参照せず整数化する。
function integer(value) {
  if (value === 0) return 0n;
  bits.setFloat64(0, value); const hi = bits.getUint32(0), lo = bits.getUint32(4), exponent = (hi >>> 20 & 2047) - 1023 - 52 + 200;
  if (exponent < 0) throw new Error("独立参照の整数単位の範囲外");
  // 仮数は 52 bit。hi 側の hidden bit は bit 20 に置く。
  const significand = BigInt(hi & 0xfffff | 0x100000) << 32n | BigInt(lo);
  return (hi >>> 31 ? -significand : significand) << BigInt(exponent);
}
const sub = (a, b) => ({ x: a.x - b.x, y: a.y - b.y });
const dot = (a, b) => a.x * b.x + a.y * b.y;
const cross = (a, b) => a.x * b.y - a.y * b.x;
const query = (x, y) => ({ x: integer(x) << 200n, y: integer(y) << 200n });

function source(item, half, alpha, clipped) {
  const strings = [...new TextEncoder().encode(ID)], xy = item.xy(half);
  const xs = xy.filter((_, i) => i % 2 === 0), ys = xy.filter((_, i) => i % 2 === 1);
  const data = { chunk_origins: item.origin?.(half) ?? [0, 0], chunk_bounds: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)],
    chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [2 * half], unit_clips: [clipped ? 0 : SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, strings.length], entity_kinds: [item.point ? V.entity_kind.point : V.entity_kind.line], entity_paths: [0], strings,
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, alpha], color_flags: [0] };
  if (item.point) Object.assign(data, { point_xy: xy, point_units: [0], point_rotations: [0], point_scales: [1], point_markers: [0, 0, 0, 0],
    point_style: [item.mark ?? V.point_mark.none, V.point_frame.circle, V.point_size.length, 2 * item.radius(half)] });
  else Object.assign(data, { line_xy: xy, line_vertices: [0], line_units: [0] });
  if (clipped) Object.assign(data, { clip_polygons: [0], polygon_vertices: [0], polygon_flags: [0], clip_xy: [-60, -60, 60, -60, 60, 60, -60, 60] });
  return bundle(data);
}

function reference(c, rotation, clipped) {
  const cos = integer(Math.cos(rotation)), sin = integer(Math.sin(rotation)), middle = 100n * UNIT;
  const at = (x, y, ox = 0, oy = 0) => ({ x: middle + cos * (integer(x) + integer(ox)) - sin * (integer(y) + integer(oy)),
    y: middle - sin * (integer(x) + integer(ox)) - cos * (integer(y) + integer(oy)) });
  const local = (x, y) => at(x, y, c.chunk_origins[0], c.chunk_origins[1]);
  const half = integer(c.unit_weights[0] / 2) << 200n, squared = half * half;
  const points = [], segments = [], joins = [];
  for (let i = 0; i < c.line_xy.length; i += 2) points.push(local(c.line_xy[i], c.line_xy[i + 1]));
  const segment = (a, b) => { const d = sub(b, a); segments.push({ a, d, length: dot(d, d) }); };
  for (let i = 1; i < points.length; i++) segment(points[i - 1], points[i]);
  for (let i = 1; i + 1 < points.length; i++) joins.push({ p: points[i], incoming: sub(points[i], points[i - 1]), outgoing: sub(points[i + 1], points[i]) });
  let circle;
  if (c.point_units.length) {
    const center = local(c.point_xy[0], c.point_xy[1]), radius = integer(c.point_style[3] / 2), norm = cos * cos + sin * sin;
    circle = { center, radiusSquared: radius * radius * norm };
    // マークの軸は -rotation の行列。長さ指定は円と同じ中心から作る。
    if (c.point_style[0] === V.point_mark.plus) {
      const x = { x: cos * radius, y: -sin * radius }, y = { x: sin * radius, y: cos * radius };
      for (const d of [x, y]) segment({ x: center.x - d.x, y: center.y - d.y }, { x: center.x + d.x, y: center.y + d.y });
    }
  }
  const clip = clipped ? [at(-60, -60), at(60, -60), at(60, 60), at(-60, 60)] : [];
  return (x, y) => {
    const p = query(x, y);
    if (clip.length && clip.some((a, i) => cross(sub(clip[(i + 1) % 4], a), sub(p, a)) > 0n)) return false;
    if (segments.some(({ a, d, length }) => { const q = sub(p, a), along = dot(q, d), across = cross(q, d); return along >= 0n && along <= length && across * across <= squared * length; })) return true;
    if (joins.some(({ p: center, incoming, outgoing }) => { const q = sub(p, center); return dot(q, incoming) >= 0n && dot(q, outgoing) <= 0n && dot(q, q) <= squared; })) return true;
    if (!circle) return false;
    // |sqrt(d²)-sqrt(r²)|<=h を二回の二乗で比べる。r の丸めも平方根も使わない。
    const q = sub(p, circle.center), distance = dot(q, q), r = circle.radiusSquared, difference = distance + r - squared;
    return difference <= 0n || difference * difference <= 4n * distance * r;
  };
}

/** 遠い原点の法線方向の距離だけを除く。近傍の cap は同じで、幅 2,000 でも全包する。 */
function localReference(c, rotation, ratio, clipped, width, height) {
  const canvas = document.createElement("canvas"); canvas.width = width; canvas.height = height;
  const ctx = canvas.getContext("2d"), cos = Math.cos(rotation), sin = Math.sin(rotation);
  const at = (x, y) => ({ x: 100 + cos * x - sin * y, y: 100 - sin * x - cos * y });
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0); ctx.fillStyle = "white"; ctx.fillRect(0, 0, SIZE, SIZE);
  if (clipped) {
    const clip = new Path2D();
    for (let i = 0; i < c.clip_xy.length; i += 2) { const p = at(c.clip_xy[i], c.clip_xy[i + 1]); clip[i ? "lineTo" : "moveTo"](p.x, p.y); }
    clip.closePath(); ctx.clip(clip);
  }
  const path = new Path2D();
  // 短線の検査だけに使う。折れ線では各辺の cap が別の方向を向くので、この移動をしない。
  for (let i = 0; i < c.line_xy.length; i += 2) { const p = at(c.line_xy[i], c.line_xy[i + 1]); path[i ? "lineTo" : "moveTo"](p.x, p.y); }
  ctx.lineCap = "butt"; ctx.lineJoin = "round"; ctx.lineWidth = 2000;
  ctx.strokeStyle = `rgba(0,0,0,${c.color_rgba[3] / 255})`; ctx.stroke(path);
  return ctx.getImageData(0, 0, width, height).data;
}

export function testWideDrawing(assert) {
  const items = [
    { name: "側面", xy: (h) => [-2 * h, h, 2 * h, h] },
    { name: "側面と二つの端", xy: (h) => [-30, h, 40, h] },
    { name: "折り返す join の円弧", xy: (h) => [-h, h, 0, h, -h, h] },
    { name: "曲がる join の円弧", xy: (h) => [-h, h, 0, h, 0, 2 * h] },
    { name: "円環の外側", point: true, radius: (h) => h, xy: (h) => [0, 2 * h] },
    { name: "円環の穴", point: true, radius: (h) => 2 * h, xy: (h) => [0, h] },
    { name: "円と十字の union", point: true, mark: V.point_mark.plus, radius: (h) => 2 * h, xy: (h) => [0, h] },
    { name: "遠い原点の短線", origin: (h) => [0, h / 2], xy: () => [-2, 0, 2, 0] },
    { name: "遠い原点の短い折れ線", origin: (h) => [0, h / 2], xy: () => [-2, 0, 0, 0, 0, 2] },
  ];
  const totals = { scenes: 0, pixels: 0, boundaryExcluded: 0, points: 0, shortLinePixels: 0 };
  for (const item of items) for (const half of [40000, 2 ** 30, 2 ** 50, 2 ** 125]) for (const rotation of [0, Math.PI / 6, Math.PI / 2]) for (const pixelRatio of [1, 1.5, 8]) {
    const alpha = rotation === 0 ? 128 : 255, clipped = pixelRatio === 1.5;
    const canvas = document.createElement("canvas"), viewer = new View2D(canvas, { width: SIZE, height: SIZE, pixelRatio, pixelsPerMillimeter: 1,
      view: { center: { x: 0, y: 0 }, scale: 1, rotation } });
    try {
      const input = source(item, half, alpha, clipped), report = viewer.setScene(input), expected = reference(input.scene.columns, rotation, clipped);
      const actual = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data, label = `${item.name}/half=${half}/rotation=${rotation}/ratio=${pixelRatio}`;
      assert(report.issues.length === 0 && viewer.getElements()[0].id === ID, `${label}: 診断・ID を保つ`);
      let mismatch = 0, first;
      const step = Math.max(1, Math.round(4 * pixelRatio));
      for (let y = 2; y < canvas.height - 2; y += step) for (let x = 2; x < canvas.width - 2; x += step) {
        const px = (x + 0.5) / pixelRatio, py = (y + 0.5) / pixelRatio, inside = expected(px, py);
        const delta = 1.5 / pixelRatio;
        if ([-1, 1].some((dx) => [-1, 1].some((dy) => expected(px + dx * delta, py + dy * delta) !== inside))) { totals.boundaryExcluded++; continue; }
        const i = (y * canvas.width + x) * 4;
        if (Math.abs(actual[i] - (inside ? 255 - alpha : 255)) > 4 || actual[i + 3] !== 255) { mismatch++; first ??= `${px},${py}: expected=${inside}/actual=${actual[i]}`; }
        totals.pixels++;
      }
      assert(mismatch === 0, `${label}: 独立した整数式の安定画素 mismatch=${mismatch}/${first}`);
      if (item.origin && input.scene.columns.line_xy.length === 4) {
        const local = localReference(input.scene.columns, rotation, pixelRatio, clipped, canvas.width, canvas.height);
        let difference = 0, peak = 0, first;
        for (let y = 1; y < canvas.height - 1; y++) for (let x = 1; x < canvas.width - 1; x++) {
          const i = (y * canvas.width + x) * 4;
          const error = Math.abs(actual[i] - local[i]);
          let stable = true;
          for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if (local[((y + dy) * canvas.width + x + dx) * 4] !== local[i]) stable = false;
          // 近傍 fill と長い Canvas stroke の AA は分ける。安定内部は 4、境界は 16/255 以下。
          if (error > (stable ? 4 : 16)) { difference++; first ??= `${x},${y}: ${actual[i]}/${local[i]}`; }
          peak = Math.max(peak, error);
          totals.shortLinePixels++;
        }
        assert(difference === 0, `${label}: 遠い短線と近傍の独立 Canvas の AA: ${difference}/peak=${peak}/${first}`);
      }
      for (const [x, y] of [[80, 80], [100, 99], [100, 101], [120, 120]]) {
        if (expected(x - 0.01, y - 0.01) !== expected(x + 0.01, y + 0.01)) continue;
        assert((viewer.hitTest({ x, y }, 0).length > 0) === expected(x, y), `${label}: 表示と点選択を独立式で比べる`); totals.points++;
      }
      totals.scenes++;
    } finally { viewer.destroy(); }
  }
  totals.contracts = testWideDrawingBudget(assert);
  return totals;
}

export function testWideDrawingBudget(assert) {
  const circle = { point: true, radius: (h) => h, xy: (h) => [0, 2 * h] };
  const input = source(circle, 40000, 128, false);
  const canvas = document.createElement("canvas"), viewer = new View2D(canvas, { width: SIZE, height: SIZE, pixelsPerMillimeter: 1,
    limits: { work: 150 }, view: { center: { x: 0, y: -1e6 }, scale: 1, rotation: 0 } });
  try {
    viewer.setScene(input);
    const before = viewer.getState(), pixels = canvas.toDataURL();
    let stopped = false;
    try { viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 1, rotation: 0 }); }
    catch (error) { stopped = error instanceof RangeError && error.message.includes("limits.work"); }
    assert(stopped && viewer.getState() === before && canvas.toDataURL() === pixels, "太い円の領域分割が予算を超えたカメラ変更は状態と画面を保つ");
    viewer.setScene(null); viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 1, rotation: 0 });
    const empty = viewer.getState(), blank = canvas.toDataURL(); stopped = false;
    try { viewer.setScene(input); } catch (error) { stopped = error instanceof RangeError && error.message.includes("limits.work"); }
    assert(stopped && viewer.getState() === empty && canvas.toDataURL() === blank, "領域の分割・結合が予算を超えた setScene は以前の画面を保つ");
  } finally { viewer.destroy(); }
  const resizing = new View2D(canvas, { width: SIZE, height: SIZE, pixelsPerMillimeter: 1, limits: { work: 5000 } });
  try {
    resizing.setScene(input);
    const before = resizing.getState(), pixels = canvas.toDataURL(); let stopped = false;
    try { resizing.resize(8192, SIZE); } catch (error) { stopped = error instanceof RangeError && error.message.includes("limits.work"); }
    assert(stopped && resizing.getState() === before && canvas.toDataURL() === pixels, "拡張した画面の円弧・結合が予算を超えた resize は状態と画面を保つ");
  } finally { resizing.destroy(); }
  return 3;
}
