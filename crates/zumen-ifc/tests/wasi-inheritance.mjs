// 写像を通る面の表示の選択・元値・診断・予算を照らす。
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [file, ...args] = process.argv.slice(2);
if (file === "--compare") {
  const names = readdirSync(args[0]).sort();
  assert.equal(names.length, 24);
  assert.deepEqual(readdirSync(args[1]).sort(), names);
  let bytes = 0;
  for (const name of names) {
    const expected = readFileSync(path.join(args[0], name));
    assert.ok(readFileSync(path.join(args[1], name)).equals(expected), name);
    bytes += expected.length;
  }
  console.log(`写像の表示: 24 ファイルの選択・元値・診断・予算が一致（${bytes} バイト）`);
} else {
  const { WASI } = await import("node:wasi");
  const wasi = new WASI({
    version: "preview1", args: [file, ...args],
    env: { ZUMEN_IFC_INHERITANCE_OUT: "/repo/target/inheritance-comparison-wasm" },
    preopens: { "/repo": fileURLToPath(new URL("../../../", import.meta.url)) }, returnOnExit: true,
  });
  const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
  process.exitCode = wasi.start(instance);
}
