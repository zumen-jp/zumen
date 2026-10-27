// Worker の側。ファイルを wasm のメモリの代わりに読んでおき、合図を受けたらページへ送る。
// JSON は UTF-8 から文字列にして送る。バイナリは 1 度だけ写して、ArrayBuffer を移す（決定 0013）。
import { parentPort, workerData } from "node:worker_threads";
import { readFileSync } from "node:fs";

const memory = new Uint8Array(readFileSync(workerData.file));
const now = () => performance.timeOrigin + performance.now();
parentPort.once("message", () => {
  const sent = now();
  if (workerData.format === "json") {
    parentPort.postMessage({ sent, text: new TextDecoder().decode(memory) });
  } else {
    const buffer = memory.slice().buffer;
    parentPort.postMessage({ sent, buffer }, [buffer]);
  }
});
parentPort.postMessage("ready");
