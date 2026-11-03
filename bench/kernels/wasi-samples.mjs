// wasm32-wasip1 に作った samples を Node.js の WASI で流す。samples/ を /samples として渡す。
// 使い方: node wasi-samples.mjs <wasm> [候補] <IFC>...（IFC は ../../samples/ からの道筋）
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { WASI } from "node:wasi";

const [file, ...rest] = process.argv.slice(2);
const prefix = "../../samples/";
const args = rest.map((a) => (a.startsWith(prefix) ? `/samples/${a.slice(prefix.length)}` : a));
const wasi = new WASI({
  version: "preview1",
  args: ["samples", ...args],
  preopens: { "/samples": resolve("../../samples") },
  returnOnExit: true,
});
const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
process.exitCode = wasi.start(instance);
