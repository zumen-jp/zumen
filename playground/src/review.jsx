import React, { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useReviewLayer, useReviewState } from "@zumen-jp/react";
import { ReviewStore } from "@zumen-jp/view2d";
import { Diagnostics } from "./inspector.jsx";

// レビューの図形と指摘（#589）。文書はこのブラウザーの中だけで作り、保存は利用者のダウンロードに限る。
// 色は図面の線（#333333）・選択（#c74700）と見分けられる赤紫にする。寸法は用紙 mm。
const style = Object.freeze({ color: "#c0007a", line_weight: 0.35, text_height: 3.5, text_padding: 1, arrow_length: 3, arrows: "end", cloud_pitch: 4, marker_size: 3 });
const kindNames = { redline: "赤線", cloud: "変更雲", text: "注記", photo: "写真の位置", location: "指摘の場所" };
const statusNames = { open: "未対応", resolved: "対応済み", closed: "完了" };
const carriedNames = { unchanged: "変更なし", changed: "変更あり", removed: "削除された" };
const TARGET = "drawing";

function emptyDocument(identity) {
  return { format: "zumen-review", version: 1, targets: [{ id: TARGET, format: identity.format, hash: identity.hash, name: identity.doc }], topics: [], markups: [] };
}
function newId() {
  return globalThis.crypto?.randomUUID?.() ?? "id-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2);
}
function download(text, name) {
  const url = URL.createObjectURL(new Blob([text], { type: "application/json" }));
  const link = Object.assign(document.createElement("a"), { href: url, download: name });
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
function readText(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(reader.error);
    reader.readAsText(file);
  });
}
/** 文書の対象のうち、開いた図面の版と合うもの。なければ最初の対象を描き、コアの診断で版の違いを示す */
function targetOf(doc, identity) {
  const targets = doc.targets ?? [];
  return (targets.find((t) => t.format === identity.format && t.hash === identity.hash) ?? targets[0])?.id ?? null;
}
const sameRef = (a, b) => a.kind === b.kind && a.id === b.id && (a.kind === "markup" || a.topic === b.topic);

export function ReviewOverlay({ setCanvas }) {
  return <canvas ref={setCanvas} className="review-overlay" aria-hidden="true"/>;
}

/** 図面の表示部品にレビューのシーンを重ね、図形と指摘を作る・直す・保存する・読み直す */
export function Review({ opened, viewer, overlay, canvas }) {
  const identity = opened.input.document, from = opened.input.result.from;
  const store = useMemo(() => new ReviewStore(emptyDocument(identity), { createId: newId, stamp: () => ({ time: new Date().toISOString() }) }), [identity]);
  const state = useReviewState(store);
  const layer = useReviewLayer(viewer, overlay, store);
  const [drawn, setDrawn] = useState({ status: "idle", diagnostics: [] });
  const [text, setText] = useState("確認してください");
  const [loadError, setLoadError] = useState(null);
  const picker = useRef(null);
  const target = state ? targetOf(state.document, identity) : null;

  // 文書か絞り込みが変わるたびに、コアで描き直す。遅れて届いた古い結果は捨てる
  useEffect(() => {
    if (!layer || !state || !target) return;
    let current = true;
    setDrawn((before) => ({ ...before, status: "drawing" }));
    opened.reviewScene(store.filteredJSON(), { target, style }).then(({ input, diagnostics }) => {
      if (!current) return;
      layer.setResult(input);
      setDrawn({ status: "ready", count: input.result.sources.length, diagnostics });
    }, (error) => {
      if (!current) return;
      layer.setResult(null);
      setDrawn({ status: "error", diagnostics: error.diagnostics ?? [], message: error.message });
    });
    return () => { current = false; };
  }, [layer, opened, store, target, state?.document, state?.filter]);

  // 図面をクリックした位置のレビューの項目を選ぶ。図面の要素の選択は表示部品が行う
  useLayoutEffect(() => {
    if (!layer || !canvas) return;
    const click = (event) => {
      const rect = canvas.getBoundingClientRect();
      const picked = layer.pick({ x: event.clientX - rect.left, y: event.clientY - rect.top }, 4);
      store.select(picked.slice(0, 1));
    };
    canvas.addEventListener("click", click);
    return () => canvas.removeEventListener("click", click);
  }, [layer, canvas, store]);

  if (!state) return null;
  // 表示の中心とその周りの大きさで、新しい図形の位置を決める
  function around() {
    const { width, height } = viewer.getSize();
    const center = viewer.toScene({ x: width / 2, y: height / 2 });
    const edge = viewer.toScene({ x: width / 2 + Math.min(width, height) / 8, y: height / 2 });
    return { center, r: Math.max(Math.hypot(edge.x - center.x, edge.y - center.y), 1e-9) };
  }
  function addCloud() {
    const { center: c, r } = around();
    const id = store.addMarkup({ kind: "cloud", target, view: from, points: [[c.x - r, c.y - r], [c.x + r, c.y - r], [c.x + r, c.y + r], [c.x - r, c.y + r]] });
    store.select([{ kind: "markup", id }]);
  }
  function addText() {
    const { center: c } = around();
    const id = store.addMarkup({ kind: "text", target, view: from, points: [[c.x, c.y]], text });
    store.select([{ kind: "markup", id }]);
  }
  function addTopic() {
    const { center: c, r } = around();
    store.batch(() => {
      const topic = store.addTopic({ title: text, status: "open" });
      const id = store.addLocation(topic, { target, view: from, pin: [c.x, c.y], extent: [c.x - r, c.y - r, c.x + r, c.y + r] });
      store.select([{ kind: "location", topic, id }]);
    });
  }
  async function load(file) {
    setLoadError(null);
    try { store.load(await readText(file)); }
    catch (error) { setLoadError(error); }
    finally { picker.current.value = ""; }
  }
  const selected = state.selection[0] ?? null;
  const items = store.items();
  const current = selected && items.find((item) => sameRef(item.ref, selected));
  function remove() {
    if (!current) return;
    if (current.ref.kind === "markup") store.removeMarkup(current.ref.id);
    else store.batch(() => {
      store.removeLocation(current.ref.topic, current.ref.id);
      const topic = store.getDocument().topics.find((t) => t.id === current.ref.topic);
      if (topic && !(topic.locations ?? []).length && !store.getDocument().markups.some((m) => m.topic === topic.id)) store.removeTopic(topic.id);
    });
  }
  const stale = drawn.diagnostics.some(({ diagnostic }) => diagnostic.code === "review.target.stale");
  const statusText = drawn.status === "drawing" ? "レビューを描いています。" : drawn.status === "error" ? stale ? "レビューの対象の版が、開いた図面と違います。" : "レビューを描けませんでした。"
    : drawn.status === "ready" ? "レビューの項目を " + drawn.count + " 件描きました。" : "";
  return <section aria-labelledby="review-title" className="export review">
    <h2 id="review-title">レビュー</h2>
    <p>図面に変更雲・注記・指摘を重ねます。レビューの文書はこのブラウザーの中で作り、保存するまでどこにも残しません。</p>
    <div className="toolbar" role="group" aria-label="レビューの操作">
      <label>文言 <input value={text} maxLength={200} onChange={(e) => setText(e.target.value)}/></label>
      <button disabled={!target} onClick={addCloud}>変更雲を追加</button>
      <button disabled={!target || !text} onClick={addText}>注記を追加</button>
      <button disabled={!target || !text} onClick={addTopic}>指摘を追加</button>
      <button disabled={!state.canUndo} onClick={() => store.undo()}>元に戻す</button>
      <button disabled={!state.canRedo} onClick={() => store.redo()}>やり直す</button>
    </div>
    <div className="toolbar" role="group" aria-label="絞り込みと保存">
      <label>状態 <select value={state.filter.statuses?.[0] ?? ""} onChange={(e) => store.setFilter(e.target.value ? { statuses: [e.target.value] } : {})}>
        <option value="">すべて</option>
        {Object.entries(statusNames).map(([value, label]) => <option key={value} value={value}>{label}の指摘</option>)}
      </select></label>
      <button onClick={() => download(store.toJSON(), "review.json")}>レビューを保存</button>
      <label htmlFor="review-file">レビューを読む</label>
      <input ref={picker} id="review-file" type="file" accept=".json,application/json" onChange={(e) => e.target.files?.[0] && load(e.target.files[0])}/>
    </div>
    <div role="status" aria-live="polite" aria-atomic="true">{loadError ? "レビューの文書を読めませんでした。" : statusText}</div>
    <h3 id="review-items-title">項目 {items.length} 件</h3>
    <ul className="rows elements" aria-labelledby="review-items-title">
      {items.map((item) => {
        const topic = item.topic ?? state.document.topics?.find((t) => t.id === item.item.topic);
        const carried = item.item.carried;
        return <li key={item.sceneId}><button aria-pressed={Boolean(selected && sameRef(item.ref, selected))} onClick={() => store.select([item.ref])}>
          {kindNames[item.ref.kind === "markup" ? item.item.kind : "location"]}{item.item.text ? "「" + item.item.text + "」" : ""}
          {topic && <span className="detail">指摘「{topic.title}」・{statusNames[topic.status] ?? topic.status}</span>}
          {carried && <span className="detail">移した位置: {carriedNames[carried.state] ?? carried.state}</span>}
        </button></li>;
      })}
    </ul>
    {current && <div className="toolbar" role="group" aria-label="選んだ項目">
      {current.ref.kind === "markup" && current.item.kind === "text" && <button disabled={!text} onClick={() => store.updateMarkup(current.ref.id, { text })}>文言を置き換え</button>}
      {current.topic && <label>指摘の状態 <select value={current.topic.status} onChange={(e) => store.updateTopic(current.topic.id, { status: e.target.value })}>
        {Object.entries(statusNames).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
      </select></label>}
      <button onClick={remove}>選んだ項目を削除</button>
    </div>}
    {loadError && <details><summary>エラーの詳細</summary><p>{loadError.message}</p></details>}
    {drawn.status === "error" && !drawn.diagnostics.some(({ diagnostic }) => diagnostic.severity === "error") && <details><summary>描けなかった理由</summary><p>{drawn.message}</p></details>}
    {drawn.diagnostics.length > 0 && <Diagnostics entries={drawn.diagnostics} issues={[]} title="レビューの診断" id="review-diagnostics-title"/>}
  </section>;
}
