import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";

function textScene({ text = "H", width = 1, oblique = 0, flags = 0, horizontal = V.horizontal.left, vertical = V.vertical.baseline, rotation = 0 } = {}) {
  const content = new TextEncoder().encode(text), id = new TextEncoder().encode("space/0/h1");
  return bundle({
    chunk_origins: [0, 0], chunk_bounds: [0, 0, 0, 0],
    chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    text_strings: [0, content.length], text_xy: [0, 0], text_second: [NaN, NaN],
    text_heights: [30], text_widths: [width], text_rotations: [rotation], text_obliques: [oblique],
    text_boxes: [0], text_aligns: [horizontal, vertical], text_flags: [flags], text_lines: [0],
    text_fonts: [SCENE_NONE], text_units: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [0], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [content.length, id.length], entity_kinds: [V.entity_kind.text], entity_paths: [0],
    color_systems: [V.color_system.none], color_codes: [0], color_files: [0, 0, 0, 0],
    color_rgba: [0, 0, 0, 255], color_flags: [0], strings: [...content, ...id],
  });
}

/** 変換行列をまねず、描かれた画素の範囲で反転と位置合わせの契約を確かめる。 */
export function testFallbackText({ assert, near }) {
  const canvas = document.createElement("canvas");
  const viewer = new View2D(canvas, { width: 200, height: 200 });
  const ctx = canvas.getContext("2d");
  const draw = (options) => {
    const report = viewer.setScene(textScene(options));
    assert(report.drawn.texts === 1 && report.issues.length === 1 && report.issues[0].reason === "browser-text", "位置合わせ後も代替文字として報告する");
    const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    const box = { minX: Infinity, minY: Infinity, maxX: -Infinity, maxY: -Infinity };
    for (let i = 0; i < pixels.length; i += 4) {
      if (pixels[i] >= 128) continue;
      const x = (i / 4) % canvas.width, y = Math.floor(i / 4 / canvas.width);
      box.minX = Math.min(box.minX, x); box.maxX = Math.max(box.maxX, x);
      box.minY = Math.min(box.minY, y); box.maxY = Math.max(box.maxY, y);
    }
    assert(Number.isFinite(box.minX), "代替文字の画素がある");
    return box;
  };
  const failures = [];
  try {
    for (const flags of [0, V.text_flag.upside_down]) for (const width of [1, 1.8]) {
      const normal = draw({ flags, width, oblique: 30 });
      const backward = draw({ flags: flags | V.text_flag.backward, width, oblique: 30 });
      // 画素の中心は x + 0.5。X=100 のまわりの反転なので、対応する端の番号の和は 199。
      near(normal.minX + backward.maxX, 199, "斜体の左端を左右反転する", 1);
      near(normal.maxX + backward.minX, 199, "斜体の右端を左右反転する", 1);
      near(normal.minY, backward.minY, "左右反転で上端を動かさない", 1);
      near(normal.maxY, backward.maxY, "左右反転で下端を動かさない", 1);
    }
  } catch (error) { failures.push(error.message); }
  try {
    for (const text of ["H", "Hg"]) for (const vertical of Object.values(V.vertical)) {
      const box = draw({ text, horizontal: V.horizontal.middle, vertical });
      near((box.minY + box.maxY + 1) / 2, 100, "middle は垂直指定によらず外形を中央に置く", 1);
    }
    for (const flags of [0, V.text_flag.backward, V.text_flag.upside_down, V.text_flag.backward | V.text_flag.upside_down]) {
      const options = { text: "Hg", width: 1.8, oblique: 30, flags };
      const baseline = draw({ ...options, horizontal: V.horizontal.center });
      const middle = draw({ ...options, horizontal: V.horizontal.middle });
      near((middle.minY + middle.maxY + 1) / 2, 100, "反転と斜体を組み合わせても外形を中央に置く", 1);
      near(middle.minX, baseline.minX, "斜体を位置合わせの移動量に掛けない（左端）", 1);
      near(middle.maxX, baseline.maxX, "斜体を位置合わせの移動量に掛けない（右端）", 1);
      for (const vertical of [V.vertical.bottom, V.vertical.middle, V.vertical.top]) {
        const aligned = draw({ ...options, horizontal: V.horizontal.center, vertical });
        near(aligned.minX, baseline.minX, "垂直の位置合わせも斜体で横へずらさない（左端）", 1);
        near(aligned.maxX, baseline.maxX, "垂直の位置合わせも斜体で横へずらさない（右端）", 1);
      }
      const rotated = draw({ ...options, horizontal: V.horizontal.middle, rotation: 90 });
      near((rotated.minX + rotated.maxX + 1) / 2, 100, "外形の中央を置いてから回転する", 1);
    }
  } catch (error) { failures.push(error.message); }
  viewer.destroy();
  assert(failures.length === 0, failures.join("\n"));
}
