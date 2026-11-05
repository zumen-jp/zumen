import { readScene } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle, fixture } from "./fixtures.mjs";
import { findSource } from "../example/source.js";

export function testState({ assert, throws }) {
  const canvas = document.createElement("canvas");
  const viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio: 2, view: { center: { x: 0, y: 0 }, scale: 2, rotation: 0 } });
  const ctx = canvas.getContext("2d");
  const pixel = (expected, message) => {
    const rgb = [...ctx.getImageData(200, 140, 1, 1).data].slice(0, 3);
    assert(rgb.every((value, i) => Math.abs(value - expected[i]) < 4), `${message}: ${rgb}`);
  };
  const columns = fixture();
  const bytes = new TextEncoder().encode("layer/0");
  columns.layer_ids = [columns.strings.length, bytes.length]; columns.strings.push(...bytes);
  columns.unit_layers[1] = 0;
  const input = bundle(columns);
  const first = { id: "space/0/h1", path: [{ id: "space/0/hinsert", column: 2, row: 3 }] };
  const second = { id: "space/0/h2", path: [] };
  const notifications = [], events = [];
  const offState = viewer.subscribe((state) => { assert(state === viewer.getState(), "通知は現在の状態"); notifications.push(state); });
  const offSelect = viewer.on("selection", (event) => events.push(event));
  assert(notifications.length === 0, "購読の登録では通知しない");
  viewer.setScene(input);
  const initial = viewer.getState();
  assert(initial === viewer.getState() && Object.isFrozen(initial) && initial.scene === viewer.getScene(), "状態は凍結した同じ参照");
  assert(initial.layers.join() === "layer/0" && viewer.getLayers() === initial.layers, "レイヤの一覧");
  viewer.render(); viewer.jumpTo(viewer.getView()); viewer.resize(200, 200, 2);
  assert(viewer.getState() === initial && notifications.length === 1, "変化のない描画とカメラは通知しない");

  viewer.select([first, first]);
  assert(viewer.getState().selection.length === 1 && events.length === 1 && events[0].type === "selection", "選択の重複と種類別の通知");
  const chosen = viewer.getState().selection[0];
  assert(Object.isFrozen(chosen.path[0]) && chosen !== first, "利用側の参照を保管しない");
  pixel([255, 136, 0], "選択した配置の色");
  const selected = viewer.getState();
  viewer.select([first]);
  assert(viewer.getState() === selected, "同じ選択は通知しない");
  first.path[0].row = 99;
  assert(chosen.path[0].row === 3, "利用側の変更を持ち込まない");
  throws(() => viewer.select([first]), "未知の配置の参照を拒否");
  assert(viewer.getState() === selected, "拒否した選択で状態を変えない");
  viewer.select([second], "add");
  assert(viewer.getState().selection.length === 2, "選択に追加");
  viewer.select([chosen], "remove");
  assert(viewer.getState().selection[0].id === second.id, "選択から除去");
  viewer.select([chosen, second], "toggle");
  assert(viewer.getState().selection.length === 1 && viewer.getState().selection[0] === chosen, "選択の切り替え");
  viewer.setHighlight("search", [chosen], "#ff0000");
  pixel([255, 136, 0], "選択は名前付き強調より上");
  viewer.setHovered(chosen); pixel([0, 170, 204], "hover は選択より上");
  viewer.select([]); viewer.setHovered(null); pixel([255, 0, 0], "名前付き強調");
  viewer.setHighlight("other", [chosen], "#00ff00"); pixel([0, 255, 0], "後の名前付き強調を優先");
  viewer.setHighlight("search", [chosen], "#ff0000"); pixel([255, 0, 0], "同じ名前の更新を最後にする");
  viewer.setHighlight("search", []); pixel([0, 255, 0], "名前付き強調を除去");
  viewer.setHighlight("other", []); pixel([0, 0, 255], "元の色へ戻る");
  throws(() => viewer.setHighlight("x", [chosen], "invalid-color"), "無効な強調色を拒否");
  viewer.select([chosen]); viewer.setHovered(chosen); viewer.setHighlight("saved", [chosen]);
  viewer.setHiddenLayers(["layer/0", "layer/0"]);
  assert(viewer.getState().hiddenLayers.length === 1 && viewer.getReport().drawn.lines === 0, "非表示レイヤを描画数から除く");
  assert(viewer.getState().selection[0] === chosen, "非表示でも選択を保持");
  pixel([255, 0, 0], "強調しても非表示は描かない");
  const hidden = viewer.getState();
  throws(() => viewer.setLayersVisible(["unknown"], true), "未知のレイヤを拒否");
  assert(viewer.getState() === hidden, "拒否したレイヤで状態を変えない");
  viewer.setLayersVisible(["layer/0"], true); pixel([0, 170, 204], "再表示で強調を戻す");
  viewer.setLayersVisible(["layer/0"], false);
  const invalid = fixture(); invalid.line_units[0] = 999;
  const before = viewer.getState();
  throws(() => viewer.setScene(bundle(invalid)), "不正な差し替えを拒否");
  assert(viewer.getState() === before, "不正な差し替えで選択も保持");
  viewer.setScene(input);
  assert(viewer.getState().selection.length === 0 && viewer.getState().highlights.length === 0 && viewer.getState().hiddenLayers.length === 0 && viewer.getState().hovered === null, "同じシーンの再設定でも状態を消す");
  assert(viewer.getState().view === initial.view && notifications.length > 1, "差し替えでカメラと購読を保持");
  offState(); offSelect(); offState();
  const count = notifications.length, eventCount = events.length;
  viewer.select([chosen]);
  assert(notifications.length === count && events.length === eventCount, "解除した関数へ通知しない");
  const currentChosen = viewer.getState().selection[0];

  let notified = 0;
  const offBad = viewer.subscribe(() => viewer.select([]));
  const offGood = viewer.subscribe(() => { notified++; });
  throws(() => viewer.setHovered(chosen), "通知中の状態変更を拒否");
  assert(notified === 1 && viewer.getState().hovered === currentChosen && viewer.getState().selection[0] === currentChosen, "通知の例外でも残りを呼び状態は更新済み");
  offBad(); offGood();
  let skipped = 0, late = 0;
  let offRemoved, offLate;
  const offRemoving = viewer.subscribe(() => { offRemoved(); offLate = viewer.subscribe(() => late++); });
  offRemoved = viewer.subscribe(() => skipped++);
  viewer.setHovered(null);
  assert(skipped === 0 && late === 0, "通知中の解除と追加の寿命");
  offRemoving(); viewer.setHovered(chosen); assert(late === 1, "追加した購読は次の変更から"); offLate();
  const offUndefined = viewer.subscribe(() => { throw undefined; });
  let caughtUndefined = false;
  try { viewer.setHovered(null); } catch (error) { caughtUndefined = error === undefined; }
  assert(caughtUndefined, "undefined の例外も返す"); offUndefined();
  viewer.setScene(null);
  assert(viewer.getLayers().length === 0 && viewer.getState().scene === null, "消去の状態");
  throws(() => viewer.select([chosen]), "シーンのない選択を拒否");
  viewer.destroy(); viewer.destroy(); offState();
  throws(() => viewer.getState(), "破棄後の状態取得");
  throws(() => viewer.subscribe(() => {}), "破棄後の購読");

  const limited = new View2D(document.createElement("canvas"), { width: 100, height: 100, limits: { stateItems: 2 } });
  limited.setScene(input); limited.select([chosen, second]);
  const full = limited.getState();
  throws(() => limited.setHovered(chosen), "状態全体の上限");
  assert(limited.getState() === full, "状態全体の上限でも変更を取り消す");
  limited.subscribe(() => {}); limited.subscribe(() => {});
  throws(() => limited.subscribe(() => {}), "購読の上限"); limited.destroy();
}

export function testPlacedSelection(zumen, { assert, throws }) {
  const session = zumen.open();
  const viewer = new View2D(document.createElement("canvas"), { width: 200, height: 200 });
  try {
    // 同じ定義を別の INSERT と、2 列 2 行の格子で配置する。
    const text = [0,"SECTION",2,"BLOCKS",0,"BLOCK",5,"10",2,"MARK",70,0,10,0,20,0,
      0,"LINE",5,"11",8,"0",10,0,20,0,11,5,21,0,0,"ENDBLK",0,"ENDSEC",
      0,"SECTION",2,"ENTITIES",0,"INSERT",5,"20",8,"0",2,"MARK",10,0,20,0,70,2,71,2,44,20,45,20,
      0,"INSERT",5,"21",8,"0",2,"MARK",10,50,20,0,0,"ENDSEC",0,"EOF",""].join("\n");
    session.push("placed", new TextEncoder().encode(text));
    session.call({ v: 1, op: "open", input: "placed" });
    const drawing = session.call({ v: 1, op: "drawing", doc: "placed" }).result;
    const result = session.call({ v: 1, op: "scene", doc: "placed", from: "space/0" }).result;
    const scene = readScene(session.take(result.blob));
    const document = { doc: "placed", format: "dxf", hash: drawing.hash };
    viewer.setScene({ scene, result, document });
    const refs = [[2,0],[22,0],[2,20],[22,20],[52,0]].map(([x,y]) => viewer.hitTest(viewer.toScreen({ x, y }), 0)[0]);
    assert(refs.every(Boolean) && new Set(refs.map((r) => r.id)).size === 1, "同じ定義の要素を 5 つの配置で検出");
    assert(new Set(refs.map((r) => JSON.stringify(r.path))).size === 5, "INSERT と格子の列・行を区別");
    viewer.select(refs);
    assert(viewer.getState().selection.length === 5, "同じ ID の配置をまとめない");
    const sources = refs.map((ref) => findSource(session, document, ref));
    assert(sources.every((entry) => Number.isInteger(entry?.source?.record) && entry.source.bytes === undefined), "定義の要素からレコードを読み、未提供のバイト範囲は作らない");
    assert(new Set(sources.map((entry) => entry.source.record)).size === 1 && sources[0].path === refs[0].path, "元のレコードは共通で配置の道筋は保持");
    throws(() => findSource(session, { ...document, hash: "old" }, refs[0]), "異なる版の出所を返さない");

    // 現行の DXF はバイト範囲を返さない。bytes を持つ API 応答のページ送りは合成応答で確かめる。
    let calls = 0;
    const paged = { call(request) {
      calls++;
      if (request.op === "drawing") return { result: { hash: document.hash } };
      assert(request.place === refs[0].id.slice(0, refs[0].id.lastIndexOf("/")) && request.fields.join() === "source", "定義の空間から出所を要求");
      return { result: request.offset === 0 ? { items: [], next_offset: 256 } : {
        items: [{ id: refs[0].id, source: { record: 7, bytes: { start: 10, end: 40 } } }],
      } };
    } };
    const ranged = findSource(paged, document, refs[0]);
    assert(calls === 3 && ranged.source.record === 7 && ranged.source.bytes.start === 10 && ranged.source.bytes.end === 40, "ページをたどり応答のバイト範囲をそのまま返す");
    throws(() => findSource(paged, document, refs[0], 1), "出所のページ送りにも上限");
  } finally { viewer.destroy(); session.close(); }
}
