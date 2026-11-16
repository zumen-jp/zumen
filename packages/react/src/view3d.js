import { useLayoutEffect, useMemo, useState } from "react";
import { MODULES, detect, modulesWith } from "@zumen-jp/zumen";
import { View3D } from "@zumen-jp/view3d";
import { connectWorker } from "./client.js";
export { useViewerEvent as useView3DEvent, useViewerState as useView3DState } from "./store.js";

const emptyOptions = Object.freeze({});
const idle = Object.freeze({ status: "idle" });
const loading = Object.freeze({ status: "loading" });
const maxBytes = 256 * 1024 * 1024;
const sceneWorker = () => new Worker(new URL("./worker3d.js", import.meta.url), { type: "module" });

/** Canvas と設定を所有する間だけ有効な 3D の表示部品。再描画では同じ設定の参照を渡す。 */
export function useView3D(canvas, options = emptyOptions) {
  const [owner, setOwner] = useState(null);
  useLayoutEffect(() => {
    if (!canvas) { setOwner(null); return; }
    const viewer = new View3D(canvas, options);
    setOwner({ canvas, options, viewer });
    return () => {
      // Activity と Suspense は state を保ったまま layout effect を切り離す。GPU の資源と RAF もここで放す。
      setOwner(null);
      viewer.destroy();
    };
  }, [canvas, options]);
  return owner && owner.canvas === canvas && owner.options === options ? owner.viewer : null;
}

/** 表示部品の attachInput を要素につなぎ、表示部品・要素・設定の差し替えとアンマウントで外す。 */
export function useView3DInput(viewer, element, options = emptyOptions) {
  useLayoutEffect(() => {
    if (!viewer || !element) return;
    return viewer.attachInput(element, options);
  }, [viewer, element, options]);
}

/** File を選んだ側で非同期に読み、途中の読み込みも中断できるようにする。 */
function readInput(file, signal) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    const abort = () => reader.abort();
    reader.onloadend = () => {
      signal.removeEventListener("abort", abort);
      if (signal.aborted) reject(new DOMException("Aborted", "AbortError"));
      else if (reader.error) reject(reader.error);
      else resolve(reader.result);
    };
    signal.addEventListener("abort", abort, { once: true });
    try { reader.readAsArrayBuffer(file); }
    catch (error) { signal.removeEventListener("abort", abort); reject(error); }
  });
}

function unsupported(format) {
  return Object.assign(new DOMException("scene3d.format.unsupported", "NotSupportedError"), { format, diagnostics: [] });
}

/**
 * 入力ごとに Worker を専有し、3D のシーン（ZMS3）と文書識別を一緒に渡す。形式は先頭のバイト列で判定し、
 * その形式の scene3d を持つ wasm のモジュールを loadModule(name, signal) で受け取る。
 */
export function useScene3D(file, { loadModule, catalog = null, tolerance }) {
  const request = useMemo(() => ({ file, loadModule, catalog, tolerance }), [file, loadModule, catalog, tolerance]);
  const [snapshot, setSnapshot] = useState(null);
  useLayoutEffect(() => {
    if (!file) return;
    const controller = new AbortController();
    const { signal } = controller;
    let client;
    async function open() {
      try {
        if (file.size > maxBytes) throw new RangeError("scene3d.input.limit");
        const bytes = await readInput(file, signal);
        if (signal.aborted) return;
        const found = detect(new Uint8Array(bytes));
        if (!found) throw unsupported(null);
        // 判定した形式を、scene3d を持つモジュールのうち最初に持つもので開く（図面のモジュールにもある形式を含む）。
        const name = modulesWith("scene3d").find((candidate) => MODULES[candidate].formats.includes(found.format));
        if (name === undefined) throw unsupported(found.format);
        const module = await loadModule(name, signal);
        if (signal.aborted) return;
        client = connectWorker(sceneWorker);
        const opened = await client.request({ op: "open", module, bytes, catalog, tolerance }, [bytes]);
        if (signal.aborted) return;
        const { blob, result, document, kind, elements, total, bounds, diagnostics } = opened;
        const building = kind === "building";
        const own = client;
        const value = {
          status: "ready", input: { blob, result, document }, kind, module: name, elements, total, bounds, diagnostics,
          getElement: (id) => own.request({ op: "element", element: id }),
          writeBcf: building ? (options) => own.request({ op: "bcf_write", ...options }) : null,
          readBcf: building ? (bytes) => {
            const copy = new Uint8Array(bytes).slice();
            return own.request({ op: "bcf_read", bytes: copy }, [copy.buffer]);
          } : null,
        };
        setSnapshot({ request, value: Object.freeze(value) });
      } catch (error) {
        client?.dispose();
        if (!signal.aborted) {
          const failure = error instanceof Error ? error : new Error(String(error));
          setSnapshot({ request, value: { status: "error", error: failure, diagnostics: failure.diagnostics ?? [] } });
          controller.abort();
        }
      }
    }
    void open();
    return () => {
      // 境界が同じ入力で再接続しても、終了した Worker の問い合わせを利用側の effect に渡さない。
      setSnapshot(null);
      controller.abort();
      client?.dispose();
    };
  }, [request]);
  if (!file) return idle;
  return snapshot?.request === request ? snapshot.value : loading;
}
