// 1 つの入力を読み、シーンの blob を親へ転送したら終了する。
import { parentPort, workerData } from "node:worker_threads";
import { loadSync, ZumenError } from "@zumen-jp/zumen/node";

parentPort.once("message", ({ input, from, budget }) => {
  let session;
  let reply;
  let transfers = [];
  try {
    const zumen = loadSync(workerData);
    session = zumen.open();
    parentPort.postMessage({ type: "started" });
    session.push("input", input);
    const opened = session.call({ v: 1, op: "open", input: "input", budget });
    const response = session.call({ v: 1, op: "scene", doc: opened.result.doc, from, budget });
    const blob = session.take(response.result.blob);
    reply = { type: "result", opened, response, blob };
    // take が wasm の外へ写した専有バッファーを転送する。wasm の memory は渡さない。
    transfers = [blob.buffer];
  } catch (error) {
    // Error の追加プロパティは structured clone で保たれないので、必要な値を別に渡す。
    const details = { name: error.name, message: error.message };
    if (error instanceof ZumenError) {
      Object.assign(details, { operation: error.operation, code: error.code, status: error.status, response: error.response });
    }
    reply = { type: "result", error: details };
  } finally {
    session?.close();
  }
  parentPort.postMessage(reply, transfers);
  parentPort.close();
});
