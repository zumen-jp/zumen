// 点群のアダプター（#817）をブラウザーの WebGL2 で検査する。位置は地図の変換の逆を独立に計算し、色・当たりはコアの ZMS3 の列と照らす。
import { load, readScene3D } from "@zumen-jp/zumen";
import { View3D, View3DError } from "../src/index.js";
import { attachPointCloud } from "../src/pointcloud.js";
import { document, encode, ORIGIN, result, synthetic } from "./fixture.mjs";

const WHITE = [255, 255, 255];
/** 地図の X 軸の向き（cos・sin）。回転を含む変換で置く。 */
const A = 0.6, B = 0.8;
/** 点群の中心（地図）を置く表示の座標。 */
const CENTER = [ORIGIN[0] - 3, ORIGIN[1] + 6.5, 0];
const MAP_CENTER = [1001.75, 2002.45, 0];
/** 地図 = s·R·p + t の t を、MAP_CENTER が CENTER に来るように決める。 */
const mapFor = (scale = 1, crs = "EPSG:6677") => {
  const rotated = [A * CENTER[0] - B * CENTER[1], B * CENTER[0] + A * CENTER[1], CENTER[2]];
  return { crs_name: crs, eastings: MAP_CENTER[0] - scale * rotated[0], northings: MAP_CENTER[1] - scale * rotated[1], height: MAP_CENTER[2] - scale * rotated[2],
    x_axis_abscissa: A * 10, x_axis_ordinate: B * 10, scale };
};
/** 地図の点を表示の座標へ（期待値）。 */
const expected = (map, point) => {
  const d = [point[0] - map.eastings, point[1] - map.northings, point[2] - map.height];
  return [(A * d[0] + B * d[1]) / map.scale, (-B * d[0] + A * d[1]) / map.scale, d[2] / map.scale];
};
/** 点群の中心の真上から見る平行投影。幅 4 単位を 200 px に写す（1 px は 0.02 単位）。 */
const pcView = (id = "pc", clips = []) => ({ id, rect: { x: 0, y: 0, width: 200, height: 200 }, clips,
  camera: { projection: "orthographic", position: [CENTER[0], CENTER[1], 10], direction: [0, 0, -1], up: [0, 1, 0], width: 4 } });
const screen = (v) => ({ x: 100 + (v[0] - CENTER[0]) / 0.02, y: 100 - (v[1] - CENTER[1]) / 0.02 });

export async function testPointCloud(wasmPath, { las, gml }) {
  let assertions = 0;
  const assert = (ok, message) => { assertions++; if (!ok) throw new Error(message); };
  const near = (a, b, message, tolerance = 1e-3) => assert(Math.abs(a - b) <= tolerance, `${message}: ${a} / ${b}`);
  const rejects = (fn, reason, message) => {
    let caught = null;
    try { fn(); } catch (error) { caught = error; }
    assert(caught instanceof View3DError && caught.reason === reason, `${message}: ${caught?.reason ?? caught}`);
  };
  const entry = await (await fetch("/packages/view3d/src/index.js")).text();
  assert(!entry.includes("pointcloud"), "表示部品の入口は点群のアダプターを読み込まない");

  // コアの LAS を点群のモジュールで開き、ZMS3 1.1 を得る
  const zumen = await load(fetch(wasmPath));
  const session = zumen.open();
  let cloudInput, featureInput;
  try {
    session.push(las.name, Uint8Array.from(las.hex.match(/../g), (h) => parseInt(h, 16)));
    session.call({ v: 1, op: "open", input: las.name });
    const response = session.call({ v: 1, op: "scene3d", doc: las.name }).result;
    cloudInput = { blob: session.take(response.blob), result: response, document: { doc: las.name, format: "las", hash: "1".repeat(64) } };
    // 地物：自作の CityGML の面（東 100〜110・北 200〜210・高さ 12）を共通の ZMS3 で得る
    session.push(gml.name, gml.text);
    session.call({ v: 1, op: "open", input: gml.name });
    const feature = session.call({ v: 1, op: "scene3d", doc: gml.name, tolerance: 0.001 }).result;
    featureInput = { blob: session.take(feature.blob), result: feature, document: { doc: gml.name, format: "citygml", hash: "3".repeat(64) } };
  } finally { session.close(); }
  const core = readScene3D(cloudInput.blob);
  assert(core.minor === 1 && core.chunks === 9 && core.rows("point_xyz") === 12, "コアの点群のシーン");

  const canvas = Object.assign(globalThis.document.createElement("canvas"), { width: 200, height: 200 });
  canvas.style.cssText = "width:200px;height:200px";
  globalThis.document.body.append(canvas);
  const viewer = new View3D(canvas, { width: 200, height: 200, shading: "unlit", views: [pcView()] });
  const gl = canvas.getContext("webgl2");
  const read = (x, y) => {
    const out = new Uint8Array(4);
    gl.readPixels(Math.floor(x), 200 - Math.floor(y) - 1, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, out);
    return [...out.slice(0, 3)];
  };
  const pixel = (at, color, message) => {
    const value = read(at.x, at.y);
    assert(value.every((v, i) => Math.abs(v - color[i]) <= 3), `${message}: ${value} / ${color}`);
  };
  rejects(() => attachPointCloud(viewer, { id: "cloud" }), "no-scene", "シーンがなければ重ねない");
  rejects(() => viewer.getElement(document, "red"), "no-scene", "シーンがなければ要素を引かない");

  const { columns } = synthetic();
  const blob = encode(columns).bytes;
  const map = mapFor();
  const sceneWith = (extra) => ({ blob, result: { ...result(columns), ...extra }, document });
  viewer.setScene(sceneWith({ map, height_crs: 6695 }));
  const state = viewer.getState();
  assert(state.scene.map.crs_name === "EPSG:6677" && state.scene.heightCrs === 6695, "表示の状態に地図の変換と高さの基準");

  // 要素の属性
  const red = viewer.getElement(document, "red");
  assert(red.id === "red" && red.class === "IfcWall" && red.name === "" && red.storey === "storey" && red.row === 0 && Object.isFrozen(red), `要素の属性: ${JSON.stringify(red)}`);
  assert(viewer.getElement(document, "storey").storey === null, "階のない要素");
  assert(viewer.getElement(document, "none") === null, "未知の ID は null");
  rejects(() => viewer.getElement({ ...document, hash: "f".repeat(64) }, "red"), "other-document", "別の文書の参照");

  // 不正な指定
  rejects(() => attachPointCloud(viewer, { id: "c", unknown: 1 }), "invalid-input", "未知の項目");
  rejects(() => attachPointCloud(viewer, { id: "c", color: "height" }), "invalid-input", "未知の色");
  rejects(() => attachPointCloud(viewer, { id: "c", pointSize: 0 }), "invalid-input", "点の大きさ");
  rejects(() => attachPointCloud(viewer, { id: "c", maxPoints: 0 }), "invalid-input", "点の上限");
  rejects(() => attachPointCloud(viewer, { id: "c", input: { ...cloudInput, result: { ...cloudInput.result, doc: "other" } } }), "invalid-input", "応答と文書の食い違い");
  rejects(() => attachPointCloud(viewer, { id: "c", input: { ...cloudInput, result: { ...cloudInput.result, counts: { ...cloudInput.result.counts, points: 11 } } } }), "invalid-input", "点の数の食い違い");
  rejects(() => attachPointCloud(viewer, { id: "c", input: sceneWith({}) }), "invalid-input", "建物のシーンは点群ではない");
  assert(viewer.getState().objects.length === 0, "断った登録を残さない");

  // 粗い段だけ（塊 0 の 4 点）を描き、位置と色を照らす
  const points = [0, 1, 2, 3].map((row) => {
    const chunk = core.chunk(0), local = core.points(0).positions.subarray(row * 3, row * 3 + 3);
    return [0, 1, 2].map((k) => chunk.origin[k] + local[k]);
  });
  const cloud = attachPointCloud(viewer, { id: "cloud", input: cloudInput, pointSize: 6, maxPoints: 4, scope: "viewer" });
  let report = cloud.getReport();
  assert(report.status === "ready" && report.reason === null && report.height === "same" && report.color === "rgba", `重ねる: ${JSON.stringify(report)}`);
  assert(report.points === 12 && report.chunks === 9 && report.drawn.points === 4 && report.drawn.chunks === 1 && report.skipped.budget === 8, `予算で粗い段を選ぶ: ${JSON.stringify(report)}`);
  assert(report.displayBytes === 0, "色の列は blob から借用する");
  viewer.render();
  const rgba = core.columns.point_rgba;
  for (const [row, point] of points.entries()) {
    const at = screen(expected(map, point));
    pixel(at, [0, 1, 2].map((k) => Math.round((rgba[row * 4 + k] / 65535) * 255)), `点 ${row} の位置と色`);
  }
  pixel({ x: 100, y: 30 }, WHITE, "点のない所");

  // 当たり
  let hit = cloud.hitTest("pc", screen(expected(map, points[1])));
  assert(hit.hits.length === 1 && hit.hits[0].kind === "point" && hit.hits[0].chunk === 0 && hit.hits[0].row === 1, `点の当たり: ${JSON.stringify(hit.hits)}`);
  const first = hit.hits[0];
  near(first.distance, 10 - points[1][2], "光線に沿う距離");
  const absolute = [0, 1, 2].map((k) => first.position.origin[k] + first.position.offset[k]);
  const want = expected(map, points[1]);
  for (let k = 0; k < 3; k++) near(absolute[k], want[k], `当たりの位置 ${k}`);
  assert(first.intensity === core.columns.point_intensity[1] && first.classification === core.columns.point_classification[1], "当たりの属性");
  assert(first.rgba.every((v, k) => v === rgba[4 + k]) && first.node.join() === "0,0,0,0" && first.document.doc === las.name, "当たりの色と節と文書");
  assert(cloud.hitTest("pc", { x: 100, y: 30 }).hits.length === 0, "点のない所は当たらない");

  // 予算を外すと同じ位置に重なる 3 段の点を、距離が同じなら塊の順に返す
  const all = attachPointCloud(viewer, { id: "all", input: cloudInput, pointSize: 6, scope: "viewer" });
  report = all.getReport();
  assert(report.drawn.points === 12 && report.drawn.chunks === 9 && report.skipped.budget === 0, `すべて描く: ${JSON.stringify(report.drawn)}`);
  hit = all.hitTest("pc", screen(expected(map, points[0])));
  assert(hit.hits.map((h) => h.chunk).join() === "0,1,5", `重なる点の順: ${hit.hits.map((h) => h.chunk)}`);
  assert(all.hitTest("pc", screen(expected(map, points[0])), { limit: 1 }).truncated, "件数の上限で切ったと返す");
  const work = attachPointCloud(viewer, { id: "work", input: cloudInput, maxWork: 2, scope: "viewer" });
  assert(work.hitTest("pc", screen(expected(map, points[3]))).truncated, "作業量の上限で切ったと返す");
  work.remove();
  all.remove();

  // 切断面の外の点は描かず当たらない
  viewer.setViews([pcView("pc", [{ point: [CENTER[0], CENTER[1], 0.5], normal: [0, 0, 1] }])]);
  viewer.render();
  pixel(screen(expected(map, points[1])), WHITE, "切断面の外（高さ 1）の点は描かない");
  pixel(screen(expected(map, points[0])), [0, 1, 2].map((k) => Math.round((rgba[k] / 65535) * 255)), "切断面の内の点は描く");
  assert(cloud.hitTest("pc", screen(expected(map, points[1]))).hits.length === 0, "切断面の外の点は当たらない");
  assert(cloud.hitTest("pc", screen(expected(map, points[0]))).hits.length === 1, "切断面の内の点は当たる");

  // 表示範囲の外の塊は描かない
  viewer.setViews([{ ...pcView("far"), camera: { ...pcView().camera, position: [CENTER[0] + 50, CENTER[1] + 50, 10] } }]);
  report = cloud.getReport();
  assert(report.drawn.points === 0 && report.skipped.outside === 9, `範囲の外: ${JSON.stringify(report)}`);
  viewer.setViews([pcView()]);
  assert(cloud.getReport().drawn.points === 4, "範囲に戻すと描き直す");

  // 強度と分類の表示色
  for (const [mode, colorOf] of [
    ["intensity", (row) => { const peak = Math.max(...core.columns.point_intensity); const v = Math.round((core.columns.point_intensity[row] * 255) / peak); return [v, v, v]; }],
    ["classification", (row) => ({ 1: [200, 200, 200], 2: [170, 120, 70], 3: [110, 190, 90] })[core.columns.point_classification[row]]],
  ]) {
    const colored = attachPointCloud(viewer, { id: mode, input: cloudInput, color: mode, pointSize: 6, maxPoints: 4, scope: "viewer" });
    cloud.setInput(null);
    assert(colored.getReport().color === mode && colored.getReport().displayBytes === 36, `${mode} の表示色の大きさ`);
    viewer.render();
    for (const [row, point] of points.entries()) pixel(screen(expected(map, point)), colorOf(row), `${mode} の点 ${row}`);
    colored.remove();
    cloud.setInput(cloudInput);
  }
  assert(cloud.getReport().status === "ready", "差し替えで戻す");

  // 尺度のある地図
  const scaled = mapFor(2);
  viewer.setScene(sceneWith({ map: scaled }));
  report = cloud.getReport();
  assert(report.status === "ready" && report.height === "unverified", `高さの基準が片方だけなら未確認: ${JSON.stringify(report)}`);
  const half = expected(scaled, points[1]);
  hit = cloud.hitTest("pc", screen(half));
  assert(hit.hits.length === 1 && hit.hits[0].row === 1, `尺度のある地図の当たり: ${JSON.stringify(hit.hits)}`);
  near(hit.hits[0].distance, 10 - half[2], "尺度のある地図の距離");
  near(hit.hits[0].position.origin[0] + hit.hits[0].position.offset[0], half[0], "尺度のある地図の位置");

  // 重ねない理由
  const reasons = [
    [{ map: mapFor(1, "urn:ogc:def:crs:EPSG::6677"), height_crs: 6695 }, null],
    [{ map: mapFor(1, "http://www.opengis.net/def/crs/EPSG/0/6677") }, null],
    [{}, "map-unknown"],
    [{ map: { ...map, crs_name: undefined } }, "crs-unknown"],
    [{ map: mapFor(1, "EPSG:6676") }, "crs-mismatch"],
    [{ map, height_crs: 6697 }, "height-mismatch"],
  ];
  for (const [extra, reason] of reasons) {
    viewer.setScene(sceneWith(extra));
    report = cloud.getReport();
    assert(report.reason === reason && report.status === (reason === null ? "ready" : "unavailable"), `理由 ${reason}: ${JSON.stringify(report)}`);
    if (reason !== null) assert(cloud.hitTest("pc", screen(expected(map, points[1]))).reason === reason, `当たりにも理由 ${reason}`);
  }
  viewer.setScene(sceneWith({ map, height_crs: 6695 }));
  for (const [change, reason] of [
    [{ unit: { kind: "LENGTHUNIT", name: "METRE", prefix: "MILLI" } }, "unit-unknown"],
    [{ map: { ...cloudInput.result.map, crs_name: undefined } }, "crs-unknown"],
    [{ map: { ...cloudInput.result.map, eastings: 1 } }, "map-unsupported"],
  ]) {
    cloud.setInput({ ...cloudInput, result: { ...cloudInput.result, ...change } });
    assert(cloud.getReport().reason === reason, `点群の ${reason}: ${JSON.stringify(cloud.getReport())}`);
  }
  cloud.setInput(cloudInput);
  assert(cloud.getReport().reason === null, "直すと重ねる");

  // 非同期の読み直し：古い結果は捨て、中断は区別する
  const waiting = [];
  const loader = attachPointCloud(viewer, { id: "loader", scope: "viewer", load: (query) => new Promise((resolve, reject) => {
    waiting.push({ query, resolve, reject });
    query.signal.addEventListener("abort", () => { if (query.cancelOnAbort) reject(query.signal.reason); });
  }) });
  assert(loader.getReport().status === "empty", "入力なしは空");
  const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
  const late = loader.request({ cancelOnAbort: false });
  await tick();
  const aborted = loader.request({ cancelOnAbort: true });
  await tick();
  assert(waiting.length === 2 && waiting[0].query.signal.aborted && waiting[0].query.signal.reason === "superseded", "新しい要求で前を中断する");
  const never = loader.request({ never: true });
  const applied = loader.request({ box: [0, 0, 0, 1, 1, 1] });
  await tick();
  assert(waiting.length === 3 && waiting[2].query.box.join() === "0,0,0,1,1,1", "始まる前に次の要求が来れば load を呼ばない");
  assert(loader.getReport().pending, "待っている要求");
  waiting[2].resolve(cloudInput);
  waiting[0].resolve(cloudInput);
  const outcomes = await Promise.all([late, aborted, never, applied]);
  assert(outcomes.map((o) => o.outcome).join() === "stale,cancelled,cancelled,applied", `要求の結果: ${outcomes.map((o) => o.outcome)}`);
  assert(outcomes[3].report.status === "ready" && !loader.getReport().pending, "要求の結果で差し替える");
  const failing = loader.request({});
  await tick();
  waiting[3].reject(new Error("読めない"));
  assert((await failing).outcome === "failed" && loader.getReport().status === "ready", "失敗は前の表示を保つ");
  report = loader.getReport();
  assert(report.requests.applied === 1 && report.requests.stale === 1 && report.requests.cancelled === 2 && report.requests.failed === 1 && report.requests.firstFailure.message === "読めない", `要求の数: ${JSON.stringify(report.requests)}`);
  const pending = loader.request({ cancelOnAbort: true });
  await tick();
  loader.remove();
  assert((await pending).outcome === "cancelled" && waiting[4].query.signal.reason === "removed", "解除で中断する");
  rejects(() => loader.setInput(cloudInput), "destroyed", "解除した後は差し替えない");

  // 大きな合成の点群：予算・時間・借用
  const timings = {};
  const big = bigCloud(64, 15625);
  let start = performance.now();
  const large = attachPointCloud(viewer, { id: "large", input: big, scope: "viewer" });
  timings.setInputMs = performance.now() - start;
  report = large.getReport();
  assert(report.points === 1000000 && report.drawn.points === 1000000 && report.displayBytes === 0, `大きな点群: ${JSON.stringify(report)}`);
  start = performance.now();
  viewer.render();
  read(0, 0);
  timings.renderMs = performance.now() - start;
  start = performance.now();
  const bigHit = large.hitTest("pc", { x: 100, y: 100 });
  timings.hitTestMs = performance.now() - start;
  assert(bigHit.hits.length > 0 && bigHit.hits.every((h, i) => i === 0 || bigHit.hits[i - 1].distance <= h.distance), "大きな点群の当たりは距離の順");
  large.remove();
  const budget = attachPointCloud(viewer, { id: "budget", input: big, maxPoints: 300000, scope: "viewer" });
  report = budget.getReport();
  assert(report.drawn.points === 19 * 15625 && report.drawn.chunks === 19 && report.skipped.budget === 45, `予算: ${JSON.stringify(report.drawn)}`);
  budget.remove();

  // 寿命：シーンの差し替え・単位の違い・破棄
  const sceneScoped = attachPointCloud(viewer, { id: "scoped", input: cloudInput });
  viewer.setScene(sceneWith({ map }));
  assert(sceneScoped.signal.aborted && sceneScoped.signal.reason === "scene" && sceneScoped.getReport().status === "removed", "シーンの差し替えで外す");
  const metreOnly = attachPointCloud(viewer, { id: "metre", input: cloudInput, scope: "viewer" });
  viewer.setScene({ blob, result: { ...result(columns), unit: { kind: "LENGTHUNIT", name: "METRE", prefix: "MILLI" }, map }, document });
  assert(metreOnly.signal.reason === "unit", "単位の違うシーンで外す");
  viewer.setScene(sceneWith({ map }));
  const last = attachPointCloud(viewer, { id: "last", input: cloudInput, scope: "viewer" });
  cloud.remove();
  cloud.remove();
  assert(cloud.getReport().status === "removed" && cloud.hitTest("pc", { x: 1, y: 1 }).reason === "removed", "解除を重ねても後始末は 1 回");
  viewer.destroy();
  assert(last.signal.reason === "destroy", "破棄で外す");
  canvas.remove();
  // 地物のシーンに重ねる：恒等の地図の変換は再投影とせず、手前の地物に隠れた点は当たらない
  const featureCanvas = Object.assign(globalThis.document.createElement("canvas"), { width: 200, height: 200 });
  globalThis.document.body.append(featureCanvas);
  const geoView = { id: "geo", rect: { x: 0, y: 0, width: 200, height: 200 },
    camera: { projection: "orthographic", position: [105, 205, 50], direction: [0, 0, -1], up: [0, 1, 0], width: 40 } };
  const geo = new View3D(featureCanvas, { width: 200, height: 200, shading: "unlit", views: [geoView] });
  geo.setScene(featureInput);
  const area = geo.getElement(featureInput.document, "area");
  assert(area !== null && area.class === "bldg:Building" && area.name === "面" && area.storey === null, `地物の ID・型・名前: ${JSON.stringify(area)}`);
  geo.select(featureInput.document, ["area"]);
  assert(geo.getState().views[0].selected.join() === "area", "地物の ID で選ぶ");
  const layer = attachPointCloud(geo, { id: "layer", input: smallCloud([[[105.5, 205.5, 0], [104, 204, 1]], [[113, 205, 2]]]) });
  report = layer.getReport();
  assert(report.status === "ready" && report.height === "unverified" && report.drawn.points === 3, `地物に重ねる: ${JSON.stringify(report)}`);
  const geoScreen = (p) => ({ x: 100 + (p[0] - 105) / 0.2, y: 100 - (p[1] - 205) / 0.2 });
  hit = layer.hitTest("geo", geoScreen([105.5, 205.5]));
  assert(hit.hits.length === 0 && hit.occluder?.id === "area", `地物に隠れた点は当たらない: ${JSON.stringify(hit)}`);
  hit = layer.hitTest("geo", geoScreen([113, 205]));
  assert(hit.hits.length === 1 && hit.hits[0].chunk === 1, `地物の外の点は当たる: ${JSON.stringify(hit.hits)}`);
  for (let k = 0; k < 3; k++) near(hit.hits[0].position.origin[k] + hit.hits[0].position.offset[k], [113, 205, 2][k], `恒等の変換の位置 ${k}`);
  geo.destroy();
  assert(layer.getReport().status === "removed", "地物の表示の破棄で外す");
  featureCanvas.remove();
  return { assertions, timings };
}

/** 地図の座標で塊ごとの点を持つ小さな点群を組む。塊は段 1 の節で、原点は塊の最初の点。高さの基準は持たない。 */
function smallCloud(chunks) {
  const points = chunks.flat(), doc = "small.las";
  const columns = {
    chunk_origins: chunks.flatMap((list) => list[0]), chunk_bounds: [], chunk_instances: chunks.map(() => 0),
    mesh_vertices: [], mesh_triangles: [], mesh_edges: [], vertex_xyz: [], triangle_vertices: [], edge_vertices: [],
    instance_meshes: [], instance_elements: [], instance_colors: [], instance_transforms: [],
    element_ids: [], element_classes: [], element_names: [], element_storeys: [], color_rgba: [], color_flags: [], strings: [],
    chunk_points: [], chunk_nodes: [], point_xyz: [], point_rgba: [], point_intensity: [], point_classification: [],
  };
  for (const [c, list] of chunks.entries()) {
    const local = list.map((p) => p.map((v, k) => v - list[0][k]));
    columns.chunk_bounds.push(...[0, 1, 2].map((k) => Math.min(...local.map((p) => p[k]))), ...[0, 1, 2].map((k) => Math.max(...local.map((p) => p[k]))));
    columns.chunk_points.push(columns.point_xyz.length / 3);
    columns.chunk_nodes.push(1, c, 0, 0);
    columns.point_xyz.push(...local.flat());
  }
  return {
    blob: encode(columns).bytes,
    result: { doc, unit: { kind: "LENGTHUNIT", name: "METRE" }, map: { crs_name: "EPSG:6677", eastings: 0, northings: 0, height: 0, x_axis_abscissa: 1, x_axis_ordinate: 0, scale: 1 },
      tolerance: 0.001, truncated: false, counts: { chunks: chunks.length, meshes: 0, instances: 0, elements: 0, colors: 0, vertices: 0, triangles: 0, edges: 0, points: points.length }, blob: 0 },
    document: { doc, format: "las", hash: "4".repeat(64) },
  };
}

/** 地図の座標で大きな点群を組む。塊は段 3 の節で、位置は塊の原点からの ±1 の範囲（決まった擬似乱数）。 */
function bigCloud(chunks, perChunk) {
  let seed = 1;
  const random = () => { seed = (seed * 1103515245 + 12345) % 2147483648; return seed / 2147483648; };
  const total = chunks * perChunk;
  const columns = {
    chunk_origins: new Float64Array(chunks * 3), chunk_bounds: new Float32Array(chunks * 6), chunk_instances: new Uint32Array(chunks),
    mesh_vertices: [], mesh_triangles: [], mesh_edges: [], vertex_xyz: [], triangle_vertices: [], edge_vertices: [],
    instance_meshes: [], instance_elements: [], instance_colors: [], instance_transforms: [],
    element_ids: [], element_classes: [], element_names: [], element_storeys: [], color_rgba: [], color_flags: [], strings: [],
    chunk_points: new Uint32Array(chunks), chunk_nodes: new Uint32Array(chunks * 4),
    point_xyz: new Float32Array(total * 3), point_rgba: new Uint16Array(total * 4), point_intensity: new Uint16Array(0), point_classification: new Uint8Array(0),
  };
  for (let c = 0; c < chunks; c++) {
    columns.chunk_origins.set([MAP_CENTER[0] + ((c % 8) - 4) * 0.25, MAP_CENTER[1] + (Math.floor(c / 8) - 4) * 0.25, 0], c * 3);
    columns.chunk_bounds.set([-1, -1, -1, 1, 1, 1], c * 6);
    columns.chunk_points[c] = c * perChunk;
    columns.chunk_nodes.set([3, c % 8, Math.floor(c / 8), 0], c * 4);
  }
  for (let i = 0; i < total * 3; i++) columns.point_xyz[i] = Math.fround(random() * 2 - 1);
  for (let i = 0; i < total * 4; i++) columns.point_rgba[i] = i % 4 === 3 ? 65535 : Math.floor(random() * 65536);
  const doc = "big.las";
  return {
    blob: encode(columns).bytes,
    result: { doc, unit: { kind: "LENGTHUNIT", name: "METRE" }, map: { crs_name: "EPSG:6677", eastings: 0, northings: 0, height: 0, x_axis_abscissa: 1, x_axis_ordinate: 0, scale: 1 },
      height_crs: 6695, tolerance: 0.001, truncated: false, counts: { chunks, meshes: 0, instances: 0, elements: 0, colors: 0, vertices: 0, triangles: 0, edges: 0, points: total }, blob: 0 },
    document: { doc, format: "las", hash: "2".repeat(64) },
  };
}
