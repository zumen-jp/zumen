import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";

const SIZE = 200;
const SEED = 0x80851a7;
const SCENES = 256;
const QUERIES = 100;
const GUARD = 1e-8;
const clamp = (value, low, high) => Math.max(low, Math.min(high, value));

function line(xy, weight) {
  const strings = [...new TextEncoder().encode("space/0/hsat")];
  return {
    chunk_origins: [0, 0], chunk_bounds: [Math.min(xy[0], xy[2]), Math.min(xy[1], xy[3]), Math.max(xy[0], xy[2]), Math.max(xy[1], xy[3])],
    chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [weight], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, strings.length], entity_kinds: [V.entity_kind.line], entity_paths: [0],
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, 255], color_flags: [0], strings,
    line_vertices: [0], line_units: [0], line_xy: xy,
  };
}

function project(column, offset, view) {
  const x = column[offset] - view.center.x, y = column[offset + 1] - view.center.y;
  const c = Math.cos(view.rotation), s = Math.sin(view.rotation);
  return { x: SIZE / 2 + view.scale * (c * x - s * y), y: SIZE / 2 - view.scale * (s * x + c * y) };
}

function paper(point, view) {
  const x = (point.x - SIZE / 2) / view.scale, y = (SIZE / 2 - point.y) / view.scale;
  const c = Math.cos(view.rotation), s = Math.sin(view.rotation);
  return [view.center.x + c * x + s * y, view.center.y - s * x + c * y];
}

function stroke(a, b, radius) {
  const dx = b.x - a.x, dy = b.y - a.y, length = Math.hypot(dx, dy);
  const along = { x: dx / length, y: dy / length }, normal = { x: -dy / length, y: dx / length };
  const offset = (p, sign) => ({ x: p.x + sign * normal.x * radius, y: p.y + sign * normal.y * radius });
  return { a, b, length, along, normal, center: { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 },
    corners: [offset(a, 1), offset(b, 1), offset(b, -1), offset(a, -1)] };
}

/** 凸四角どうしの分離軸。交点や本体の包含判定は使わない。 */
function sat(shape, rect) {
  const query = [{ x: rect.min_x, y: rect.min_y }, { x: rect.max_x, y: rect.min_y },
    { x: rect.max_x, y: rect.max_y }, { x: rect.min_x, y: rect.max_y }];
  const origin = { x: (rect.min_x + rect.max_x) / 2, y: (rect.min_y + rect.max_y) / 2 };
  let margin = Infinity;
  for (const axis of [{ x: 1, y: 0 }, { x: 0, y: 1 }, shape.along, shape.normal]) {
    const range = (points) => {
      let low = Infinity, high = -Infinity;
      for (const p of points) {
        const value = (p.x - origin.x) * axis.x + (p.y - origin.y) * axis.y;
        low = Math.min(low, value); high = Math.max(high, value);
      }
      return { low, high };
    };
    const a = range(shape.corners), b = range(query);
    margin = Math.min(margin, a.high - b.low, b.high - a.low);
  }
  return { hit: margin >= 0, stable: Math.abs(margin) > GUARD };
}

/** 公開 f32 列の butt stroke を、決定的な独立 SAT と点・矩形各 25,600 件で照合する。 */
export function testStrokeSat(assert) {
  let state = SEED;
  const random = () => { state = (Math.imul(state, 1664525) + 1013904223) >>> 0; return state / 0x100000000; };
  const signed = () => random() * 2 - 1;
  const viewer = new View2D(document.createElement("canvas"), { width: SIZE, height: SIZE, pixelRatio: 1, pixelsPerMillimeter: 1 });
  const totals = { seed: SEED, scenes: SCENES, points: 0, rectangles: 0, pointHits: 0, rectangleHits: 0, boundaryExcluded: 0 };
  const rectAt = (p) => ({ min_x: p.x, min_y: p.y, max_x: p.x, max_y: p.y });
  const check = (kind, query, shape, context) => {
    const expected = sat(shape, query);
    if (!expected.stable) { totals.boundaryExcluded++; return false; }
    const actual = kind === "points" ? viewer.hitTest({ x: query.min_x, y: query.min_y }, 0).length !== 0 : viewer.hitTestRect(query).length !== 0;
    totals[kind]++;
    if (expected.hit) totals[kind === "points" ? "pointHits" : "rectangleHits"]++;
    if (actual !== expected.hit) {
      assert(false, `独立 SAT と ${kind} が不一致: ${JSON.stringify({ seed: SEED, ...context, query, expected: expected.hit, actual })}`);
    } else assert(true, "encoded f32 の stroke と独立 SAT の内外が一致する");
    return true;
  };
  try {
    const explicitView = { center: { x: 0, y: 0 }, scale: 1, rotation: 0 };
    viewer.jumpTo(explicitView);
    const explicit = bundle(line([-1000, 200, 1000, -200], 0)); viewer.setScene(explicit);
    const explicitShape = stroke(project(explicit.scene.columns.line_xy, 0, explicitView), project(explicit.scene.columns.line_xy, 2, explicitView), 0.5);
    const crossing = { min_x: 90.5, min_y: 100.1, max_x: 110.5, max_y: 110.1 };
    assert(sat(explicitShape, crossing).hit && viewer.hitTestRect(crossing).length === 1, "交点が丸まっても中心と四隅が外にある矩形の交差を落とさない");
    assert(viewer.hitTest({ x: 101, y: 100.2 }, 0).length === 1, "明示した矩形内の線上点も選べる");

    for (let scene = 0; scene < SCENES; scene++) {
      const length = [0.0001, 0.01, 0.24, 1, 8, 64, 400, 2000][scene % 8];
      const pixelRatio = [1, 1.5, 2, 8][Math.floor(scene / 8) % 4];
      const weight = [0, 0.125, 1, 4][Math.floor(scene / 32) % 4];
      const view = { center: { x: signed() * 12, y: signed() * 12 }, scale: [0.125, 1, 8][scene % 3], rotation: signed() * Math.PI };
      const center = { x: 40 + random() * 120, y: 40 + random() * 120 }, angle = signed() * Math.PI;
      const dx = Math.cos(angle) * length / 2, dy = Math.sin(angle) * length / 2;
      const xy = [...paper({ x: center.x - dx, y: center.y - dy }, view), ...paper({ x: center.x + dx, y: center.y + dy }, view)];
      viewer.resize(SIZE, SIZE, pixelRatio); viewer.jumpTo(view);
      const input = bundle(line(xy, weight)); viewer.setScene(input);
      const column = input.scene.columns.line_xy, radius = Math.max(1 / pixelRatio, input.scene.columns.unit_weights[0]) / 2;
      const shape = stroke(project(column, 0, view), project(column, 2, view), radius);
      assert(shape.length > 0, "独立 SAT の合成線は f32 化してもゼロ長にならない");
      const context = { scene, line_xy: Array.from(column), view, pixelRatio, weight };
      const near = (along, across) => ({ x: shape.center.x + shape.along.x * along + shape.normal.x * across,
        y: shape.center.y + shape.along.y * along + shape.normal.y * across });
      for (let ordinal = 0; ordinal < QUERIES; ordinal++) {
        let pointDone = false, rectDone = false;
        for (let attempt = 0; attempt < 16 && (!pointDone || !rectDone); attempt++) {
          if (!pointDone) {
            let p;
            switch (ordinal % 6) {
              case 0: p = { x: 4 + random() * 192, y: 4 + random() * 192 }; break;
              case 1: p = near(signed() * Math.min(shape.length * 0.7, 70), signed() * radius * 2); break;
              case 2: p = near(signed() * Math.min(shape.length * 0.35, 35), signed() * radius * 0.6); break;
              case 3: p = near(signed() * Math.min(shape.length * 0.35, 35), (random() < 0.5 ? -1 : 1) * (radius + 0.01 + random() * 3)); break;
              case 4: p = near((random() < 0.5 ? -1 : 1) * (shape.length / 2 + 0.01 + random() * 3), signed() * radius * 0.7); break;
              default: p = near(signed() * Math.min(shape.length * 0.35, 35), (random() < 0.5 ? -1 : 1) * radius * (random() < 0.5 ? 0.8 : 1.2)); break;
            }
            p = { x: clamp(p.x, 4, 196), y: clamp(p.y, 4, 196) };
            pointDone = check("points", rectAt(p), shape, { ...context, ordinal });
          }
          if (!rectDone) {
            let rect;
            if (ordinal % 5 === 0 && shape.length < 100) {
              rect = { min_x: Math.min(...shape.corners.map((p) => p.x)) - 0.05, min_y: Math.min(...shape.corners.map((p) => p.y)) - 0.05,
                max_x: Math.max(...shape.corners.map((p) => p.x)) + 0.05, max_y: Math.max(...shape.corners.map((p) => p.y)) + 0.05 };
            } else {
              let p;
              if (ordinal % 5 === 1) p = { x: 32 + random() * 136, y: 32 + random() * 136 };
              else p = near(signed() * Math.min(shape.length * 0.6, 55), signed() * (radius + 12));
              const x = clamp(p.x, 32, 168), y = clamp(p.y, 32, 168);
              const halfX = 0.0001 + random() * 24, halfY = 0.0001 + random() * 24;
              rect = { min_x: x - halfX, min_y: y - halfY, max_x: x + halfX, max_y: y + halfY };
            }
            rectDone = check("rectangles", rect, shape, { ...context, ordinal });
          }
        }
        if (!pointDone || !rectDone) assert(false, `SAT の境界除外後に問い合わせを作れない: ${JSON.stringify({ seed: SEED, scene, ordinal })}`);
      }
    }
    assert(totals.points === SCENES * QUERIES && totals.rectangles === SCENES * QUERIES, "点と矩形を各 25,600 件照合した");
    assert(totals.pointHits > 1000 && totals.pointHits < totals.points - 1000 && totals.rectangleHits > 1000 && totals.rectangleHits < totals.rectangles - 1000,
      "独立集合は点・矩形とも内側と外側を十分含む");
    return totals;
  } finally { viewer.destroy(); }
}

/** 独立した矩形への距離で、許容距離つき点選択の候補生成も照合する。 */
export function testStrokeTolerance(assert) {
  const seed = 0x808c1c1e;
  let state = seed;
  const random = () => { state = (Math.imul(state, 1664525) + 1013904223) >>> 0; return state / 0x100000000; };
  const viewer = new View2D(document.createElement("canvas"), { width: SIZE, height: SIZE, pixelsPerMillimeter: 1 });
  const totals = { seed, scenes: 128, points: 0, hits: 0, boundaryExcluded: 0 };
  try {
    for (let scene = 0; scene < totals.scenes; scene++) {
      const length = [0.01, 0.24, 1, 8, 64, 2000][scene % 6], pixelRatio = [1, 2, 8][scene % 3];
      const weight = [0, 0.125, 2][Math.floor(scene / 3) % 3];
      const view = { center: { x: 0, y: 0 }, scale: 1, rotation: random() * 6.28 };
      viewer.resize(SIZE, SIZE, pixelRatio); viewer.jumpTo(view);
      const input = bundle(line([-length / 2, 0, length / 2, 0], weight)); viewer.setScene(input);
      const column = input.scene.columns.line_xy, radius = Math.max(1 / pixelRatio, input.scene.columns.unit_weights[0]) / 2;
      const shape = stroke(project(column, 0, view), project(column, 2, view), radius);
      for (let ordinal = 0; ordinal < QUERIES; ordinal++) {
        const tolerance = [0.1, 1, 3, 8][ordinal % 4];
        let done = false;
        for (let attempt = 0; attempt < 16 && !done; attempt++) {
          const along = (random() * 2 - 1) * Math.min(shape.length * 0.75, 50), across = (random() * 2 - 1) * (radius + tolerance * 1.5);
          const point = { x: shape.center.x + shape.along.x * along + shape.normal.x * across,
            y: shape.center.y + shape.along.y * along + shape.normal.y * across };
          const x = point.x - shape.center.x, y = point.y - shape.center.y;
          const du = Math.max(0, Math.abs(x * shape.along.x + y * shape.along.y) - shape.length / 2);
          const dv = Math.max(0, Math.abs(x * shape.normal.x + y * shape.normal.y) - radius);
          const distance = Math.hypot(du, dv);
          if (Math.abs(distance - tolerance) <= GUARD) { totals.boundaryExcluded++; continue; }
          const expected = distance <= tolerance, actual = viewer.hitTest(point, tolerance).length !== 0;
          totals.points++; if (expected) totals.hits++;
          assert(expected === actual, `許容距離つき点選択と独立距離が一致する: ${JSON.stringify({ seed, scene, ordinal, line_xy: Array.from(column), view, pixelRatio, weight, point, tolerance, distance, actual })}`);
          done = true;
        }
        if (!done) assert(false, `距離の境界除外後に問い合わせを作れない: ${JSON.stringify({ seed, scene, ordinal })}`);
      }
    }
    assert(totals.points === 12800 && totals.hits > 1000 && totals.hits < totals.points - 1000, "許容距離つき点選択の内外を 12,800 件照合した");
    return totals;
  } finally { viewer.destroy(); }
}
