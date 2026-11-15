// コアの等価検査を WASI で流す。ファイルの出力は、指定した記録先だけに置く。
import { mkdirSync, readFileSync } from "node:fs";
import { basename, resolve } from "node:path";
import { WASI } from "node:wasi";

const [wasm, inputs, evidence, name, ...options] = process.argv.slice(2);
if (!wasm || !inputs || !evidence || !name || basename(name) !== name) {
  throw new Error("使い方: node bench/live/core-wasi.mjs <wasm> <入力フォルダー> <記録フォルダー> <図面名> [core-live の引数]");
}
mkdirSync(evidence, { recursive: true });
const wasi = new WASI({
  version: "preview1",
  args: ["core-live", "compare", `/inputs/${name}`, "--evidence-dir", "/evidence", ...options],
  env: {},
  preopens: { "/inputs": resolve(inputs), "/evidence": resolve(evidence) },
  returnOnExit: true,
});
const { instance } = await WebAssembly.instantiate(readFileSync(wasm), wasi.getImportObject());
process.exitCode = wasi.start(instance);
