// 入力・出力は公開サンプルか自作だけ。リポジトリ以外をWASIに公開しない。
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [file, ...args] = process.argv.slice(2);
if (file === "--compare") {
  const names = ["IFC2X3.json", "IFC4.json", "IFC4X3_ADD2.json", "contract.txt", "public_arbitrary.json", "public_circle.json", "public_rectangle.json"];
  const pick = folder => readdirSync(folder).filter(name => name.endsWith(".json") || name === "contract.txt").sort();
  assert.deepEqual(pick(args[0]), names);
  assert.deepEqual(pick(args[1]), names);
  let bytes = 0;
  for (const name of names) {
    const expected = readFileSync(path.join(args[0], name));
    assert.deepEqual(readFileSync(path.join(args[1], name)), expected, name);
    bytes += expected.length;
  }
  console.log(`断面と押し出し: JSON 6件と診断・全Kindの契約記録が一致（${bytes}バイト）`);
} else {
  const { WASI } = await import("node:wasi");
  const wasi = new WASI({ version: "preview1", args: [file, ...args],
    env: { ZUMEN_IFC_PROFILES_OUT: "/repo/target/profile-comparison-wasm" },
    preopens: { "/repo": fileURLToPath(new URL("../../../", import.meta.url)) }, returnOnExit: true });
  const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
  process.exitCode = wasi.start(instance);
}
