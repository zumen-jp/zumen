// 入力の所有権・旧モジュールへの互換・wasm メモリを確かめる（決定 0271）。
// node bindings/js/test/input.mjs <wasm> [measure <owned|legacy> <bytes>]
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { Zumen, ZumenError } from "../src/index.js";

const [wasmPath, command, mode, amount] = process.argv.slice(2);
if (!wasmPath) {
  throw new Error("wasm のパスが必要");
}
const module = new WebAssembly.Module(readFileSync(wasmPath));
const sample = new TextEncoder().encode("0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n2\n0\nENDSEC\n0\nEOF\n");
const MiB = 1024 * 1024;

function fresh(legacy = false, change = () => {}) {
  const instance = new WebAssembly.Instance(module, {});
  const exports = { ...instance.exports };
  assert.equal(typeof exports.zm_push_owned, "function", "この検査には新しい wasm が必要");
  if (legacy) delete exports.zm_push_owned;
  change(exports, instance.exports);
  return { zumen: new Zumen({ exports }), raw: instance.exports };
}

if (command === "measure") {
  const size = Number(amount);
  assert(Number.isSafeInteger(size) && size > 0 && size <= 512 * MiB);
  assert(["owned", "legacy"].includes(mode));
  const { zumen, raw } = fresh(mode === "legacy");
  const session = zumen.open();
  const before = raw.memory.buffer.byteLength;
  session.push("input", new Uint8Array(size));
  const after = raw.memory.buffer.byteLength;
  session.close();
  console.log(JSON.stringify({ mode, size, before, after, delta: after - before }));
} else {
  for (const legacy of [false, true]) {
    const calls = [];
    const { zumen, raw } = fresh(legacy, (exports, actual) => {
      for (const name of ["zm_push", ...(legacy ? [] : ["zm_push_owned"])]) {
        exports[name] = (...args) => { calls.push(name); return actual[name](...args); };
      }
    });
    const session = zumen.open();
    session.push("a.dxf", new Uint8Array());
    const split = Math.floor(sample.length / 2);
    session.push("a.dxf", sample.subarray(0, split));
    session.push("a.dxf", sample.subarray(split));
    assert.equal(session.call({ v: 1, op: "open", input: "a.dxf" }).result.format, "dxf");
    assert(calls.every((name) => name === (legacy ? "zm_push" : "zm_push_owned")));
    const copied = sample.slice();
    session.push("b.dxf", copied);
    copied.fill(0);
    assert.equal(session.call({ v: 1, op: "open", input: "b.dxf" }).result.format, "dxf");
    const invalid = (error) => error instanceof ZumenError && error.status === "invalid" && error.operation === "zm_push";
    const large = new Uint8Array(4 * MiB);
    assert.throws(() => session.push("../bad", large), invalid);
    const afterFirst = raw.memory.buffer.byteLength;
    for (let i = 0; i < 16; ++i) assert.throws(() => session.push("../bad", large), invalid);
    assert(raw.memory.buffer.byteLength <= afterFirst + MiB, "無効な名前で入力の領域が残る");
    session.close();
    for (let i = 0; i < 16; ++i) assert.throws(() => session.push("closed", large), invalid);
    assert(raw.memory.buffer.byteLength <= afterFirst + MiB, "閉じたセッションで入力の領域が残る");
  }

  // 名前の確保から memory.grow を起こしても、同じ wasm のメモリを指す入力が無効にならない
  for (const legacy of [false, true]) for (const buffer of [false, true]) for (const wasmName of [false, true]) {
    const { zumen, raw } = fresh(legacy, (exports, actual) => {
      exports.zm_alloc = (length) => {
        const ptr = actual.zm_alloc(length);
        actual.memory.grow(1);
        return ptr;
      };
    });
    const session = zumen.open();
    const name = new TextEncoder().encode("a.dxf");
    const namePtr = wasmName ? raw.zm_alloc(name.length) : 0;
    const ptr = raw.zm_alloc(sample.length);
    const view = (at, length) => buffer
      ? Buffer.from(raw.memory.buffer, at, length)
      : new Uint8Array(raw.memory.buffer, at, length);
    const nameInput = wasmName ? view(namePtr, name.length) : "a.dxf";
    if (wasmName) nameInput.set(name);
    const input = view(ptr, sample.length);
    input.set(sample);
    session.push(nameInput, input);
    raw.zm_dealloc(ptr, sample.length);
    if (wasmName) raw.zm_dealloc(namePtr, name.length);
    assert.equal(session.call({ v: 1, op: "open", input: "a.dxf" }).result.format, "dxf");
    session.close();
  }

  // 引き渡し中の trap のあとは、後片付けでも wasm に戻らない
  let deallocations = 0;
  const { zumen } = fresh(false, (exports, actual) => {
    exports.zm_push_owned = () => actual.zm_alloc(0x80000000);
    exports.zm_dealloc = (...args) => { deallocations += 1; return actual.zm_dealloc(...args); };
  });
  const session = zumen.open();
  deallocations = 0;
  assert.throws(() => session.push("a", sample), (error) => error instanceof ZumenError && error.status === "panicked");
  assert.equal(zumen.broken, true);
  session.close();
  assert.equal(deallocations, 0);

  // インスタンスが残る GC の時期に依存しないよう、各測定を別の Node.js で行う
  const measure = (mode) => JSON.parse(execFileSync(process.execPath, [
    fileURLToPath(import.meta.url), wasmPath, "measure", mode, String(32 * MiB),
  ], { encoding: "utf8", windowsHide: true }));
  const owned = measure("owned");
  const legacy = measure("legacy");
  assert(owned.delta <= owned.size * 1.25, JSON.stringify(owned));
  assert(legacy.delta >= legacy.size * 1.75, JSON.stringify(legacy));
  assert(legacy.delta - owned.delta >= owned.size * 0.75);
  console.log(JSON.stringify({ owned, legacy }));
  console.log("入力の所有権・追記・失敗・旧モジュールへの互換・メモリの削減を確認した");
}
