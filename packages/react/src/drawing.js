import { useEffect, useMemo, useState } from "react";
import { readScene } from "@zumen-jp/zumen";
import { connectWorker } from "./client.js";

const idle = Object.freeze({ status: "idle" });
const loading = Object.freeze({ status: "loading" });
const maxBytes = 64 * 1024 * 1024;

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

/** 入力と起点ごとに Worker を専有し、その版のシーンと出所を一緒に渡す。 */
export function useDrawing(file, { loadModule, from = null, catalog = null }) {
  const request = useMemo(() => ({ file, loadModule, from, catalog }), [file, loadModule, from, catalog]);
  const [snapshot, setSnapshot] = useState(null);
  useEffect(() => {
    setSnapshot(null);
    if (!file) return;
    const controller = new AbortController();
    const { signal } = controller;
    let client;
    async function open() {
      try {
        if (file.size > maxBytes) throw new RangeError("drawing.input.limit");
        const module = await loadModule(signal);
        if (signal.aborted) return;
        const bytes = await readInput(file, signal);
        if (signal.aborted) return;
        client = connectWorker();
        const { blob, result, document, drawing, diagnostics } = await client.request({ op: "open", module, bytes, from, catalog }, [bytes]);
        if (signal.aborted) return;
        const input = { scene: readScene(blob), result, document };
        const getSource = (element) => client.request({ op: "source", element });
        setSnapshot({ request, value: { status: "ready", input, drawing, diagnostics, getSource } });
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
    return () => { controller.abort(); client?.dispose(); };
  }, [request]);
  if (!file) return idle;
  return snapshot?.request === request ? snapshot.value : loading;
}
