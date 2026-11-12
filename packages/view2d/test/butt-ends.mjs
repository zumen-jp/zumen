import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";

function lines(xy, weight) {
  const strings = [...new TextEncoder().encode("space/0/hbutt")];
  const xs = xy.filter((_, i) => i % 2 === 0);
  return { chunk_origins: [0, 0], chunk_bounds: [Math.min(...xs), 0, Math.max(...xs), 0],
    chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [weight], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, strings.length], entity_kinds: [V.entity_kind.line], entity_paths: [0],
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, 255], color_flags: [0], strings,
    line_vertices: Array.from({ length: xy.length / 4 }, (_, i) => i * 2), line_units: Array(xy.length / 4).fill(0), line_xy: xy };
}

/** 遠方の端だけを延ばしても、近い butt の端と隙間は動かない。 */
export function testButtEnds(assert) {
  const canvas = document.createElement("canvas");
  const viewer = new View2D(canvas, { width: 200, height: 200, pixelsPerMillimeter: 1 });
  const totals = { scenes: 0, points: 0, rectangles: 0, whiteGaps: 0, tiny: 0 };
  const check = (p, expected, message) => {
    const point = viewer.hitTest(p, 0), rect = viewer.hitTestRect({ min_x: p.x, min_y: p.y, max_x: p.x, max_y: p.y });
    assert(point.length === expected && rect.length === expected, `${message}: 点=${point.length}/矩形=${rect.length}`);
    totals.points++; totals.rectangles++;
    if (expected) assert(point[0].id === "space/0/hbutt" && rect[0].id === point[0].id, "同じ配置の ID を保つ");
  };
  try {
    for (const extent of [16, 2e9, 2e15, 3e38]) for (const pixelRatio of [1, 1.5, 8]) for (const weight of [0, 2, 8]) for (const edge of [2, Math.fround(5e-7)]) {
      for (const rotation of [0, Math.PI / 6, Math.PI / 4, Math.PI / 2, 5 * Math.PI / 6, Math.PI, 3 * Math.PI / 2]) for (const reverse of [false, true]) {
        const h = Math.fround(extent), xy = [-h, 0, -edge, 0, edge, 0, h, 0], step = Math.min(0.25, edge / 2);
        if (reverse) { for (let i = 0; i < xy.length; i += 4) [xy[i], xy[i + 2]] = [xy[i + 2], xy[i]]; }
        viewer.resize(200, 200, pixelRatio); viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 1, rotation });
        viewer.setScene(bundle(lines(xy, weight))); totals.scenes++;
        const label = `extent=${h}/gap=${edge * 2}/ratio=${pixelRatio}/weight=${weight}/rotation=${rotation}/reverse=${reverse}`;
        check({ x: 100, y: 100 }, 0, `butt の隙間: ${label}`);
        if (edge === 2) {
          const rgba = canvas.getContext("2d").getImageData(100 * pixelRatio, 100 * pixelRatio, 1, 1).data;
          assert(rgba.every((value) => value === 255), `隙間の中心は白: ${label}`); totals.whiteGaps++;
        }
        for (const sign of [-1, 1]) {
          const at = (x) => viewer.toScreen({ x: sign * x, y: 0 });
          check(at(edge), 1, `butt の境界: ${label}`);
          check(at(edge + step), 1, `butt の内側: ${label}`);
          check(at(edge - step), 0, `butt の外側: ${label}`);
          const outside = at(edge - step), half = step / 25;
          assert(viewer.hitTestRect({ min_x: outside.x - half, min_y: outside.y - half, max_x: outside.x + half, max_y: outside.y + half }).length === 0,
            `面積のある矩形も butt の外側を拾わない: ${label}`); totals.rectangles++;
          assert(viewer.hitTest(outside, step * 0.8).length === 0 && viewer.hitTest(outside, step * 1.2).length === 1,
            `許容円は実際の端から測る: ${label}`); totals.points += 2;
        }
      }
    }
    // CSS x=100 近傍より細かい非ゼロ線も、長さによる最小値を設けない。
    for (const length of [1e-16, 1e-12, 1e-8, 0.00001, 0.24]) for (const pixelRatio of [1, 8]) for (const reverse of [false, true]) {
      viewer.resize(Math.max(1e-8, length * 4), 1e-8, pixelRatio); viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 1, rotation: 0 });
      const input = bundle(lines(reverse ? [length, 0, 0, 0] : [0, 0, length, 0], 8));
      viewer.setScene(input);
      const encoded = input.scene.columns.line_xy, end = Math.max(encoded[0], encoded[2]);
      const label = `length=${length}/ratio=${pixelRatio}/reverse=${reverse}`;
      check(viewer.toScreen({ x: end / 2, y: 0 }), 1, `極短線の中点: ${label}`);
      check(viewer.toScreen({ x: 0, y: 0 }), 1, `極短線の始端: ${label}`);
      check(viewer.toScreen({ x: -end / 4, y: 0 }), 0, `極短線の始端の外: ${label}`);
      check(viewer.toScreen({ x: end, y: 0 }), 1, `極短線の終端: ${label}`);
      check(viewer.toScreen({ x: end * 1.25, y: 0 }), 0, `極短線の終端の外: ${label}`);
      totals.tiny++;
    }
  } finally { viewer.destroy(); }
  return totals;
}
