// 開発用 React の StrictMode と本番用 React で 3D のフックを使い、Worker・購読・入力・表示部品の作成と破棄を数える（#818）。
import React, { StrictMode, useLayoutEffect } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { View3D } from "@zumen-jp/view3d";
import { useScene3D, useView3D, useView3DEvent, useView3DInput, useView3DState } from "@zumen-jp/react/view3d";
import { loadModule } from "../src/scene3d-module.js";

const counts = { workers: 0, starts: 0, ends: 0, transferred: 0, state: 0, events: 0, inputs: 0, created: 0, destroyed: 0 };
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
for (const [name, key] of [["subscribe", "state"], ["on", "events"], ["attachInput", "inputs"]]) {
  const method = View3D.prototype[name];
  View3D.prototype[name] = function (...args) {
    const off = method.apply(this, args);
    let active = true;
    counts[key]++;
    return () => { if (active) { active = false; counts[key]--; } off(); };
  };
}
const destroy = View3D.prototype.destroy;
View3D.prototype.destroy = function () { counts.destroyed++; return destroy.call(this); };
const options = Object.freeze({ width: 160, height: 120, shading: "unlit",
  views: [{ id: "main", rect: { x: 0, y: 0, width: 160, height: 120 },
    camera: { projection: "orthographic", position: [0, 0, 100], direction: [0, 0, -1], up: [0, 1, 0], width: 100 } }] });

const canvas = document.querySelector("canvas");
const delayed = [];
const delayModule = (name, signal) => new Promise((resolve) => delayed.push({ name, signal, resolve }));
let root, file, opened, viewer, snapshot, picks = 0, lastViewer;
function Probe({ file, loader }) {
  viewer = useView3D(canvas, options);
  snapshot = useView3DState(viewer);
  useView3DInput(viewer, canvas);
  useView3DEvent(viewer, "pick", () => { picks++; });
  opened = useScene3D(file, { loadModule: loader });
  useLayoutEffect(() => {
    if (!viewer) return;
    if (viewer !== lastViewer) { counts.created++; lastViewer = viewer; }
    viewer.setScene(opened.status === "ready" ? opened.input : null);
  }, [viewer, opened]);
  return null;
}
window.probe = {
  counts, delayed,
  get status() { return opened?.status; }, get format() { return opened?.input?.document.format; },
  get module() { return opened?.module; }, get error() { return opened?.error?.message; },
  get scene() { return snapshot?.document?.format ?? null; }, get picks() { return picks; },
  get stats() { return viewer?.getStats() ?? lastViewer?.getStats(); },
  render(bytes, delay = false) {
    if (!root) root = createRoot(document.querySelector("#root"));
    if (bytes !== undefined) file = bytes && new Blob([Uint8Array.from(bytes)]);
    flushSync(() => root.render(<StrictMode><Probe file={file} loader={delay ? delayModule : loadModule} /></StrictMode>));
  },
  async release() {
    const module = await loadModule("bim", new AbortController().signal);
    for (const task of delayed) task.resolve(module);
  },
  holdNext() { holdNext = true; },
  get held() { return Boolean(reply); },
  deliver() { reply(); reply = undefined; },
  async finish() {
    const lookup = opened.getElement, id = opened.elements[0]?.id ?? "";
    const pending = lookup(id).catch((error) => error.name);
    flushSync(() => root.unmount());
    root = null;
    return [await pending, await lookup(id).catch((error) => error.name)];
  },
};
