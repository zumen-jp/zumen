// 利用側の追加描画と入力（#815）をブラウザーの WebGL2 で検査する。画素は表示の確認で、ID・順序・後始末は独立した期待値と照らす。
import * as THREE from "three";
import { View3D, View3DError } from "../src/index.js";
import { document, encode, ORIGIN, result, synthetic, topView } from "./fixture.mjs";

const RED = [255, 0, 0], GREEN = [0, 255, 0], WHITE = [255, 255, 255], BLUE = [0, 0, 255], BLACK = [0, 0, 0], SELECTED = [255, 136, 0];
const METRE = { name: "METRE" };
const at = (x, y, z = 0) => [ORIGIN[0] + x, ORIGIN[1] + y, z];
const square = (x0, y0, x1, y1, z = 0) => [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]];

export async function testExtras() {
  let assertions = 0;
  const assert = (ok, message) => { assertions++; if (!ok) throw new Error(message); };
  const near = (a, b, message, tolerance = 1e-9) => assert(Math.abs(a - b) <= tolerance, `${message}: ${a} / ${b}`);
  const rejects = (fn, reason, message) => {
    let caught = null;
    try { fn(); } catch (error) { caught = error; }
    assert(caught instanceof View3DError && caught.reason === reason, `${message}: ${caught?.reason ?? caught}`);
  };
  const throwsSame = (fn, expected, message) => {
    let caught = null;
    try { fn(); } catch (error) { caught = error; }
    assert(caught === expected, `${message}: ${caught}`);
  };
  const canvas = Object.assign(globalThis.document.createElement("canvas"), { width: 200, height: 200 });
  canvas.style.cssText = "width:200px;height:200px;display:block";
  globalThis.document.body.append(canvas);
  const viewer = new View3D(canvas, { width: 200, height: 200, shading: "unlit", edges: false, views: [topView()] });
  const gl = canvas.getContext("webgl2");
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
  const input = { blob: encode(columns).bytes, result: result(columns), document };
  const extras = {};
  try {
    rejects(() => viewer.addGeometry([{ kind: "line", points: [[0, 0, 0], [1, 0, 0]] }], { id: "early", origin: ORIGIN, unit: METRE }), "no-scene", "scope が scene の登録にはシーンが要る");
    const early = viewer.addGeometry([{ kind: "line", points: [[0, 0, 0], [1, 0, 0]] }], { id: "early", origin: ORIGIN, unit: METRE, scope: "viewer" });
    viewer.setScene(input);
    assert(!early.signal.aborted && viewer.getState().objects.length === 1, "viewer の登録はシーンの差し替えで残る");
    early.remove();

    // 大きな原点の近くの小さな差。1e12 の原点で 0.04 単位の隙間を 4 px として描き分ける
    const close = { id: "close", rect: { x: 0, y: 0, width: 200, height: 200 },
      camera: { projection: "orthographic", position: at(2.5, 2.5, 10), direction: [0, 0, -1], up: [0, 1, 0], width: 2 } };
    viewer.setViews([close]);
    const pair = viewer.addGeometry([
      { kind: "region", points: square(-0.5, -0.5, -0.02, 0.5), color: "#0000ff" },
      { kind: "region", points: square(0.02, -0.5, 0.5, 0.5), color: "#00ff00" },
    ], { id: "pair", origin: at(2.5, 2.5), unit: METRE });
    viewer.render();
    pixel(90, 100, BLUE, "大きな原点の左の範囲");
    pixel(110, 100, GREEN, "大きな原点の右の範囲");
    pixel(100, 100, WHITE, "0.04 単位の隙間");
    const left = viewer.hitTest("close", { x: 90, y: 100 }).hits[0];
    assert(left?.kind === "geometry" && left.id === "pair" && left.part === 0 && left.priority === 0, `幾何の記述の当たり: ${JSON.stringify(left)}`);
    near(left.distance, 10, "幾何の記述の距離");
    near(left.position.offset[0], -0.1, "幾何の記述の相対位置", 1e-12);
    assert(left.position.origin[0] === ORIGIN[0] + 2.5, "当たりの f64 の原点");
    assert(viewer.hitTest("close", { x: 110, y: 100 }).hits[0]?.part === 1 && viewer.hitTest("close", { x: 100, y: 100 }).hits.length === 0, "部分と隙間の当たり");
    assert(pair.parts.length === 2 && pair.parts[0].kind === "region", "部分の一覧");
    pair.remove();
    viewer.setViews([topView()]);

    // 要素と同じ距離の順序は優先度・種類・ID で固定する。手前と奥は距離で並べ、追加物を常に手前とは扱わない
    const high = viewer.addGeometry([{ kind: "region", points: square(-7.5, 2.5, -6.5, 3.5), color: "#0000ff" }], { id: "tie-high", origin: ORIGIN, unit: METRE, priority: 1 });
    const zero = viewer.addGeometry([{ kind: "region", points: square(-7.5, 2.5, -6.5, 3.5), color: "#0000ff" }], { id: "tie-zero", origin: ORIGIN, unit: METRE });
    const order = viewer.hitTest("top", { x: 30, y: 70 }).hits;
    assert(order.map((h) => h.id).join() === "tie-high,red,tie-zero" && order.every((h) => h.distance === 10), `同じ距離の順序: ${order.map((h) => `${h.id}@${h.distance}`)}`);
    high.remove(); zero.remove();
    const front = viewer.addGeometry([{ kind: "region", points: square(-7.5, -3.5, -6.5, -2.5, 1), color: "#0000ff" }], { id: "front", origin: ORIGIN, unit: METRE, priority: -5 });
    const back = viewer.addGeometry([{ kind: "region", points: square(-8.5, -3.5, -7.6, -2.5, -1), color: "#00ff00" }], { id: "back", origin: ORIGIN, unit: METRE, priority: 5 });
    viewer.render();
    pixel(30, 130, BLUE, "手前の追加物が要素を隠す");
    pixel(22, 130, RED, "奥の追加物は要素に隠れる");
    assert(viewer.hitTest("top", { x: 30, y: 130 }).hits.map((h) => h.id).join() === "front,red", "手前の追加物が先");
    assert(viewer.hitTest("top", { x: 22, y: 130 }).hits.map((h) => h.id).join() === "red,back", "優先度より距離が先");
    front.remove(); back.remove();

    // 借用した Object3D。透明度 0・非表示・切断面を描画と選択で照らし、利用側の値を書き換えない
    const geometry = new THREE.PlaneGeometry(1, 1), material = new THREE.MeshBasicMaterial({ color: 0x00ff00 });
    let disposed = 0;
    geometry.addEventListener("dispose", () => { disposed++; });
    material.addEventListener("dispose", () => { disposed++; });
    const root = new THREE.Mesh(geometry, material);
    root.position.set(0.25, 0, 0);
    root.userData.tag = "own";
    const before = JSON.stringify({ p: root.position.toArray(), q: root.quaternion.toArray(), s: root.scale.toArray(), u: root.userData, c: material.color.getHex(), o: material.opacity });
    let disposeCalls = 0;
    const borrowed = viewer.addObject(root, { id: "machine", origin: at(2.25, 2.5), unit: METRE, dispose: () => { disposeCalls++; } });
    rejects(() => viewer.addObject(root, { id: "again", origin: ORIGIN, unit: METRE }), "invalid-input", "親のある root");
    rejects(() => viewer.addGeometry([{ kind: "line", points: [[0, 0, 0], [1, 0, 0]] }], { id: "machine", origin: ORIGIN, unit: METRE }), "invalid-input", "重なる ID");
    viewer.render();
    pixel(125, 75, GREEN, "借用した Object3D を描く");
    let hit = viewer.hitTest("top", { x: 125, y: 75 }).hits[0];
    assert(hit?.kind === "object" && hit.id === "machine" && hit.object === root && hit.part === undefined, `Object3D の当たり: ${hit?.kind} ${hit?.id}`);
    material.transparent = true; material.opacity = 0;
    assert(viewer.hitTest("top", { x: 125, y: 75 }).hits.length === 0, "透明度 0 の材質は選ばない");
    material.transparent = false; material.opacity = 1;
    root.visible = false;
    viewer.render();
    pixel(125, 75, WHITE, "非表示の Object3D は描かない");
    assert(viewer.hitTest("top", { x: 125, y: 75 }).hits.length === 0, "非表示の Object3D は選ばない");
    root.visible = true;
    viewer.setViews([{ ...topView(), clips: [{ point: at(2.25, 0), normal: [1, 0, 0] }] }]);
    viewer.render();
    pixel(120, 75, GREEN, "切断面の残す側の Object3D");
    pixel(127, 75, WHITE, "切断面の除く側の Object3D");
    assert(viewer.hitTest("top", { x: 121, y: 75 }).hits[0]?.id === "machine" && viewer.hitTest("top", { x: 127, y: 75 }).hits.length === 0, "切断面と当たり");
    const after = JSON.stringify({ p: root.position.toArray(), q: root.quaternion.toArray(), s: root.scale.toArray(), u: root.userData, c: material.color.getHex(), o: material.opacity });
    assert(after === before && root.material === material && root.geometry === geometry, "借用物を改変しない");

    // 切り抜きに対応しない宣言の物は、切断面のあるビューで描かず選ばず、理由を返す
    const custom = new THREE.Mesh(new THREE.PlaneGeometry(1, 1), new THREE.MeshBasicMaterial({ color: 0x0000ff }));
    const shader = viewer.addObject(custom, { id: "shader", origin: at(0, 6), unit: METRE, clipping: false });
    let excluded = null;
    const offRender = viewer.on("render", (event) => { excluded = event.excluded; });
    viewer.render();
    pixel(100, 40, WHITE, "切り抜きに対応しない物は切断面のあるビューで描かない");
    assert(excluded.length === 1 && excluded[0].id === "shader" && excluded[0].reason === "clipping-unsupported", `除いた理由: ${JSON.stringify(excluded)}`);
    assert(viewer.hitTest("top", { x: 100, y: 40 }).hits.length === 0, "切り抜きに対応しない物は選ばない");
    viewer.setViews([topView()]);
    viewer.render();
    pixel(100, 40, BLUE, "切断面のないビューでは描く");
    assert(excluded.length === 0 && viewer.hitTest("top", { x: 100, y: 40 }).hits[0]?.id === "shader", "切断面のないビューでは選ぶ");
    offRender();
    shader.remove();
    custom.geometry.dispose(); custom.material.dispose();

    // 登録解除。何回呼んでも後始末を重ねず、借用した資源を解放しない
    borrowed.remove();
    borrowed.remove();
    assert(root.parent === null && disposeCalls === 1 && disposed === 0, `借用物の後始末: ${root.parent} ${disposeCalls} ${disposed}`);
    assert(borrowed.signal.aborted && borrowed.signal.reason === "removed", "解除の通知");
    viewer.render();
    pixel(125, 75, WHITE, "解除した物は描かない");

    // 別の表示が共有する資源を保つ
    const shareCanvas = Object.assign(globalThis.document.createElement("canvas"), { width: 50, height: 50 });
    const other = new View3D(shareCanvas, { width: 50, height: 50, shading: "unlit", views: [{ id: "o", rect: { x: 0, y: 0, width: 50, height: 50 },
      camera: { projection: "orthographic", position: [0, 0, 10], direction: [0, 0, -1], up: [0, 1, 0], width: 2 } }] });
    try {
      const a = new THREE.Mesh(geometry, material), b = new THREE.Mesh(geometry, material);
      const onThis = viewer.addObject(a, { id: "shared", origin: at(2.25, 2.5), unit: METRE });
      other.addObject(b, { id: "shared", origin: [0, 0, 0], unit: METRE, scope: "viewer" });
      viewer.render(); other.render();
      onThis.remove();
      other.render();
      const otherGl = shareCanvas.getContext("webgl2"), out = new Uint8Array(4);
      otherGl.readPixels(25, 25, 1, 1, otherGl.RGBA, otherGl.UNSIGNED_BYTE, out);
      assert(out[1] > 250 && out[0] < 5 && disposed === 0, `共有の資源を保つ: ${[...out]}`);
    } finally { other.destroy(); }
    assert(disposed === 0, "別の表示を破棄しても借用物を解放しない");

    // 幾何の記述の資源は部品が所有し、解除で解放する
    const label = new ImageData(new Uint8ClampedArray(Array.from({ length: 16 }, () => [0, 0, 255, 255]).flat()), 4, 4);
    viewer.render();
    const baseline = viewer.getStats();
    const drawing = viewer.addGeometry([
      { kind: "line", points: [[-8.95, -9, 0], [-8.95, -5, 0]], color: "#000000" },
      { kind: "circle", center: [-3, 6, 0], radius: 0.5, tolerance: 0.01, color: "#000000" },
      { kind: "dimension", from: [0, -9, 0], to: [4, -9, 0], offset: [0, 0.5, 0], label: { image: label, width: 8, height: 8 } },
      { kind: "label", position: [5, -6, 0], image: label, width: 20, height: 10, anchor: [0, 0] },
    ], { id: "drawing", origin: ORIGIN, unit: METRE });
    viewer.render();
    const added = viewer.getStats();
    assert(added.geometries > baseline.geometries && added.textures === baseline.textures + 2, `部品が作った資源: ${JSON.stringify(added)}`);
    assert(drawing.parts[2].length === 4 && drawing.parts[1].segments >= 8, `寸法の長さ（文書単位）と円の分割: ${JSON.stringify(drawing.parts)}`);
    pixel(10, 170, BLACK, "線");
    assert(viewer.hitTest("top", { x: 11, y: 170 }).hits[0]?.part === 0, "線の当たりは CSS px の許容差");
    assert(viewer.hitTest("top", { x: 15, y: 170 }).hits.length === 0, "許容差の外の線");
    // ラベルは CSS px の寸法で、左下を位置に合わせる
    pixel(155, 155, BLUE, "ラベルの内側");
    assert(viewer.hitTest("top", { x: 165, y: 155 }).hits[0]?.part === 3 && viewer.hitTest("top", { x: 172, y: 155 }).hits.length === 0, "ラベルの当たりは CSS px の範囲");
    drawing.remove();
    viewer.render();
    const freed = viewer.getStats();
    assert(freed.geometries === baseline.geometries && freed.textures === baseline.textures, `解除で解放する: ${JSON.stringify(freed)} / ${JSON.stringify(baseline)}`);
    // three.js が共有する Sprite の geometry は、最後のラベルを外すときに GPU の写しを解放し、次に描くときに転送し直す
    const tag = [{ kind: "label", position: [5, -6, 0], image: label, width: 20, height: 10, anchor: [0, 0] }];
    const first = viewer.addGeometry(tag, { id: "tag-1", origin: ORIGIN, unit: METRE }), second = viewer.addGeometry(tag, { id: "tag-2", origin: ORIGIN, unit: METRE });
    viewer.render();
    const both = viewer.getStats();
    first.remove(); viewer.render();
    assert(viewer.getStats().geometries === both.geometries, "ほかのラベルが残る間は共有の geometry を残す");
    pixel(155, 155, BLUE, "残したラベル");
    second.remove(); viewer.render();
    assert(viewer.getStats().geometries === baseline.geometries, `最後のラベルで共有の geometry を解放する: ${JSON.stringify(viewer.getStats())}`);
    const third = viewer.addGeometry(tag, { id: "tag-3", origin: ORIGIN, unit: METRE });
    viewer.render();
    pixel(155, 155, BLUE, "解放の後のラベル");
    third.remove(); viewer.render();

    // 不正な登録は状態を変えない
    const kept = viewer.getState();
    const line = [{ kind: "line", points: [[0, 0, 0], [1, 0, 0]] }];
    rejects(() => viewer.addGeometry(line, { id: "u", origin: ORIGIN, unit: { name: "METRE", prefix: "MILLI" } }), "invalid-input", "シーンと違う単位");
    rejects(() => viewer.addGeometry(line, { id: "n", origin: [NaN, 0, 0], unit: METRE }), "invalid-input", "有限でない原点");
    rejects(() => viewer.addGeometry(line, { id: "m", origin: ORIGIN, unit: METRE, matrix: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] }), "invalid-input", "逆を持たない変換");
    rejects(() => viewer.addGeometry(line, { id: "p", origin: ORIGIN, unit: METRE, matrix: [1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] }), "invalid-input", "アフィンでない変換");
    rejects(() => viewer.addGeometry([{ kind: "label", position: [0, 0, 0], text: "A", width: 10, height: 10 }], { id: "t", origin: ORIGIN, unit: METRE }), "invalid-input", "文字列からフォントで字形を作らない");
    rejects(() => viewer.addGeometry([{ kind: "region", points: [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 1]] }], { id: "r", origin: ORIGIN, unit: METRE }), "invalid-input", "平面にない範囲");
    rejects(() => viewer.addGeometry([{ kind: "circle", center: [0, 0, 0], radius: 1, tolerance: 1e-12 }], { id: "c", origin: ORIGIN, unit: METRE }), "limit", "円の分割数の上限");
    rejects(() => viewer.addGeometry(line, { id: "x", origin: ORIGIN, unit: METRE, url: "https://example.invalid/" }), "invalid-input", "未知の項目");
    rejects(() => viewer.addObject(new THREE.Scene(), { id: "s", origin: ORIGIN, unit: METRE }), "invalid-input", "Scene の root");
    assert(viewer.getState() === kept, "断った登録は状態を変えない");

    // scope と破棄。scene はシーンの差し替えで、単位の違う viewer も外す
    const sceneScoped = viewer.addGeometry(line, { id: "scene-scoped", origin: ORIGIN, unit: METRE });
    const viewerScoped = viewer.addGeometry(line, { id: "viewer-scoped", origin: ORIGIN, unit: METRE, scope: "viewer" });
    viewer.setScene(input);
    assert(sceneScoped.signal.reason === "scene" && !viewerScoped.signal.aborted, "scene の登録は差し替えで外す");
    viewer.setScene({ ...input, result: { ...input.result, unit: { kind: "LENGTHUNIT", name: "METRE", prefix: "MILLI" } } });
    assert(viewerScoped.signal.reason === "unit" && viewer.getState().objects.length === 0, "単位の違うシーンでは viewer の登録も外す");
    viewer.setScene(input);
    const failing = new Error("後始末の失敗");
    const broken = viewer.addObject(new THREE.Group(), { id: "broken", origin: ORIGIN, unit: METRE, dispose: () => { throw failing; } });
    throwsSame(() => broken.remove(), failing, "後始末の例外を返す");
    assert(broken.signal.aborted && viewer.getState().objects.length === 0, "後始末が失敗しても残りを行う");
    extras.objects = assertions;

    // 入力。正規化した入力で、選択・移動・回転・拡大・2 本指・中断を確かめる
    const camera0 = viewer.getState().views[0].camera;
    const events = [];
    const offInput = viewer.on("input", (event) => events.push(`${event.action}:${event.gesture}${event.reason ? `:${event.reason}` : ""}`));
    let picked = null;
    const offPick = viewer.on("pick", (event) => { picked = event; });
    const down = (x, y, extra = {}) => viewer.handleInput({ type: "pointerdown", pointerId: 1, button: 0, point: { x, y }, ...extra });
    const move = (x, y, extra = {}) => viewer.handleInput({ type: "pointermove", pointerId: 1, point: { x, y }, ...extra });
    const up = (x, y, extra = {}) => viewer.handleInput({ type: "pointerup", pointerId: 1, button: 0, point: { x, y }, ...extra });
    assert(down(30, 70) && up(31, 70), "クリックを扱う");
    assert(viewer.getState().views[0].selected.join() === "red" && picked.hits[0].id === "red" && picked.view === "top", "クリックで選ぶ");
    viewer.render();
    pixel(30, 130, SELECTED, "クリックで選んだ要素の色");
    down(180, 20); up(180, 20);
    assert(viewer.getState().views[0].selected.length === 0, "何もない所のクリックは選択を外す");
    down(30, 70); move(60, 70); up(60, 70);
    assert(viewer.getState().views[0].selected.length === 0, "動かしたら選ばない");
    assert(!viewer.handleInput({ type: "wheel", point: { x: 100, y: 100 }, deltaY: 100 }), "修飾キーのないホイールは DOM に残す");
    assert(!viewer.handleInput({ type: "pointerdown", pointerId: 3, button: 2, point: { x: 100, y: 100 } }), "右ボタンは DOM に残す");

    // 中ボタンで移動。Escape で元のカメラへ戻す
    assert(down(100, 100, { button: 1 }) && move(110, 100), "移動を扱う");
    near(viewer.getState().views[0].camera.position[0], ORIGIN[0] - 1, "10 px で 1 単位動かす");
    assert(viewer.handleInput({ type: "keydown", key: "Escape" }), "Escape を扱う");
    assert(viewer.getState().views[0].camera === camera0 || viewer.getState().views[0].camera.position[0] === ORIGIN[0], "Escape で元に戻す");
    assert(!move(120, 100), "中断後の移動は扱わない");
    down(100, 100, { button: 1 }); move(100, 90); up(100, 90);
    near(viewer.getState().views[0].camera.position[1], ORIGIN[1] - 1, "上へのドラッグでカメラは下へ動く");
    // Shift と中ボタンで注視点の周りに回す
    viewer.setViews([{ ...topView(), camera: { projection: "perspective", position: at(0, -20, 20), direction: [0, 1, -1], up: [0, 0, 1], fov: 45 } }]);
    const eye0 = viewer.getState().views[0].camera;
    down(100, 100, { button: 1, shiftKey: true }); move(130, 100, { shiftKey: true });
    const eye1 = viewer.getState().views[0].camera;
    const pivot = at(0, 0, 0);
    near(Math.hypot(...eye1.position.map((v, i) => v - pivot[i])), Math.hypot(...eye0.position.map((v, i) => v - pivot[i])), "注視点からの距離を保つ", 1e-2);
    assert(Math.abs(eye1.direction[0]) > 0.1 && eye1.up[2] === 1, "水平に回す");
    up(130, 100);
    move(130, 100);
    // Ctrl とホイールで拡大。平行投影は幅、透視投影は点の下へ近づく
    viewer.setViews([topView()]);
    assert(viewer.handleInput({ type: "wheel", point: { x: 100, y: 100 }, deltaY: -100, ctrlKey: true }), "Ctrl とホイール");
    near(viewer.getState().views[0].camera.width, 20 * Math.exp(-0.2), "幅を縮める");
    near(viewer.getState().views[0].camera.position[0], ORIGIN[0], "中心の点は動かない");
    viewer.setViews([topView()]);
    viewer.handleInput({ type: "wheel", point: { x: 30, y: 70 }, deltaY: 100, metaKey: true });
    const zoomed = viewer.getState().views[0];
    const ray = viewer.getRay("top", { x: 30, y: 70 });
    near(ray.origin[0] + ray.offset[0], ORIGIN[0] - 7, "点の下を保つ", 1e-3);
    assert(zoomed.camera.width > 20, "縮小");
    // 2 本指で移動と拡大
    viewer.setViews([topView()]);
    const touch = (type, id, x, y) => viewer.handleInput({ type, pointerId: id, pointerType: "touch", point: { x, y } });
    assert(touch("pointerdown", 11, 90, 100) && touch("pointerdown", 12, 110, 100), "2 本指を扱う");
    touch("pointermove", 11, 80, 100); touch("pointermove", 12, 120, 100);
    near(viewer.getState().views[0].camera.width, 10, "指の間を 2 倍にすると幅が半分");
    touch("pointerup", 12, 120, 100); touch("pointerup", 11, 80, 100);
    viewer.setViews([topView()]);
    touch("pointerdown", 21, 30, 70); touch("pointerup", 21, 30, 70);
    assert(viewer.getState().views[0].selected.join() === "red", "1 本指のタップで選ぶ");
    viewer.select(document, []);

    // 枠をまたぐ操作は押したビューに固定する
    viewer.setViews([topView("left", { x: 0, y: 0, width: 100, height: 200 }), topView("right", { x: 100, y: 0, width: 100, height: 200 })]);
    const right0 = JSON.stringify(viewer.getState().views[1].camera);
    down(50, 100, { button: 1 }); move(150, 100);
    assert(JSON.stringify(viewer.getState().views[1].camera) === right0 && viewer.getState().views[0].camera.position[0] < ORIGIN[0] - 5, "押したビューだけを動かす");
    assert(viewer.handleInput({ type: "pointercancel", pointerId: 1 }), "pointercancel");
    assert(viewer.getState().views[0].camera.position[0] === ORIGIN[0], "pointercancel で元に戻す");
    // ビュー・シーン・設定の変更と例外で中断する
    down(50, 100, { button: 1 }); move(60, 100);
    viewer.setViews([topView()]);
    assert(!move(70, 100), "ビューの差し替えで中断する");
    down(100, 100, { button: 1 });
    viewer.setScene(input);
    assert(!move(110, 100), "シーンの差し替えで中断する");
    viewer.setInputOptions({ pan: false });
    assert(!down(100, 100, { button: 1 }), "無効にした割り当て");
    viewer.setInputOptions({ pan: true });
    rejects(() => viewer.setInputOptions({ fly: true }), "invalid-input", "未知の入力の設定");
    const boom = new Error("通知の失敗");
    const offBoom = viewer.on("input", (event) => { if (event.action === "start") throw boom; });
    throwsSame(() => down(100, 100, { button: 1 }), boom, "通知の例外を返す");
    offBoom();
    assert(!move(110, 100), "例外の後は操作を残さない");
    const offNested = viewer.on("pick", () => { viewer.handleInput({ type: "keydown", key: "a" }); });
    down(30, 70);
    rejects(() => up(30, 70), "reentrant", "通知の中から入力を渡さない");
    offNested();
    assert(events.join() === ["start:pan", "cancel:pan:escape", "start:pan", "end:pan", "start:orbit", "end:orbit", "start:touch", "end:touch",
      "start:pan", "cancel:pan:pointercancel", "start:pan", "cancel:pan:views", "start:pan", "cancel:pan:scene", "start:pan", "cancel:pan:error"].join(), `操作の通知: ${events.join()}`);
    rejects(() => viewer.handleInput({ type: "pointerdown", pointerId: 1.5, point: { x: 0, y: 0 } }), "invalid-input", "不正な入力");
    offInput(); offPick();

    // DOM の接続。扱った操作だけ既定の動作を止め、解除後は扱わない
    viewer.setViews([topView()]);
    const box = canvas.getBoundingClientRect();
    const fire = (type, x, y, init = {}) => {
      const Event = type === "wheel" ? WheelEvent : PointerEvent;
      const event = new Event(type, { clientX: box.left + x, clientY: box.top + y, bubbles: true, cancelable: true, pointerId: 7, pointerType: "mouse", ...init });
      canvas.dispatchEvent(event);
      return event.defaultPrevented;
    };
    const detach = viewer.attachInput(canvas);
    assert(fire("pointerdown", 100, 100, { button: 1 }) && fire("pointermove", 110, 100, { button: 1 }), "DOM の移動を扱う");
    near(viewer.getState().views[0].camera.position[0], ORIGIN[0] - 1, "DOM の点を CSS px に写す", 1e-6);
    fire("pointerup", 110, 100, { button: 1 });
    assert(!fire("wheel", 100, 100, { deltaY: 10 }), "DOM のホイールは既定の動作に残す");
    assert(fire("wheel", 100, 100, { deltaY: 10, ctrlKey: true }), "DOM の Ctrl とホイール");
    detach();
    detach();
    assert(!fire("pointerdown", 100, 100, { button: 1 }), "解除後は扱わない");
    const again = viewer.attachInput(canvas);
    fire("pointerdown", 100, 100, { button: 1 });
    const live = viewer.addObject(new THREE.Group(), { id: "live", origin: ORIGIN, unit: METRE, scope: "viewer" });
    viewer.destroy();
    again();
    assert(live.signal.reason === "destroy" && !fire("pointermove", 120, 100, { button: 1 }), "破棄で登録と接続を外す");
    extras.input = assertions - extras.objects;
  } finally {
    viewer.destroy();
    canvas.remove();
  }
  return { assertions, extras };
}
