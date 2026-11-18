// 実際の CLI と Node.js 接続で、往復と差分の適用を測る。
// node --expose-gc bench/live/native-ipc.mjs <release の zumen> [要素数か入力のファイル] [試行数] [スレッド数]
// 要素数なら LINE だけの DXF を作る。ファイルなら、描く順の行のうち移動を受ける行を選んで測る
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { cpus } from "node:os";
import { performance } from "node:perf_hooks";
import { LiveProcess } from "../../bindings/js/src/live.js";

const [executable, size = "10000", trials = "1000", threadCount = "1"] = process.argv.slice(2);
const inputPath = /^\d+$/.test(size) ? null : size;
const count = inputPath === null ? Number(size) : 1, iterations = Number(trials), threads = Number(threadCount);
if (!executable || !Number.isInteger(count) || count < 1 || count > 1_000_000
  || !Number.isInteger(iterations) || iterations < 10 || iterations > 10000
  || !Number.isInteger(threads) || threads < 1 || threads > 64) throw new Error("実行ファイル・要素数 1〜1000000・試行数 10〜10000・スレッド数 1〜64 を指定する");

function* inputParts() {
  yield "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n";
  for (let row = 0; row < count; row++) {
    const x = row % 1000, y = Math.floor(row / 1000);
    yield `0\nLINE\n5\n${(row + 16).toString(16)}\n8\n0\n10\n${x}\n20\n${y}\n11\n${x + 1}\n21\n${y}\n`;
  }
  yield "0\nENDSEC\n0\nEOF\n";
}

function input() {
  if (inputPath !== null) return readFileSync(inputPath);
  const bytes = Buffer.alloc(count * 100 + 256);
  let offset = 0;
  for (const part of inputParts()) offset += bytes.write(part, offset);
  return bytes.subarray(0, offset);
}

function assertOriginal(bytes) {
  let position = 0;
  for (const part of inputParts()) {
    // 合成入力は ASCII のみ。全体の写しを持たず、全バイトを直接照合する。
    for (let i = 0; i < part.length; i++) {
      if (bytes[position++] !== part.charCodeAt(i)) throw new Error("undo 後の保存バイト列が入力と違う");
    }
  }
  assert.equal(position, bytes.length);
}

function peak(pid) {
  if (process.platform === "win32") {
    return Number(execFileSync("pwsh", ["-NoProfile", "-Command", `(Get-Process -Id ${pid}).PeakWorkingSet64`], { encoding: "utf8", windowsHide: true }).trim());
  }
  if (process.platform === "linux") {
    const value = readFileSync(`/proc/${pid}/status`, "utf8").match(/^VmHWM:\s+(\d+) kB$/m);
    return value ? Number(value[1]) * 1024 : null;
  }
  return null;
}

function summary(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const at = (fraction) => sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)];
  return { median: at(0.5), p95: at(0.95), p99: at(0.99), maximum: sorted.at(-1), over1000us: sorted.filter((value) => value > 1000).length };
}

function hash(values) {
  const digest = createHash("sha256");
  for (const value of values) digest.update(value);
  return digest.digest("hex");
}

const child = spawn(executable, ["live", "--stdio"], { stdio: ["pipe", "pipe", "ignore"], windowsHide: true, shell: false });
const live = await LiveProcess.create(child, { timeoutMs: 120000 });
let columns, revision = null;
function apply(reply) {
  assert.equal(reply.status, "ok");
  assert.equal(reply.displayStatus, "ok");
  assert(reply.display);
  if (!reply.display.full) assert.equal(reply.base, revision);
  if (!columns) columns = new Array(13);
  for (const column of reply.display.columns) {
    const length = column.rows * column.stride;
    if (reply.display.full || columns[column.id]?.length !== length) columns[column.id] = Buffer.alloc(length);
    for (const block of column.blocks()) columns[column.id].set(block.bytes, block.start * column.stride);
  }
  revision = reply.revision;
  live.acknowledge(revision);
}

try {
  let bytes = input();
  const inputBytes = bytes.length;
  const inputHash = hash([bytes]);
  const begin = performance.now();
  let opened = await live.open(bytes, { threads });
  const initialPacketBytes = opened.data.length;
  const initialDisplay = opened.display;
  assert(initialDisplay.full);
  apply(opened);
  const openAndMirrorMs = performance.now() - begin;
  const initialMirrorHash = hash(columns);
  bytes = null;
  opened = null;
  if (global.gc) global.gc();
  // 入力のファイルでは、保存の比較先を開いた直後の保存にする（形式によっては読み書きで正規化する）
  const firstSaved = inputPath === null ? null : await live.save();
  if (firstSaved) assert.equal(firstSaved.status, "ok");
  const firstSavedHash = firstSaved ? hash([firstSaved.data]) : inputHash;
  const savedEqualsInput = firstSaved ? Buffer.compare(firstSaved.data, input()) === 0 : true;
  const targetUs = [], transformUs = [], undoUs = [], packetBytes = [];
  const run = async (row, record) => {
    let start = performance.now();
    const target = await live.target(row, revision);
    assert.equal(target.status, "ok");
    if (record) targetUs.push((performance.now() - start) * 1000);
    start = performance.now();
    const changed = await live.transform({ revision, rows: [row], matrix: [1, 0, 1, 0, 1, 2] });
    if (changed.status === "rejected") return false;
    apply(changed);
    if (record) { transformUs.push((performance.now() - start) * 1000); packetBytes.push(changed.data.length); }
    start = performance.now();
    apply(await live.undo(revision));
    if (record) undoUs.push((performance.now() - start) * 1000);
    return true;
  };
  // 移動を受ける行。合成の DXF はすべての行が受ける。ファイルでは、描く順の行を混ぜた順に試し、受けた行を 1000 行まで使う
  let rows = null, drawnRows = count, rejectedRows = 0;
  if (inputPath !== null) {
    const order = await live.order(revision);
    assert.equal(order.status, "ok");
    const drawn = [...order.order.rows()];
    drawnRows = drawn.length;
    rows = [];
    for (let i = 0; i < drawn.length && rows.length < 1000; i++) {
      const row = drawn[(Math.imul(i, 2654435761) >>> 0) % drawn.length];
      if (rows.includes(row)) continue;
      if (await run(row, false)) rows.push(row);
      else rejectedRows += 1;
    }
    assert(rows.length > 0, "移動を受ける行がない");
  }
  const rowAt = (i) => rows === null ? i % count : rows[i % rows.length];
  for (let i = 0; i < 100; i++) assert(await run(rowAt(i), false));
  if (global.gc) global.gc();
  for (let i = 0; i < iterations; i++) assert(await run(rowAt(Math.imul(i, 2654435761) >>> 0), true));
  const editPeakBytes = peak(child.pid);
  const saved = await live.save();
  assert.equal(saved.status, "ok");
  if (inputPath === null) assertOriginal(saved.data);
  for (const column of initialDisplay.columns) {
    const actual = columns[column.id];
    assert.equal(actual.length, column.rows * column.stride);
    for (const block of column.blocks()) {
      const start = block.start * column.stride;
      assert.equal(Buffer.compare(actual.subarray(start, start + block.bytes.length), block.bytes), 0);
    }
  }
  assert.equal(hash([saved.data]), firstSavedHash);
  assert.equal(hash(columns), initialMirrorHash);
  console.log(JSON.stringify({
    platform: process.platform, architecture: process.arch, node: process.version, cpu: cpus()[0]?.model.trim(),
    nativeCommit: process.env.ZUMEN_BENCH_COMMIT ?? null, count, iterations, threads,
    input: inputPath, drawnRows, movableRows: rows?.length ?? count, rejectedRows, savedEqualsInput,
    inputBytes, initialPacketBytes, openAndMirrorMs, childEditPeakBytes: editPeakBytes,
    childPeakIncludingSaveBytes: peak(child.pid), parentPeakBytes: peak(process.pid), parentRssAfterBytes: process.memoryUsage().rss,
    undoSaveHash: firstSavedHash, undoMirrorHash: initialMirrorHash,
    targetUs: summary(targetUs), transformWithDisplayUs: summary(transformUs), undoWithDisplayUs: summary(undoUs),
    deltaPacketBytes: { min: Math.min(...packetBytes), max: Math.max(...packetBytes) },
  }, null, 2));
} finally { await live.close(); }
