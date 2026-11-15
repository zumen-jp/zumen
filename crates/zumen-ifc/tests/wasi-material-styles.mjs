// 材料の表示の元値・出所・診断・共有予算を照らす。
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [file, ...args] = process.argv.slice(2);
if (file === "--compare") {
  const names = readdirSync(args[0]).sort();
  assert.equal(names.length, 12);
  assert.deepEqual(readdirSync(args[1]).sort(), names);
  let bytes = 0;
  for (const name of names) {
    const expected = readFileSync(path.join(args[0], name));
    assert.ok(readFileSync(path.join(args[1], name)).equals(expected), name);
    bytes += expected.length;
  }
  console.log(`材料の表示: 12 ファイルの元値・出所・診断・予算が一致（${bytes} バイト）`);
} else {
  const { WASI } = await import("node:wasi");
  const wasi = new WASI({
    version: "preview1", args: [file, ...args],
    env: { ZUMEN_IFC_MATERIAL_OUT: "/repo/target/material-comparison-wasm" },
    preopens: { "/repo": fileURLToPath(new URL("../../../", import.meta.url)) }, returnOnExit: true,
  });
  const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
  process.exitCode = wasi.start(instance);
}
