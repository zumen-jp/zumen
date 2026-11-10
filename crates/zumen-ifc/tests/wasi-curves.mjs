// 曲線の Rust のテストを WASI で流す。使い方: node wasi-curves.mjs <test.wasm> --test-threads 1
// リポジトリだけを開き、ホストの環境変数を渡さない。JSON は target/curve-comparison-wasm に書く。
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { WASI } from "node:wasi";

const [file, ...args] = process.argv.slice(2);
const root = fileURLToPath(new URL("../../../", import.meta.url));
const wasi = new WASI({
  version: "preview1",
  args: [file, ...args],
  env: { ZUMEN_IFC_CURVES_OUT: "/repo/target/curve-comparison-wasm" },
  preopens: { "/repo": root },
  returnOnExit: true,
});
const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
process.exitCode = wasi.start(instance);
