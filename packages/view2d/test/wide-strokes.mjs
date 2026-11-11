import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";

const SIZE = 200, ID = "space/0/h828";

function source(xy, width, separate = false) {
  const strings = [...new TextEncoder().encode(ID)];
  const xs = xy.filter((_, i) => i % 2 === 0), ys = xy.filter((_, i) => i % 2 === 1);
  return { chunk_origins: [0, 0], chunk_bounds: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)],
    chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [width], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, strings.length], entity_kinds: [V.entity_kind.line], entity_paths: [0],
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, 128], color_flags: [0], strings,
    line_vertices: separate ? [0, 2] : [0], line_units: separate ? [0, 0] : [0], line_xy: xy };
}

const screen = (x, y, angle) => ({ x: 100 + Math.cos(angle) * x - Math.sin(angle) * y,
  y: 100 - Math.sin(angle) * x - Math.cos(angle) * y });

/** 巨大な幅を渡さず、近傍の端と十分な有限幅から独立した Canvas を作る。 */
function reference(canvas, input, angle, ratio, item) {
  const c = input.scene.columns, ctx = canvas.getContext("2d"), paths = [];
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0); ctx.fillStyle = "white"; ctx.fillRect(0, 0, SIZE, SIZE);
  const clip = new Path2D();
  if (item.clip) {
    for (let i = 0; i < c.clip_xy.length; i += 2) {
      const p = screen(c.clip_xy[i], c.clip_xy[i + 1], angle); clip[i ? "lineTo" : "moveTo"](p.x, p.y);
    }
    clip.closePath(); ctx.clip(clip);
  }
  // 1,000 CSS px の半幅でも、ここで使う端・join から viewport 全体を覆う。
  // 通常の幅と fallback の例は元と同じ幅で比較する。
  ctx.lineWidth = Math.min(2000, Math.max(1 / ratio, c.unit_weights[0]));
  ctx.lineCap = "butt"; ctx.lineJoin = "round"; ctx.strokeStyle = "rgba(0,0,0,0.5019607843137255)";
  if (item.point) {
    const path = new Path2D();
    const contours = item.point === "plus" ? [[-2, 0, 2, 0], [0, -2, 0, 2]] : [[-2, -2, 2, -2, 2, 2, -2, 2]];
    for (const xy of contours) {
      for (let i = 0; i < xy.length; i += 2) { const p = screen(xy[i], xy[i + 1], angle); path[i ? "lineTo" : "moveTo"](p.x, p.y); }
      if (item.point === "square") path.closePath();
    }
    paths.push(path); ctx.stroke(path);
  }
  for (let line = 0; line < (c.line_vertices?.length ?? 0); line++) {
    const path = new Path2D(), start = c.line_vertices[line], finish = c.line_vertices[line + 1] ?? c.line_xy.length / 2;
    for (let i = start; i < finish; i++) {
      const p = screen(c.line_xy[i * 2], c.line_xy[i * 2 + 1], angle);
      path[i === start ? "moveTo" : "lineTo"](p.x, p.y);
    }
    paths.push(path); ctx.stroke(path);
  }
  return { pixels: ctx.getImageData(0, 0, canvas.width, canvas.height).data,
    contains: (p) => (!item.clip || ctx.isPointInPath(clip, p.x * ratio, p.y * ratio)) &&
      paths.some((path) => ctx.isPointInStroke(path, p.x * ratio, p.y * ratio)) };
}

/** cap・join・同じ part の union を、幅を制限した独立 Canvas と照合する。 */
export function testWideStrokes(assert) {
  const huge = [2e9, 2e15, 2e38];
  const items = [
    { name: "画面を覆う水平線", xy: [-2000, 0, 2000, 0] },
    { name: "短い水平線", xy: [-2, 0, 2, 0], widths: [0, 2, 8, ...huge] },
    { name: "短い垂直線", xy: [0, -2, 0, 2] },
    { name: "斜めの cap", xy: [-2, 2, 2, -2], widths: [0, 2, 8, ...huge] },
    { name: "0.24 の短線", xy: [-0.12, 0, 0.12, 0] },
    { name: "逆向きの斜線", xy: [2, -2, -2, 2] },
    { name: "同じ方向の共線", xy: [-2, 0, 0, 0, 2, 0] },
    { name: "折返しの round join", xy: [-2, 2, 2, -2, -2, 2] },
    { name: "曲がる round join", xy: [-20, 0, 0, 0, 0, 20] },
    { name: "重複した頂点", xy: [-2, 0, 0, 0, 0, 0, 2, 0] },
    { name: "別 primitive の重なり", xy: [-2, 2, 2, -2, -2, 2, 2, -2], separate: true },
    { name: "完全に画面外の cap", xy: [-400, 0, -300, 0] },
    { name: "ゼロ長", xy: [0, 0, 0, 0] },
    { name: "内側の clip", xy: [-2000, 0, 2000, 0], clip: true },
    { name: "同じ part の十字の点マーク", xy: [-2, -2, 2, 2], point: "plus" },
    // 巨大な枠は元頂点の円盤の全包から単一矩形にし、通常の枠は従来経路を保つ。
    { name: "閉じた四角の点マーク", xy: [-2, -2, 2, 2], point: "square", widths: [0, 2, 8, 300, ...huge] },
    // 一部の幅が近傍を覆わなければ、part 全体を従来の一度の stroke に戻す。
    { name: "part 全体の fallback", xy: [-20, 0, 200, 0, 200, 100], widths: [300] },
  ];
  const totals = { scenes: 0, pixels: 0, points: 0, rectangles: 0, boundaryExcluded: 0,
    pixelRatios: [1, 1.5, 2, 8], rotations: [0, Math.PI / 6, Math.PI / 2], widths: [0, 2, 8, ...huge, 300] };
  for (const item of items) for (const width of item.widths ?? huge) for (const pixelRatio of totals.pixelRatios) for (const rotation of totals.rotations) {
    const canvas = document.createElement("canvas"), near = document.createElement("canvas");
    const viewer = new View2D(canvas, { width: SIZE, height: SIZE, pixelRatio, pixelsPerMillimeter: 1,
      view: { center: { x: 0, y: 0 }, scale: 1, rotation } });
    try {
      const data = source(item.xy, width, item.separate);
      if (item.point) {
        delete data.line_vertices; delete data.line_units; delete data.line_xy;
        Object.assign(data, { entity_kinds: [V.entity_kind.point], point_xy: [0, 0], point_units: [0], point_rotations: [0],
          point_markers: [0, 0, 0, 0], point_scales: [1], point_style: [item.point === "plus" ? V.point_mark.plus : V.point_mark.none,
            item.point === "square" ? V.point_frame.square : 0, V.point_size.length, 4] });
      }
      if (item.clip) Object.assign(data, { unit_clips: [0], clip_polygons: [0], polygon_vertices: [0], polygon_flags: [0],
        clip_xy: [-30, -30, 30, -30, 30, 30, -30, 30] });
      const input = bundle(data), report = viewer.setScene(input), element = viewer.getElements()[0];
      const context = `${item.name}/width=${width}/ratio=${pixelRatio}/rotation=${rotation}`;
      assert((item.point ? report.drawn.points === 1 : report.drawn.lines === (item.separate ? 2 : 1)) && report.issues.length === 0 && element.id === ID,
        `${context}: 原本の描画件数・診断・ID を保つ`);
      near.width = canvas.width; near.height = canvas.height;
      const expected = reference(near, input, rotation, pixelRatio, item);
      const actual = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
      let samples = 0, mismatch = 0;
      const step = Math.max(1, Math.floor(pixelRatio));
      for (let y = 1; y < canvas.height - 1; y += step) for (let x = 1; x < canvas.width - 1; x += step) {
        const i = (y * canvas.width + x) * 4, value = expected.pixels[i];
        if (![255, 127, 63].includes(value)) continue;
        let stable = true;
        for (let dy = -1; dy <= 1 && stable; dy++) for (let dx = -1; dx <= 1; dx++) {
          if (Math.abs(expected.pixels[((y + dy) * canvas.width + x + dx) * 4] - value) > 1) { stable = false; break; }
        }
        if (!stable) continue;
        samples++;
        if ([0, 1, 2].some((offset) => Math.abs(actual[i + offset] - value) > 4) || actual[i + 3] !== 255) mismatch++;
      }
      assert(samples > 1000 && mismatch === 0, `${context}: cap・join・alpha の独立 Canvas: samples=${samples}/mismatch=${mismatch}`);
      for (const [x, y] of [[0, 0], [-10, -10], [10, 10], [-5, 5], [40, 0]]) {
        const p = screen(x, y, rotation), inside = expected.contains(p);
        if ([[0.001, 0], [-0.001, 0], [0, 0.001], [0, -0.001]].some(([dx, dy]) => expected.contains({ x: p.x + dx, y: p.y + dy }) !== inside)) {
          totals.boundaryExcluded++; continue;
        }
        const queries = [viewer.hitTest(p, 0), viewer.hitTestRect({ min_x: p.x, min_y: p.y, max_x: p.x, max_y: p.y })];
        for (const hits of queries) assert(inside ? hits.length === 1 && hits[0] === element : hits.length === 0,
          `${context}: 点と矩形が同じ元 ID と cap/join の内外を返す: x=${x}/y=${y}/expected=${inside}/hits=${hits.length}`);
        totals.points++; totals.rectangles++;
      }
      totals.scenes++; totals.pixels += samples;
    } finally { viewer.destroy(); }
  }
  return totals;
}
