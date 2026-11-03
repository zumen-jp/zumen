// Node.js とサーバーで使う形を確かめる（決定 0259）。
//
// 使い方: node bindings/js/test/server.mjs <zumen_abi1.wasm>
//
// - 包みを一時のフォルダーに写して wasm を置き、Node.js の入口（src/node.js）で読み込んで図面を開く。wasm がなくて
//   失敗したあとも、置けば読み込める
// - 別の realm（node:vm）で作った WebAssembly.Module と ArrayBuffer と、別の realm の Response と同じ形のものを、
//   load・loadSync・Zumen.open・Session.push・readScene に渡す
// - wasm の中で trap が起きたら、その呼び出しと、それからの呼び出しが panicked の ZumenError になり、close は投げない。
//   読み込むときの trap も panicked になる。failed の応答では、インスタンスは使えるまま。読み込み直したインスタンスで
//   は続けられ、分けたモジュールの Modules は、使えなくなったインスタンスと、読み込みに失敗したものを読み込み直す

import assert from "node:assert/strict";
import { copyFileSync, cpSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import vm from "node:vm";

import { Zumen, ZumenError, load, loadSync, readScene } from "../src/index.js";
import { Modules } from "../src/modules.js";

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

  /** バイト列を、別の realm の ArrayBuffer に写す */
  const foreign = vm.runInContext("(data) => new Uint8Array(data).buffer", context);
  const buffer = foreign(bytes);
  assert.equal(buffer instanceof ArrayBuffer, false, "試験の前提: 別の realm の ArrayBuffer");
  assert.equal(openSample(await load(buffer)), "dxf");
  assert.equal(openSample(loadSync(buffer)), "dxf");

  // node:vm の realm には Response がないので、別の realm の Response と同じく、instanceof が偽で、型の名前が
  // Response のものを作る
  const response = vm.runInContext(
    "(data) => ({ [Symbol.toStringTag]: 'Response', ok: true, status: 200, arrayBuffer: async () => new Uint8Array(data).buffer })",
    context,
  )(bytes);
  assert.equal(response instanceof Response, false, "試験の前提: 別の realm の Response");
  assert.equal(openSample(await load(response)), "dxf");

  // セッションの設定・入力・シーンの blob
  const zumen = loadSync(module);
  // 設定の ArrayBuffer を値と取り違えると、JSON の {} になり、v がないので invalid になる
  assert.throws(() => zumen.open({}), (error) => error instanceof ZumenError && error.status === "invalid");
  const session = zumen.open(foreign(new TextEncoder().encode(JSON.stringify({ v: 1 }))));
  try {
    session.push("a.dxf", foreign(sample));
    assert.equal(session.call({ v: 1, op: "open", input: "a.dxf" }).result.format, "dxf");
    const { result, blobs } = session.call({ v: 1, op: "scene", doc: "a.dxf", from: "space/0" });
    const scene = readScene(foreign(session.take(blobs.find((blob) => blob.kind === "scene").id)));
    assert.equal(scene.chunks, result.counts.chunks);
    assert.equal(scene.rows("text_xy"), result.counts.texts);
  } finally {
    session.close();
  }
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
  /** name の関数で trap を起こすインスタンス。2 GiB 以上の確保は Vec の容量があふれてパニックし、wasm では trap になる */
  function trapping(name = "zm_call") {
    const instance = new WebAssembly.Instance(module, {});
    return { exports: { ...instance.exports, [name]: () => instance.exports.zm_alloc(0x80000000) } };
  }

  const zumen = new Zumen(trapping());
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
  const other = new Zumen(trapping()).open();
  assert.throws(() => {
    try {
      other.call({ v: 1, op: "capabilities" });
    } finally {
      other.close();
    }
  }, isPanicked("zm_call"));

  // 読み込むときの trap も panicked になる
  assert.throws(() => new Zumen(trapping("zm_abi_version")), isPanicked("zm_abi_version"));

  // 分けたモジュールの Modules は、使えなくなったインスタンスを捨てて、読み込み直す
  let loads = 0;
  const modules = new Modules({ drawing: () => module }, async () => {
    loads += 1;
    return new Zumen(trapping());
  });
  const first = await modules.get("drawing");
  assert.equal(await modules.get("drawing"), first, "使えるうちは同じインスタンスを返す");
  assert.throws(() => openSample(first), isPanicked("zm_call"));
  const second = await modules.get("drawing");
  assert.notEqual(second, first);
  assert.equal(second.broken, false);
  assert.equal(loads, 2);

  // 読み込みに失敗したら覚えず、次に引いたときにやり直す
  let attempts = 0;
  const retrying = new Modules({ drawing: () => module }, async (source) => {
    attempts += 1;
    if (attempts === 1) {
      throw new Error("一時的な失敗");
    }
    return load(source);
  });
  await assert.rejects(retrying.get("drawing"), { message: "一時的な失敗" });
  assert.equal(openSample(await retrying.get("drawing")), "dxf");
  assert.equal(attempts, 2);

  // 読み込み直せば続けられる
  assert.equal(openSample(await load(module)), "dxf");
}

console.log("Node.js の入口・別の realm の値・trap のあとの扱いを確かめた");
