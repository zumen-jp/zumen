// L1 の形状表現の Rust テストを WASI で流す。ホストの環境変数は渡さず、repo だけを開く。
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { WASI } from "node:wasi";

const [file, ...args] = process.argv.slice(2);
const wasi = new WASI({
  version: "preview1",
  args: [file, ...args],
  env: { ZUMEN_IFC_SHAPES_OUT: "/repo/target/shape-comparison-wasm" },
  preopens: { "/repo": fileURLToPath(new URL("../../../", import.meta.url)) },
  returnOnExit: true,
});
const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
process.exitCode = wasi.start(instance);
