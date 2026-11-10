import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";

function source(xy) {
  const strings = [...new TextEncoder().encode("space/0/h1")];
  const xs = xy.filter((_, i) => i % 2 === 0), ys = xy.filter((_, i) => i % 2 === 1);
  return { chunk_origins: [0, 0], chunk_bounds: xy.length ? [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)] : [0, 0, 0, 0],
    chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [0], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, strings.length], entity_kinds: [V.entity_kind.line], entity_paths: [0],
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, 128], color_flags: [0], strings,
    line_vertices: [0], line_units: [0], line_xy: xy };
}

/** 巨大な元座標に依存しない近傍の独立 Canvas と、線・join・dash・clip・点を比較する。 */
export function testLargeStrokes(assert) {
  const huge = Math.fround(2e15);
  const cases = [
    { name: "斜線", data: source([-2000, 2000, 2000, -2000]), scale: 1e12, paths: [[[[-100, -100], [300, 300]]]], hit: [100, 100] },
    { name: "水平線", data: source([-huge, 0, huge, 0]), paths: [[[[-100, 100], [300, 100]]]], hit: [100, 100] },
    { name: "垂直線", data: source([0, huge, 0, -huge]), paths: [[[[100, -100], [100, 300]]]], hit: [100, 100] },
    { name: "見える元の round join", data: source([-huge, 0, 0, 0, 0, huge]), paths: [[[[-100, 100], [100, 100], [100, -100]]]], hit: [100, 100] },
    { name: "画面外を回って戻る同じ折れ線", data: source([-huge, 20, huge, 20, huge, huge, -huge, huge, -huge, -20, huge, -20]),
      paths: [[[[-100, 80], [300, 80]], [[-100, 120], [300, 120]]]], hit: [100, 80], gap: [100, 100] },
    { name: "コアが作った dash の隙間", data: { ...source([-2e9, 0, -2, 0, 2, 0, 2e9, 0]), line_vertices: [0, 2], line_units: [0, 0] },
      paths: [[[[-100, 100], [98, 100]]], [[[102, 100], [300, 100]]]], hit: [90, 100], gap: [100, 100] },
    // #827 の極大 butt の選択は未解消。ここでは描画が元の隙間を埋めないことを独立に検査する。
    { name: "極大 dash の描画の隙間（選択は #827）", data: { ...source([-huge, 0, -2, 0, 2, 0, huge, 0]), line_vertices: [0, 2], line_units: [0, 0] },
      paths: [[[[-100, 100], [98, 100]]], [[[102, 100], [300, 100]]]], hit: [90, 100], gap: [100, 100], unresolvedGap: true },
  ];
  for (const mark of [V.point_mark.plus, V.point_mark.cross, V.point_mark.tick]) {
    const data = source([]); delete data.line_vertices; delete data.line_units; delete data.line_xy;
    Object.assign(data, { point_xy: [0, 0], point_units: [0], point_rotations: [0], point_markers: [0, 0, 0, 0], point_scales: [1],
      point_style: [mark, 0, V.point_size.length, huge * 2] });
    const paths = mark === V.point_mark.plus ? [[[-100, 100], [300, 100]], [[100, -100], [100, 300]]] :
      mark === V.point_mark.cross ? [[[-100, -100], [300, 300]], [[-100, 300], [300, -100]]] : [[[100, 100], [100, -100]]];
    cases.push({ name: `直線の point ${mark}`, data, paths: [paths], hit: [100, 90] });
    if (mark === V.point_mark.cross) cases[cases.length - 1].hit = [90, 90];
  }
  const corner = source([]); delete corner.line_vertices; delete corner.line_units; delete corner.line_xy;
  Object.assign(corner, { chunk_origins: [huge, -huge], point_xy: [0, 0], point_units: [0], point_rotations: [0], point_markers: [0, 0, 0, 0], point_scales: [1],
    point_style: [V.point_mark.dot, V.point_frame.square, V.point_size.length, huge * 2] });
  cases.push({ name: "巨大な point の四角の見える閉じた角", data: corner, paths: [[[[100, 300], [100, 100], [300, 100]]]], hit: [100, 110] });
  for (const outside of [false, true]) {
    const data = source([-huge, 0, huge, 0]);
    Object.assign(data, { unit_clips: [0], clip_polygons: [0], polygon_flags: [Number(outside)], polygon_vertices: [0], clip_xy: [-20, -20, 20, -20, 20, 20, -20, 20] });
    cases.push({ name: `通常の clip ${outside ? "外" : "内"}`, data, paths: [[[[-100, 100], [300, 100]]]], outside, clip: true,
      hit: outside ? [60, 100] : [100, 100], gap: outside ? [100, 100] : [60, 100] });
  }
  for (const pixelRatio of [1, 1.5, 2]) for (const item of cases) {
    const width = 8;
    item.data.unit_weights = [width];
    const canvas = document.createElement("canvas"), reference = document.createElement("canvas");
    const viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio, pixelsPerMillimeter: 1,
      view: { center: { x: 0, y: 0 }, scale: item.scale ?? 1, rotation: 0 } });
    try {
      viewer.setScene(bundle(item.data));
      reference.width = canvas.width; reference.height = canvas.height;
      const ctx = reference.getContext("2d"); ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
      ctx.fillStyle = "white"; ctx.fillRect(0, 0, 200, 200);
      if (item.clip) {
        const clip = new Path2D(); if (item.outside) clip.rect(0, 0, 200, 200); clip.rect(80, 80, 40, 40); ctx.clip(clip, "evenodd");
      }
      ctx.strokeStyle = "rgba(0,0,0,0.5019607843137255)"; ctx.lineWidth = width; ctx.lineCap = "butt"; ctx.lineJoin = "round";
      for (const group of item.paths) {
        const path = new Path2D();
        for (const points of group) for (let i = 0; i < points.length; i++) path[i ? "lineTo" : "moveTo"](...points[i]);
        ctx.stroke(path);
      }
      const expected = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      const actual = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
      let mismatch = 0, ink = 0, clearInk = 0;
      for (let i = 0; i < actual.length; i += 4) {
        if (actual[i] !== 255) ink++;
        // 遠方の端点の違いだけでも、Canvas は縁のアンチエイリアスを変える。
        // 近傍 3×3 画素まで同じ内部か外部の所で、欠落・隙間と alpha の重なりを比べる。
        const x = i / 4 % canvas.width, y = Math.floor(i / 4 / canvas.width), value = expected[i];
        const full = value >= 126 && value <= 128, empty = value === 255;
        if (!full && !empty) continue;
        let clear = true;
        for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
          if (x + dx < 0 || x + dx >= canvas.width || y + dy < 0 || y + dy >= canvas.height) continue;
          const neighbor = expected[((y + dy) * canvas.width + x + dx) * 4];
          if (full ? neighbor < 126 || neighbor > 128 : neighbor !== 255) clear = false;
        }
        if (clear) { if (full) clearInk++; if (Math.abs(actual[i] - value) > 4) mismatch++; }
      }
      assert(ink > 0 && clearInk > 0 && mismatch === 0,
        `${item.name}: 近傍の独立 Canvas と内部/外部の画素を比較: ink=${ink}/clear=${clearInk}/mismatch=${mismatch}/ratio=${pixelRatio}`);
      const pixel = (x, y) => canvas.getContext("2d").getImageData(Math.floor(x * pixelRatio), Math.floor(y * pixelRatio), 1, 1).data[0];
      assert(pixel(...item.hit) >= 126 && pixel(...item.hit) <= 128, `${item.name}: 同じ stroke 内の重なりに alpha を重ねない`);
      assert(viewer.hitTest({ x: item.hit[0], y: item.hit[1] }, 0).length === 1, `${item.name}: 描かれる所を選べる`);
      if (item.gap) {
        const gapPixel = pixel(...item.gap), gapHits = viewer.hitTest({ x: item.gap[0], y: item.gap[1] }, 0).length;
        assert(gapPixel === 255, `${item.name}: 描画は元の隙間をつながない: pixel=${gapPixel}`);
        if (!item.unresolvedGap) assert(gapHits === 0, `${item.name}: 隙間を選択しない: hits=${gapHits}`);
      }
    } finally { viewer.destroy(); }
  }
}
