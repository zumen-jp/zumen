import { load, readScene, SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle, fixture } from "./fixtures.mjs";
import { testFallbackText } from "./fallback.mjs";
import { testPlacedSelection, testState } from "./state.mjs";
import { testHit } from "./hit.mjs";
import { testSnap } from "./snap.mjs";
import { testTools } from "./tools.mjs";
import { testUnderlays } from "./underlays.mjs";

export async function testView2D(wasmPath, inputs) {
  let assertions = 0;
  const assert = (ok, message) => { assertions++; if (!ok) throw new Error(message); };
  const near = (a, b, message, tolerance = 1e-8) => assert(Math.abs(a - b) <= tolerance, `${message}: ${a} / ${b}`);
  const throws = (fn, message) => { let caught = false; try { fn(); } catch (error) { caught = error instanceof RangeError || error instanceof Error; } assert(caught, message); };
  testFallbackText({ assert, near });
  testState({ assert, throws });
  const largeStrokeRaster = testHit({ assert, near });
  testSnap({ assert, near });
  testTools({ assert, near, throws });
  await testUnderlays({ assert, near, throws });
  const canvas = document.createElement("canvas");
  document.body.append(canvas);
  const viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio: 2, foreground: "#123456", pixelsPerMillimeter: 2 });
  const ctx = canvas.getContext("2d");
  const rgb = (x, y) => Array.from(ctx.getImageData(Math.floor(x * viewer.getSize().pixelRatio), Math.floor(y * viewer.getSize().pixelRatio), 1, 1).data).slice(0, 3);
  const pixel = (x, y, expected, message) => assert(rgb(x, y).every((v, i) => Math.abs(v - expected[i]) <= 4), `${message}: ${rgb(x, y)} / ${expected}`);
  const source = fixture();
  const input = bundle(source, { extent: { min_x: -45, min_y: -45, max_x: 45, max_y: 45 } });
  viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 2, rotation: 0 });
  const report = viewer.setScene(input);
  assert(report.drawn.lines === 1 && report.drawn.faces === 2 && report.drawn.texts === 2 && report.drawn.glyphs === 1, "種類ごとの描画数");
  assert(report.issues.filter((i) => i.reason === "browser-text").length === 1, "字形がある文字は代替しない");
  assert(report.issues.some((i) => i.reason === "resource-unresolved" && i.disposition === "omitted" && i.element.id === "space/0/h5"), "画像の未解決を要素付きで返す");
  pixel(100, 100, [255, 255, 255], "外側だけの切り抜き");
  pixel(130, 100, [255, 0, 0], "面の色");
  pixel(150, 50, [255, 255, 255], "面の穴");
  pixel(100, 70, [0, 0, 255], "同じ塊の後の単位の線を面より前に描く");
  pixel(100, 73, [255, 0, 0], "用紙 mm の線幅");
  pixel(40, 160, [0, 180, 0], "切り抜きの組の共通部分");
  pixel(54, 150, [255, 0, 0], "共通部分の外は描かない");
  pixel(42, 48, [0, 0, 255], "字形の輪郭");
  pixel(48, 42, [255, 0, 0], "字形の非零規則の穴");
  pixel(105, 25, [255, 0, 0], "字形付きの元文字列を重ねない");
  pixel(170, 30, [0x12, 0x34, 0x56], "背景に対する前景色");
  assert(!report.issues.some((i) => i.primitive.kind === "point" && i.primitive.index === 1), "前景色を未解決と報告しない");
  const fallbackPixels = ctx.getImageData(60, 308, 160, 20).data;
  assert(fallbackPixels.some((v, i) => i % 4 === 2 && v > 200 && fallbackPixels[i - 2] < 100), "代替文字を描く");
  const twoChunks = fixture();
  twoChunks.chunk_origins = [0, 0, 0, 0]; twoChunks.chunk_bounds.push(...twoChunks.chunk_bounds);
  twoChunks.chunk_lines = [0, 1]; twoChunks.chunk_faces = [0, 1]; twoChunks.chunk_texts = [0, 2];
  twoChunks.chunk_points = [0, 2]; twoChunks.chunk_images = [0, 1];
  twoChunks.unit_clips[6] = SCENE_NONE;
  viewer.setScene(bundle(twoChunks));
  pixel(100, 70, [0, 180, 0], "後の塊の面を前の塊の線より前に描く");
  const alpha = fixture(); alpha.color_rgba[3] = 128;
  viewer.setScene(bundle(alpha));
  pixel(130, 100, [255, 127, 127], "RGBA の不透明度");
  viewer.setScene(input);

  const original = viewer.getView();
  const p = { x: 7.25, y: -9.75 };
  viewer.rotateBy(Math.PI / 3);
  viewer.panBy(9, -12);
  viewer.zoomBy(1.75, { x: 35, y: 71 });
  const back = viewer.toScene(viewer.toScreen(p));
  near(back.x, p.x, "回転後の X の逆変換"); near(back.y, p.y, "回転後の Y の逆変換");
  const anchor = { x: 45, y: 30 }, before = viewer.toScene(anchor);
  viewer.rotateBy(-Math.PI / 5, anchor); viewer.zoomBy(0.8, anchor);
  const after = viewer.toScene(anchor);
  near(before.x, after.x, "固定点を保つ X"); near(before.y, after.y, "固定点を保つ Y");
  const oldScreen = viewer.toScreen(p); viewer.panBy(8, 11); const newScreen = viewer.toScreen(p);
  near(newScreen.x - oldScreen.x, 8, "画面上のパン X"); near(newScreen.y - oldScreen.y, 11, "画面上のパン Y");
  viewer.fitBounds({ min_x: -10, min_y: -20, max_x: 30, max_y: 40 }, 10);
  for (const x of [-10, 30]) for (const y of [-20, 40]) {
    const s = viewer.toScreen({ x, y }); assert(s.x >= 9.99 && s.x <= 190.01 && s.y >= 9.99 && s.y <= 190.01, "回転を含む fitBounds");
  }
  viewer.jumpTo(original);
  viewer.resize(300, 150, 1.5);
  assert(canvas.width === 450 && canvas.height === 225 && viewer.getView().scale === original.scale, "サイズと画素比");
  near(viewer.toScreen({ x: 0, y: 0 }).x, 150, "CSS px の中心");
  viewer.resize(200, 200, 2);

  // 大きい原点で、原点に相対座標を足してからカメラを引く実装を検出する。
  const large = fixture(1e16);
  large.line_xy = [0.25, 0, 0.25, 10];
  large.unit_weights[1] = 0.5;
  const huge = bundle(large);
  viewer.setScene(huge); viewer.jumpTo({ center: { x: 1e16, y: 1e16 }, scale: 16, rotation: 0 });
  pixel(104, 50, [0, 0, 255], "塊の相対座標の下位桁を保つ");
  viewer.setScene(input); viewer.jumpTo(original);

  // シーン・応答・文書の一括差し替え。呼び出し側のメタデータの変更を持ち込まない。
  const replacement = bundle(fixture(), { from: "sheet/7", view: 2, unit: "millimeter", tolerance: 0.2 });
  replacement.document = { doc: "second", format: "jww", hash: "v2:22222222222222222222222222222222" };
  viewer.setScene(replacement);
  replacement.result.unit = "meter"; replacement.document.hash = "changed";
  const current = viewer.getScene();
  assert(current.result.from === "sheet/7" && current.result.view === 2 && current.result.unit === "millimeter" && current.result.tolerance === 0.2 && current.document.doc === "second" && current.document.hash !== "changed", "メタデータと文書を一緒に保持");
  const malformed = [
    (c) => { c.line_units[0] = 999; }, (c) => { c.unit_entities[0] = 999; },
    (c) => { c.unit_clips[0] = 999; }, (c) => { c.glyph_shapes[0] = 999; },
    (c) => { c.line_xy[0] = NaN; }, (c) => { c.clip_xy[0] = Infinity; },
    (c) => { c.text_second[0] = Infinity; }, (c) => { c.text_second[1] = 0; },
    (c) => { c.text_aligns[0] = V.horizontal.fit; },
    (c) => { c.polygon_vertices[1] = 999; }, (c) => { c.face_rings[1] = 0xFFFFFFFF; },
    (c) => { c.text_strings[0] = 999999; }, (c) => { c.chunk_lines = []; },
    (c) => { c.text_xy.pop(); }, (c) => { c.unit_weights[0] = -1; },
  ];
  for (const mutate of malformed) {
    const c = fixture(); mutate(c);
    throws(() => viewer.setScene(bundle(c)), "不正な列を拒否");
    assert(viewer.getScene() === current, "拒否後に前の一組を保つ");
  }
  const wrongCount = bundle(fixture()); wrongCount.result.counts.lines++;
  throws(() => viewer.setScene(wrongCount), "別の応答の数を拒否");
  const wrongExtent = bundle(fixture(), { extent: null });
  throws(() => viewer.setScene(wrongExtent), "不正な範囲のメタデータを拒否");
  const missingHash = bundle(fixture()); delete missingHash.document.hash;
  throws(() => viewer.setScene(missingHash), "文書の hash が必要");
  throws(() => viewer.jumpTo({ ...original, scale: NaN }), "不正なカメラ");
  throws(() => viewer.zoomBy(0), "倍率 0");
  throws(() => viewer.resize(100000, 100000), "巨大な Canvas");
  throws(() => viewer.fitBounds({ min_x: 1, max_x: 0, min_y: 0, max_y: 1 }), "逆転した範囲");
  assert(viewer.getScene() === current, "不正な操作でシーンを変えない");
  const tiny = new View2D(document.createElement("canvas"), { width: 100, height: 100, limits: { columnValues: 100 } });
  throws(() => tiny.setScene(input), "列の上限"); tiny.destroy();
  const limited = new View2D(document.createElement("canvas"), { width: 100, height: 100, limits: { work: 2000 } });
  const many = fixture();
  many.line_vertices = Array.from({ length: 200 }, (_, i) => i * 2);
  many.line_xy = Array.from({ length: 800 }, (_, i) => i % 2);
  many.line_units = Array(200).fill(0);
  throws(() => limited.setScene(bundle(many)), "共有切り抜きの反復の上限"); limited.destroy();
  const manyGlyphs = fixture();
  manyGlyphs.text_glyphs = [0, 100]; manyGlyphs.glyph_shapes = Array(100).fill(0);
  manyGlyphs.glyph_axes = Array.from({ length: 400 }, (_, i) => i % 4 === 0 || i % 4 === 3 ? 1 : 0);
  manyGlyphs.glyph_xy = Array(200).fill(0); manyGlyphs.outline_vertices = [0];
  manyGlyphs.outline_xy = Array.from({ length: 2000 }, (_, i) => i % 3);
  const glyphLimited = new View2D(document.createElement("canvas"), { width: 100, height: 100, limits: { work: 10000 } });
  throws(() => glyphLimited.setScene(bundle(manyGlyphs)), "共有字形の反復の上限"); glyphLimited.destroy();
  if (typeof OffscreenCanvas !== "undefined") {
    const offscreen = new View2D(new OffscreenCanvas(100, 100));
    assert(offscreen.setScene(input).drawn.glyphs === 1, "OffscreenCanvas がある環境でも描く");
    offscreen.destroy();
  }
  const repeated = fixture();
  repeated.strings.push(...Array(500).fill(65));
  for (let i = 1; i < repeated.entity_ids.length; i += 2) repeated.entity_ids[i] = 500;
  const stringsLimited = new View2D(document.createElement("canvas"), { width: 100, height: 100, limits: { work: 3000 } });
  throws(() => stringsLimited.setScene(bundle(repeated)), "同じ文字列の参照を繰り返す復号の上限"); stringsLimited.destroy();
  const issueLimited = new View2D(document.createElement("canvas"), { width: 100, height: 100, limits: { issues: 1 } });
  throws(() => issueLimited.setScene(input), "報告の上限"); issueLimited.destroy();
  const oldColumns = fixture();
  for (const key of ["text_glyphs", "glyph_shapes", "glyph_axes", "glyph_xy", "shape_rings", "outline_vertices", "outline_xy"]) oldColumns[key] = [];
  assert(viewer.setScene(bundle(oldColumns)).issues.filter((i) => i.reason === "browser-text").length === 2, "字形の列がないシーンを代替する");
  assert(viewer.setScene(bundle({})).drawn.lines === 0, "空のシーン");
  const unsupported = fixture(); unsupported.text_flags[1] = V.text_flag.formatted;
  unsupported.point_markers[0] = unsupported.entity_ids[0]; unsupported.point_markers[1] = unsupported.entity_ids[1];
  unsupported.color_flags[1] = V.color_flag.unresolved;
  const unavailable = viewer.setScene(bundle(unsupported));
  assert(unavailable.issues.some((i) => i.reason === "text-layout-unsupported" && i.disposition === "omitted"), "未対応の文字配置を報告");
  assert(unavailable.issues.some((i) => i.reason === "marker-unresolved" && i.disposition === "fallback"), "既定義マーカーを代替して報告");
  assert(unavailable.issues.some((i) => i.reason === "color-unresolved" && i.element.path[0]?.column === 2 && i.element.path[0]?.row === 3), "色の代替に配置の道筋を残す");

  // 実際の wasm の読み取りと scene の応答を、そのまま受け取る。
  const zumen = await load(fetch(wasmPath));
  testPlacedSelection(zumen, { assert, throws });
  const formats = [];
  for (const source of inputs) {
    const session = zumen.open();
    try {
      const bytes = source.hex ? Uint8Array.from(source.hex.match(/../g), (s) => parseInt(s, 16)) : new TextEncoder().encode(source.text);
      session.push(source.name, bytes);
      const opened = session.call({ v: 1, op: "open", input: source.name }).result;
      const drawing = session.call({ v: 1, op: "drawing", doc: source.name }).result;
      const from = drawing.spaces[0]?.id ?? drawing.sheets[0]?.id;
      const result = session.call({ v: 1, op: "scene", doc: source.name, from, tolerance: 0.05 }).result;
      const scene = readScene(session.take(result.blob));
      const rendered = viewer.setScene({ scene, result, document: { doc: source.name, format: opened.format, hash: drawing.hash } });
      viewer.fit();
      assert(rendered.drawn.lines > 0, `${opened.format} の線を描く`);
      const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      assert(pixels.some((value, i) => i % 4 !== 3 && value < 200), `${opened.format} の画素`);
      formats.push(opened.format);
    } finally { session.close(); }
  }
  assert(formats.join(",") === "dxf,jww,sxf", "3 形式をコアから描く");
  viewer.setScene(null); assert(viewer.getScene() === null && viewer.getReport().issues.length === 0, "消去で前の報告を捨てる");
  viewer.destroy(); viewer.destroy();
  assert(ctx.getImageData(0, 0, 1, 1).data[3] === 0, "破棄で画面を消す");
  throws(() => viewer.render(), "破棄後の描画"); throws(() => viewer.setScene(input), "破棄後の差し替え");
  return { assertions, formats, largeStrokeRaster };
}
