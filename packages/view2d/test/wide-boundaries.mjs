import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";

const ID = "space/0/h828side";
const bits = new DataView(new ArrayBuffer(8));
// 元の binary64 を共通単位 2^-1074 の整数にし、投影は 3 積の共通単位で比べる。
function integer(value) {
  bits.setFloat64(0, value); const hi = bits.getUint32(0), lo = bits.getUint32(4), exponent = hi >>> 20 & 2047;
  let n = BigInt(hi & 0xfffff) << 32n | BigInt(lo);
  if (exponent) n = (n | 1n << 52n) << BigInt(exponent - 1);
  return hi >>> 31 ? -n : n;
}
const query = (x, y) => ({ x: integer(x) << 2148n, y: integer(y) << 2148n });
const cross = (x, y, dx, dy) => x * dy - y * dx;
function reference(c, rotation) {
  const cosine = integer(Math.cos(rotation)), sine = integer(Math.sin(rotation)), scale = integer(1), middle = integer(100) << 2148n;
  const at = (x, y) => ({ x: middle + scale * (cosine * integer(x) - sine * integer(y)), y: middle - scale * (sine * integer(x) + cosine * integer(y)) });
  const a = at(c.line_xy[0], c.line_xy[1]), b = at(c.line_xy[2], c.line_xy[3]), dx = b.x - a.x, dy = b.y - a.y;
  const squared = dx * dx + dy * dy, half = integer(c.unit_weights[0] / 2) << 2148n;
  return { point(x, y, tolerance) {
    const p = query(x, y), n = cross(p.x - a.x, p.y - a.y, dx, dy), radius = half + (integer(tolerance) << 2148n);
    return n * n <= radius * radius * squared;
  }, rect(rect, adjustment = 0) {
    const values = [];
    for (const x of [rect.min_x, rect.max_x]) for (const y of [rect.min_y, rect.max_y]) {
      const p = query(x, y), dot = (p.x - a.x) * dx + (p.y - a.y) * dy;
      if (dot < 0n || dot > squared) throw new Error("独立参照の矩形は両 cap の間に置く");
      values.push(cross(p.x - a.x, p.y - a.y, dx, dy));
    }
    const min = values.reduce((a, b) => a < b ? a : b), max = values.reduce((a, b) => a > b ? a : b);
    if (min <= 0n && max >= 0n) return true;
    const nearest = min > 0n ? min : max;
    const radius = half + (integer(adjustment) << 2148n);
    return nearest * nearest <= radius * radius * squared;
  } };
}

/** 巨大な線幅の側面を、半幅で広がる数値 guard を持たない独立した二乗式と比較する。 */
export function testWideBoundaries(assert) {
  const totals = { scenes: 0, points: 0, rectangles: 0, boundaryExcluded: 0 };
  for (const half of [Math.fround(2e9), Math.fround(2e15), Math.fround(1e38)]) for (const rotation of [0, Math.PI / 6, Math.PI / 2]) for (const pixelRatio of [1, 1.5, 8]) {
    const strings = [...new TextEncoder().encode(ID)], input = bundle({
      chunk_origins: [0, 0], chunk_bounds: [-2 * half, half, 2 * half, half], chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
      line_vertices: [0], line_units: [0], line_xy: [-2 * half, half, 2 * half, half],
      unit_entities: [0], unit_colors: [0], unit_weights: [2 * half], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
      entity_ids: [0, strings.length], entity_kinds: [V.entity_kind.line], entity_paths: [0], strings,
      color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, 128], color_flags: [0],
    });
    const viewer = new View2D(document.createElement("canvas"), { width: 200, height: 200, pixelRatio, pixelsPerMillimeter: 1,
      view: { center: { x: 0, y: 0 }, scale: 1, rotation } });
    try {
      viewer.setScene(input); const expected = reference(input.scene.columns, rotation), element = viewer.getElements()[0];
      for (const dx of [-1, -0.25, -1e-6, 0, 1e-6, 0.25, 1]) for (const dy of [-1, -0.25, -1e-6, 0, 1e-6, 0.25, 1]) {
        const x = 100 + dx, y = 100 + dy;
        for (const tolerance of [0, 0.25, 2]) {
          // 回転した境界の最終 CSS 座標の丸め帯だけを除く。半幅によって広げない。
          if (rotation !== 0 && expected.point(x, y, tolerance - 1e-10) !== expected.point(x, y, tolerance + 1e-10)) { totals.boundaryExcluded++; continue; }
          const inside = expected.point(x, y, tolerance), hits = viewer.hitTest({ x, y }, tolerance);
          assert(inside ? hits.length === 1 && hits[0] === element : hits.length === 0,
            `巨大な側面の点: half=${half}/rotation=${rotation}/ratio=${pixelRatio}/p=${x},${y}/t=${tolerance}/expected=${inside}/actual=${hits.length}`);
          totals.points++;
        }
        const rect = { min_x: x, min_y: y, max_x: x + 0.25, max_y: y + 0.5 };
        if (rotation !== 0 && expected.rect(rect, -1e-10) !== expected.rect(rect, 1e-10)) { totals.boundaryExcluded++; continue; }
        const inside = expected.rect(rect), hits = viewer.hitTestRect(rect);
        assert(inside ? hits.length === 1 && hits[0] === element : hits.length === 0, "巨大な側面の矩形を整数二乗式で比較する"); totals.rectangles++;
      }
      totals.scenes++;
    } finally { viewer.destroy(); }
  }
  return totals;
}
