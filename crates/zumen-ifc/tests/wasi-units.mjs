// 自作・公開の入力だけを使う。ホストの環境変数を渡さず、repo だけを開く。
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [file, ...args] = process.argv.slice(2);
if (file === "--compare") {
  const pick = folder => readdirSync(folder).filter(name => name.endsWith(".json") || name.endsWith("contract.txt")).sort();
  const names = pick(args[0]);
  assert.deepEqual(pick(args[1]), names);
  let bytes = 0;
  for (const name of names) {
    const expected = readFileSync(path.join(args[0], name));
    assert.deepEqual(readFileSync(path.join(args[1], name)), expected, name);
    bytes += expected.length;
  }
  console.log(`SI単位: JSONと値・出所・診断・全Kindの契約記録が一致（${bytes}バイト）`);
} else {
  const { WASI } = await import("node:wasi");
  const wasi = new WASI({ version: "preview1", args: [file, ...args],
    env: { ZUMEN_IFC_UNITS_OUT: "/repo/target/unit-comparison-wasm" },
    preopens: { "/repo": fileURLToPath(new URL("../../../", import.meta.url)) }, returnOnExit: true });
  const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
  process.exitCode = wasi.start(instance);
}
