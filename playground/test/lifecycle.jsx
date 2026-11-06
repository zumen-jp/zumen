// 開発用 React の StrictMode で同じフックを使い、実際の購読・Worker の作成と破棄を数える。
import React, { StrictMode, useLayoutEffect } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { View2D } from "@zumen-jp/view2d";
import { useDrawing, useDrawingViewer, useViewerEvent, useViewerState } from "@zumen-jp/react";
import { loadModule } from "../src/module.js";

const counts = { workers: 0, starts: 0, ends: 0, transferred: 0, state: 0, events: 0, destroyed: 0 };
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

const canvas = document.querySelector("canvas");
const root = createRoot(document.querySelector("#root"));
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
