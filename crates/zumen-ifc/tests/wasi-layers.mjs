// レイヤの元値・診断・予算を WASI とネイティブで照らす。repo 以外は開かない。
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [file, ...args] = process.argv.slice(2);
if (file === "--compare") {
  const names = ["IFC2X3.json", "IFC4.json", "IFC4X3_ADD2.json", "contract.txt", "public.json"];
  const pick = (folder) => readdirSync(folder).filter((name) => name.endsWith(".json") || name === "contract.txt").sort();
  assert.deepEqual(pick(args[0]), names);
  assert.deepEqual(pick(args[1]), names);
  let bytes = 0;
  for (const name of names) {
    const expected = readFileSync(path.join(args[0], name));
    assert.ok(readFileSync(path.join(args[1], name)).equals(expected), name);
    bytes += expected.length;
  }
  console.log(`レイヤ: JSON 4 件と診断・全 Kind の記録が一致（${bytes} バイト）`);
} else {
  const { WASI } = await import("node:wasi");
  const wasi = new WASI({
    version: "preview1", args: [file, ...args],
    env: { ZUMEN_IFC_LAYERS_OUT: "/repo/target/layer-comparison-wasm" },
    preopens: { "/repo": fileURLToPath(new URL("../../../", import.meta.url)) }, returnOnExit: true,
  });
  const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
  process.exitCode = wasi.start(instance);
}
