import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle } from "./fixtures.mjs";
import { imageUnderlayRenderer } from "../example/image.js";

export function imageScene({ count = 1, origin = 0, at = [-40, -20], edges = [80, 0, 0, 40], frame = V.image_frame.corners, kind = V.image_kind.image, clips = [], pages = [] } = {}) {
  const bytes = [], str = (value) => { const encoded = new TextEncoder().encode(value), start = bytes.length; bytes.push(...encoded); return [start, encoded.length]; };
  const c = { chunk_origins: [origin, origin], chunk_bounds: [-100, -100, 100, 100], chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    entity_ids: [], entity_kinds: [], entity_paths: [], unit_entities: [], unit_colors: [], unit_weights: [], unit_clips: [], unit_layers: [],
    image_names: [], image_pages: [], image_xy: [], image_edges: [], image_kinds: [], image_frames: [], image_units: [],
    layer_ids: str("images"), clip_polygons: clips.length ? [0] : [], polygon_vertices: [], polygon_flags: [], clip_xy: [] };
  for (const clip of clips) { c.polygon_vertices.push(c.clip_xy.length / 2); c.polygon_flags.push(clip.inverted ? 1 : 0); c.clip_xy.push(...clip.xy); }
  c.step_ids = str("space/0/hinsert"); c.step_cells = [2, 3];
  for (let i = 0; i < count; i++) {
    c.entity_ids.push(...str(`space/0/h${i}`)); c.entity_kinds.push(V.entity_kind.underlay); c.entity_paths.push(0);
    c.unit_entities.push(i); c.unit_colors.push(SCENE_NONE); c.unit_weights.push(0); c.unit_clips.push(clips.length ? 0 : SCENE_NONE); c.unit_layers.push(0);
    c.image_names.push(...str(`https://example.invalid/do-not-fetch-${i}`)); c.image_pages.push(...str(pages[i] ?? ""));
    c.image_xy.push(...at); c.image_edges.push(...edges); c.image_kinds.push(kind); c.image_frames.push(frame); c.image_units.push(i);
  }
  c.strings = bytes;
  return c;
}

const flush = async () => { for (let i = 0; i < 32; i++) await Promise.resolve(); };
const deferred = () => { let resolve, reject; const promise = new Promise((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; };
const canvasOf = (width, height) => { const canvas = document.createElement("canvas"); canvas.width = width; canvas.height = height; return canvas; };
function raster(request, color = "#ff0000", disposed = () => {}) {
  const source = canvasOf(request.width, request.height), ctx = source.getContext("2d");
  ctx.fillStyle = color; ctx.fillRect(0, 0, source.width, source.height);
  return { source, dispose() { disposed(); source.width = 0; source.height = 0; } };
}
function quadrants(request) {
  const value = raster(request), ctx = value.source.getContext("2d"), r = request.region;
  ctx.clearRect(0, 0, request.width, request.height);
  ctx.setTransform(request.width / (r.max_x - r.min_x), 0, 0, -request.height / (r.max_y - r.min_y),
    -r.min_x * request.width / (r.max_x - r.min_x), r.max_y * request.height / (r.max_y - r.min_y));
  for (const [x, y, color] of [[0, 0, "red"], [50, 0, "lime"], [0, 25, "blue"], [50, 25, "yellow"]]) { ctx.fillStyle = color; ctx.fillRect(x, y, 50, 25); }
  return value;
}

export async function testUnderlays({ assert, near, throws }) {
  await testUnderlayNotifications(assert);
  await testReplacedRelease(assert);
  testStableReport(assert);
  const canvas = canvasOf(200, 200), viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio: 2 });
  const ctx = canvas.getContext("2d");
  const pixel = (x, y, expected, message) => {
    const ratio = viewer.getSize().pixelRatio, actual = ctx.getImageData(Math.floor(x * ratio), Math.floor(y * ratio), 1, 1).data;
    assert(expected.every((value, i) => Math.abs(actual[i] - value) <= 3), `${message}: ${actual}`);
  };
  const state = () => viewer.getState().underlays[0];
  viewer.setScene(bundle(imageScene()));
  assert(state().status === "resource-unresolved" && viewer.getReport().issues[0].element.id === "space/0/h0", "解決関数がなければ ID 付きの未描画を保つ");
  const requests = [], resolves = [], released = [], events = [];
  const renderer = (request) => {
    resolves.push(request);
    assert(Object.isFrozen(request) && Object.isFrozen(request.limits) && request.element.path[0].column === 2, "要求を凍結し full path を渡す");
    throws(() => viewer.setScene(null), "解決関数からの変更再入を断る");
    return { width: 100, height: 50, render(request) { requests.push(request); return quadrants(request); }, dispose() { released.push(request.index); } };
  };
  const off = viewer.on("underlay", (event) => { events.push(event.state); });
  viewer.setUnderlayRenderer(renderer);
  assert(resolves.length === 0 && state().status === "resource-pending", "解決関数は同期通知を抜けてから呼ぶ");
  await flush();
  assert(state().status === "ready" && viewer.getReport().drawn.images === 1 && viewer.getReport().issues.length === 0, "成功した画像を報告と状態へ反映する");
  assert(requests.length === 1 && requests[0].width === 160 && requests[0].height === 80, "枠の辺と画素比から要求サイズを決める");
  assert(events[events.length - 1] === viewer.getState(), "非同期の描画後に最新の状態を通知する");
  const beforeInvalid = viewer.getState(), invalid = imageScene(); invalid.image_edges[0] = NaN;
  throws(() => viewer.setScene(bundle(invalid)), "不正な次のシーンを断る");
  assert(viewer.getState() === beforeInvalid && !resolves[0].signal.aborted && released.length === 0, "失敗した差し替えで現在の画像と資源を保つ");
  pixel(80, 110, [255, 0, 0], "画像の左下を置く"); pixel(120, 110, [0, 255, 0], "画像の右下を置く");
  pixel(80, 90, [0, 0, 255], "画像の左上を置く"); pixel(120, 90, [255, 255, 0], "画像の右上を置く");
  const readyReport = viewer.getReport();
  viewer.invalidate(); await flush();
  assert(requests.length === 1 && viewer.getReport() === readyReport, "同じ表示の再描画で資源と描画済みの報告を再利用する");
  viewer.resize(200, 200, 1); await flush(); assert(requests[1].width === 80 && requests[1].height === 40, "画素比の変更で画素を作り直す");
  viewer.rotateBy(Math.PI / 2); await flush();
  pixel(110, 120, [255, 0, 0], "回転後の左下の色"); pixel(90, 80, [255, 255, 0], "回転後の右上の色");
  viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 1, rotation: 0 });
  viewer.setScene(bundle(imageScene({ frame: V.image_frame.unit, at: [-50, -25], edges: [1, 0, 0, 1], pages: ["named-page"], kind: V.image_kind.pdf })));
  await flush();
  assert(resolves[resolves.length - 1].page === "named-page" && resolves[resolves.length - 1].kind === "pdf", "PDF のページ名を番号へ推測せず渡す");
  assert(requests[requests.length - 1].width === 100 && requests[requests.length - 1].height === 50, "unit は内容の寸法を辺に掛ける");
  pixel(60, 120, [255, 0, 0], "unit の全体の左下");
  viewer.setScene(bundle(imageScene({ origin: 1e16, at: [.25, .25], edges: [2, 0, 0, 2] })));
  viewer.jumpTo({ center: { x: 1e16, y: 1e16 }, scale: 16, rotation: 0 }); await flush();
  pixel(108, 92, [255, 0, 0], "大きな原点の画像でも相対座標を保つ");
  viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 1, rotation: 0 });
  viewer.setScene(bundle(imageScene({ at: [-40, -20], edges: [-60, 10, 20, 50] }))); await flush();
  pixel(52, 105, [255, 0, 0], "反転と傾斜のある枠を保つ");

  viewer.setScene(bundle(imageScene({ clips: [{ xy: [-30, -15, 30, -15, 30, 15, -30, 15] }, { xy: [-5, -5, 5, -5, 5, 5, -5, 5], inverted: true }] }))); await flush();
  pixel(100, 100, [255, 255, 255], "画像の反転した切り抜き"); pixel(140, 100, [255, 255, 255], "画像の内側を残す切り抜きの外"); pixel(80, 110, [255, 0, 0], "画像の切り抜きの共通部分");
  const clippedRequest = requests[requests.length - 1];
  assert(clippedRequest.region.min_x > 0 && clippedRequest.width === 60, "内側の切り抜きの外形まで画素要求を絞る");
  const beforeHidden = resolves.length;
  viewer.setHiddenLayers(["images"]); await flush();
  assert(state().status === "resource-outside-view" && resolves.length === beforeHidden && viewer.getReport().drawn.images === 0, "非表示レイヤは資源を解放する");
  viewer.setHiddenLayers([]); await flush(); assert(resolves.length === beforeHidden + 1, "再表示で資源を解決する");
  viewer.setScene(bundle(imageScene({ at: [-500, -20], edges: [1000, 0, 0, 40] }))); await flush();
  const cropped = requests[requests.length - 1];
  assert(cropped.width === 200 && cropped.region.min_x === 40 && cropped.region.max_x === 60, "画面外の画素を要求しない");
  pixel(60, 110, [255, 0, 0], "部分画像を内容の元の位置へ戻す");
  viewer.panBy(1000, 0); await flush(); assert(state().status === "resource-outside-view", "画面から外れた画像を解放する");
  viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 1, rotation: 0 }); await flush();

  // 描画順は、画像を最背面へ動かさず、同じ単位と塊の順へ入れる。
  const ordered = imageScene();
  ordered.line_vertices = [0]; ordered.line_units = [0]; ordered.line_xy = [-35, 0, 35, 0];
  ordered.face_rings = [0]; ordered.face_units = [0]; ordered.ring_vertices = [0]; ordered.face_xy = [-50, -30, 50, -30, 50, 30, -50, 30];
  viewer.setScene(bundle(ordered)); await flush(); pixel(80, 101, [255, 0, 0], "同じ単位の線と面の後に画像を描く");
  assert(viewer.getReport().drawn.images === 1, "画像の描画数を保つ");
  off(); viewer.destroy(); await flush(); assert(released.length === resolves.length, "保持したすべての資源を解放する");
  await testUnderlayLifetime({ assert, throws });
  await testUnderlayLimits({ assert, throws, near });
  const adaptedCanvas = canvasOf(200, 200), adapted = new View2D(adaptedCanvas, { width: 200, height: 200 });
  let decoded, closed = 0;
  adapted.setUnderlayRenderer(imageUnderlayRenderer(() => {
    decoded = quadrants({ width: 100, height: 50, region: { min_x: 0, min_y: 0, max_x: 100, max_y: 50 } });
    return { source: decoded.source, dispose() { closed++; decoded.dispose(); } };
  }, canvasOf));
  adapted.setScene(bundle(imageScene({ at: [-500, -20], edges: [1000, 0, 0, 40] }))); await flush();
  const red = adaptedCanvas.getContext("2d").getImageData(60, 110, 1, 1).data;
  assert(red[0] === 255 && red[1] === 0 && red[2] === 0, "利用側の画像アダプターで部分画像を切り出す");
  adapted.destroy(); await flush(); assert(closed === 1 && decoded.source.width === 0, "画像アダプターが元の画像を解放する");
  if (typeof createImageBitmap === "function") {
    const bitmapCanvas = canvasOf(200, 200), bitmaps = [], bitmapViewer = new View2D(bitmapCanvas, { width: 200, height: 200 });
    bitmapViewer.setUnderlayRenderer(() => ({ width: 100, height: 50, async render(request) {
      const pixels = raster(request), source = await createImageBitmap(pixels.source); pixels.dispose(); bitmaps.push(source);
      return { source, dispose() { source.close(); } };
    }, dispose() {} }));
    bitmapViewer.setScene(bundle(imageScene()));
    await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => { off(); reject(new Error("ImageBitmap が完了しませんでした")); }, 5000);
      const off = bitmapViewer.on("underlay", ({ state }) => { if (state.underlays[0].status !== "ready") return; clearTimeout(timeout); off(); resolve(); });
    });
    assert(bitmapViewer.getReport().drawn.images === 1, "ImageBitmap の結果も合成する");
    bitmapViewer.destroy(); assert(bitmaps.length === 1 && bitmaps[0].width === 0, "破棄で ImageBitmap.close を呼ぶ");
  }
}

async function testUnderlayNotifications(assert) {
  const viewer = new View2D(canvasOf(200, 200), { width: 200, height: 200 });
  const calls = [];
  viewer.subscribe((state) => calls.push({ type: "all", state }));
  for (const type of ["scene", "layers", "view", "underlay"]) viewer.on(type, (event) => calls.push(event));
  const check = (types, status) => {
    const state = viewer.getState();
    assert(calls.map((event) => event.type).join() === types && calls.every((event) => event.state === state)
      && state.underlays[0]?.status === status, `同期通知 ${types} が同じ状態 ${status} を 1 回ずつ受け取る`);
    calls.length = 0;
  };
  viewer.setScene(bundle(imageScene()));
  check("all,scene,underlay", "resource-unresolved");
  viewer.setUnderlayRenderer(() => ({ width: 8, height: 4, render: raster, dispose() {} }));
  check("all,underlay", "resource-pending");
  await flush();
  assert(calls[calls.length - 1].state.underlays[0].status === "ready", "非同期の描画完了も画像の購読へ届く");
  calls.length = 0;
  viewer.setHiddenLayers(["images"]);
  check("all,layers,underlay", "resource-outside-view");
  viewer.invalidate(); await flush();
  check("", "resource-outside-view");
  viewer.setHiddenLayers([]);
  check("all,layers,underlay", "resource-pending");
  await flush(); calls.length = 0;
  viewer.panBy(1000, 0);
  check("all,view,underlay", "resource-outside-view");
  viewer.setScene(null);
  check("all,scene,underlay", undefined);
  viewer.destroy();
}

async function testReplacedRelease(assert) {
  for (const change of ["layers", "view", "raster", "waiting"]) {
    const canvas = canvasOf(200, 200), viewer = new View2D(canvas, { width: 200, height: 200,
      limits: { underlayResources: change === "waiting" ? 1 : 2 } });
    const releasing = deferred(), closed = [], freed = [];
    let resources = 0, images = 0;
    viewer.setUnderlayRenderer(() => {
      const resource = ++resources;
      return {
        width: 100, height: 50,
        render(request) {
          const n = ++images, image = raster(request, n === 1 ? "red" : "blue");
          return { source: image.source, dispose() {
            image.dispose(); freed.push(n);
            if (change === "raster" && n === 1) return releasing.promise;
          } };
        },
        dispose() {
          closed.push(resource);
          if (change !== "raster" && resource === 1) return releasing.promise;
        },
      };
    });
    viewer.setScene(bundle(imageScene())); await flush();
    if (change === "layers" || change === "waiting") { viewer.setHiddenLayers(["images"]); viewer.setHiddenLayers([]); }
    else if (change === "view") { viewer.panBy(1000, 0); viewer.panBy(-1000, 0); }
    else viewer.resize(200, 200, 2);
    await flush();
    if (change === "waiting") assert(images === 1 && viewer.getState().underlays[0].status === "resource-limit", "再表示が旧資源の解放を待つ");
    else assert(images === 2 && viewer.getState().underlays[0].status === "ready", `${change} で旧資源の解放中に新しい画素を描く`);
    releasing.reject(new Error("old release")); await flush();
    const center = canvas.width / 2, pixel = canvas.getContext("2d").getImageData(center, center, 1, 1).data;
    assert(viewer.getState().underlays[0].status === "ready" && viewer.getReport().drawn.images === 1
      && pixel[0] === 0 && pixel[2] === 255 && freed.join() === "1", `${change} の旧解放失敗で新しい画素を消さない`);
    viewer.destroy(); await flush();
    assert(freed.join() === "1,2" && closed.join() === (change === "raster" ? "1" : "1,2"), `${change} の各世代を 1 回ずつ解放する`);
  }
}

function testStableReport(assert) {
  const c = imageScene({ count: 0 }), encode = new TextEncoder();
  c.line_units = []; c.line_xy = []; c.line_vertices = [];
  for (let i = 0; i < 1000; i++) {
    const id = encode.encode(`space/0/h${i}`);
    c.entity_ids.push(c.strings.length, id.length); c.strings.push(...id);
    c.entity_paths.push(0); c.entity_kinds.push(V.entity_kind.line); c.unit_entities.push(i);
    c.unit_colors.push(SCENE_NONE); c.unit_weights.push(0); c.unit_layers.push(0); c.unit_clips.push(SCENE_NONE);
    c.line_units.push(i); c.line_vertices.push(i * 2); c.line_xy.push(-10, 0, 10, 0);
  }
  const viewer = new View2D(canvasOf(200, 200), { width: 200, height: 200 });
  const report = viewer.setScene(bundle(c));
  assert(report.drawn.images === 0 && report.issues.length === 1000, "画像がなく未解決色の線だけの報告を作る");
  const decode = TextDecoder.prototype.decode;
  let decoded = 0;
  try {
    TextDecoder.prototype.decode = function (...args) { decoded++; return decode.apply(this, args); };
    const after = viewer.invalidate();
    assert(decoded === 0 && after === report, `同じ表示の再描画で報告と ID を再利用する（復号 ${decoded} 回）`);
    assert(viewer.panBy(1, 0) === report && decoded === 0, "パンでも同じ報告を再利用する");
    viewer.select([report.issues[999].element]);
    assert(viewer.getState().selection[0] === report.issues[999].element, "報告と選択が同じ配置参照を使う");
    assert(viewer.setHiddenLayers(["images"]).issues.length === 0, "レイヤ変更時には報告を更新する");
    const visible = viewer.setHiddenLayers([]);
    assert(visible.issues.length === 1000 && visible.issues[999].element === report.issues[999].element && decoded === 0,
      "再表示の報告でもシーンの ID と配置の参照を使う");
  } finally { TextDecoder.prototype.decode = decode; viewer.destroy(); }
}

async function testUnderlayLifetime({ assert, throws }) {
  const orderedCanvas = canvasOf(200, 200), ordered = new View2D(orderedCanvas, { width: 200, height: 200 }), oldPixels = deferred();
  let oldRequest, oldClosed = 0, oldImageClosed = 0;
  ordered.setUnderlayRenderer((request) => ({ width: 100, height: 50,
    render(part) { if (request.page === "old") { oldRequest = part; return oldPixels.promise; } return raster(part, "blue"); },
    dispose() { if (request.page === "old") oldClosed++; },
  }));
  ordered.setScene(bundle(imageScene({ pages: ["old"] }))); await flush();
  const replacement = bundle(imageScene({ pages: ["new"] })); replacement.document.hash = "v2:new";
  ordered.setScene(replacement); await flush();
  assert(oldRequest.signal.aborted && ordered.getState().underlays[0].document.hash === "v2:new", "同じ ID の別の文書の版へ差し替える");
  oldPixels.resolve(raster(oldRequest, "red", () => oldImageClosed++)); await flush();
  const blueAfterOld = orderedCanvas.getContext("2d").getImageData(100, 100, 1, 1).data;
  assert(blueAfterOld[0] === 0 && blueAfterOld[2] === 255 && oldClosed === 1 && oldImageClosed === 1, "新しい結果の後で届いた古い画素を描かず、旧資源だけ解放する");
  ordered.destroy();
  for (const change of ["scene", "renderer", "refresh", "cancel", "destroy", "hidden", "view"]) {
    const canvas = canvasOf(200, 200), viewer = new View2D(canvas, { width: 200, height: 200 }), answer = deferred(), disposed = [], requests = [];
    viewer.setUnderlayRenderer((request) => { requests.push(request); return answer.promise; }); viewer.setScene(bundle(imageScene())); await flush();
    const changeNow = () => {
      if (change === "scene") viewer.setScene(bundle(imageScene({ pages: ["different"] })));
      else if (change === "renderer") viewer.setUnderlayRenderer(null);
      else if (change === "refresh") viewer.refreshUnderlays();
      else if (change === "cancel") viewer.cancelUnderlays();
      else if (change === "destroy") viewer.destroy();
      else if (change === "hidden") viewer.setHiddenLayers(["images"]);
      else viewer.panBy(1000, 0);
    };
    changeNow();
    assert(requests[0].signal.aborted, `${change} で解決中の signal を中断する`);
    let rendered = 0;
    answer.resolve({ width: 100, height: 50, render(request) { rendered++; return raster(request, "blue"); }, dispose() { disposed.push("resource"); } }); await flush();
    assert(disposed.length >= 1, `${change} の後の解決結果も解放する`);
    if (!["scene", "refresh"].includes(change)) assert(rendered === 0, `${change} の古い資源から画像を作らない`);
    viewer.destroy(); await flush();
  }
  const canvas = canvasOf(200, 200), viewer = new View2D(canvas, { width: 200, height: 200 }), pending = [], disposed = [], signals = [];
  viewer.setUnderlayRenderer(() => ({ width: 100, height: 50, render(request) { const answer = deferred(); pending.push({ request, answer }); signals.push(request.signal); return answer.promise; }, dispose() { disposed.push("resource"); } }));
  viewer.setScene(bundle(imageScene())); await flush();
  viewer.resize(200, 200, 2); await flush();
  assert(signals[0].aborted && pending.length === 1, "画素比を変えると古い描画を中断し、同じ資源の完了を待つ");
  pending[0].answer.resolve(raster(pending[0].request, "red", () => disposed.push("old-raster"))); await flush();
  assert(disposed[0] === "old-raster" && pending.length === 2 && viewer.getReport().drawn.images === 0, "古い画素を描かず新しい要求を始める");
  pending[1].answer.resolve(raster(pending[1].request, "blue", () => disposed.push("new-raster"))); await flush();
  assert(viewer.getReport().drawn.images === 1, "現行の非同期結果だけを描く");
  viewer.refreshUnderlays(); await flush();
  assert(disposed.join() === "old-raster,new-raster,resource", "再読込は画像から先に資源を解放する");
  const current = pending[pending.length - 1]; viewer.destroy();
  assert(signals[signals.length - 1].aborted && disposed.length === 3, "破棄時は描画中の資源を完了まで保持する");
  current.answer.resolve(raster(current.request, "lime", () => disposed.push("late-raster"))); await flush();
  assert(disposed.slice(-2).join() === "late-raster,resource", "破棄後に届いた画素を解放してから資源を解放する");

  const failed = new View2D(canvasOf(200, 200), { width: 200, height: 200 });
  const errors = [];
  failed.on("underlay", ({ state }) => errors.push(state.underlays[0]?.status));
  failed.setUnderlayRenderer(() => { throw new Error("expected"); }); failed.setScene(bundle(imageScene())); await flush();
  assert(errors.includes("resource-resolve-failed") && failed.getReport().issues[0].reason === "resource-resolve-failed", "解決の例外を ID 付きで通知する");
  let times = 0, released = 0;
  failed.setUnderlayRenderer(() => ({ width: 100, height: 50, render() { times++; return Promise.reject(new Error("expected")); }, dispose() { released++; } })); await flush();
  assert(times === 1 && released === 1 && failed.getState().underlays[0].status === "resource-render-failed", "描画の拒否を通知して資源を解放する");
  failed.invalidate(); await flush(); assert(times === 1, "失敗した描画を自動で無限に試し直さない");
  failed.refreshUnderlays(); await flush(); assert(times === 2, "再読込で失敗をやり直す");
  failed.setUnderlayRenderer(() => ({ width: 100, height: 50, render(request) { const result = raster(request); result.source.width++; return result; }, dispose() {} })); await flush();
  assert(failed.getReport().issues[0].reason === "resource-invalid", "返った画像の寸法も検査する");
  failed.setUnderlayRenderer(() => ({ width: 100, height: 50, render(request) { return raster(request); }, dispose() { throws(() => failed.refreshUnderlays(), "解放関数からの変更再入を断る"); } })); await flush();
  failed.cancelUnderlays(); await flush(); assert(failed.getState().underlays[0].status === "resource-cancelled", "明示した中断を再描画で再開しない");
  failed.refreshUnderlays(); await flush(); assert(failed.getState().underlays[0].status === "ready", "再読込で中断を解除する");
  failed.destroy();

  // 非同期の解放も、完了までは資源と画素の上限に含める。
  const freeingPixels = deferred(), freeingResource = deferred(), freed = [], bounded = new View2D(canvasOf(200, 200), { width: 200, height: 200, limits: { underlayResources: 1 } });
  let opened = 0;
  bounded.setUnderlayRenderer(() => {
    opened++;
    return { width: 100, height: 50, render(request) { const image = raster(request); return { source: image.source, async dispose() { freed.push("pixels-start"); await freeingPixels.promise; image.dispose(); freed.push("pixels-end"); } }; },
      async dispose() { freed.push("resource-start"); await freeingResource.promise; freed.push("resource-end"); } };
  });
  bounded.setScene(bundle(imageScene())); await flush(); bounded.refreshUnderlays(); await flush();
  assert(opened === 1 && freed.join() === "pixels-start", "画像の非同期解放より先に資源を解放しない");
  freeingPixels.resolve(); await flush();
  assert(opened === 1 && freed.join() === "pixels-start,pixels-end,resource-start", "資源の非同期解放を上限に数える");
  freeingResource.resolve(); await flush();
  assert(opened === 2 && freed[3] === "resource-end", "解放の完了後に次の解決を始める");
  bounded.destroy(); await flush();

  const lateFailure = deferred(), retry = new View2D(canvasOf(200, 200), { width: 200, height: 200 });
  let attempts = 0;
  retry.setUnderlayRenderer(() => {
    const current = ++attempts;
    return { width: 100, height: 50, render(request) { const image = raster(request); return { source: image.source, dispose() { image.dispose(); return current === 1 ? lateFailure.promise : undefined; } }; }, dispose() {} };
  });
  retry.setScene(bundle(imageScene())); await flush(); retry.refreshUnderlays(); await flush();
  lateFailure.reject(new Error("old release")); await flush();
  assert(attempts === 2 && retry.getState().underlays[0].status === "ready", "再読込前の解放の失敗で新しい結果を消さない");
  retry.destroy();

  const broken = new View2D(canvasOf(200, 200), { width: 200, height: 200 });
  let resourceFreed = 0;
  broken.setUnderlayRenderer(() => ({ width: 100, height: 50, render(request) { const image = raster(request); return { source: image.source, dispose() { image.dispose(); return Promise.reject(new Error("expected disposal")); } }; }, dispose() { resourceFreed++; } }));
  broken.setScene(bundle(imageScene())); await flush(); broken.cancelUnderlays(); await flush();
  assert(resourceFreed === 1 && broken.getState().underlays[0].status === "resource-release-failed", "画像の解放が拒否されても資源を解放し、失敗を通知する");
  broken.destroy();
}

async function testUnderlayLimits({ assert, throws }) {
  const limited = new View2D(canvasOf(200, 200), { width: 200, height: 200, limits: { underlayPixels: 100, underlaySide: 100 } });
  let resolved = 0, rendered = 0;
  limited.setUnderlayRenderer(() => { resolved++; return { width: 100, height: 50, render(request) { rendered++; return raster(request); }, dispose() {} }; });
  limited.setScene(bundle(imageScene())); await flush();
  assert(resolved === 0 && limited.getReport().issues[0].reason === "resource-limit", "corners の過大な要求は解決前に止める");
  limited.setScene(bundle(imageScene({ frame: V.image_frame.unit, edges: [1, 0, 0, 1] }))); await flush();
  assert(resolved === 1 && rendered === 0 && limited.getState().underlays[0].status === "resource-limit", "unit の大きさを解いた後も画素の確保前に止める");
  limited.setScene(bundle(imageScene({ frame: V.image_frame.unit, clips: [{ xy: [200, 200, 300, 200, 300, 300, 200, 300] }] }))); await flush();
  assert(resolved === 1 && limited.getState().underlays[0].status === "resource-outside-view", "寸法が未解決の unit も切り抜きが画面外なら解決しない");
  limited.setScene(bundle(imageScene({ frame: 255 }))); await flush(); assert(limited.getReport().issues[0].reason === "resource-frame-invalid", "未知の画像枠を推測しない");
  limited.setScene(bundle(imageScene({ kind: V.image_kind.dwf }))); await flush(); assert(limited.getReport().issues[0].reason === "resource-kind-unsupported", "未対応の資源種別を報告する");
  limited.destroy();

  const pending = [], released = [], viewer = new View2D(canvasOf(200, 200), { width: 200, height: 200, limits: { underlayJobs: 1, underlayResources: 1, underlayPixels: 400 } });
  const resolve = (request) => { const answer = deferred(); pending.push({ answer, request }); return answer.promise; };
  viewer.setUnderlayRenderer(resolve); viewer.setScene(bundle(imageScene({ count: 2, edges: [20, 0, 0, 20] }))); await flush();
  assert(pending.length === 1, "解決の並行数と資源数を制限する");
  viewer.refreshUnderlays(); await flush(); assert(pending.length === 1, "中断して未完了の要求も上限に数える");
  pending[0].answer.resolve({ width: 20, height: 20, render(request) { return raster(request); }, dispose() { released.push(0); } }); await flush();
  assert(pending.length === 2 && released.length === 1, "古い要求の完了後にだけ空いた枠を使う");
  pending[1].answer.resolve({ width: 20, height: 20, render(request) { return raster(request); }, dispose() {} }); await flush();
  assert(viewer.getReport().drawn.images === 1 && viewer.getState().underlays[1].status === "resource-limit", "保持する資源の上限も守る");
  viewer.destroy();
  const pixels = new View2D(canvasOf(200, 200), { width: 200, height: 200, limits: { underlayPixels: 400 } });
  pixels.setUnderlayRenderer(() => ({ width: 20, height: 20, render(request) { return raster(request); }, dispose() {} }));
  pixels.setScene(bundle(imageScene({ count: 2, edges: [20, 0, 0, 20] }))); await flush();
  assert(pixels.getReport().drawn.images === 1 && pixels.getState().underlays[1].status === "resource-limit", "画像ごとでなく保持した総画素数を制限する");
  pixels.destroy();
  throws(() => new View2D(canvasOf(10, 10), { limits: { underlayJobs: 0 } }), "上限の不正な値を断る");
  const short = new View2D(canvasOf(10, 10), { width: 10, height: 10, limits: { textLength: 2 } });
  throws(() => short.setScene(bundle(imageScene())), "参照名を読む長さにも上限を付ける"); short.destroy();
  if (typeof OffscreenCanvas !== "undefined") {
    const offscreen = new View2D(new OffscreenCanvas(200, 200));
    offscreen.setUnderlayRenderer(() => ({ width: 10, height: 10, render(request) { const source = new OffscreenCanvas(request.width, request.height); source.getContext("2d").fillRect(0, 0, source.width, source.height); return { source, dispose() { source.width = 0; } }; }, dispose() {} }));
    offscreen.setScene(bundle(imageScene())); await flush(); assert(offscreen.getReport().drawn.images === 1, "OffscreenCanvas だけでも画像を合成する"); offscreen.destroy();
  }
}
