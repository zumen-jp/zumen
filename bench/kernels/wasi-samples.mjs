// wasm32-wasip1 に作った samples を Node.js の WASI で流す。試料のフォルダー（環境変数 SAMPLES、既定は
// ../../samples/ifc）を /samples として渡し、その中の IFC の道筋を /samples からの道筋に直す。
// 使い方: node wasi-samples.mjs <wasm> [候補] <IFC>...
import { readFileSync } from "node:fs";
import { resolve, sep } from "node:path";
import { WASI } from "node:wasi";

const [file, ...rest] = process.argv.slice(2);
const root = resolve(process.env.SAMPLES ?? "../../samples/ifc");
const args = rest.map((a) => {
  const path = resolve(a);
  return path.startsWith(root + sep) ? `/samples/${path.slice(root.length + 1).split(sep).join("/")}` : a;
});
const wasi = new WASI({
  version: "preview1",
  args: ["samples", ...args],
  preopens: { "/samples": root },
  returnOnExit: true,
});
const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
process.exitCode = wasi.start(instance);
