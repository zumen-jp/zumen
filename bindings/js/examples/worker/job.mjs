// アプリに写して使う、1 要求に 1 worker を使う例。npm の公開 API ではない（#532）。
import { Worker } from "node:worker_threads";

/**
 * コンパイル済みの module と、呼び出す側だけが持つ input（ArrayBuffer）を渡す。
 * input は転送で使えなくなる。result は worker が終了してから成功か失敗になる。
 */
export function createSceneJob(module, input, { from = "space/0", budget, signal } = {}) {
  signal?.throwIfAborted();
  if (!(input instanceof ArrayBuffer)) {
    throw new TypeError("入力は専有する ArrayBuffer にする");
  }
  // Module は structured clone で共有する。Instance・Session は worker の中だけで作る。
  const worker = new Worker(new URL("./worker.mjs", import.meta.url), { workerData: module });
  const result = new Promise((resolve, reject) => {
    let reply;
    let failure;
    const terminate = () => {
      void worker.terminate().catch(reject);
    };
    const abort = () => {
      failure = signal.reason ?? new DOMException("処理を中断した", "AbortError");
      terminate();
    };
    worker.on("message", (message) => {
      if (message.type === "result") {
        reply = message;
      }
    });
    worker.once("error", (error) => {
      failure ??= error;
    });
    worker.once("messageerror", (error) => {
      failure ??= error;
      terminate();
    });
    worker.once("exit", (code) => {
      signal?.removeEventListener("abort", abort);
      if (failure !== undefined) {
        reject(failure);
      } else if (code !== 0 || reply === undefined) {
        reject(new Error(`worker が結果を返さず終了した（${code}）`));
      } else if (reply.error) {
        reject(Object.assign(new Error(reply.error.message), reply.error));
      } else {
        resolve(reply);
      }
    });
    signal?.addEventListener("abort", abort, { once: true });
    try {
      // Buffer の共有プールや SharedArrayBuffer を転送リストに入れない。
      worker.postMessage({ input, from, budget }, [input]);
    } catch (error) {
      failure = error;
      terminate();
    }
  });
  return { worker, result };
}
