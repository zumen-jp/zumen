// 適合試験を、wasm のモジュールで流す（決定 0012）。
//
// 使い方: node bindings/conformance/wasm.mjs <zumen_abi1.wasm>
//
// cases/*.json を、Rust の側（src/lib.rs）と同じ規則で流し、ハッシュの一覧が hashes.txt と一致することを確かめる。
// あわせて、モジュールが import を持たず、schema/boundary.toml の wasm の関数を出していることを確かめる（決定 0011）。

import { createHash } from "node:crypto";
import { appendFileSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { brotliCompressSync } from "node:zlib";

import { ACTIONS, bytesOf, checkResponse, compareHashes, loadCases, pushOf, repository, requestOf } from "./cases.mjs";

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) {
  console.error("使い方: node bindings/conformance/wasm.mjs <zumen_abi1.wasm>");
  process.exit(2);
}

const failures = [];

// 状態コードは、生成したヘッダーから読む
const header = readFileSync(join(repository, "bindings/c/include/zumen.h"), "utf8");
const statusNames = new Map();
for (const [, name, value] of header.matchAll(/#define ZM_([A-Z]+) (\d+)$/gm)) {
  statusNames.set(Number(value), name.toLowerCase());
}
const abiVersion = Number(header.match(/#define ZM_ABI_VERSION (\d+)/)[1]);

// wasm のモジュールが出す関数は、schema/boundary.toml の targets で決まる
const boundary = readFileSync(join(repository, "schema/boundary.toml"), "utf8");
const expectedExports = boundary
  .split("[[functions]]")
  .slice(1)
  .filter((block) => {
    const targets = block.match(/^targets = \[([^\]]*)\]/m);
    return !targets || targets[1].includes('"wasm"');
  })
  .map((block) => block.match(/^name = "([^"]+)"/m)[1])
  .sort();

const bytes = readFileSync(wasmPath);
const module = new WebAssembly.Module(bytes);
const imports = WebAssembly.Module.imports(module);
if (imports.length > 0) {
  failures.push(`モジュールが import を持つ: ${imports.map((i) => `${i.module}.${i.name}`).join(", ")}`);
}
const functions = WebAssembly.Module.exports(module)
  .filter((entry) => entry.kind === "function")
  .map((entry) => entry.name)
  .sort();
if (JSON.stringify(functions) !== JSON.stringify(expectedExports)) {
  failures.push(`出す関数が合わない: ${functions.join(", ")}（期待は ${expectedExports.join(", ")}）`);
}

const instance = new WebAssembly.Instance(module, {});
const e = instance.exports;
if (e.zm_abi_version() !== abiVersion) {
  failures.push(`zm_abi_version が ${e.zm_abi_version()}（期待は ${abiVersion}）`);
}

function memory() {
  return new Uint8Array(e.memory.buffer);
}

function view() {
  return new DataView(e.memory.buffer);
}

/// バイト列をモジュールのメモリに写して fn に渡し、終わったら返す
function withBytes(data, fn) {
  const ptr = data.length === 0 ? 0 : e.zm_alloc(data.length) >>> 0;
  if (data.length > 0) {
    memory().set(data, ptr);
  }
  try {
    return fn(ptr, data.length);
  } finally {
    if (data.length > 0) {
      e.zm_dealloc(ptr, data.length);
    }
  }
}

/// zm_buf（wasm32 では ptr と len の 2 つの u32）を確保して fn に渡し、書かれたバイト列を写して返す
function withBuf(fn) {
  const buf = e.zm_alloc(8) >>> 0;
  view().setUint32(buf, 0, true);
  view().setUint32(buf + 4, 0, true);
  try {
    const status = fn(buf);
    const ptr = view().getUint32(buf, true);
    const len = view().getUint32(buf + 4, true);
    let data = ptr === 0 ? null : memory().slice(ptr, ptr + len);
    if (data === null && status === 0) {
      data = new Uint8Array();
    }
    e.zm_free(buf);
    return [status, data];
  } finally {
    e.zm_dealloc(buf, 8);
  }
}

class Engine {
  constructor() {
    this.session = 0;
  }

  close() {
    e.zm_session_close(this.session);
    this.session = 0;
  }

  sessionNew(options) {
    this.close();
    const out = e.zm_alloc(4) >>> 0;
    view().setUint32(out, 0, true);
    const status = withBytes(options, (ptr, len) => e.zm_session_new(ptr, len, out));
    this.session = view().getUint32(out, true);
    e.zm_dealloc(out, 4);
    return status;
  }

  push(name, data) {
    return withBytes(name, (namePtr, nameLen) =>
      withBytes(data, (dataPtr, dataLen) => e.zm_push(this.session, namePtr, nameLen, dataPtr, dataLen)),
    );
  }

  call(request) {
    return withBytes(request, (ptr, len) => withBuf((buf) => e.zm_call(this.session, ptr, len, buf)));
  }

  take(blob) {
    return withBuf((buf) => e.zm_take(this.session, blob, buf));
  }
}

const hashes = [];
const cases = loadCases();
for (const { name, steps } of cases) {
  const engine = new Engine();
  let hasSession = false;
  steps.forEach((step, index) => {
    const at = `${name} の手順 ${index}`;
    const action = ACTIONS.find((key) => Object.hasOwn(step, key));
    const expected = step.status ?? "ok";
    if (action !== "session" && !hasSession) {
      engine.sessionNew(new Uint8Array());
      hasSession = true;
    }
    const input = step[action];
    let status;
    let output = null;
    if (action === "session") {
      hasSession = true;
      status = engine.sessionNew(bytesOf(input, "options", "options_text"));
    } else if (action === "push") {
      status = engine.push(...pushOf(input));
    } else if (action === "take") {
      const [taken, data] = engine.take(input);
      status = taken;
      output = data && ["blob", data];
    } else {
      const [called, response] = engine.call(requestOf(action, input));
      status = called;
      output = response && ["response", response];
    }
    const actual = statusNames.get(status) ?? `不明な状態 ${status}`;
    if (actual !== expected) {
      failures.push(`${at}: 状態が ${actual}（期待は ${expected}）`);
    }
    if (!output) {
      return;
    }
    const [kind, data] = output;
    hashes.push(`${name} ${index} ${kind} ${createHash("sha256").update(data).digest("hex")}`);
    if (kind === "response") {
      checkResponse(at, step, JSON.parse(new TextDecoder().decode(data)), failures);
    }
  });
  // 動いている呼び出しがないので、何もしない
  e.zm_cancel(engine.session);
  engine.close();
}

compareHashes(hashes, failures);

// モジュールの大きさを記録する。分け方を見直す材料にする（決定 0013）
const brotli = brotliCompressSync(bytes).length;
const summary = `モジュール: ${bytes.length} バイト（brotli で ${brotli} バイト）、試験の組 ${cases.length} 件、ハッシュ ${hashes.length} 件`;
console.log(summary);
if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### wasm32 の適合試験\n\n${summary}\n`);
}
if (failures.length > 0) {
  console.error(`期待と違った:\n${failures.join("\n")}`);
  process.exit(1);
}
console.log("ネイティブと同じ一覧になった");
