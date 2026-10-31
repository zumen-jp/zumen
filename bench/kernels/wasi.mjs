// wasm32-wasip1 に作った compare を Node.js の WASI で流す。使い方: node wasi.mjs <wasm> [候補] [組]
import { readFileSync } from "node:fs";
import { WASI } from "node:wasi";

const [file, ...args] = process.argv.slice(2);
const wasi = new WASI({ version: "preview1", args: ["compare", ...args], returnOnExit: true });
const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
process.exitCode = wasi.start(instance);
