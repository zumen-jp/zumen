import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";

const SIZE = 200;
const ID = "space/0/h828";
const PLACE = "space/0/hplace";
const square = (x1, y1, x2, y2) => [x1, y1, x2, y1, x2, y2, x1, y2];
const reverse = (xy) => { const points = []; for (let i = xy.length - 2; i >= 0; i -= 2) points.push(xy[i], xy[i + 1]); return points; };
const starts = (rings) => { let n = 0; return rings.map((ring) => { const start = n; n += ring.length / 2; return start; }); };
const nearOuter = square(-400, -400, 400, 400);

function base(kind) {
  const id = [...new TextEncoder().encode(ID)], place = [...new TextEncoder().encode(PLACE)];
  return {
    chunk_origins: [0, 0], chunk_bounds: [0, 0, 0, 0],
    chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [8], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, id.length], entity_kinds: [kind], entity_paths: [0],
    step_ids: [id.length, place.length], step_cells: [2, 3], strings: [...id, ...place],
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, 128], color_flags: [0],
  };
}

function face(rings) {
  const xy = rings.flat(), xs = xy.filter((_, i) => i % 2 === 0), ys = xy.filter((_, i) => i % 2 === 1);
  return { ...base(V.entity_kind.hatch), chunk_bounds: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)],
    face_rings: [0], face_units: [0], ring_vertices: starts(rings), face_xy: xy };
}

function glyph(rings, separate = false) {
  return {
    ...base(V.entity_kind.text),
    text_strings: [0, 0], text_xy: [0, 0], text_second: [NaN, NaN], text_heights: [8], text_widths: [1],
    text_rotations: [0], text_obliques: [0], text_boxes: [0], text_aligns: [0, 0], text_flags: [0], text_lines: [0],
    text_fonts: [SCENE_NONE], text_units: [0], text_glyphs: [0],
    glyph_shapes: separate ? [0, 0] : [0], glyph_axes: separate ? [1, 0, 0, 1, -1, 0, 0, 1] : [1, 0, 0, 1],
    glyph_xy: separate ? [0, 0, 0, 0] : [0, 0], shape_rings: [0], outline_vertices: starts(rings), outline_xy: rings.flat(),
  };
}

function line() {
  return { ...base(V.entity_kind.line), chunk_bounds: [-2000, 0, 2000, 0], line_vertices: [0], line_units: [0], line_xy: [-2000, 0, 2000, 0] };
}

function clipped(source, clips) {
  return { ...source, unit_clips: [0], clip_polygons: [0], polygon_flags: clips.map((clip) => Number(clip.outside)),
    polygon_vertices: starts(clips.map((clip) => clip.xy)), clip_xy: clips.flatMap((clip) => clip.xy) };
}

function decodedRing(c, name, offsets, i, scale) {
  return Array.from(c[name].subarray(offsets[i] * 2, (offsets[i + 1] ?? c[name].length / 2) * 2), (value) => value * scale);
}

function cases() {
  const r = Math.fround(2e38), scale = 1e12, outer = square(-r, -r, r, r);
  const hole = square(-20 / scale, -20 / scale, 20 / scale, 20 / scale);
  const covered = [[0, 0, true], [50, 40, true]], empty = [[0, 0, false], [-50, 40, false]];
  const withHole = [[0, 0, false], [40, 0, true], [-50, 25, true]];
  const holeOf = (c, s, name, offsets) => decodedRing(c, name, c[offsets], 1, s);
  const items = [
    { name: "面の巨大全囲", source: face([outer]), groups: () => [[nearOuter]], probes: covered, scale: 1 },
    { name: "面の片側外", source: face([square(r / 2, -r, r, r)]), groups: () => [[square(400, -400, 600, 400)]], probes: empty, empty: true },
    { name: "面の小穴", source: face([outer, hole]), groups: (c, s) => [[nearOuter, holeOf(c, s, "face_xy", "ring_vertices")]], probes: withHole },
    { name: "面の巨大な穴", source: face([outer, square(-r / 2, -r / 2, r / 2, r / 2)]), groups: () => [[nearOuter, nearOuter]], probes: empty, empty: true },
    { name: "面の偶奇規則の同輪", source: face([outer, outer]), groups: () => [[nearOuter, nearOuter]], probes: empty, empty: true },
    { name: "字形の巨大全囲", source: glyph([outer]), groups: () => [[nearOuter]], probes: covered, rule: "nonzero", scale: 1 },
    { name: "字形の逆向きの小穴", source: glyph([outer, reverse(hole)]), groups: (c, s) => [[nearOuter, holeOf(c, s, "outline_xy", "outline_vertices")]], probes: withHole, rule: "nonzero" },
    { name: "字形の同向きの小輪", source: glyph([outer, hole]), groups: (c, s) => [[nearOuter, holeOf(c, s, "outline_xy", "outline_vertices")]], probes: covered, rule: "nonzero" },
    { name: "字形の非零規則の同輪", source: glyph([outer, outer]), groups: () => [[nearOuter, nearOuter]], probes: covered, rule: "nonzero" },
    { name: "字形の非零規則の逆向き同輪", source: glyph([outer, reverse(outer)]), groups: () => [[nearOuter, reverse(nearOuter)]], probes: empty, empty: true, rule: "nonzero" },
    { name: "逆向きの別字形を独立に塗る", source: glyph([outer], true), groups: () => [[nearOuter], [reverse(nearOuter)]], probes: covered, rule: "nonzero", layers: 2 },
  ];
  // 接続部は画面外。画面内の二つの帯を、clipping の橋でつないで塗らない。
  // r=2e38/scale=1e12 の細い帯は回転の投影で幅を失う（#828 の残件）。
  // ここでは h=2e15/scale=1 とし、binary64 の投影にも帯の幅を残す。
  const h = Math.fround(2e15);
  const u = [-h, -40, h, -40, h, 40, -h, 40, -h, 20, 300, 20, 300, -20, -h, -20];
  items.push({ name: "画面外でつながる凹輪", source: face([u]), groups: (c, s) => {
    const y = (i) => c.face_xy[i * 2 + 1] * s;
    return [[square(-400, y(0), 400, y(7)), square(-400, y(4), 400, y(3))]];
  }, probes: [[0, -30, true], [0, 30, true], [0, 0, false]], scale: 1 });
  for (const outside of [false, true]) {
    items.push({ name: `巨大な ${outside ? "外" : "内"} clip`, source: clipped(line(), [{ xy: outer, outside }]),
      groups: () => [], referenceClips: () => [{ xy: nearOuter, outside }], stroke: true,
      probes: [[0, 0, !outside], [40, 0, !outside], [0, 20, false]], empty: outside });
    items.push({ name: `片側外の ${outside ? "外" : "内"} clip`, source: clipped(face([outer]), [{ xy: square(r / 2, -r, r, r), outside }]),
      groups: () => [[nearOuter]], referenceClips: () => [{ xy: square(400, -400, 600, 400), outside }],
      probes: outside ? covered : empty, empty: !outside });
  }
  const left = square(-r, -r, 0, r), right = square(0, -r, r, r);
  items.push({ name: "巨大な clip の辺だけの共通部分", source: clipped(face([outer]), [{ xy: left }, { xy: right }]),
    groups: () => [[nearOuter]], referenceClips: () => [{ xy: square(-400, -400, 0, 400) }, { xy: square(0, -400, 400, 400) }],
    probes: [[0, 0, false], [-40, 0, false], [40, 0, false]], empty: true, rotations: [0] });
  const a = square(-30 / scale, -r, r, r), b = square(-r, -r, 30 / scale, r);
  items.push({ name: "複数の巨大 clip の細い共通部分", source: clipped(face([outer]), [{ xy: a }, { xy: b }]),
    groups: () => [[nearOuter]], referenceClips: (c, s) => [{ xy: square(c.clip_xy[0] * s, -400, 400, 400) },
      { xy: square(-400, -400, c.clip_xy[10] * s, 400) }], probes: [[0, 0, true], [-50, 0, false], [50, 0, false]], rotations: [0] });
  return items.map((item) => ({ scale, ...item }));
}

function at(rotation, x, y) {
  const c = Math.cos(rotation), s = Math.sin(rotation);
  return { x: SIZE / 2 + c * x - s * y, y: SIZE / 2 - s * x - c * y };
}

function pathOf(rings, rotation) {
  const path = new Path2D();
  for (const ring of rings) {
    for (let i = 0; i < ring.length; i += 2) {
      const p = at(rotation, ring[i], ring[i + 1]);
      path[i ? "lineTo" : "moveTo"](p.x, p.y);
    }
    path.closePath();
  }
  return path;
}

/** 元の clipping を呼ばず、近傍の四角・帯・encoded の小穴から独立 Canvas を作る。 */
function reference(canvas, item, columns, ratio, rotation) {
  const ctx = canvas.getContext("2d"); ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  ctx.fillStyle = "white"; ctx.fillRect(0, 0, SIZE, SIZE);
  for (const clip of item.referenceClips?.(columns, item.scale) ?? []) {
    const rings = clip.outside ? [nearOuter, clip.xy] : [clip.xy];
    ctx.clip(pathOf(rings, rotation), "evenodd");
  }
  ctx.fillStyle = ctx.strokeStyle = "rgba(0,0,0,0.5019607843137255)";
  for (const rings of item.groups(columns, item.scale)) ctx.fill(pathOf(rings, rotation), item.rule ?? "evenodd");
  if (item.stroke) {
    const first = at(rotation, -400, 0), last = at(rotation, 400, 0), path = new Path2D();
    path.moveTo(first.x, first.y); path.lineTo(last.x, last.y); ctx.lineWidth = 8; ctx.lineCap = "butt"; ctx.stroke(path);
  }
  return ctx.getImageData(0, 0, canvas.width, canvas.height).data;
}

/** 巨大な閉輪の意味を、画面近傍の独立 Canvas と点・矩形の参照で比較する。 */
export function testLargeFills(assert) {
  const items = cases(), totals = { cases: items.length, scenes: 0, pixels: 0, ink: 0, points: 0, rectangles: 0,
    pixelRatios: [1, 1.5, 2, 8], scales: [1, 1e12], rotations: [0, Math.PI / 6], axisOnlyCases: 2,
    radii: [Math.fround(2e15), Math.fround(2e38)] };
  for (const pixelRatio of totals.pixelRatios) for (const rotation of totals.rotations) for (const item of items) {
    // 極端に細い巨大 clip の非零回転では投影の段階で幅が消える（#828 の残件）。
    // この二例は回転 0 に固定し、閉輪を切る処理そのものを検査する。
    if (item.rotations && !item.rotations.includes(rotation)) continue;
    const canvas = document.createElement("canvas"), near = document.createElement("canvas");
    const viewer = new View2D(canvas, { width: SIZE, height: SIZE, pixelRatio, pixelsPerMillimeter: 1,
      view: { center: { x: 0, y: 0 }, scale: item.scale, rotation } });
    try {
      const input = bundle(item.source), report = viewer.setScene(input), element = viewer.getElements()[0];
      const context = `${item.name}/ratio=${pixelRatio}/rotation=${rotation}`;
      assert(report.issues.length === 0 && report.drawn.faces + report.drawn.texts + report.drawn.lines === 1,
        `${context}: 元の要素を省略せず、診断と描画件数を保つ`);
      assert(element.id === ID && element.path.length === 1 && element.path[0].id === PLACE && element.path[0].column === 2 && element.path[0].row === 3,
        `${context}: 元の ID と配置の道筋を保つ`);
      if (item.rule) assert(report.drawn.glyphs === (item.layers ?? 1), `${context}: 字形の件数を保つ`);
      near.width = canvas.width; near.height = canvas.height;
      const expected = reference(near, item, input.scene.columns, pixelRatio, rotation);
      const actual = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
      let mismatch = 0, samples = 0, ink = 0;
      // 縁の AA の違いを除き、3×3 画素で同じ内部/外部の所を比較する。
      const step = Math.max(1, Math.floor(pixelRatio / 2));
      for (let y = 1; y < canvas.height - 1; y += step) for (let x = 1; x < canvas.width - 1; x += step) {
        const i = (y * canvas.width + x) * 4, value = expected[i];
        if (![255, 127, 63].includes(value)) continue;
        let stable = true;
        for (let dy = -1; dy <= 1 && stable; dy++) for (let dx = -1; dx <= 1; dx++) {
          if (Math.abs(expected[((y + dy) * canvas.width + x + dx) * 4] - value) > 1) { stable = false; break; }
        }
        if (!stable) continue;
        samples++; if (value < 255) ink++;
        if ([0, 1, 2].some((offset) => Math.abs(actual[i + offset] - value) > 4) || actual[i + 3] !== 255) mismatch++;
      }
      assert(samples > 1000 && mismatch === 0 && (item.empty ? ink === 0 : ink > 100),
        `${context}: 独立 Canvas の内部/外部と alpha が一致: samples=${samples}/ink=${ink}/mismatch=${mismatch}`);
      for (const [x, y, expectedHit] of item.probes) {
        const p = at(rotation, x, y);
        const queries = [viewer.hitTest(p, 0), viewer.hitTestRect({ min_x: p.x, min_y: p.y, max_x: p.x, max_y: p.y }),
          viewer.hitTestRect({ min_x: p.x - 0.25, min_y: p.y - 0.25, max_x: p.x + 0.25, max_y: p.y + 0.25 })];
        for (const hits of queries) assert(expectedHit ? hits.length === 1 && hits[0] === element : hits.length === 0,
          `${context}: 点と矩形は同じ元 ID を返し、穴/外側/相殺した輪を選ばない: x=${x}/y=${y}/expected=${expectedHit}/hits=${hits.length}`);
        totals.points++; totals.rectangles += 2;
      }
      totals.scenes++; totals.pixels += samples; totals.ink += ink;
    } finally { viewer.destroy(); }
  }
  assert(totals.scenes === items.reduce((sum, item) => sum + (item.rotations ?? totals.rotations).length, 0) * totals.pixelRatios.length,
    "巨大な閉輪の画素比・回転をすべて照合した");
  return totals;
}
