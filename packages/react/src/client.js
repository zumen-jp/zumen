/** 1 回の読み込みに属する Worker。wasm の実行中でも terminate で専用のメモリとセッションを解放する。 */
export function connectWorker() {
  const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  const pending = new Map();
  let closed = false;
  let serial = 0;
  function dispose(reason = new DOMException("Aborted", "AbortError")) {
    if (closed) return;
    closed = true;
    worker.onmessage = worker.onerror = worker.onmessageerror = null;
    worker.terminate();
    for (const { reject } of pending.values()) reject(reason);
    pending.clear();
  }
  worker.onmessage = ({ data }) => {
    const request = pending.get(data.id);
    if (!request) return;
    pending.delete(data.id);
    if (data.error) {
      request.reject(Object.assign(new Error(data.error.message), data.error));
    } else {
      request.resolve(data.value);
    }
  };
  worker.onerror = (event) => dispose(new Error(event.message || "worker.error"));
  worker.onmessageerror = () => dispose(new Error("worker.message"));
  return {
    dispose,
    request(message, transfer = []) {
      if (closed) return Promise.reject(new DOMException("Aborted", "AbortError"));
      if (pending.size >= 32) return Promise.reject(new RangeError("worker.requests.limit"));
      return new Promise((resolve, reject) => {
        const id = ++serial;
        pending.set(id, { resolve, reject });
        try { worker.postMessage({ id, ...message }, transfer); }
        catch (error) { pending.delete(id); reject(error); }
      });
    },
  };
}
