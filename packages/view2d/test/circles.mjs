import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";
import { circleIntersects, intersectCircleLine, intersectCirclePair } from "../src/circle.js";
import { circleArcs } from "../src/circle-path.js";

const ID = "space/0/h828circle", H = Math.fround(2e15), SIZE = 200;
function source(radius, direction = 0, weight = 0, alpha = 255, extra = {}) {
  const origins = [[radius, 0], [-radius, 0], [0, radius], [0, -radius]][direction];
  const strings = [...new TextEncoder().encode(ID)];
  return { chunk_origins: origins, chunk_bounds: [0, 0, 0, 0], chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [weight], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, strings.length], entity_kinds: [V.entity_kind.point], entity_paths: [0], strings,
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, alpha], color_flags: [0],
    point_xy: [0, 0], point_units: [0], point_rotations: [0], point_markers: [0, 0, 0, 0], point_scales: [1],
    point_style: [V.point_mark.none, V.point_frame.circle, V.point_size.length, radius * 2], ...extra };
}

// 製品の exact.js を参照しない。すべての binary64 を共通単位 2^-1074 の整数へ写す独立参照。
const bits = new DataView(new ArrayBuffer(8));
function integer(value) {
  bits.setFloat64(0, value); const hi = bits.getUint32(0), lo = bits.getUint32(4), exponent = hi >>> 20 & 2047;
  let n = BigInt(hi & 0xfffff) << 32n | BigInt(lo);
  if (exponent) n = (n | 1n << 52n) << BigInt(exponent - 1);
  return hi >>> 31 ? -n : n;
}
const squared = (x, y, cx, cy) => (integer(x) - integer(cx)) ** 2n + (integer(y) - integer(cy)) ** 2n;
function expectedPoint(p, tolerance, c, radius, half) {
  const far = integer(radius) + integer(half) + integer(tolerance), near = integer(radius) - integer(half) - integer(tolerance);
  const d = squared(p.x, p.y, c.x, c.y);
  return d <= far * far && (near <= 0n || d >= near * near);
}
function expectedRect(rect, c, radius, half) {
  const far = integer(radius) + integer(half), near = integer(radius) - integer(half);
  const x = Math.max(rect.min_x, Math.min(c.x, rect.max_x)), y = Math.max(rect.min_y, Math.min(c.y, rect.max_y));
  if (squared(x, y, c.x, c.y) > far * far) return false;
  return near <= 0n || [rect.min_x, rect.max_x].some((a) => [rect.min_y, rect.max_y].some((b) => squared(a, b, c.x, c.y) >= near * near));
}

export function testCircles(assert) {
  const totals = { scenes: 0, points: 0, rectangles: 0, numeric: 0, contracts: 0, ratios: [1, 1.5, 2, 8], radii: [20, H], rotations: [0, Math.PI / 6, Math.PI / 2], scales: [0.5, 1, 2] };
  for (const radius of totals.radii) for (let direction = 0; direction < 4; direction++) for (const pixelRatio of totals.ratios)
    for (const rotation of totals.rotations) for (const scale of totals.scales) {
    const canvas = document.createElement("canvas"), viewer = new View2D(canvas, { width: SIZE, height: SIZE, pixelRatio, pixelsPerMillimeter: 1,
      view: { center: { x: 0, y: 0 }, rotation, scale } });
    try {
      const input = bundle(source(radius, direction)), report = viewer.setScene(input), element = viewer.getElements()[0];
      const [ox, oy] = input.scene.columns.chunk_origins, r = input.scene.columns.point_style[3] * scale / 2, half = 0.5 / pixelRatio;
      const c = { x: 100 + scale * (Math.cos(rotation) * ox - Math.sin(rotation) * oy), y: 100 - scale * (Math.sin(rotation) * ox + Math.cos(rotation) * oy) };
      const label = `R=${r}/direction=${direction}/ratio=${pixelRatio}/rotation=${rotation}/scale=${scale}`;
      assert(report.drawn.points === 1 && report.issues.length === 0 && element.id === ID, `${label}: 元の件数・ID・診断`);
      const distance = Math.hypot(c.x - 100, c.y - 100), nx = (c.x - 100) / distance, ny = (c.y - 100) / distance;
      for (const along of [-20, -1, 0, 1, 20]) for (const across of [-half - 1 / 64, -half, -half + 1 / 64, 0, half - 1 / 64, half, half + 1 / 64]) {
        const p = { x: 100 + nx * across - ny * along, y: 100 + ny * across + nx * along };
        for (const tolerance of [0, 0.25, 2]) {
          const expected = expectedPoint(p, tolerance, c, r, half), hits = viewer.hitTest(p, tolerance);
          assert(expected ? hits.length === 1 && hits[0] === element : hits.length === 0, `${label}: 点の厳密参照 p=${JSON.stringify(p)}/t=${tolerance}/expected=${expected}/hits=${hits.length}`);
          totals.points++;
        }
        for (const [dx, dy] of [[0, 0], [0, 1], [1, 0], [0.25, 0.5]]) {
          const rect = { min_x: p.x, min_y: p.y, max_x: p.x + dx, max_y: p.y + dy };
          const expected = expectedRect(rect, c, r, half), hits = viewer.hitTestRect(rect);
          assert(expected ? hits.length === 1 && hits[0] === element : hits.length === 0, `${label}: 矩形の厳密参照 rect=${JSON.stringify(rect)}/expected=${expected}/hits=${hits.length}`);
          totals.rectangles++;
        }
      }
      totals.scenes++;
    } finally { viewer.destroy(); }
  }
  // 既存の入力上限を広げる検査ではなく、演算自体の差・平方・半径和の overflow を直接検査する。
  for (const radius of [H, 1e162, 2 ** 1023]) for (const [x, y, expected] of [[-1, 0, true], [-1, 1, false], [1, 0, true], [1, 1, true], [2, 0, false]]) {
    const center = { x, y }, query = { center, circle: { ...center, radius: 0 } };
    assert(circleIntersects({ x: radius, y: 0, radius }, 1, false, query) === expected, `二乗と R±half の中間丸めを避ける: R=${radius}/p=${x},${y}`);
    totals.numeric++;
  }
  const canvas = document.createElement("canvas"), viewer = new View2D(canvas, { width: SIZE, height: SIZE, pixelRatio: 1, pixelsPerMillimeter: 1 });
  const hit = (x, y, expected, label, tolerance = 0) => {
    const element = viewer.getElements()[0], points = viewer.hitTest({ x, y }, tolerance);
    assert(expected ? points.length === 1 && points[0] === element : !points.length, label); totals.contracts++;
  };
  try {
    // 先に二度丸めた bounds で除外すると、数学的には届く小さい tolerance が失われる通常値の例。
    viewer.resize(SIZE, SIZE, 1.2);
    const encoded = bundle(source(30.060644149780273, 0, 0, 255, { chunk_origins: [8.229347278202411, 0] }));
    assert(encoded.scene.columns.point_style[3] / 2 === 30.060644149780273, "bounds の反例は encoded f32 の半径を変えない"); totals.contracts++;
    viewer.setScene(encoded);
    hit(138.70665809464936, 100, true, "encoded bounds の先行丸めで tolerance を切り捨てない", 1.1368683772161604e-14);
    viewer.resize(SIZE, SIZE, 1);
    viewer.setScene(bundle(source(20, 0, 0, 255, { chunk_origins: [0, 0], point_style: [V.point_mark.dot, 0, V.point_size.length, 40] })));
    hit(100.75, 100, true, "塗られた点の円"); hit(101.25, 100, false, "点の円の外側"); hit(101.25, 100, true, "点の円に届く指定 tolerance", 0.25);
    const repeated = source(20, 0, 2, 128, { chunk_origins: [0, 0], point_xy: [0, 0, 0, 0], point_units: [0, 0], point_rotations: [0, 0],
      point_markers: Array(8).fill(0), point_scales: [1, 1] });
    assert(viewer.setScene(bundle(repeated)).drawn.points === 2, "別 primitive の描画件数を保持"); totals.contracts++;
    hit(120, 100, true, "同じ ID の別 primitive は元の参照を一度だけ返す");
    // circle 以外の contour が同じ part にあれば短絡しない。マーク中心と四角の角も元の union へ残す。
    viewer.setScene(bundle(source(20, 0, 2, 128, { chunk_origins: [0, 0], point_style: [V.point_mark.plus, V.point_frame.circle, V.point_size.length, 40] })));
    hit(100, 100, true, "同じ part の十字と円の union"); hit(105, 105, false, "union の空白");
    viewer.setScene(bundle(source(20, 0, 2, 128, { chunk_origins: [0, 0], point_style: [V.point_mark.none, V.point_frame.circle | V.point_frame.square, V.point_size.length, 40] })));
    hit(80, 80, true, "同じ part の円と四角の union"); hit(100, 100, false, "円と四角の穴");
    for (const outside of [0, 1]) {
      viewer.setScene(bundle(source(20, 0, 2, 128, { chunk_origins: [0, 0], unit_clips: [0], clip_polygons: [0], polygon_vertices: [0], polygon_flags: [outside],
        clip_xy: [-30, -30, 0, -30, 0, 30, -30, 30] })));
      hit(80, 100, !outside, "内外の clip を従来の配置検査へ残す"); hit(120, 100, !!outside, "内外の clip の反対側");
    }
    // viewport に外側から接する円環は面積のある可視領域を持たない。query が内部でない場合は短絡しない。
    viewer.setScene(bundle(source(20, 0, 2, 255, { chunk_origins: [-121, 0] })));
    hit(0, 100, false, "viewport 外からの接触だけを拾わない");
    assert(!viewer.hitTestRect({ min_x: 0, min_y: 99, max_x: 1, max_y: 101 }).length, "viewport に接する矩形も配置検査へ残す"); totals.contracts++;
    const transparent = source(H, 0, 0, 0); viewer.setScene(bundle(transparent)); hit(100, 100, false, "alpha=0 は exact 経路でも拾わない");
    viewer.select([{ id: ID, path: [] }]); hit(100, 100, true, "表示色の override を exact 経路にも適用する"); viewer.select([]);
    const layered = source(H); layered.layer_ids = [...layered.entity_ids]; layered.unit_layers = [0]; viewer.setScene(bundle(layered));
    viewer.setHiddenLayers([ID]); hit(100, 100, false, "非表示レイヤを exact 経路へ渡さない"); viewer.setHiddenLayers([]); hit(100, 100, true, "再表示した元 ID");
  } finally { viewer.destroy(); }
  // viewport 接触を短絡せず従来の配置検査へ戻す場合も、問い合わせの共有予算で停止する。
  const budget = new View2D(document.createElement("canvas"), { width: SIZE, height: SIZE, pixelsPerMillimeter: 1, limits: { work: 1000 } });
  try {
    const many = source(20, 0, 2, 255, { chunk_origins: [-121, 0] }); many.point_xy = Array(80).fill(0); many.point_units = Array(40).fill(0); many.point_rotations = Array(40).fill(0);
    many.point_markers = Array(160).fill(0); many.point_scales = Array(40).fill(1);
    assert(budget.setScene(bundle(many)).drawn.points === 40, "予算の反例は先に描画の上限を通る"); totals.contracts++;
    let stopped = false; try { budget.hitTest({ x: 0, y: 100 }, 0); } catch (error) { stopped = error instanceof RangeError; }
    assert(stopped, "viewport 接触の fallback は共有の問い合わせ予算を守る"); totals.contracts++;
  } finally { budget.destroy(); }
  return totals;
}

/** r≈2e15 の画面内の弧は、画素未満の弦の差を除けば近傍の直線と一致する。 */
export function testCircleDrawing(assert) {
  const totals = { scenes: 0, pixels: 0, contracts: 0, ratios: [1, 1.5, 2, 8], widths: [0, 2, 8], directions: 4 };
  for (let direction = 0; direction < 4; direction++) for (const pixelRatio of totals.ratios) for (const weight of totals.widths)
    for (const combination of ["circle", "plus", "square"]) {
    const canvas = document.createElement("canvas"), near = document.createElement("canvas");
    const viewer = new View2D(canvas, { width: SIZE, height: SIZE, pixelRatio, pixelsPerMillimeter: 1,
      view: { center: { x: 0, y: 0 }, rotation: 0, scale: 1 } });
    try {
      const mark = combination === "plus" ? V.point_mark.plus : V.point_mark.none;
      const frame = V.point_frame.circle | (combination === "square" ? V.point_frame.square : 0);
      const input = bundle(source(H, direction, weight, 128, { point_style: [mark, frame, V.point_size.length, H * 2] }));
      const report = viewer.setScene(input), width = Math.max(1 / pixelRatio, input.scene.columns.unit_weights[0]);
      assert(report.drawn.points === 1 && report.issues.length === 0 && viewer.getElements()[0].id === ID, "円の描画の件数と ID を保つ");
      near.width = canvas.width; near.height = canvas.height;
      const ctx = near.getContext("2d"), path = new Path2D();
      ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0); ctx.fillStyle = "white"; ctx.fillRect(0, 0, SIZE, SIZE);
      const horizontal = direction >= 2;
      path.moveTo(horizontal ? -200 : 100, horizontal ? 100 : -200);
      path.lineTo(horizontal ? 400 : 100, horizontal ? 100 : 400);
      if (combination === "plus") {
        path.moveTo(100, 100);
        path.lineTo(direction === 0 ? 400 : direction === 1 ? -200 : 100, direction === 2 ? -200 : direction === 3 ? 400 : 100);
      }
      if (combination === "square") {
        path.moveTo(horizontal ? -200 : 100, horizontal ? 100 : -200);
        path.lineTo(horizontal ? 400 : 100, horizontal ? 100 : 400);
      }
      ctx.strokeStyle = "rgba(0,0,0,0.5019607843137255)"; ctx.lineWidth = width; ctx.lineCap = "butt"; ctx.lineJoin = "round"; ctx.stroke(path);
      const expected = ctx.getImageData(0, 0, near.width, near.height).data;
      const actual = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
      let mismatch = 0, ink = 0;
      for (let y = 1; y < canvas.height - 1; y++) for (let x = 1; x < canvas.width - 1; x++) {
        const i = (y * canvas.width + x) * 4;
        if (expected[i] < 250) ink++;
        if (Math.abs(actual[i] - expected[i]) > 4 || actual[i + 3] !== 255) mismatch++;
        totals.pixels++;
      }
      assert(ink > 50 && mismatch === 0, `巨大な円の近傍の輪郭と alpha: direction=${direction}/ratio=${pixelRatio}/width=${weight}/${combination}/mismatch=${mismatch}`);
      totals.scenes++;
    } finally { viewer.destroy(); }
  }
  // 弧の曲率が数画素見える縦長の画面。独立参照は相殺しない有理化した式を使う。
  for (const weight of totals.widths) {
    const canvas = document.createElement("canvas"), near = document.createElement("canvas"), radius = 1_000_001, height = 4096;
    const viewer = new View2D(canvas, { width: SIZE, height, pixelsPerMillimeter: 1 });
    try {
      viewer.setScene(bundle(source(radius, 0, weight, 128)));
      near.width = SIZE; near.height = height;
      const ctx = near.getContext("2d"), path = new Path2D();
      ctx.fillStyle = "white"; ctx.fillRect(0, 0, SIZE, height);
      for (let y = -16; y <= height + 16; y += 4) {
        const dy = y - height / 2, x = 100 + dy * dy / (radius + Math.sqrt(radius * radius - dy * dy));
        path[y === -16 ? "moveTo" : "lineTo"](x, y);
      }
      ctx.strokeStyle = "rgba(0,0,0,0.5019607843137255)"; ctx.lineWidth = Math.max(1, weight); ctx.lineCap = "butt"; ctx.lineJoin = "round"; ctx.stroke(path);
      const a = canvas.getContext("2d").getImageData(0, 0, SIZE, height).data, b = ctx.getImageData(0, 0, SIZE, height).data;
      let mismatch = 0, maximum = 0;
      // 別の分割位置の round join には AA の差がある。形は下の独立式で検査する。
      for (let i = 0; i < a.length; i += 4) { maximum = Math.max(maximum, Math.abs(a[i] - b[i])); if (Math.abs(a[i] - b[i]) > 16 || a[i + 3] !== 255) mismatch++; totals.pixels++; }
      assert(mismatch === 0, `円弧を画面に合わせて分割して曲率を保つ: width=${weight}/mismatch=${mismatch}/maximum=${maximum}`);
      const box = { min_x: -16, min_y: -16, max_x: SIZE + 16, max_y: height + 16 };
      for (const points of circleArcs({ x: radius + 100, y: height / 2 }, radius, box, 1 / 64, () => {})) {
        for (let i = 1; i < points.length; i++) {
          const mid = { x: (points[i - 1].x + points[i].x) / 2, y: (points[i - 1].y + points[i].y) / 2 };
          for (const p of [points[i - 1], mid, points[i]]) {
            const dy = p.y - height / 2, x = 100 + dy * dy / (radius + Math.sqrt(radius * radius - dy * dy));
            assert(Math.abs(p.x - x) <= (p === mid ? 1 / 64 : 1e-9), "独立式と円弧の端点・弦の誤差上限が一致する");
          }
        }
      }
      totals.scenes++;
    } finally { viewer.destroy(); }
  }
  const canvas = document.createElement("canvas");
  const viewer = new View2D(canvas, { width: 200, height: 200, pixelsPerMillimeter: 1, limits: { work: 150 } });
  try {
    viewer.setScene(bundle(source(1_000_001)));
    const before = viewer.getState(), pixels = canvas.toDataURL();
    let stopped = false;
    try { viewer.resize(200, 8192); } catch (error) { stopped = error instanceof RangeError && error.message.includes("limits.work"); }
    assert(stopped && viewer.getState() === before && canvas.toDataURL() === pixels, "円弧の分割が予算を超えた resize は状態と画面を保つ"); totals.contracts++;
    viewer.setScene(bundle(source(20)));
    viewer.resize(200, 8192);
    const oldScene = viewer.getScene(), oldPixels = canvas.toDataURL();
    stopped = false;
    try { viewer.setScene(bundle(source(1_000_001))); } catch (error) { stopped = error instanceof RangeError && error.message.includes("limits.work"); }
    assert(stopped && viewer.getScene() === oldScene && canvas.toDataURL() === oldPixels, "円弧の分割が予算を超えた setScene は状態と画面を保つ"); totals.contracts++;
  } finally { viewer.destroy(); }
  return totals;
}

/** 全画面を残す clip の有無で、円環の数学的な集合や元 ID を変えない。 */
export function testCircleClips(assert) {
  let points = 0, rectangles = 0, contracts = 0;
  for (const radius of [20, H]) for (let direction = 0; direction < 4; direction++) for (const pixelRatio of [1, 1.5, 8])
    for (const rotation of [0, Math.PI / 6, Math.PI / 2]) for (const scale of [0.5, 1, 2]) {
    const viewer = new View2D(document.createElement("canvas"), { width: SIZE, height: SIZE, pixelRatio, pixelsPerMillimeter: 1,
      view: { center: { x: 0, y: 0 }, rotation, scale } });
    try {
      const input = bundle(source(radius, direction, 0, 128, { unit_clips: [0], clip_polygons: [0], polygon_vertices: [0], polygon_flags: [0],
        clip_xy: [-1000, -1000, 1000, -1000, 1000, 1000, -1000, 1000] }));
      viewer.setScene(input);
      const element = viewer.getElements()[0], [ox, oy] = input.scene.columns.chunk_origins;
      const c = { x: 100 + scale * (Math.cos(rotation) * ox - Math.sin(rotation) * oy), y: 100 - scale * (Math.sin(rotation) * ox + Math.cos(rotation) * oy) };
      const r = input.scene.columns.point_style[3] * scale / 2, half = 0.5 / pixelRatio, distance = Math.hypot(c.x - 100, c.y - 100);
      const nx = (c.x - 100) / distance, ny = (c.y - 100) / distance;
      for (const along of [-20, -1, 0, 1, 20]) for (const across of [-half - 0.25, -half, -half + 0.25, 0, half - 0.25, half, half + 0.25]) {
        const p = { x: 100 + nx * across - ny * along, y: 100 + ny * across + nx * along };
        for (const tolerance of [0, 0.25, 2]) {
          const expected = expectedPoint(p, tolerance, c, r, half), hit = viewer.hitTest(p, tolerance);
          assert(expected ? hit.length === 1 && hit[0] === element : hit.length === 0, `clip を通る円の点: R=${r}/d=${direction}/ratio=${pixelRatio}/p=${JSON.stringify(p)}/t=${tolerance}/expected=${expected}/actual=${hit.length}`);
          points++;
        }
        const rect = { min_x: p.x, min_y: p.y, max_x: p.x + 0.25, max_y: p.y + 0.5 }, expected = expectedRect(rect, c, r, half), hit = viewer.hitTestRect(rect);
        assert(expected ? hit.length === 1 && hit[0] === element : hit.length === 0, "clip を通る円の矩形を独立した整数距離で照合する"); rectangles++;
      }
    } finally { viewer.destroy(); }
  }
  const viewer = new View2D(document.createElement("canvas"), { width: SIZE, height: SIZE, pixelsPerMillimeter: 1 });
  const hit = (x, y, tolerance, expected, label) => {
    assert(viewer.hitTest({ x, y }, tolerance).length === Number(expected), label); contracts++;
  };
  const clip = (xy, outside = 0) => ({ unit_clips: [0], clip_polygons: [0], polygon_vertices: [0], polygon_flags: [outside], clip_xy: xy });
  try {
    for (const outside of [0, 1]) {
      viewer.setScene(bundle(source(H, 0, 2, 128, clip([-10, -100, 10, -100, 10, 0, -10, 0], outside))));
      hit(100, 120, 0, !outside, "部分 clip の内外を保つ"); hit(100, 80, 0, !!outside, "部分 clip の反対側");
      hit(100, outside ? 120 : 80, 20, true, "切り抜き外からの許容円が境界へ届く");
      hit(100, outside ? 120 : 80, 19.9, false, "切り抜き外からの許容円が届かない");
    }
    viewer.setScene(bundle(source(H, 0, 2, 128, clip([-10, -100, 10, -100, 10, 0, -10, 0]))));
    hit(98, 99, 1.42, true, "円環と clip の角へ届く許容円"); hit(98, 99, 1.4, false, "円環と clip の角の直前");
    viewer.setScene(bundle(source(H, 0, 2, 128, clip([-1000, -1000, -1, -1000, -1, 1000, -1000, 1000]))));
    hit(99, 100, 0, false, "外側の円に接するだけの clip は可視面積がない");
    viewer.setScene(bundle(source(H, 0, 2, 128, clip([1, -1000, 1000, -1000, 1000, 1000, 1, 1000]))));
    hit(101, 100, 0, true, "円の穴に接する clip の二次の可視領域を残す");
    for (const shift of [0, 0.25]) {
      viewer.setScene(bundle(source(H, 0, 2, 128, { chunk_origins: [-H - 101 + shift, 0] })));
      hit(0, 100, 0, shift > 0, "viewport 外からの接触と微小な可視領域を区別する");
      assert(viewer.hitTestRect({ min_x: 0, min_y: 99, max_x: 1, max_y: 101 }).length === Number(shift > 0), "viewport 境界の矩形選択"); contracts++;
    }
    for (const frame of [V.point_frame.circle, V.point_frame.circle | V.point_frame.square]) {
      viewer.setScene(bundle(source(H, 0, 0, 128, { point_style: [V.point_mark.plus, frame, V.point_size.length, H * 2] })));
      hit(99, 80, 0, false, "同じ part の円・マーク・四角の外側"); hit(100, 80, 0, true, "同じ part の円の輪郭");
      hit(110, 100, 0, true, "同じ part の十字"); hit(90, 100, 0, false, "同じ part の空白");
      hit(99.5, 80, 0, !!(frame & V.point_frame.square), "円環の外側でも四角の辺だけには触れる点");
    }
  } finally { viewer.destroy(); }
  for (const radius of [H, 1e162, 2 ** 1023]) {
    const circle = { x: radius, y: 0, radius }, line = [], pair = [], expectedX = 0.5 / radius;
    intersectCircleLine(circle, { a: { x: -1, y: 1 }, b: { x: 1, y: 1 } }, (p) => { line.push(p); return false; });
    intersectCirclePair(circle, { x: 0, y: 0, radius: 1 }, (p) => { pair.push(p); return false; });
    assert(line.length === 1 && line[0].x === expectedX && line[0].y === 1, "円と線分の相殺・二乗の桁あふれを避ける"); contracts++;
    assert(pair.length === 2 && pair.every((p) => p.x === expectedX && Math.abs(p.y) === 1) && pair[0].y !== pair[1].y,
      "二円の共通弦の基点と判別式を途中で丸めない"); contracts++;
  }
  return { points, rectangles, contracts };
}
