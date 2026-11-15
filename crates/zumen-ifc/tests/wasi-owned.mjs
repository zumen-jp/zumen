// 所有する形の元値・出所・ハッシュ・診断・共有予算を照らす。
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [file, ...args] = process.argv.slice(2);
if (file === "--compare") {
  const names = ["IFC2X3.txt", "IFC4.txt", "IFC4X3_ADD2.txt", "public_column.txt", "public_wall.txt"];
  for (const folder of args) assert.deepEqual(readdirSync(folder).sort(), names);
  let bytes = 0;
  for (const name of names) {
    const expected = readFileSync(path.join(args[0], name));
    assert.ok(readFileSync(path.join(args[1], name)).equals(expected), name);
    bytes += expected.length;
  }
  console.log(`所有する形: 5 件の元値・出所・ハッシュ・診断・予算が一致（${bytes} バイト）`);
} else {
  const { WASI } = await import("node:wasi");
  const wasi = new WASI({
    version: "preview1", args: [file, ...args],
    env: { ZUMEN_IFC_OWNED_OUT: "/repo/target/owned-comparison-wasm" },
    preopens: { "/repo": fileURLToPath(new URL("../../../", import.meta.url)) }, returnOnExit: true,
  });
  const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
  process.exitCode = wasi.start(instance);
}
