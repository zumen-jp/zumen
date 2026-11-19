// 地面の 2D のアダプター（#816）をブラウザーの WebGPU で検査する。座標と ID は View2D の変換と当たり判定を独立の期待値にする。
import { View2D } from "../../view2d/src/index.js";
import { bundle, fixture } from "../../view2d/test/fixtures.mjs";
import { View3D, View3DError } from "../src/index.js";
import { attachGround2D } from "../src/ground2d.js";
import { document, encode, ORIGIN, result, synthetic } from "./fixture.mjs";

const WHITE = [255, 255, 255], RED = [255, 0, 0];
const ORIGIN_2D = 1e7;
/** 3D の上からの平行投影。幅 120 単位を 200 px に写す（1 px は 0.6 単位）。 */
const wide = { id: "wide", rect: { x: 0, y: 0, width: 200, height: 200 },
  camera: { projection: "orthographic", position: [ORIGIN[0], ORIGIN[1], 10], direction: [0, 0, -1], up: [0, 1, 0], width: 120 } };
const plane = (z = -1, yAxis = [0, 1, 0]) => ({ origin: [ORIGIN[0], ORIGIN[1], z], xAxis: [1, 0, 0], yAxis, anchor: { x: ORIGIN_2D, y: ORIGIN_2D } });
/** 3D の画面の点の真下の、平面の上の 2D の起点の座標の差。 */
const under = (px, py) => ({ x: (px - 100) * 0.6, y: (100 - py) * 0.6 });

export async function testGround2D() {
  let assertions = 0;
  const assert = (ok, message) => { assertions++; if (!ok) throw new Error(message); };
  const near = (a, b, message, tolerance = 1e-6) => assert(Math.abs(a - b) <= tolerance, `${message}: ${a} / ${b}`);
  const rejects = (fn, reason, message) => {
    let caught = null;
    try { fn(); } catch (error) { caught = error; }
    assert(caught instanceof View3DError && caught.reason === reason, `${message}: ${caught?.reason ?? caught}`);
  };
  // 表示部品の入口が View2D とアダプターを読み込まないこと
  const entry = await (await fetch("/packages/view3d/src/index.js")).text();
  assert(!entry.includes("ground2d") && !entry.includes("view2d"), "表示部品の入口は地面の 2D と View2D を読み込まない");
  const adapter = await (await fetch("/packages/view3d/src/ground2d.js")).text();
  assert(!/from\s+["'][^"']*view2d/.test(adapter), "アダプターは View2D を読み込まず、利用側の実体を受け取る");

  const canvas = Object.assign(globalThis.document.createElement("canvas"), { width: 200, height: 200 });
  globalThis.document.body.append(canvas);
  const viewer = await View3D.create(canvas, { width: 200, height: 200, shading: "unlit", edges: false, views: [wide] });
  let shot3d = null;
  const render = () => { shot3d = null; return viewer.render(); };
  const read = async (x, y) => {
    if (shot3d === null) shot3d = await viewer.snapshot("wide");
    const offset = (y * shot3d.width + x) * 4;
    return [shot3d.pixels[offset], shot3d.pixels[offset + 1], shot3d.pixels[offset + 2]];
  };
  const pixel = async (x, y, expected, message, tolerance = 3) => {
    const value = await read(x, y);
    assert(value.every((v, i) => Math.abs(v - expected[i]) <= tolerance), `${message}: ${value} / ${expected}`);
  };
  const { columns } = synthetic();
  const input3d = { blob: encode(columns).bytes, result: result(columns), document };
  viewer.setScene(input3d);

  // 2D: 大きな原点の合成の図面を、回転と pixel ratio 2 で 100 CSS px の Canvas に描く。1 CSS px は 1 m。
  const flat = Object.assign(globalThis.document.createElement("canvas"), { width: 200, height: 200 });
  const view2d = await View2D.create(flat, { width: 100, height: 100, pixelRatio: 2, background: "#ffff00" });
  const metre = bundle(fixture(ORIGIN_2D), { unit: "meter" });
  view2d.setScene(metre);
  view2d.jumpTo({ center: { x: ORIGIN_2D, y: ORIGIN_2D }, scale: 1, rotation: 0.3 });
  const snap2d = await view2d.snapshot();
  const read2d = (x, y) => {
    const px = Math.max(0, Math.min(snap2d.width - 1, Math.floor(x * 2)));
    const py = Math.max(0, Math.min(snap2d.height - 1, Math.floor(y * 2)));
    const offset = (py * snap2d.width + px) * 4;
    return [snap2d.pixels[offset], snap2d.pixels[offset + 1], snap2d.pixels[offset + 2]];
  };

  try {
    rejects(() => attachGround2D(viewer, { id: "g", view2d, canvas: flat, plane: { ...plane(), yAxis: [1, 1, 0] } }), "invalid-input", "直交しない軸は拒む");
    rejects(() => attachGround2D(viewer, { id: "g", view2d, canvas: flat, plane: plane(), extra: 1 }), "invalid-input", "未知の項目は拒む");
    rejects(() => attachGround2D(viewer, { id: "g", view2d, canvas: flat, plane: plane(), scale: 0 }), "invalid-input", "0 の尺度は拒む");

    const ground = attachGround2D(viewer, { id: "ground", view2d, canvas: flat, plane: plane() });
    render();
    let report = ground.getReport();
    assert(report.status === "ready" && report.scale === 1, `m の 2D と m の 3D は 1:1: ${JSON.stringify(report)}`);
    assert(report.texture.width === 200 && report.texture.height === 200, "texture は pixel ratio を反映した Canvas の画素");
    near(report.resolution.scene, 0.5, "texture の 1 画素は 0.5 m");
    near(report.resolution.document, 0.5, "3D の文書単位でも 0.5");
    assert(report.issues === view2d.getReport().issues, "元の View2D の未描画・代替の報告を返す");
    assert(viewer.getState().objects.some((o) => o.id === "ground" && o.kind === "object" && !o.pickable), "地面は追加描画として登録し、要素の当たりに混ぜない");

    // 交点を元の 2D の座標・CSS px に戻し、View2D の当たり判定と照らす
    let matched = 0, compared = 0;
    for (let py = 20; py <= 180; py += 16) {
      for (let px = 20; px <= 180; px += 16) {
        const hit = ground.hitTest("wide", { x: px + 0.5, y: py + 0.5 });
        if (hit.status === "occluded") continue;
        const expected = under(px + 0.5, py + 0.5);
        const screen = view2d.toScreen({ origin: { x: ORIGIN_2D, y: ORIGIN_2D }, offset: expected });
        const inside = screen.x >= 0 && screen.x <= 100 && screen.y >= 0 && screen.y <= 100;
        if (!inside) { assert(hit.status === "miss" && hit.reason === "outside", `枠の外: ${px},${py} ${hit.status}`); continue; }
        assert(hit.status === "hit", `枠の中は当たる: ${px},${py} ${hit.reason}`);
        near(hit.scene.origin.x, ORIGIN_2D, "2D の原点 x", 0);
        near(hit.scene.offset.x, expected.x, `2D の差 x ${px},${py}`, 1e-6);
        near(hit.scene.offset.y, expected.y, `2D の差 y ${px},${py}`, 1e-6);
        near(hit.screen.x, screen.x, "2D の CSS px x", 1e-6);
        near(hit.screen.y, screen.y, "2D の CSS px y", 1e-6);
        assert(JSON.stringify(hit.elements) === JSON.stringify(view2d.hitTest(screen, 3)), `View2D の当たりと同じ: ${px},${py}`);
        near(hit.distance, 11, "距離は文書単位");
        // 画素: 2D の Canvas の周りが一様なら、3D の画素も同じ色
        const base = read2d(screen.x, screen.y);
        const uniform = [[-2, 0], [2, 0], [0, -2], [0, 2]].every(([dx, dy]) => read2d(screen.x + dx, screen.y + dy).every((v, i) => Math.abs(v - base[i]) <= 2));
        if (uniform) { compared++; await pixel(px, py, base, `2D の画素と同じ色 ${px},${py}`, 12); }
        if (hit.elements.length > 0) matched++;
      }
    }
    assert(compared >= 20 && matched >= 1, `画素と ID を照らした点の数: ${compared}・${matched}`);
    const line = ground.hitTest("wide", { x: 100 + 6 / 0.6, y: 100 - 15 / 0.6 });
    assert(line.status === "hit" && line.elements.some((e) => e.id === "space/0/h1"), `線の要素を ID で返す: ${JSON.stringify(line.elements)}`);
    await pixel(2, 2, WHITE, "2D の枠の外は 3D の背景");

    // 3D の要素との重なり。地面は z = -1、赤の要素は z = 0
    const redPoint = { x: 100 + -7 / 0.6, y: 100 - 3 / 0.6 };
    const covered = ground.hitTest("wide", redPoint);
    assert(covered.status === "occluded" && covered.occluder.id === "red" && covered.occluder.distance < covered.distance, `手前の 3D の要素で隠れる: ${covered.status}`);
    await pixel(Math.round(redPoint.x), Math.round(redPoint.y), RED, "手前の要素を描く");

    // 2D の状態の変化を、複製せず同じ Canvas から転送する
    const h1 = view2d.getElements().find((e) => e.id === "space/0/h1");
    const before = await read(Math.round(100 + 6 / 0.6), Math.round(100 - 15 / 0.6));
    view2d.setHighlight("mark", [h1], "#ff00ff");
    render();
    const after = await read(Math.round(100 + 6 / 0.6), Math.round(100 - 15 / 0.6));
    assert(JSON.stringify(before) !== JSON.stringify(after), `2D の強調が 3D の地面に出る: ${before} → ${after}`);
    view2d.setHighlight("mark", []);

    // 回転と表示範囲の変更で四隅を当て直す
    view2d.jumpTo({ center: { x: ORIGIN_2D + 20, y: ORIGIN_2D }, scale: 2, rotation: -0.5 });
    render();
    const moved = ground.hitTest("wide", { x: 120.5, y: 100.5 });
    const movedExpected = under(120.5, 100.5);
    assert(moved.status === "hit", `表示範囲の変更後も当たる: ${moved.reason}`);
    near(moved.screen.x, view2d.toScreen({ origin: { x: ORIGIN_2D, y: ORIGIN_2D }, offset: movedExpected }).x, "回転後の CSS px");
    near(ground.getReport().resolution.scene, 0.25, "拡大で 1 画素の長さが変わる");
    const far = ground.hitTest("wide", { x: 30.5, y: 100.5 });
    assert(far.status === "miss" && far.reason === "outside", "縮めた枠の外");

    // 切断面の外は描かず選ばない
    viewer.setViews([{ ...wide, clips: [{ point: [ORIGIN[0], ORIGIN[1], -0.5], normal: [0, 0, -1] }] }]);
    render();
    const clipped = ground.hitTest("wide", { x: 120.5, y: 100.5 });
    assert(clipped.status === "miss" && clipped.reason === "clipped", "切断面で除いた地面は当たらない");
    await pixel(120, 100, WHITE, "切断面で除いた地面は描かない");
    viewer.setViews([wide]);

    // 単位: 紙・単位不明・不一致は重ねず理由を返す。差し替えで戻る
    const reasons = [];
    for (const [changes, expected] of [[{ from: "sheet/h1" }, "paper"], [{ view: 1 }, "paper"], [{ from: "block/h2" }, "definition"], [{ unit: "unknown" }, "unit-unknown"]]) {
      view2d.setScene(bundle(fixture(ORIGIN_2D), { unit: "meter", ...changes }));
      render();
      reasons.push(ground.getReport().reason);
      assert(ground.getReport().status === "unavailable" && ground.getReport().reason === expected, `重ねない理由: ${expected} / ${ground.getReport().reason}`);
      assert(ground.hitTest("wide", { x: 120.5, y: 100.5 }).reason === expected, "当たり判定も同じ理由");
      await pixel(120, 100, WHITE, `重ねない 2D は描かない: ${expected}`);
    }
    view2d.setScene(bundle(fixture(ORIGIN_2D), { unit: "millimeter" }));
    assert(ground.getReport().status === "ready" && ground.getReport().scale === 0.001, "mm の 2D は 3D の m へ 0.001 倍");
    view2d.setScene(metre);
    assert(ground.getReport().status === "ready" && ground.getReport().scale === 1, "2D の差し替えで戻る");
    const explicit = attachGround2D(viewer, { id: "explicit", view2d, canvas: flat, plane: plane(-2), scale: 1000 });
    assert(explicit.getReport().reason === "unit-mismatch", "単位と食い違う尺度は重ねない");
    explicit.remove();
    view2d.setScene(bundle(fixture(ORIGIN_2D), { unit: "unknown" }));
    const unknown = attachGround2D(viewer, { id: "unknown", view2d, canvas: flat, plane: plane(-2), scale: 2 });
    assert(unknown.getReport().status === "ready" && unknown.getReport().scale === 2, "単位の分からない 2D は明示した尺度で重ねる");
    unknown.remove();
    view2d.setScene(metre);
    view2d.jumpTo({ center: { x: ORIGIN_2D, y: ORIGIN_2D }, scale: 1, rotation: 0.3 });

    // 画素の上限
    const limited = attachGround2D(viewer, { id: "limited", view2d, canvas: flat, plane: plane(-2), maxSide: 100 });
    assert(limited.getReport().reason === "limit" && limited.getReport().texture === null, "一辺の上限を超える Canvas は重ねない");
    limited.remove();
    const pixels = attachGround2D(viewer, { id: "pixels", view2d, canvas: flat, plane: plane(-2), maxPixels: 39999 });
    assert(pixels.getReport().reason === "limit", "画素数の上限");
    pixels.remove();

    // 表裏: 法線（xAxis × yAxis）の裏から見ると front では描かず選ばない
    ground.remove();
    const back = attachGround2D(viewer, { id: "back", view2d, canvas: flat, plane: plane(-1, [0, -1, 0]), side: "front" });
    render();
    assert(back.hitTest("wide", { x: 120.5, y: 100.5 }).reason === "back", "裏から見た front の地面は当たらない");
    await pixel(120, 100, WHITE, "裏から見た front の地面は描かない");
    back.remove();
    const both = attachGround2D(viewer, { id: "both", view2d, canvas: flat, plane: plane(-1, [0, -1, 0]) });
    render();
    const mirrored = both.hitTest("wide", { x: 120.5, y: 100.5 });
    assert(mirrored.status === "hit" && Math.abs(mirrored.scene.offset.y + under(120.5, 100.5).y) < 1e-6, "double は裏からも描き、y を反転して戻す");
    both.remove();

    // 透過: 背景が透明な 2D を要素の上に敷くと、3D の要素が透けて見える
    const clear = Object.assign(globalThis.document.createElement("canvas"), { width: 200, height: 200 });
    const clear2d = await View2D.create(clear, { width: 100, height: 100, pixelRatio: 2, background: "rgba(0,0,0,0)" });
    clear2d.setScene(metre);
    clear2d.jumpTo({ center: { x: ORIGIN_2D - 100, y: ORIGIN_2D }, scale: 1, rotation: 0 });
    const over = attachGround2D(viewer, { id: "over", view2d: clear2d, canvas: clear, plane: { ...plane(1), anchor: { x: ORIGIN_2D - 100, y: ORIGIN_2D } } });
    render();
    const empty = clear2d.hitTest(clear2d.toScreen({ origin: { x: ORIGIN_2D - 100, y: ORIGIN_2D }, offset: { x: -7, y: 3 } }), 3);
    assert(empty.length === 0, "赤の要素の上は 2D の要素がない透明な部分");
    await pixel(Math.round(redPoint.x), Math.round(redPoint.y), RED, "透明な部分から 3D の要素が見える");
    const above = over.hitTest("wide", redPoint);
    assert(above.status === "hit" && above.distance < 11, "上に敷いた地面は手前で当たる");
    over.remove();
    clear2d.destroy();

    // 同じ建物の ID の連動。対応は利用側が渡す
    const linked = attachGround2D(viewer, { id: "linked", view2d, canvas: flat, plane: plane(), link: {
      document, to3d: (ref) => (ref.id === "space/0/h1" ? ["red"] : []),
      to2d: (id) => (id === "red" ? [h1] : [{ id: "missing", path: [] }]),
    } });
    view2d.select([h1]);
    assert(JSON.stringify(viewer.getState().views[0].selected) === '["red"]', "2D の選択を 3D へ");
    viewer.select(document, ["mirror", "red"]);
    assert(view2d.getState().selection.length === 1 && view2d.getState().selection[0].id === "space/0/h1", `3D の選択を 2D へ: ${JSON.stringify(view2d.getState().selection)} ${linked.getReport().link.firstFailure}`);
    assert(linked.getReport().link.unmatched === 1, "2D にない参照は数えて捨てる");
    view2d.select([]);
    assert(viewer.getState().views[0].selected.length === 0, "2D の解除を 3D へ");
    linked.remove();
    linked.remove();
    view2d.select([h1]);
    assert(viewer.getState().views[0].selected.length === 0, "解除の後は連動しない");
    assert(linked.signal.aborted && linked.getReport().status === "removed" && linked.hitTest("wide", { x: 1, y: 1 }).reason === "removed", "解除の後の状態");
    assert(!viewer.getState().objects.some((o) => o.id === "linked"), "登録を外す");

    // 寿命: 3D のシーンの差し替えと破棄
    const scoped = attachGround2D(viewer, { id: "scoped", view2d, canvas: flat, plane: plane() });
    const kept = attachGround2D(viewer, { id: "kept", view2d, canvas: flat, plane: plane(-2), scope: "viewer" });
    viewer.setScene(input3d);
    assert(scoped.signal.aborted && scoped.signal.reason === "scene" && !kept.signal.aborted, "scene の地面はシーンの差し替えで外す");
    const milli = { ...input3d, result: { ...result(columns), unit: { kind: "LENGTHUNIT", name: "METRE", prefix: "MILLI" } } };
    viewer.setScene(milli);
    assert(kept.signal.aborted && kept.signal.reason === "unit", "単位の違うシーンでは viewer の地面も外す");
    const scaled = attachGround2D(viewer, { id: "scaled", view2d, canvas: flat, plane: plane() });
    assert(scaled.getReport().scale === 1000, "3D が mm なら m の 2D は 1000 倍");
    viewer.destroy();
    assert(scaled.signal.aborted && scaled.signal.reason === "destroy", "破棄で外す");
    view2d.select([]);
    const empty3d = await View3D.create(Object.assign(globalThis.document.createElement("canvas"), { width: 10, height: 10 }), { width: 10, height: 10 });
    try { rejects(() => attachGround2D(empty3d, { id: "x", view2d, canvas: flat, plane: plane() }), "no-scene", "3D のシーンがなければ拒む"); }
    finally { empty3d.destroy(); }
  } finally {
    viewer.destroy();
    view2d.destroy();
    canvas.remove();
  }
  return { assertions };
}
