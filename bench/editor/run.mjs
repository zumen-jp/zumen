// エディターの設計（#693・docs/editor.md）のための測り。prepare.mjs が書いた場合の一覧（plan.txt）を wasm で流す。
// src/main.rs が同じ一覧をネイティブで流す。
//
//   node run.mjs <図面用の wasm> <plan.txt> time [名前に含む文字]
//   node run.mjs <図面用の wasm> <plan.txt> work [名前に含む文字]
//
// 場合ごとに新しいインスタンスとセッションを作り、準備の push と call を流してから、測る call（1 つか、続けて流す
// いくつか）を流す。測る時間には、応答の blob を取り出す時間を含める。memory は、測る call のあとの wasm のメモリの大きさ（バイト）。
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { performance } from "node:perf_hooks";
import { Zumen } from "../../bindings/js/src/index.js";

const [wasmPath, planPath, mode = "time", filter = ""] = process.argv.slice(2);
if (!wasmPath || !planPath) {
  throw new Error("使い方: node run.mjs <図面用の wasm> <plan.txt> time|work [名前に含む文字]");
}
const module = new WebAssembly.Module(readFileSync(wasmPath));
const dir = dirname(planPath);

function readPlan() {
  const cases = [];
  let current = null;
  for (const line of readFileSync(planPath, "utf8").split("\n")) {
    const [tag, ...rest] = line.split("\t");
    if (tag === "case") {
      current = { name: rest[0], repeat: Number(rest[1]), kinds: rest[2] === "-" ? [] : rest[2].split(","), setup: [], timed: [] };
    } else if (tag === "push") {
      current.setup.push({ push: rest[0], path: join(dir, rest[1]) });
    } else if (tag === "call") {
      current.setup.push({ call: line.slice(5) });
    } else if (tag === "time") {
      current.timed.push(line.slice(5));
    } else if (tag === "end") {
      cases.push(current);
      current = null;
    }
  }
  return cases;
}

const files = new Map();
const fileOf = (path) => {
  if (!files.has(path)) files.set(path, readFileSync(path));
  return files.get(path);
};

function prepared(c) {
  const instance = new WebAssembly.Instance(module, {});
  const session = new Zumen(instance).open();
  for (const step of c.setup) {
    if (step.push) {
      session.push(step.push, fileOf(step.path));
    } else {
      session.callBytes(step.call);
    }
  }
  return { instance, session };
}

const decoder = new TextDecoder();

// 測る call を順に流し、応答の blob を取り出す。応答と blob のバイト数の和を返す。どれかが失敗なら null
function run(session, requests) {
  let replies = 0;
  let blobs = 0;
  for (const request of requests) {
    let reply;
    try {
      reply = session.callBytes(request);
    } catch (error) {
      if (error.status === "failed") return null;
      throw error;
    }
    replies += reply.length;
    for (const blob of JSON.parse(decoder.decode(reply)).blobs ?? []) {
      blobs += session.take(blob.id).length;
    }
  }
  return [replies, blobs];
}

function time(c) {
  const times = [];
  let sizes = [0, 0];
  let memory = 0;
  for (let i = 0; i < c.repeat; i++) {
    const { instance, session } = prepared(c);
    const start = performance.now();
    const result = run(session, c.timed);
    const elapsed = performance.now() - start;
    if (result === null) {
      console.log(`wasm\t${c.name}\tfailed`);
      return;
    }
    sizes = result;
    memory = instance.exports.memory.buffer.byteLength;
    times.push(elapsed);
    session.close();
  }
  times.sort((a, b) => a - b);
  const median = times[Math.floor(times.length / 2)];
  console.log(`wasm\t${c.name}\t${median.toFixed(2)}\t${times[0].toFixed(2)}\t${times.at(-1).toFixed(2)}\t${sizes[0]}\t${sizes[1]}\t${memory}`);
}

function passes(c, kind, limit) {
  const { session } = prepared(c);
  const requests = c.timed.map((request) => request.replace("{", `{"budget":{"${kind}":${limit}},`));
  const result = run(session, requests);
  session.close();
  return result !== null;
}

// 通る最小の予算を、単位ごとに二分探索する（src/main.rs と同じ手順）
function work(c) {
  for (const kind of c.kinds) {
    let low = 0;
    let high = 1;
    while (!passes(c, kind, high)) {
      low = high;
      high *= 4;
    }
    while (high - low > 1) {
      const mid = low + Math.floor((high - low) / 2);
      if (passes(c, kind, mid)) high = mid;
      else low = mid;
    }
    const used = low === 0 && passes(c, kind, 0) ? 0 : high;
    console.log(`wasm\t${c.name}\t${kind}\t${used}`);
  }
}

for (const c of readPlan()) {
  if (!c.name.includes(filter)) continue;
  if (mode === "time") time(c);
  else if (mode === "work" && c.kinds.length > 0) work(c);
}
