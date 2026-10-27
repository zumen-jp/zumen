// ページの側。Worker から 1 つの形式の結果を受け取り、描画に使える型付き配列にするまでを測る。
// 使い方: node --expose-gc page.mjs <形式> <ファイル>
import { Worker } from "node:worker_threads";
import { decoders, now } from "./decoders.mjs";

const [format, file] = process.argv.slice(2);

globalThis.gc?.();
const before = process.memoryUsage();
let peak = before.rss;
const sampler = setInterval(() => {
  peak = Math.max(peak, process.memoryUsage().rss);
}, 1);
const worker = new Worker(new URL("./worker.mjs", import.meta.url), { workerData: { format, file } });
worker.once("message", (ready) => {
  if (ready !== "ready") throw new Error(String(ready));
  worker.once("message", (message) => {
    const received = now();
    const scene = decoders[format](message);
    const done = now();
    peak = Math.max(peak, process.memoryUsage().rss);
    clearInterval(sampler);
    const vertices = scene.coords.length / 2;
    let sum = 0;
    for (let i = 0; i < scene.coords.length; i += 9973) sum += scene.coords[i];
    console.log(
      JSON.stringify({
        format,
        send_ms: received - message.sent,
        page_ms: done - received,
        parse_ms: scene.parsed ? scene.parsed - received : undefined,
        total_ms: done - message.sent,
        peak_rss_mb: (peak - before.rss) / 2 ** 20,
        vertices,
        check: `${vertices}:${sum.toFixed(3)}:${scene.origins[2]}:${scene.start[1000]}`,
      }),
    );
    worker.terminate();
  });
  worker.postMessage("go");
});
