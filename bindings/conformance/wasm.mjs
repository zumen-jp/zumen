// 適合試験を、wasm のモジュールで流す（決定 0012）。
//
// 使い方: node bindings/conformance/wasm.mjs <zumen_abi1.wasm>
//
// cases/*.json を、Rust の側（src/lib.rs）と同じ規則で流し、ハッシュの一覧が hashes.txt と一致することを確かめる。
// あわせて、モジュールが import を持たず、schema/boundary.toml の wasm の関数を出していることを確かめる（決定 0011）。

import { createHash } from "node:crypto";
import { appendFileSync, readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync } from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));
const repository = join(here, "..", "..");

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

const encoder = new TextEncoder();

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

function decodeHex(hex) {
  const digits = hex.replace(/\s+/g, "");
  if (!/^([0-9a-fA-F]{2})*$/.test(digits)) {
    throw new Error(`16 進として読めない: ${hex}`);
  }
  return Uint8Array.from(digits.match(/../g) ?? [], (pair) => parseInt(pair, 16));
}

function pointer(value, path) {
  if (path === "") {
    return value;
  }
  let current = value;
  for (const raw of path.slice(1).split("/")) {
    const token = raw.replaceAll("~1", "/").replaceAll("~0", "~");
    if (Array.isArray(current)) {
      if (!/^(0|[1-9][0-9]*)$/.test(token) || Number(token) >= current.length) {
        return undefined;
      }
      current = current[Number(token)];
    } else if (current !== null && typeof current === "object" && Object.hasOwn(current, token)) {
      current = current[token];
    } else {
      return undefined;
    }
  }
  return current;
}

function same(a, b) {
  if (Array.isArray(a) || Array.isArray(b)) {
    return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((x, i) => same(x, b[i]));
  }
  if (a !== null && b !== null && typeof a === "object" && typeof b === "object") {
    const keys = Object.keys(a);
    return keys.length === Object.keys(b).length && keys.every((key) => Object.hasOwn(b, key) && same(a[key], b[key]));
  }
  return a === b;
}

const ACTIONS = ["session", "push", "call", "call_text", "call_hex", "take"];

function bytesOf(input, valueKey, textKey) {
  if (input === null || input === undefined) {
    return new Uint8Array();
  }
  if (Object.hasOwn(input, valueKey)) {
    return encoder.encode(JSON.stringify(input[valueKey]));
  }
  if (Object.hasOwn(input, textKey)) {
    return encoder.encode(input[textKey]);
  }
  return new Uint8Array();
}

// 公開サンプル（決定 0171）を読む。道筋はリポジトリの根から書き、samples/ の中に限る
function readSample(file) {
  const parts = file.split("/");
  if (parts[0] !== "samples" || parts.length < 2 || parts.slice(1).some((p) => p === "" || p === "." || p === ".." || p.includes("\\"))) {
    throw new Error(`push の file は samples/ の中のファイルにする: ${file}`);
  }
  return new Uint8Array(readFileSync(join(repository, file)));
}

const hashes = [];
const caseFiles = readdirSync(join(here, "cases"))
  .filter((name) => name.endsWith(".json"))
  .sort();
for (const file of caseFiles) {
  const name = file.slice(0, -".json".length);
  const steps = JSON.parse(readFileSync(join(here, "cases", file), "utf8")).steps;
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
      const nameBytes = Object.hasOwn(input, "name_hex") ? decodeHex(input.name_hex) : encoder.encode(input.name);
      const data = Object.hasOwn(input, "hex")
        ? decodeHex(input.hex)
        : Object.hasOwn(input, "file")
          ? readSample(input.file)
          : encoder.encode(input.text ?? "");
      status = engine.push(nameBytes, data);
    } else if (action === "take") {
      const [taken, data] = engine.take(input);
      status = taken;
      output = data && ["blob", data];
    } else {
      const request =
        action === "call_text"
          ? encoder.encode(input)
          : action === "call_hex"
            ? decodeHex(input)
            : encoder.encode(JSON.stringify(input));
      const [called, response] = engine.call(request);
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
    if (kind !== "response") {
      return;
    }
    const response = JSON.parse(new TextDecoder().decode(data));
    for (const [path, value] of Object.entries(step.expect ?? {})) {
      const found = pointer(response, path);
      if (found === undefined) {
        failures.push(`${at}: ${path} がない`);
      } else if (!same(found, value)) {
        failures.push(`${at}: ${path} が ${JSON.stringify(found)}（期待は ${JSON.stringify(value)}）`);
      }
    }
    for (const path of step.absent ?? []) {
      if (pointer(response, path) !== undefined) {
        failures.push(`${at}: ${path} があってはいけない`);
      }
    }
  });
  // 動いている呼び出しがないので、何もしない
  e.zm_cancel(engine.session);
  engine.close();
}

const expectedHashes = readFileSync(join(here, "hashes.txt"), "utf8")
  .split("\n")
  .filter((line) => line !== "" && !line.startsWith("#"));
if (JSON.stringify(hashes) !== JSON.stringify(expectedHashes)) {
  const missing = expectedHashes.filter((line) => !hashes.includes(line)).map((line) => `- ${line}`);
  const extra = hashes.filter((line) => !expectedHashes.includes(line)).map((line) => `+ ${line}`);
  failures.push(`ハッシュの一覧が hashes.txt と合わない:\n${[...missing, ...extra].join("\n")}`);
}

// モジュールの大きさを記録する。分け方を見直す材料にする（決定 0013）
const brotli = brotliCompressSync(bytes).length;
const summary = `モジュール: ${bytes.length} バイト（brotli で ${brotli} バイト）、試験の組 ${caseFiles.length} 件、ハッシュ ${hashes.length} 件`;
console.log(summary);
if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### wasm32 の適合試験\n\n${summary}\n`);
}
if (failures.length > 0) {
  console.error(`期待と違った:\n${failures.join("\n")}`);
  process.exit(1);
}
console.log("ネイティブと同じ一覧になった");
