// Node.js とサーバーで使う形を確かめる（決定 0259）。
//
// 使い方: node bindings/js/test/server.mjs <zumen_abi1.wasm>
//
// - 包みを一時のフォルダーに写して wasm を置き、Node.js の入口（src/node.js）で読み込んで図面を開く。wasm がなくて
//   失敗したあとも、置けば読み込める
// - 別の realm（node:vm）で作った WebAssembly.Module と ArrayBuffer を、load・loadSync で読み込む
// - wasm の中で trap が起きたら、その呼び出しと、それからの呼び出しが panicked の ZumenError になり、close は投げない。
//   failed の応答では、インスタンスは使えるまま。読み込み直したインスタンスでは続けられる

import assert from "node:assert/strict";
import { copyFileSync, cpSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import vm from "node:vm";

import { Zumen, ZumenError, load, loadSync } from "../src/index.js";

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) {
  console.error("使い方: node bindings/js/test/server.mjs <zumen_abi1.wasm>");
  process.exit(2);
}

const bytes = readFileSync(wasmPath);
const sample = readFileSync(fileURLToPath(new URL("../../../samples/dxf/text_alignments.dxf", import.meta.url)));

/** 公開サンプルの DXF を開き、判定した形式を返す */
function openSample(zumen) {
  const session = zumen.open();
  try {
    session.push("a.dxf", sample);
    return session.call({ v: 1, op: "open", input: "a.dxf" }).result.format;
  } finally {
    session.close();
  }
}

function isPanicked(operation) {
  return (error) =>
    error instanceof ZumenError &&
    error.status === "panicked" &&
    (operation === undefined || error.operation === operation) &&
    error.cause instanceof WebAssembly.RuntimeError;
}

// Node.js の入口
{
  const staged = mkdtempSync(join(tmpdir(), "zumen-js-"));
  try {
    cpSync(fileURLToPath(new URL("../src", import.meta.url)), join(staged, "src"), { recursive: true });
    const { loadBundled } = await import(pathToFileURL(join(staged, "src", "node.js")).href);
    await assert.rejects(loadBundled(), { code: "ENOENT" });
    copyFileSync(wasmPath, join(staged, "zumen_abi1.wasm"));
    const first = await loadBundled();
    const second = await loadBundled();
    assert.notEqual(first, second, "loadBundled は呼ぶたびに新しいインスタンスを返す");
    assert.equal(openSample(first), "dxf");
    assert.equal(openSample(second), "dxf");
  } finally {
    rmSync(staged, { recursive: true, force: true });
  }
}

// 別の realm で作ったもの
{
  const context = vm.createContext({ bytes });
  const module = vm.runInContext("new WebAssembly.Module(bytes)", context);
  assert.equal(module instanceof WebAssembly.Module, false, "試験の前提: 別の realm のモジュール");
  assert.equal(openSample(await load(module)), "dxf");
  assert.equal(openSample(loadSync(module)), "dxf");
  const buffer = vm.runInContext("new Uint8Array(bytes).buffer", context);
  assert.equal(buffer instanceof ArrayBuffer, false, "試験の前提: 別の realm の ArrayBuffer");
  assert.equal(openSample(await load(buffer)), "dxf");
}

// failed の応答では、インスタンスは使えるまま
{
  const zumen = await load(bytes);
  const session = zumen.open();
  assert.throws(
    () => session.call({ v: 1, op: "open", input: "none.dxf" }),
    (error) => error instanceof ZumenError && error.status === "failed" && error.cause === undefined,
  );
  session.close();
  assert.equal(zumen.broken, false);
  assert.equal(openSample(zumen), "dxf");
}

// trap
{
  const module = new WebAssembly.Module(bytes);
  /** zm_call の中で trap を起こすインスタンス。2 GiB 以上の確保は Vec の容量があふれてパニックし、wasm では trap になる */
  function trapping() {
    const instance = new WebAssembly.Instance(module, {});
    return new Zumen({ exports: { ...instance.exports, zm_call: () => instance.exports.zm_alloc(0x80000000) } });
  }

  const zumen = trapping();
  assert.equal(zumen.broken, false);
  const session = zumen.open();
  session.push("a.dxf", sample);
  assert.throws(() => session.call({ v: 1, op: "open", input: "a.dxf" }), isPanicked("zm_call"));
  assert.equal(zumen.broken, true);
  // それからの呼び出しも panicked で、元の例外を持つ
  assert.throws(() => session.push("b.dxf", sample), isPanicked());
  assert.throws(() => zumen.open(), isPanicked());
  assert.throws(() => zumen.abiVersion(), isPanicked());
  // cancel と close は投げない
  session.cancel();
  session.close();
  assert.equal(session.closed, true);

  // finally の close が、先に出た例外を隠さない
  const other = trapping().open();
  assert.throws(() => {
    try {
      other.call({ v: 1, op: "capabilities" });
    } finally {
      other.close();
    }
  }, isPanicked("zm_call"));

  // 読み込み直せば続けられる
  assert.equal(openSample(await load(module)), "dxf");
}

console.log("Node.js の入口・別の realm のモジュール・trap のあとの扱いを確かめた");
