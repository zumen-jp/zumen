import { readScene } from "@zumen-jp/zumen";
import { ReviewLayer, ReviewStore, reviewReference, reviewSceneId, View2D } from "../src/index.js";

const DXF = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n" +
  "0\nLINE\n5\nA1\n8\n0\n10\n0.0\n20\n0.0\n11\n50.0\n21\n30.0\n0\nENDSEC\n0\nEOF\n";
const STYLE = { color: "#ff0000", line_weight: 0.35, text_height: 2.5, text_padding: 0.5, arrow_length: 2, arrows: "end", cloud_pitch: 3, marker_size: 2 };

function documentOf(hash) {
  return {
    format: "zumen-review", version: 1, x_note: "保つ",
    targets: [{ id: "plan", format: "dxf", hash }, { id: "other", format: "dxf", hash: "v2:00000000000000000000000000000000" }],
    topics: [{ id: "t1", title: "寸法", status: "open", x_topic: 1, locations: [
      { id: "l1", target: "plan", kind: "2d", view: "space/0", pin: [5, 5] },
      { id: "l3", target: "plan", kind: "3d", camera: { projection: "orthographic", width: 10, position: [0, 0, 10], direction: [0, 0, -1], up: [0, 1, 0] } },
    ] }],
    markups: [{ id: "m1", kind: "redline", target: "plan", view: "space/0", points: [[0, 0], [10, 0]], topic: "t1", x_markup: true }],
  };
}

/** 編集の状態だけを確かめる。コアを呼ばない */
export function testReviewStore({ assert, throws }) {
  let next = 0;
  const store = new ReviewStore(JSON.stringify(documentOf("v2:1")), {
    createId: () => `n${++next}`, stamp: () => ({ author: "検査", time: "2026-11-18T00:00:00Z" }), limits: { history: 2 },
  });
  const events = [];
  const stop = store.subscribe((state) => events.push(state.revision));
  const docs = [];
  store.on("document", (event) => docs.push(event.change.reason));
  assert(store.getState() === store.getState() && Object.isFrozen(store.getDocument()), "状態は凍結した同じ値を返す");

  const id = store.addMarkup({ kind: "cloud", target: "plan", view: "space/0", points: [[20, 0], [30, 0], [30, 10]] });
  assert(id === "n1" && store.getDocument().markups[1].id === "n1", "利用側の関数で ID を作る");
  throws(() => store.addMarkup({ id: "m1", kind: "text", target: "plan", points: [[0, 0]] }), "ID の重複");
  throws(() => store.addMarkup({ kind: "arrow", target: "plan", points: [] }), "未知の種類");
  throws(() => store.addMarkup({ kind: "text", target: "none", points: [[0, 0]] }), "ない対象");
  throws(() => store.addMarkup({ kind: "text", target: "plan", points: [[0, Infinity]] }), "有限でない座標");
  throws(() => store.updateMarkup("m1", { x_markup: false }), "未知のキーは編集で変えない");

  store.updateMarkup("m1", { points: [[0, 0], [20, 0]] });
  const m1 = store.getDocument().markups[0];
  assert(m1.x_markup === true && m1.points[1][0] === 20 && m1.topic === "t1", "未知のキーと変えないキーを保つ");
  store.updateMarkup("m1", { topic: null });
  assert(!("topic" in store.getDocument().markups[0]), "null で省略できるキーを消す");
  throws(() => store.updateMarkup("m1", { kind: null }), "必須のキーは消さない");

  store.select([{ kind: "markup", id: "m1" }, { kind: "location", topic: "t1", id: "l1" }]);
  assert(store.getState().selection.length === 2, "選択");
  throws(() => store.select([{ kind: "markup", id: "zz" }]), "ない項目は選べない");
  store.select([{ kind: "markup", id: "m1" }], "toggle");
  assert(store.getState().selection.length === 1 && store.getState().selection[0].kind === "location", "選択を切り替える");

  // 履歴は 2 段まで。古い段を捨てる
  assert(store.undo() && store.undo() && !store.undo(), "上限の 2 段まで戻す");
  assert(store.getDocument().markups[0].points[1][0] === 10 && store.getDocument().markups.length === 2, "戻した文書");
  assert(store.redo() && store.getDocument().markups[0].points[1][0] === 20, "やり直す");
  store.updateMarkup("m1", { text: "a" });
  assert(!store.getState().canRedo, "新しい変更でやり直しを捨てる");

  const topic = store.addTopic({ title: "新しい指摘", status: "open", labels: ["構造"] });
  const added = store.getDocument().topics[1];
  assert(topic === "n2" && added.author === "検査" && added.created === "2026-11-18T00:00:00Z" && added.locations.length === 0, "作者と時刻は stamp から付ける");
  store.updateTopic("t1", { status: "resolved" });
  const t1 = store.getDocument().topics[0];
  assert(t1.status === "resolved" && t1.modified_by === "検査" && t1.x_topic === 1 && t1.locations.length === 2, "指摘の更新で場所と未知のキーを保つ");
  const location = store.addLocation(topic, { target: "plan", view: "space/0", pin: [1, 2], extent: [0, 0, 4, 4] });
  throws(() => store.addLocation(topic, { target: "plan", extent: [4, 0, 0, 4] }), "範囲の順");
  throws(() => store.updateLocation("t1", "l3", { pin: [0, 0] }), "3D の場所は編集しない");
  store.updateMarkup(id, { topic });
  throws(() => store.removeTopic(topic), "図形が参照する指摘は消さない");

  // まとめた変更は 1 回で戻し、例外なら変更を捨てる
  const before = store.getDocument();
  store.batch(() => { store.removeMarkup(id); store.removeLocation(topic, location); store.removeTopic(topic); });
  assert(store.getDocument().topics.length === 1 && docs[docs.length - 1] === "remove", "まとめた変更");
  store.undo();
  assert(store.getDocument() === before, "まとめた変更を 1 回で戻す");
  throws(() => store.batch(() => { store.removeMarkup(id); throw new RangeError("中断"); }), "batch の例外");
  assert(store.getDocument() === before, "例外で変更を捨てる");

  // 消した項目は選択から外す
  store.select([{ kind: "location", topic: "t1", id: "l1" }]);
  store.removeLocation("t1", "l1");
  assert(store.getState().selection.length === 0, "消した項目を選択から外す");
  store.undo();

  store.setFilter({ statuses: ["open"] });
  assert(store.items().map((item) => item.ref.id).join() === "n1,n3", "指摘の状態で図形と場所を絞る");
  store.setFilter({ kinds: ["location"], topics: ["t1"] });
  const items = store.items();
  assert(items.length === 1 && items[0].ref.id === "l1" && items[0].sceneId === "review/location/7431/6c31", "3D の場所を除き、シーンの ID を付ける");
  const filtered = JSON.parse(store.filteredJSON());
  assert(filtered.markups.length === 0 && filtered.topics[0].locations.length === 2 && filtered.x_note === "保つ", "絞った文書は 3D の場所と未知のキーを保つ");
  store.setFilter({});

  // 往復で ID と未知のキーを保つ
  const again = new ReviewStore(store.toJSON());
  assert(again.toJSON() === store.toJSON() && JSON.parse(again.toJSON()).x_note === "保つ", "JSON の往復");
  assert(reviewSceneId({ kind: "markup", id: "確" }) === "review/markup/e7a2ba", "UTF-8 の 16 進");
  const ref = reviewReference({ id: "review/location/7431/6c31", kind: "location", topic: "t1", location: "l1" });
  assert(ref.kind === "location" && ref.topic === "t1" && ref.id === "l1", "sources の 1 件から参照へ");

  // 通知の中では状態を変えない
  const off = store.on("filter", () => { store.select([]); });
  throws(() => store.setFilter({ kinds: ["text"] }), "通知の中の変更");
  off(); stop();
  const count = events.length;
  store.load(documentOf("v2:2"));
  assert(events.length === count && !store.getState().canUndo && docs[docs.length - 1] === "load", "読み直しで履歴を消し、解除した購読は呼ばない");
  throws(() => new ReviewStore({ format: "other", version: 1 }), "形式の違う文書");
  throws(() => new ReviewStore(documentOf("v2:1"), { limits: { items: 2 } }), "項目数の上限");
}

/** 実際の wasm の review_scene に編集した文書を渡し、図面に重ねて描き、当たり判定から編集の項目へ戻す */
export function testReviewCore(zumen, { assert, throws }) {
  const session = zumen.open();
  const base = new View2D(document.createElement("canvas"), { width: 200, height: 160, pixelRatio: 1 });
  const overlayCanvas = document.createElement("canvas");
  const overlay = new View2D(overlayCanvas, { width: 10, height: 10, background: "transparent" });
  let layer = null;
  try {
    session.push("a.dxf", new TextEncoder().encode(DXF));
    const opened = session.call({ v: 1, op: "open", input: "a.dxf" }).result;
    const drawing = session.call({ v: 1, op: "drawing", doc: "a.dxf" }).result;
    const identity = { doc: "a.dxf", format: opened.format, hash: drawing.hash };
    const sceneResult = session.call({ v: 1, op: "scene", doc: "a.dxf", from: "space/0", tolerance: 0.01 }).result;
    base.setScene({ scene: readScene(session.take(sceneResult.blob)), result: sceneResult, document: identity });
    base.jumpTo({ center: { x: 25, y: 15 }, scale: 3, rotation: 0 });
    const store = new ReviewStore(documentOf(drawing.hash), { createId: () => "n1" });
    store.addMarkup({ kind: "text", target: "plan", view: "space/0", points: [[0, 20]], text: "確認" });
    layer = new ReviewLayer(base, overlay, store);
    const size = overlay.getSize();
    assert(size.width === 200 && size.height === 160 && size.pixelRatio === 1, "重ねた View2D の大きさを図面に合わせる");
    const draw = (text) => {
      // 同じ名前への push は続きとして足すため、描いた後に登録を捨てる
      session.push("review.json", new TextEncoder().encode(text));
      try {
        const response = session.call({ v: 1, op: "review_scene", doc: "a.dxf", review: "review.json", target: "plan", from: "space/0", tolerance: 0.01, style: STYLE });
        layer.setResult({ scene: readScene(session.take(response.result.blob)), result: response.result, document: identity });
        return response.result;
      } finally { session.call({ v: 1, op: "close", inputs: ["review.json"] }); }
    };
    const all = draw(store.toJSON());
    const refs = all.sources.map(reviewReference);
    assert(refs.map((r) => r.id).join() === "m1,n1,l1" && all.sources.every((s, i) => s.id === reviewSceneId(refs[i])), "コアの対応と同じ ID を作る");
    assert(all.sources.every((s) => store.items().some((item) => item.sceneId === s.id)), "描いた項目を一覧から引ける");
    assert(layer.sourceOf("review/markup/6d31")?.markup === "m1" && layer.sourceOf("none") === null, "シーンの ID から sources を引く");

    // 赤入れの線 (0,0)-(10,0) の中ほどを当てる
    const onLine = base.toScreen({ x: 6, y: 0 });
    const picked = layer.pick(onLine, 3);
    assert(picked.some((ref) => ref.kind === "markup" && ref.id === "m1"), "当たり判定から図形へ戻す");
    const ctx = overlayCanvas.getContext("2d");
    const alpha = (point) => ctx.getImageData(Math.floor(point.x), Math.floor(point.y), 1, 1).data[3];
    assert(alpha(base.toScreen({ x: 45, y: 25 })) === 0, "背景を透明にして図面を隠さない");
    assert(alpha(onLine) > 0, "赤入れを描く");

    // 文書の選択を重ねたシーンの選択に写し、描いていない項目は選ばない
    store.select([{ kind: "markup", id: "m1" }, { kind: "location", topic: "t1", id: "l3" }]);
    assert(overlay.getState().selection.map((ref) => ref.id).join() === "review/markup/6d31", "文書の選択を写す");

    // 図面の表示範囲に付いていく
    base.panBy(15, -10);
    base.zoomBy(2);
    const a = base.getView(), b = overlay.getView();
    assert(a.center.x === b.center.x && a.center.y === b.center.y && a.scale === b.scale && a.rotation === b.rotation, "表示範囲を図面に合わせる");
    base.resize(120, 90);
    assert(overlay.getSize().width === 120 && overlay.getSize().height === 90, "大きさの変更に付いていく");

    store.setFilter({ kinds: ["text"] });
    const text = draw(store.filteredJSON());
    assert(text.sources.length === 1 && text.sources[0].markup === "n1" && text.counts.texts === 1, "絞った文書だけを描く");
    assert(overlay.getState().selection.length === 0, "描かなくなった項目の選択を外す");
    layer.setResult(null);
    assert(overlay.getScene() === null && layer.sourceOf("review/markup/6e31") === null, "消す");
    layer.destroy(); layer.destroy();
    throws(() => layer.pick({ x: 0, y: 0 }), "破棄後の当たり判定");
    base.panBy(5, 5);
    assert(overlay.getView().center.x !== base.getView().center.x, "破棄で購読を外す");
    throws(() => new ReviewLayer(base, base), "同じ View2D は重ねない");
  } finally { layer?.destroy(); overlay.destroy(); base.destroy(); session.close(); }
}

/**
 * 実際の wasm の review_scene の形を ReviewLayer で描き、docs/formats/review.md の規則から別に求めた位置の画素を確かめる。
 * 雲は入力の向きによらず外へ膨らみ、矢じりは指定した端だけを塗り、写真と指摘の記号・範囲は内側を塗らない。
 */
export function testReviewDrawn(zumen, { assert }) {
  const session = zumen.open();
  const base = new View2D(document.createElement("canvas"), { width: 960, height: 240, pixelRatio: 1 });
  const overlayCanvas = document.createElement("canvas");
  const overlay = new View2D(overlayCanvas, { width: 10, height: 10, background: "transparent" });
  let layer = null;
  try {
    session.push("a.dxf", new TextEncoder().encode(DXF));
    const opened = session.call({ v: 1, op: "open", input: "a.dxf" }).result;
    const drawing = session.call({ v: 1, op: "drawing", doc: "a.dxf" }).result;
    const identity = { doc: "a.dxf", format: opened.format, hash: drawing.hash };
    const sceneResult = session.call({ v: 1, op: "scene", doc: "a.dxf", from: "space/0", tolerance: 0.01 }).result;
    base.setScene({ scene: readScene(session.take(sceneResult.blob)), result: sceneResult, document: identity });
    base.jumpTo({ center: { x: 42.5, y: 6 }, scale: 10, rotation: 0 });
    const on = (id, kind, points, extra = {}) => ({ id, kind, target: "plan", view: "space/0", points, ...extra });
    const review = {
      format: "zumen-review", version: 1,
      targets: [{ id: "plan", format: "dxf", hash: drawing.hash }],
      topics: [{ id: "t1", title: "場所", status: "open", locations: [
        { id: "l1", target: "plan", kind: "2d", view: "space/0", pin: [70, 6] },
        { id: "l2", target: "plan", kind: "2d", view: "space/0", extent: [75, 0, 85, 10] },
      ] }],
      markups: [
        on("ccw", "cloud", [[0, 0], [12, 0], [12, 12], [0, 12]]),
        on("cw", "cloud", [[20, 0], [20, 12], [32, 12], [32, 0]]),
        on("line", "redline", [[40, 0], [55, 0]]),
        on("photo", "photo", [[60, 6]], { text: "p.jpg" }),
      ],
    };
    session.push("review.json", new TextEncoder().encode(JSON.stringify(review)));
    const response = session.call({ v: 1, op: "review_scene", doc: "a.dxf", review: "review.json", target: "plan", from: "space/0", tolerance: 0.01, style: { ...STYLE, line_weight: 0, arrow_length: 4 } });
    assert(response.result?.sources?.length === 6, `6 件を描く: ${JSON.stringify(response)}`);
    layer = new ReviewLayer(base, overlay, new ReviewStore(review));
    layer.setResult({ scene: readScene(session.take(response.result.blob)), result: response.result, document: identity });
    const ctx = overlayCanvas.getContext("2d");
    // 線の縁のにじみで 1 画素ずれても読めるように、3×3 の画素の最大の不透明度を見る
    const alpha = (x, y) => {
      const p = base.toScreen({ x, y });
      return Math.max(...Array.from(ctx.getImageData(Math.floor(p.x) - 1, Math.floor(p.y) - 1, 3, 3).data).filter((_, i) => i % 4 === 3));
    };
    const painted = (x, y, message) => assert(alpha(x, y) > 0, `${message}: (${x}, ${y})`);
    const empty = (x, y, message) => assert(alpha(x, y) === 0, `${message}: (${x}, ${y})`);

    // 雲: 辺 12・刻み 3 で 1 辺に 4 個、半径 1.5 の半円。頂点は辺の外へ 1.5、隣の円弧との境は辺の上
    for (const [dx, name] of [[0, "左回り"], [20, "右回り"]]) {
      for (const t of [1.5, 4.5, 7.5, 10.5]) {
        painted(dx + t, -1.5, `${name}の雲の下の辺の円弧の頂点`);
        painted(dx + t, 13.5, `${name}の雲の上の辺の円弧の頂点`);
        painted(dx - 1.5, t, `${name}の雲の左の辺の円弧の頂点`);
        painted(dx + 13.5, t, `${name}の雲の右の辺の円弧の頂点`);
        empty(dx + t, 1.5, `${name}の雲は内へ膨らまない`);
      }
      empty(dx + 3, -1.2, `${name}の雲の円弧の境は辺の上`);
      empty(dx + 6, 6, `${name}の雲の内側を塗らない`);
    }
    // 赤線: 長さ 4・幅 4/3 の塗った三角形を終わりの端だけに置く。先端から 3 の所の幅の半分は 0.5
    painted(52, 0.35, "終わりの矢じりを塗る");
    painted(52, -0.35, "終わりの矢じりは線の両側を塗る");
    empty(52, 0.85, "矢じりの幅は長さの 1/3");
    empty(50.5, 0.35, "矢じりは長さより手前を塗らない");
    empty(43, 0.35, "始めの端に矢じりを置かない");
    painted(47.5, 0, "赤線");
    // 写真と指摘の位置: 直径 2 の円と中心の十字
    for (const [x, name] of [[60, "写真"], [70, "指摘のピン"]]) {
      painted(x, 6, `${name}の中心`);
      painted(x + Math.SQRT1_2, 6 + Math.SQRT1_2, `${name}の円`);
      empty(x + 0.4, 6 + 0.4, `${name}の円の内側を塗らない`);
      empty(x + 1.5, 6 + 1.5, `${name}の円の外`);
    }
    // 指摘の範囲: 左下と右上で閉じた長方形
    painted(80, 0, "範囲の下の辺");
    painted(85, 5, "範囲の右の辺");
    empty(80, 5, "範囲の内側を塗らない");
    return { width: overlayCanvas.width, height: overlayCanvas.height };
  } finally { layer?.destroy(); overlay.destroy(); base.destroy(); session.close(); }
}
