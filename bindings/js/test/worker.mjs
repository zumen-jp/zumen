// worker の例で、入力と blob の転送・主スレッドのタイマー・失敗と中断の後始末を確かめる（#532）。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { markAsUntransferable } from "node:worker_threads";
import { loadSync, readScene } from "../src/index.js";
import { createSceneJob } from "../examples/worker/job.mjs";

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) {
  console.error("使い方: node bindings/js/test/worker.mjs <zumen_abi1.wasm>");
  process.exit(2);
}
const module = await WebAssembly.compile(readFileSync(wasmPath));
const encoder = new TextEncoder();
function input(lines) {
  return encoder.encode("0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n" +
    "0\nLINE\n8\n0\n10\n0\n20\n0\n11\n10\n21\n20\n".repeat(lines) + "0\nENDSEC\n0\nEOF\n").buffer;
}

// 同期 API と同じシーンを返し、入力の所有権を移す。result を受けたときには worker は終了している。
{
  const bytes = input(3);
  const session = loadSync(module).open();
  let expected;
  try {
    session.push("input", bytes);
    session.call({ v: 1, op: "open", input: "input" });
    const response = session.call({ v: 1, op: "scene", doc: "input", from: "space/0" });
    expected = session.take(response.result.blob);
  } finally {
    session.close();
  }
  const job = createSceneJob(module, bytes);
  assert.equal(bytes.byteLength, 0, "転送後の入力は使えない");
  const { blob } = await job.result;
  assert.deepEqual(blob, expected);
  assert.equal(job.worker.threadId, -1);
}

// 起動の待ち時間を除き、大きな DXF の同期処理中にも主スレッドのタイマーが進む。
{
  const lines = 200_000;
  const job = createSceneJob(module, input(lines), { signal: AbortSignal.timeout(60_000) });
  let started;
  let previous;
  let maximum = 0;
  let ticks = 0;
  job.worker.on("message", ({ type }) => {
    if (type === "started") started = previous = performance.now();
  });
  const timer = setInterval(() => {
    if (started === undefined) return;
    const now = performance.now();
    maximum = Math.max(maximum, now - previous);
    previous = now;
    ticks += 1;
  }, 5);
  let reply;
  let elapsed;
  try {
    reply = await job.result;
    elapsed = performance.now() - started;
    maximum = Math.max(maximum, performance.now() - previous);
  } finally {
    clearInterval(timer);
  }
  assert.equal(reply.opened.result.format, "dxf");
  assert.equal(reply.response.result.counts.lines, lines);
  assert.equal(readScene(reply.blob).rows("line_xy"), lines * 2);
  assert.ok(ticks >= 2, "読み込みが終わる前にタイマーが繰り返し動く");
  // CI のスケジューリングの揺れを許し、読み込み全体を待つような遅延は断る。
  assert.ok(maximum < Math.max(500, elapsed / 2), `タイマーの最大間隔 ${maximum} ms、処理 ${elapsed} ms`);
  assert.equal(job.worker.threadId, -1);
  console.log(`worker: ${lines} 本、${Math.round(elapsed)} ms、タイマー ${ticks} 回、最大間隔 ${Math.round(maximum)} ms`);
}

// API の失敗は診断を失わず、worker も終了する。
{
  const job = createSceneJob(module, encoder.encode("invalid").buffer);
  await assert.rejects(job.result, (error) => error.status === "failed" && error.response.diagnostics.length > 0);
  assert.equal(job.worker.threadId, -1);
  const limited = createSceneJob(module, input(1), { budget: { input_bytes: 0 } });
  await assert.rejects(limited.result, (error) =>
    error.status === "failed" && error.response.diagnostics.some((d) => d.code === "budget.input-bytes.exceeded"));
  assert.equal(limited.worker.threadId, -1);
}

// wasm を実行中はメッセージで cancel を呼べないので、worker ごと止める。
{
  const controller = new AbortController();
  const job = createSceneJob(module, input(200_000), { signal: controller.signal });
  job.worker.on("message", ({ type }) => { if (type === "started") controller.abort(); });
  await assert.rejects(job.result, { name: "AbortError" });
  assert.equal(job.worker.threadId, -1);
  const untouched = input(1);
  assert.throws(() => createSceneJob(module, untouched, { signal: controller.signal }), { name: "AbortError" });
  assert.notEqual(untouched.byteLength, 0);
}

// 転送に失敗した場合と、結果を送る前に終了した場合も、Promise を保留のままにしない。
{
  const bytes = input(1);
  markAsUntransferable(bytes);
  const job = createSceneJob(module, bytes);
  await assert.rejects(job.result, { name: "DataCloneError" });
  assert.equal(job.worker.threadId, -1);
  assert.notEqual(bytes.byteLength, 0);
  const stopped = createSceneJob(module, input(1));
  const rejected = assert.rejects(stopped.result, /結果を返さず終了/);
  await stopped.worker.terminate();
  await rejected;
  assert.equal(stopped.worker.threadId, -1);
}

console.log("worker の転送・タイマー・失敗と中断の後始末を確かめた");
