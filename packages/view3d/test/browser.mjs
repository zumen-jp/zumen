// View3D をブラウザーの WebGL2 で検査する。画素は表示の確認で、ID・位置・当たりは独立した期待値と照らす。
import { load, readScene3D } from "@zumen-jp/zumen";
import { View3D, View3DError } from "../src/index.js";
import { document, encode, ORIGIN, result, synthetic, topView } from "./fixture.mjs";
import { testExtras } from "./extras.mjs";

const RED = [255, 0, 0], SELECTED = [255, 136, 0], GREEN = [0, 255, 0], WHITE = [255, 255, 255], HALF = [128, 128, 255], BLUE = [0, 0, 255];

export async function testView3D(wasmPath, ifc) {
  let assertions = 0;
  const assert = (ok, message) => { assertions++; if (!ok) throw new Error(message); };
  const near = (a, b, message, tolerance = 1e-9) => assert(Math.abs(a - b) <= tolerance, `${message}: ${a} / ${b}`);
  const rejects = (fn, reason, message) => {
    let caught = null;
    try { fn(); } catch (error) { caught = error; }
    assert(caught instanceof View3DError && caught.reason === reason, `${message}: ${caught?.reason ?? caught}`);
  };
  const canvas = Object.assign(globalThis.document.createElement("canvas"), { width: 200, height: 200 });
  canvas.style.cssText = "width:200px;height:200px";
  globalThis.document.body.append(canvas);
  const viewer = new View3D(canvas, { width: 200, height: 200, shading: "unlit", unresolvedColor: "#00ff00", edgeColor: "#000000", views: [topView()] });
  const gl = canvas.getContext("webgl2");
  // 描いた直後の同じタスクで読む（preserveDrawingBuffer を使わない）
  const read = (x, y) => {
    const out = new Uint8Array(4);
    gl.readPixels(x, 200 - y - 1, 1, 1, gl.RGBA, gl.UNSIGNED_BYTE, out);
    return [...out.slice(0, 3)];
  };
  const pixel = (x, y, expected, message) => {
    const value = read(x, y);
    assert(value.every((v, i) => Math.abs(v - expected[i]) <= 3), `${message}: ${value} / ${expected}`);
  };
  const { columns } = synthetic();
  const blob = encode(columns).bytes;
  const input = { blob, result: result(columns), document };
  const timings = {};
  let start = performance.now();
  const report = viewer.setScene(input);
  timings.setSceneMs = performance.now() - start;
  assert(report.drawn.instances === 6 && report.skipped.transparent === 1, `描く配置と透明度 0: ${JSON.stringify(report)}`);
  assert(report.drawn.individual === 2, "反転と非一様な尺度は個別の Mesh");
  assert(report.drawn.batches === 4, `相似の配置をまとめる: ${report.drawn.batches}`);
  assert(report.issues.length === 1 && report.issues[0].reason === "color-unresolved" && report.issues[0].element === "unresolved", "未解決の色を要素つきで返す");
  const state = viewer.getState();
  assert(state === viewer.getState() && Object.isFrozen(state), "変わるまで同じ snapshot");
  assert(state.document === input.document || (state.document.hash === document.hash && state.document.doc === document.doc), "文書の識別");
  assert(state.scene.counts.elements === 7 && state.scene.counts.instances === 7, "形のない要素も数える");

  start = performance.now();
  viewer.render();
  timings.renderMs = performance.now() - start;
  start = performance.now();
  pixel(30, 70, RED, "大きな原点の近くの小さな形");
  timings.gpuFinishMs = performance.now() - start;
  pixel(30, 130, RED, "同じ要素の 2 つめの配置");
  pixel(150, 70, RED, "反転した配置");
  pixel(133, 135, RED, "非一様な尺度の左端");
  pixel(157, 135, RED, "非一様な尺度の右端");
  pixel(100, 100, HALF, "半透明");
  pixel(100, 40, WHITE, "透明度 0 は描かない");
  pixel(100, 170, GREEN, "未解決の色は代替色");
  pixel(180, 20, WHITE, "背景");
  pixel(110, 150, WHITE, "反転した配置は外へはみ出さない");

  // 当たり判定。光線の原点はカメラの f64 の位置、相対位置と距離は独立に求めた値
  const expectHit = (x, y, id, offset) => {
    const { hits } = viewer.hitTest("top", { x, y });
    assert(hits.length >= 1 && hits[0].id === id, `(${x}, ${y}) の当たり: ${hits.map((h) => h.id)}`);
    const hit = hits[0];
    assert(hit.kind === "element" && hit.document.hash === document.hash, "当たりの種類と文書");
    assert(hit.position.origin[0] === ORIGIN[0] && hit.position.origin[1] === ORIGIN[1] && hit.position.origin[2] === 10, "f64 の原点");
    near(hit.distance, 10, "距離");
    offset.forEach((v, k) => near(hit.position.offset[k], v, `相対位置 ${k}`));
    return hit;
  };
  const first = expectHit(30, 70, "red", [-7, 3, -10]);
  const second = expectHit(30, 130, "red", [-7, -3, -10]);
  assert(first.instance === 0 && second.instance === 1, "配置は元のシーンの行");
  expectHit(150, 70, "mirror", [5, 3, -10]);
  expectHit(145, 135, "stretch", [4.5, -3.5, -10]);
  expectHit(100, 100, "half", [0, 0, -10]);
  expectHit(100, 170, "unresolved", [0, -7, -10]);
  assert(viewer.hitTest("top", { x: 100, y: 40 }).hits.length === 0, "透明度 0 は選ばない");
  assert(viewer.hitTest("top", { x: 180, y: 20 }).hits.length === 0, "何もない所");
  rejects(() => viewer.hitTest("top", { x: 201, y: 10 }), "outside-view", "枠の外の点");
  rejects(() => viewer.hitTest("none", { x: 1, y: 1 }), "unknown-view", "未知のビュー");
  rejects(() => viewer.hitTest("top", { x: NaN, y: 1 }), "invalid-input", "有限でない点");

  // 選択と非表示。別文書や未知の ID は読み替えず、状態を変えない
  let notified = 0;
  const off = viewer.subscribe(() => { notified++; });
  viewer.select(document, ["red"]);
  assert(viewer.getState().views[0].selected.length === 1 && viewer.getState().views[0].selected[0] === "red" && notified === 1, "選択の状態と通知");
  viewer.render();
  pixel(30, 70, SELECTED, "選択の色");
  pixel(30, 130, SELECTED, "同じ要素のすべての配置");
  pixel(150, 70, RED, "選んでいない要素");
  const before = viewer.getState();
  rejects(() => viewer.select(document, ["missing"]), "unknown-element", "未知の ID");
  rejects(() => viewer.select({ ...document, hash: "1".repeat(64) }, ["red"]), "other-document", "別の版の文書");
  rejects(() => viewer.select({ ...document, doc: "other.ifc" }, ["red"]), "other-document", "別の文書");
  assert(viewer.getState() === before, "断った操作は状態を変えない");
  viewer.select(document, ["mirror", "stretch"]);
  viewer.render();
  pixel(30, 70, RED, "選択を外した色");
  pixel(150, 70, SELECTED, "個別の Mesh の選択");
  viewer.setHidden(document, ["stretch", "red"]);
  viewer.render();
  pixel(145, 135, WHITE, "個別の Mesh を隠す");
  pixel(30, 70, WHITE, "まとめ描きの配置を隠す");
  assert(viewer.hitTest("top", { x: 145, y: 135 }).hits.length === 0 && viewer.hitTest("top", { x: 30, y: 70 }).hits.length === 0, "隠した要素は選ばない");
  viewer.setHidden(document, []);
  viewer.select(document, []);
  viewer.render();
  pixel(145, 135, RED, "隠すのをやめる");
  off();

  // 透視投影の光線
  viewer.setViews([{ id: "eye", rect: { x: 0, y: 0, width: 200, height: 200 }, camera: { projection: "perspective", position: [ORIGIN[0], ORIGIN[1], 10], direction: [0, 0, -1], up: [0, 1, 0], fov: 90 } }]);
  const ray = viewer.getRay("eye", { x: 100, y: 100 });
  assert(ray.direction[2] === -1 && ray.offset.every((v) => v === 0), "中心の光線");
  const corner = viewer.getRay("eye", { x: 200, y: 0 });
  near(corner.direction[0], 1 / Math.sqrt(3), "角の光線の x"); near(corner.direction[1], 1 / Math.sqrt(3), "角の光線の y");
  const perspectiveHit = viewer.hitTest("eye", { x: 100, y: 100 }).hits[0];
  assert(perspectiveHit.id === "half", "透視投影の当たり");
  viewer.render();
  pixel(100, 100, HALF, "透視投影の描画");
  // 複数の枠。重なりは断り、前のビューを保つ
  viewer.setViews([topView("left", { x: 0, y: 0, width: 100, height: 200 }), topView("right", { x: 100, y: 0, width: 100, height: 200 })]);
  viewer.render();
  pixel(50, 100, HALF, "左の枠");
  pixel(150, 100, HALF, "右の枠");
  const views = viewer.getState().views;
  rejects(() => viewer.setViews([topView("a"), topView("b", { x: 50, y: 50, width: 100, height: 100 })]), "invalid-input", "重なる枠");
  rejects(() => viewer.setViews([{ ...topView("a"), camera: { ...topView().camera, up: [0, 0, 2] } }]), "invalid-input", "平行な上方向");
  rejects(() => viewer.setViews([{ ...topView("a"), camera: { ...topView().camera, width: 0 } }]), "invalid-input", "幅 0 の平行投影");
  assert(viewer.getState().views === views, "断ったビューは置き換えない");
  viewer.setViews([topView()]);
  viewer.fit("top");
  const fitted = viewer.getState().views[0].camera;
  assert(fitted.position[2] > 0 && fitted.width > 15, "範囲に合わせる");
  viewer.setViews([topView()]);

  // ビューごとの選択・非表示・色。左右とも red の中心を見る
  const onRed = (id, x) => ({ id, rect: { x, y: 0, width: 100, height: 200 },
    camera: { projection: "orthographic", position: [ORIGIN[0] - 7, ORIGIN[1] + 3, 10], direction: [0, 0, -1], up: [0, 1, 0], width: 10 } });
  viewer.setViews([onRed("left", 0), onRed("right", 100)]);
  viewer.select(document, ["red"], { view: "left" });
  viewer.render();
  pixel(50, 100, SELECTED, "左だけ選ぶ");
  pixel(150, 100, RED, "右は選ばない");
  viewer.setHidden(document, ["red"], { view: "right" });
  viewer.render();
  pixel(50, 100, SELECTED, "左は隠さない");
  pixel(150, 100, WHITE, "右だけ隠す");
  assert(viewer.hitTest("left", { x: 50, y: 100 }).hits[0]?.id === "red" && viewer.hitTest("right", { x: 150, y: 100 }).hits.length === 0, "当たりもビューごと");
  rejects(() => viewer.hitTest("left", { x: 150, y: 100 }), "outside-view", "別の枠の点は受けない");
  viewer.setHidden(document, [], { view: "right" });
  viewer.setColors(document, [{ color: "#0000ff", ids: ["red"] }], { view: "right" });
  viewer.render();
  pixel(150, 100, BLUE, "右だけ色を変える");
  pixel(50, 100, SELECTED, "選択の色が優先する");
  const perView = viewer.getState().views;
  assert(perView[0].selected[0] === "red" && perView[1].selected.length === 0 && perView[1].colored[0].color === "#0000FF" && perView[1].colored[0].ids[0] === "red", "ビューごとの状態");
  rejects(() => viewer.setColors(document, [{ color: "blue", ids: ["red"] }]), "invalid-input", "色の書式");
  rejects(() => viewer.setColors(document, [{ color: "#000000", ids: ["red"] }, { color: "#ffffff", ids: ["red"] }]), "invalid-input", "同じ要素に 2 つの色");
  rejects(() => viewer.select(document, ["red"], { view: "none" }), "unknown-view", "未知のビュー");
  assert(viewer.getState().views === perView, "断った操作はビューを変えない");
  // pixel ratio 2 の画素は snapshot で読み、CSS px の位置と照らす
  viewer.setSize(200, 200, 2);
  const shot = viewer.snapshot("right");
  assert(shot.width === 200 && shot.height === 400 && shot.pixelRatio === 2, `snapshot の寸法: ${shot.width}x${shot.height}`);
  const at = (x, y) => [...shot.pixels.subarray((y * shot.width + x) * 4, (y * shot.width + x) * 4 + 3)];
  assert(at(100, 200).every((v, i) => Math.abs(v - BLUE[i]) <= 3), `pixel ratio 2 の右の枠: ${at(100, 200)}`);
  assert(at(5, 5).every((v) => v >= 252), "pixel ratio 2 の背景");
  assert(viewer.hitTest("right", { x: 150, y: 100 }).hits[0]?.id === "red", "pixel ratio を変えても CSS px で当てる");
  viewer.setSize(200, 200, 1);

  // 切断面。法線の向きを除き、描画と当たり判定で同じ条件
  viewer.setViews([{ ...topView(), clips: [{ point: [ORIGIN[0] - 7, ORIGIN[1], 0], normal: [1, 0, 0] }] }]);
  viewer.render();
  pixel(25, 70, RED, "切断面の残す側");
  pixel(35, 70, WHITE, "切断面の除く側");
  pixel(150, 70, WHITE, "除く側の別の要素");
  assert(viewer.hitTest("top", { x: 25, y: 70 }).hits[0]?.id === "red", "残す側の当たり");
  assert(viewer.hitTest("top", { x: 35, y: 70 }).hits.length === 0 && viewer.hitTest("top", { x: 150, y: 70 }).hits.length === 0, "除く側は当たらない");
  assert(viewer.hitTest("top", { x: 30, y: 70 }).hits[0]?.id === "red", "面の上は残す");
  viewer.setViews([{ ...topView(), box: { min: [ORIGIN[0] - 0.5, ORIGIN[1] - 5, -1], max: [ORIGIN[0] + 5, ORIGIN[1] + 5, 1] } }]);
  viewer.render();
  pixel(100, 100, HALF, "箱の中");
  pixel(92, 100, WHITE, "箱の外");
  pixel(30, 70, WHITE, "箱の外の要素");
  assert(viewer.hitTest("top", { x: 100, y: 100 }).hits[0]?.id === "half" && viewer.hitTest("top", { x: 92, y: 100 }).hits.length === 0, "箱の当たり");
  rejects(() => viewer.setViews([{ ...topView(), clips: [{ point: [0, 0, 0], normal: [0, 0, 0] }] }]), "invalid-input", "0 の法線");
  rejects(() => viewer.setViews([{ ...topView(), box: { min: [1, 0, 0], max: [0, 1, 1] } }]), "invalid-input", "逆の箱");
  rejects(() => viewer.setViews([{ ...topView(), clips: Array.from({ length: 11 }, () => ({ point: [0, 0, 0], normal: [0, 0, 1] })), box: { min: [0, 0, 0], max: [1, 1, 1] } }]), "limit", "切断面の数");

  // レビュー文書の 3D の場所との往復。m・1:1 は厳密、縦横比の違う枠は明示した調整だけ
  const reviewed = { ...topView(), camera: { projection: "perspective", position: [ORIGIN[0], ORIGIN[1], 30], direction: [0, 0.1, -1], up: [0, 1, 0], fov: 50 },
    clips: [{ point: [ORIGIN[0] - 7, ORIGIN[1], 0], normal: [1, 0, 0] }], box: { min: [ORIGIN[0] - 9, ORIGIN[1] - 9, -1], max: [ORIGIN[0] + 9, ORIGIN[1] + 9, 1] },
    selected: ["mirror"], visible: true, exceptions: ["stretch"], colored: [{ color: "#80FF0000", ids: ["half"] }] };
  viewer.setViews([reviewed]);
  const exported = viewer.toReviewPlace("top");
  assert(exported.strict && exported.adjustments.length === 0 && exported.issues.length === 0, "m・1:1 は厳密");
  assert(exported.place.camera.field_of_view === 50 && exported.place.clips.length === 7 && exported.place.colored[0].color === "#80FF0000", "カメラ・切断面・色");
  const json = JSON.parse(JSON.stringify(exported.place));
  viewer.setViews([topView()]);
  const applied = viewer.applyReviewPlace("top", document, json);
  assert(applied.strict && applied.frame.width === 200, `当てた結果: ${JSON.stringify(applied)}`);
  assert(JSON.stringify(viewer.toReviewPlace("top").place) === JSON.stringify(exported.place), "レビューの場所を往復する");
  viewer.setViews([{ ...topView(), selected: ["mirror"] }]);
  viewer.render();
  pixel(150, 70, SELECTED, "要素の状態を持つビュー");
  const missing = viewer.applyReviewPlace("top", document, { kind: "3d", selected: [{ id: "red" }, { id: "nope" }], colored: [{ color: "#00FF00", elements: [{ id: "red" }] }, { color: "#0000FF", elements: [{ id: "red" }] }] });
  const reasons = missing.issues.map((i) => i.reason).join(",");
  assert(!missing.strict && reasons === "element-missing,color-overridden" && missing.issues[0].elements[0] === "nope", `見つからない要素と重なる色: ${reasons}`);
  assert(viewer.getState().views[0].selected.join() === "red" && viewer.getState().views[0].colored[0].color === "#0000FF", "見つかった要素だけ採る");
  const hashed = viewer.applyReviewPlace("top", document, { kind: "3d", selected: [{ id: "red", hash: "a" }, { id: "mirror", hash: "b" }] }, { hashes: new Map([["red", "a"], ["mirror", "c"]]) });
  assert(hashed.issues[0].reason === "element-changed" && viewer.getState().views[0].selected.join() === "red", "ハッシュの違う要素は採らない");
  rejects(() => viewer.applyReviewPlace("top", { ...document, doc: "other.ifc" }, json), "other-document", "別文書の場所");
  rejects(() => viewer.applyReviewPlace("top", document, { ...json, camera: { ...json.camera, up: [0, 0.1, -1] } }), "invalid-input", "平行な方向");
  rejects(() => viewer.applyReviewPlace("top", document, { ...json, camera: { ...json.camera, projection: "fisheye" } }), "invalid-input", "未知の投影");
  rejects(() => viewer.applyReviewPlace("top", document, { kind: "2d" }), "invalid-input", "2D の場所");
  viewer.setViews([{ ...topView("wide", { x: 0, y: 50, width: 200, height: 100 }), camera: { ...reviewed.camera } }]);
  rejects(() => viewer.toReviewPlace("wide"), "aspect-ratio", "1:1 でない枠");
  const contained = viewer.toReviewPlace("wide", { frame: "contain" });
  const widened = (2 * Math.atan(Math.tan((50 * Math.PI) / 360) * 2) * 180) / Math.PI;
  near(contained.place.camera.field_of_view, widened, "枠を含む正方形の視野角", 1e-9);
  assert(!contained.strict && contained.adjustments[0].reason === "square-frame", "調整を返す");
  const tall = viewer.applyReviewPlace("wide", document, { kind: "3d", camera: { projection: "orthographic", width: 4, position: [0, 0, 1], direction: [0, 0, -1], up: [0, 1, 0] } });
  assert(tall.adjustments[0].field === "camera.width" && tall.adjustments[0].to === 8 && tall.frame.width === 100 && tall.frame.x === 50, "正方形を枠に収める");
  viewer.setViews([topView()]);

  // 不正な列・参照・非有限値・応答の食い違いは、前のシーンを保つ
  viewer.render();
  pixel(30, 70, RED, "差し替え前");
  const nan = encode(columns); nan.view.setFloat32(nan.starts.vertex_xyz, NaN, true);
  const badReference = encode(columns); badReference.view.setUint32(badReference.starts.instance_meshes, 9, true);
  const kept = viewer.getState();
  rejects(() => viewer.setScene({ ...input, blob: nan.bytes }), "invalid-scene", "有限でない値");
  rejects(() => viewer.setScene({ ...input, blob: badReference.bytes }), "invalid-scene", "範囲外の参照");
  rejects(() => viewer.setScene({ ...input, blob: blob.subarray(0, 100) }), "invalid-scene", "途中で切れた blob");
  rejects(() => viewer.setScene({ ...input, result: { ...input.result, counts: { ...input.result.counts, instances: 8 } } }), "invalid-input", "応答の数の食い違い");
  rejects(() => viewer.setScene({ ...input, result: { ...input.result, doc: "other.ifc" } }), "invalid-input", "応答と文書の食い違い");
  rejects(() => viewer.setScene({ ...input, extra: 1 }), "invalid-input", "未知の項目");
  rejects(() => viewer.setScene({ ...input, document: { ...document, hash: "" } }), "invalid-input", "空のハッシュ");
  assert(viewer.getState() === kept, "失敗した差し替えは状態を変えない");
  viewer.render();
  pixel(30, 70, RED, "失敗後も前のシーンを描く");

  // 資源の上限。同じ Canvas の別の表示部品で確かめる
  const spare = Object.assign(globalThis.document.createElement("canvas"), { width: 50, height: 50 });
  for (const [limits, message] of [[{ instances: 6 }, "インスタンス"], [{ gpuBytes: 200 }, "GPU のバイト数"], [{ work: 5 }, "作業量"], [{ readWork: 10 }, "読み手の作業量"], [{ readBytes: 100 }, "読み手のバイト数"]]) {
    const limited = new View3D(spare, { width: 50, height: 50, limits });
    try { rejects(() => limited.setScene(input), "limit", message); } finally { limited.destroy(); }
  }
  rejects(() => new View3D(spare, { width: 5000, height: 5000 }), "limit", "Canvas の画素数");
  rejects(() => new View3D(spare, { width: 50, height: 50, limits: { nope: 1 } }), "invalid-input", "未知の上限");
  rejects(() => new View3D(spare, { width: 50, height: 50, background: "red" }), "invalid-input", "色の書式");
  const twoDimensional = globalThis.document.createElement("canvas"); twoDimensional.getContext("2d");
  rejects(() => new View3D(twoDimensional, { width: 50, height: 50 }), "webgl2-unavailable", "WebGL2 を使えない Canvas");

  // 差し替えを繰り返しても資源が増え続けない
  const settled = viewer.getStats().geometries;
  for (let i = 0; i < 10; i++) { viewer.setScene(input); viewer.render(); }
  assert(viewer.getStats().geometries === settled, `差し替えで資源が増えない: ${viewer.getStats().geometries} / ${settled}`);
  viewer.setScene(null);
  viewer.render();
  assert(viewer.getStats().geometries === 0, "シーンを外せば資源を解放する");
  pixel(30, 70, WHITE, "シーンなし");
  viewer.setScene(input);

  // context loss と復帰
  const extension = gl.getExtension("WEBGL_lose_context");
  let contextLoss = "unavailable";
  if (extension) {
    const lost = new Promise((resolve) => viewer.on("contextlost", resolve));
    extension.loseContext();
    await lost;
    // 通知の配送が終わってから戻す。配送中は戻す許可がまだない
    await new Promise((resolve) => setTimeout(resolve, 0));
    assert(viewer.getState().contextLost && viewer.render() === false, "失った context では描かない");
    const restored = new Promise((resolve) => viewer.on("contextrestored", resolve));
    extension.restoreContext();
    await restored;
    assert(!viewer.getState().contextLost, "復帰の状態");
    viewer.render();
    pixel(30, 70, RED, "復帰後に描き直す");
    contextLoss = "checked";
  }

  // 破棄。予約・購読・通知を外し、二度目は何もしない
  let finalState = null;
  viewer.subscribe((value) => { finalState = value; });
  viewer.on("render", () => {});
  viewer.invalidate();
  assert(viewer.getStats().frame && viewer.getStats().subscribers === 1, "予約と購読");
  viewer.destroy();
  viewer.destroy();
  assert(finalState?.destroyed === true && viewer.getState().destroyed, "破棄を通知する");
  rejects(() => viewer.render(), "destroyed", "破棄後は描かない");
  rejects(() => viewer.setScene(input), "destroyed", "破棄後は差し替えない");

  // コアが作った ZMS3（建物の写像・大きな座標・形のない要素）と列を照らす
  const zumen = await load(fetch(wasmPath));
  const session = zumen.open();
  let core;
  try {
    session.push(ifc.name, ifc.text);
    session.call({ v: 1, op: "open", input: ifc.name });
    const hash = session.call({ v: 1, op: "building", doc: ifc.name }).result.hash;
    const response = session.call({ v: 1, op: "scene3d", doc: ifc.name, tolerance: 1 }).result;
    const bytes = session.take(response.blob);
    const scene = readScene3D(bytes);
    const coreViewer = new View3D(canvas, { width: 200, height: 200, views: [{ id: "iso", rect: { x: 0, y: 0, width: 200, height: 200 }, camera: { projection: "perspective", position: [0, 0, 0], direction: [1, 1, -1], up: [0, 0, 1], fov: 45 } }] });
    try {
      const coreDocument = { doc: ifc.name, format: "ifc", hash };
      start = performance.now();
      const coreReport = coreViewer.setScene({ blob: bytes, result: response, document: coreDocument });
      const setSceneMs = performance.now() - start;
      assert(coreReport.drawn.instances + coreReport.skipped.transparent === scene.rows("instance_meshes"), "コアの配置をすべて扱う");
      const counts = coreViewer.getState().scene.counts;
      for (const key of Object.keys(response.counts)) assert(counts[key] === response.counts[key], `コアの列の数 ${key}`);
      const ids = Array.from({ length: scene.rows("element_ids") }, (_, row) => scene.element(row).id);
      coreViewer.select(coreDocument, ids);
      assert(coreViewer.getState().views[0].selected.length === ids.length, "コアの要素 ID をすべて選べる");
      assert(coreViewer.toReviewPlace("iso").issues[0]?.reason === "unit-not-metre", "mm の文書は厳密な往復の対象外");
      coreViewer.select(coreDocument, []);
      coreViewer.fit("iso");
      start = performance.now();
      coreViewer.render();
      const renderMs = performance.now() - start;
      // 格子の点で、描いた画素と当たりの有無を比べる。当たりはコアの要素 ID に限る
      let drawn = 0, hit = 0, agree = 0;
      const found = new Set();
      for (let y = 5; y < 200; y += 10) {
        for (let x = 5; x < 200; x += 10) {
          const painted = !read(x, y).every((v) => v === 255);
          const { hits } = coreViewer.hitTest("iso", { x: x + 0.5, y: y + 0.5 });
          for (const h of hits) { assert(ids.includes(h.id), "当たりはコアの要素 ID"); found.add(h.id); }
          drawn += painted; hit += hits.length > 0; agree += painted === (hits.length > 0);
        }
      }
      assert(drawn > 0 && hit > 0 && agree >= 380, `描画と当たりの一致: ${drawn} ${hit} ${agree}`);
      core = { elements: ids.length, instances: scene.rows("instance_meshes"), meshes: scene.rows("mesh_vertices"), bytes: bytes.length,
        gpuBytes: coreReport.gpuBytes, setSceneMs, renderMs, grid: { drawn, hit, agree, elements: found.size } };
    } finally { coreViewer.destroy(); }

    // m・プロジェクト空間・1:1 の枠で、コアの bcf_write と bcf_read を通して BCF 2.1・3.0 を往復する
    const metre = ifc.text.replace("IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)", "IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.)");
    assert(metre !== ifc.text, "m の IFC を作る");
    session.push("m.ifc", metre);
    session.call({ v: 1, op: "open", input: "m.ifc" });
    const metreHash = session.call({ v: 1, op: "building", doc: "m.ifc" }).result.hash;
    const metreScene = session.call({ v: 1, op: "scene3d", doc: "m.ifc", tolerance: 0.001 }).result;
    const metreDocument = { doc: "m.ifc", format: "ifc", hash: metreHash };
    const bcfViewer = new View3D(canvas, { width: 200, height: 200, views: [{ id: "bcf", rect: { x: 0, y: 0, width: 200, height: 200 }, camera: { projection: "perspective", position: [0, 0, 0], direction: [1, 1, -1], up: [0, 0, 1], fov: 45 } }] });
    const bcf = {};
    try {
      bcfViewer.setScene({ blob: session.take(metreScene.blob), result: metreScene, document: metreDocument });
      bcfViewer.fit("bcf");
      const [wallA, wallB, column] = ["0000000000000000000050", "0000000000000000000060", "0000000000000000000070"];
      const fittedView = bcfViewer.getState().views[0];
      bcfViewer.setViews([{ ...fittedView, clips: [{ point: [1e9, 3, 0.5], normal: [0, 0, 1] }],
        selected: [wallA], visible: true, exceptions: [column], colored: [{ color: "#FF00FF", ids: [wallB] }] }]);
      const before = bcfViewer.toReviewPlace("bcf");
      assert(before.strict, `m・1:1 の場所は厳密: ${JSON.stringify(before.issues)}`);
      // push は同じ名前に足していくので、入力ごとに名前を変える
      const review = (place) => JSON.stringify({ format: "zumen-review", version: 1, targets: [{ id: "model", format: "ifc", hash: metreHash, name: "m.ifc" }],
        topics: [{ id: "t-1", title: "3D", status: "open", locations: [{ id: "t-1/l1", target: "model", ...place }] }] });
      for (const projection of ["perspective", "orthographic"]) {
        if (projection === "orthographic") {
          const v = bcfViewer.getState().views[0];
          bcfViewer.setViews([{ ...v, camera: { projection: "orthographic", position: v.camera.position, direction: v.camera.direction, up: v.camera.up, width: 12.5 } }]);
        }
        const expected = bcfViewer.toReviewPlace("bcf").place;
        session.push(`review-${projection}.json`, review(expected));
        for (const version of ["2.1", "3.0"]) {
          const name = `view-${version}-${projection}.bcf`;
          const written = session.call({ v: 1, op: "bcf_write", review: `review-${projection}.json`, target: "model", kind: "Issue", author: "a", date: "2026-11-17T00:00:00Z", version });
          session.push(name, session.take(written.blobs.find((b) => b.kind === "file").id));
          const read = session.call({ v: 1, op: "bcf_read", doc: "m.ifc", input: name, target: "model" });
          const text = new TextDecoder().decode(session.take(read.blobs.find((b) => b.kind === "file").id));
          const location = JSON.parse(text).topics[0].locations[0];
          const other = bcfViewer.getState().views[0];
          bcfViewer.setViews([{ ...other, camera: { projection: "perspective", position: [0, 0, 0], direction: [0, 1, 0], up: [0, 0, 1], fov: 30 }, clips: [], selected: [], exceptions: [], colored: [] }]);
          const applied = bcfViewer.applyReviewPlace("bcf", metreDocument, location);
          assert(applied.adjustments.length === 0 && applied.issues.every((i) => i.reason === "hash-unchecked"), `BCF ${version} を当てた報告: ${JSON.stringify(applied.issues)}`);
          const after = bcfViewer.toReviewPlace("bcf").place;
          const plain = (place) => JSON.stringify({ ...place, selected: place.selected.map((r) => r.id), exceptions: place.exceptions.map((r) => r.id),
            colored: place.colored.map((c) => ({ color: c.color, ids: c.elements.map((r) => r.id) })) });
          assert(plain(after) === plain(expected), `BCF ${version}・${projection} の往復: ${plain(after)} / ${plain(expected)}`);
          bcf[`${version}/${projection}`] = applied.issues.find((i) => i.reason === "hash-unchecked")?.count ?? 0;
        }
      }
      bcfViewer.render();
    } finally { bcfViewer.destroy(); }
    core.bcf = bcf;
  } finally { session.close(); }

  canvas.remove();
  const added = await testExtras();
  assertions += added.assertions;
  const memory = globalThis.performance?.memory?.usedJSHeapSize ?? null;
  return { assertions, contextLoss, timings, bytes: { blob: blob.length, gpu: report.gpuBytes }, memory, core, extras: added.extras };
}
