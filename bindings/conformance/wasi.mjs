// wasm32-wasip1 の zumen.wasm を、Node.js の WASI（node:wasi）で動かす（#403）。
//
// 使い方: node bindings/conformance/wasi.mjs <zumen.wasm> [CLI の引数]...
//
// 標準入出力だけを渡し、フォルダーは渡さない。終了コードは CLI の終了コード。

import { readFileSync } from "node:fs";
import { WASI } from "node:wasi";

const [path, ...args] = process.argv.slice(2);
if (!path) {
  console.error("使い方: node bindings/conformance/wasi.mjs <zumen.wasm> [CLI の引数]...");
  process.exit(2);
}
const wasi = new WASI({ version: "preview1", args: ["zumen", ...args], env: {}, preopens: {}, returnOnExit: true });
const module = await WebAssembly.compile(readFileSync(path));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exitCode = wasi.start(instance);
