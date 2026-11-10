// 開発用 React の StrictMode で同じフックを使い、実際の購読・Worker の作成と破棄を数える。
import React, { Activity, StrictMode, Suspense, useEffect, useLayoutEffect } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { View2D } from "@zumen-jp/view2d";
import { useDrawing, useDrawingViewer, useViewerEvent, useViewerState } from "@zumen-jp/react";
import { loadModule } from "../src/module.js";
import { Playground } from "../src/playground.jsx";

const counts = { workers: 0, starts: 0, ends: 0, transferred: 0, state: 0, events: 0, destroyed: 0, inputs: 0 };
const NativeWorker = window.Worker;
let holdNext = false, reply;
window.Worker = class extends NativeWorker {
  constructor(...args) { super(...args); counts.workers++; counts.starts++; }
  postMessage(message, transfer) {
    if (holdNext && message.op === "open") {
      holdNext = false;
      const deliver = this.onmessage;
      this.onmessage = (event) => { reply = () => deliver(event); };
    }
    super.postMessage(message, transfer);
    if (message.op === "open" && message.bytes.byteLength === 0) counts.transferred++;
  }
  terminate() { counts.workers--; counts.ends++; return super.terminate(); }
};
for (const [name, key] of [["subscribe", "state"], ["on", "events"]]) {
  const method = View2D.prototype[name];
  View2D.prototype[name] = function (...args) {
    const off = method.apply(this, args);
    counts[key]++;
    return () => { counts[key]--; off(); };
  };
}
const destroy = View2D.prototype.destroy;
View2D.prototype.destroy = function () { counts.destroyed++; return destroy.call(this); };
const attach = View2D.prototype.attachInput;
View2D.prototype.attachInput = function (...args) {
  const detach = attach.apply(this, args); let active = true;
  counts.inputs++;
  return () => { if (active) { active = false; counts.inputs--; detach(); } };
};

const canvas = document.querySelector("canvas");
let root;
const delayed = [];
const delayModule = (signal) => new Promise((resolve) => delayed.push({ signal, resolve }));
let opened, viewer, eventVersion, snapshot;
function Probe({ file, loader, version }) {
  viewer = useDrawingViewer(canvas);
  snapshot = useViewerState(viewer);
  useViewerEvent(viewer, "selection", () => { eventVersion = version; });
  opened = useDrawing(file, { loadModule: loader });
  useLayoutEffect(() => {
    if (viewer) viewer.setScene(opened.status === "ready" ? opened.input : null);
  }, [viewer, opened]);
  return null;
}
let file;
window.probe = {
  counts, delayed, get status() { return opened?.status; }, get format() { return opened?.input?.document.format; },
  get selected() { return snapshot?.selection[0]?.id; }, get eventVersion() { return eventVersion; },
  render(bytes, delay = false, version = 0) {
    if (!root) root = createRoot(document.querySelector("#root"));
    if (bytes) file = new Blob([Uint8Array.from(bytes)]);
    flushSync(() => root.render(<StrictMode><Probe file={file} loader={delay ? delayModule : loadModule} version={version}/></StrictMode>));
  },
  async release() {
    const module = await loadModule(new AbortController().signal);
    for (const task of delayed) task.resolve(module);
  },
  holdNext() { holdNext = true; },
  get held() { return Boolean(reply); },
  deliver() { reply(); reply = undefined; },
  select() { viewer.select([viewer.getElements()[0]]); },
  async finish() {
    const lookup = opened.getSource, element = viewer.getElements()[0];
    const pending = lookup(element).catch((error) => error.name);
    flushSync(() => root.unmount());
    return [await pending, await lookup(element).catch((error) => error.name)];
  },
};

// Canvas は親が保持する。境界から戻った利用側の layout effect でも、新しい表示部品を直接操作できることを確かめる。
let pending, resume, boundaryRoot, connected, previous, events = 0;
let boundaryFile, holdModule = false;
const loaders = [], calls = [];
function loadBoundaryModule(signal) {
  if (!holdModule) return loadModule(signal);
  return new Promise((resolve) => loaders.push(() => resolve(loadModule(signal))));
}
function observe(opened, phase) {
  const call = { phase, status: opened.status, workers: counts.workers };
  calls.push(call);
  if (opened.status === "ready") {
    opened.getSource({ id: "space/0/r1", path: [] }).then(
      (entry) => { call.record = entry?.source?.record; },
      (error) => { call.error = error.name; },
    );
  }
}
function Reconnect({ suspend }) {
  const value = useDrawingViewer(canvas);
  const state = useViewerState(value);
  useViewerEvent(value, "view", () => { events++; });
  const opened = useDrawing(boundaryFile, { loadModule: loadBoundaryModule });
  useLayoutEffect(() => { observe(opened, "layout"); }, [opened]);
  useEffect(() => { observe(opened, "passive"); }, [opened]);
  useLayoutEffect(() => {
    if (value) { value.getState(); connected = value; }
    return () => { connected = null; };
  }, [value]);
  if (suspend) throw pending;
  return <output id="camera">{state?.view.center.x}</output>;
}
window.boundary = {
  begin(bytes) { boundaryFile = new Blob([Uint8Array.from(bytes)]); },
  hold() { holdModule = true; },
  mark() { calls.length = 0; },
  release() { holdModule = false; for (const release of loaders.splice(0)) release(); },
  calls, get waiting() { return loaders.length; },
  render(kind, show) {
    if (!boundaryRoot) boundaryRoot = createRoot(document.querySelector("#root"));
    if (!show) previous = connected;
    if (kind === "Suspense" && !show) pending = new Promise((resolve) => { resume = resolve; });
    if (show && resume) { resume(); resume = null; }
    const child = kind === "Activity" ? <Activity mode={show ? "visible" : "hidden"}><Reconnect /></Activity>
      : <Suspense fallback={<p>待機</p>}><Reconnect suspend={!show}/></Suspense>;
    flushSync(() => boundaryRoot.render(<StrictMode>{child}</StrictMode>));
  },
  move() { connected.panBy(10, 0); },
  get fresh() { return Boolean(connected && connected !== previous); },
  get events() { return events; },
  end() {
    flushSync(() => boundaryRoot.unmount());
    boundaryRoot = null; connected = previous = null; boundaryFile = null;
    calls.length = 0; events = 0;
  },
};

// 同じ本番コンポーネントを切り離し、予約された寸法更新と入力・Worker の所有権も確かめる。
let playgroundRoot, stylesheet;
window.resizeFixture = {
  async mount() {
    if (!stylesheet) stylesheet = new Promise((resolve, reject) => {
      const link = document.createElement("link"); link.rel = "stylesheet"; link.href = "../viewer.css";
      link.onload = resolve; link.onerror = () => reject(new Error("viewer.css")); document.head.append(link);
    });
    await stylesheet;
    canvas.remove();
    playgroundRoot = createRoot(document.querySelector("#root"));
    flushSync(() => playgroundRoot.render(<StrictMode><Playground/></StrictMode>));
  },
  unmount() {
    if (!playgroundRoot) return;
    flushSync(() => playgroundRoot.unmount()); playgroundRoot = null;
  },
};
