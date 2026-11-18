import { readScene } from "@zumen-jp/zumen";
import { ReviewStore, reviewReference, reviewSceneId, View2D } from "../src/index.js";

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

/** 実際の wasm の review_scene に編集した文書を渡し、sources から編集の項目へ戻す */
export function testReviewCore(zumen, { assert }) {
  const session = zumen.open();
  const viewer = new View2D(document.createElement("canvas"), { width: 200, height: 200 });
  try {
    session.push("a.dxf", new TextEncoder().encode(DXF));
    const opened = session.call({ v: 1, op: "open", input: "a.dxf" }).result;
    const drawing = session.call({ v: 1, op: "drawing", doc: "a.dxf" }).result;
    const store = new ReviewStore(documentOf(drawing.hash), { createId: () => "n1" });
    store.addMarkup({ kind: "text", target: "plan", view: "space/0", points: [[0, 20]], text: "確認" });
    let pushed = 0;
    const draw = (text) => {
      // 同じ名前への push は続きとして足すため、描くたびに名前を変える
      const name = `review-${++pushed}.json`;
      session.push(name, new TextEncoder().encode(text));
      const response = session.call({ v: 1, op: "review_scene", doc: "a.dxf", review: name, target: "plan", from: "space/0", tolerance: 0.01, style: STYLE });
      const scene = readScene(session.take(response.result.blob));
      viewer.setScene({ scene, result: response.result, document: { doc: "a.dxf", format: opened.format, hash: drawing.hash } });
      return response.result;
    };
    const all = draw(store.toJSON());
    const refs = all.sources.map(reviewReference);
    assert(refs.map((r) => r.id).join() === "m1,n1,l1" && all.sources.every((s, i) => s.id === reviewSceneId(refs[i])), "コアの対応と同じ ID を作る");
    assert(all.sources.every((s) => store.items().some((item) => item.sceneId === s.id)), "描いた項目を一覧から引ける");
    store.setFilter({ kinds: ["text"] });
    const text = draw(store.filteredJSON());
    assert(text.sources.length === 1 && text.sources[0].markup === "n1" && text.counts.texts === 1, "絞った文書だけを描く");
  } finally { viewer.destroy(); session.close(); }
}
