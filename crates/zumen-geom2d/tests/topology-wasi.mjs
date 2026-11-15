// ファイルと環境変数を渡さず、固定した幾何演算の記録だけを標準出力へ返す。
import { readFileSync } from "node:fs";
import { WASI } from "node:wasi";

const [file] = process.argv.slice(2);
const wasi = new WASI({ version: "preview1", args: [file], env: {}, returnOnExit: true });
const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
process.exitCode = wasi.start(instance);
