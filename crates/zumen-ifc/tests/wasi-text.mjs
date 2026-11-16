// 文字列の値・ID・ハッシュ・診断・予算をネイティブと照らす。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const [file, ...args] = process.argv.slice(2);
if (file === "--compare") {
  const expected = readFileSync(path.join(args[0], "text.txt"));
  assert.ok(readFileSync(path.join(args[1], "text.txt")).equals(expected));
  console.log(`IFC の文字列: 3 版・3 指定の値・診断・予算が一致（${expected.length} バイト）`);
} else {
  const { WASI } = await import("node:wasi");
  const wasi = new WASI({
    version: "preview1", args: [file, ...args],
    env: { ZUMEN_IFC_TEXT_OUT: "/repo/target/text-comparison-wasm" },
    preopens: { "/repo": fileURLToPath(new URL("../../../", import.meta.url)) }, returnOnExit: true,
  });
  const { instance } = await WebAssembly.instantiate(readFileSync(file), wasi.getImportObject());
  process.exitCode = wasi.start(instance);
}
