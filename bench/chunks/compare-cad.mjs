// ネイティブで測った同じ DWG・JWW を WASI に渡し、塊の設定ごとに監査結果とシーン全体を照らす。
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { closeSync, mkdirSync, openSync, readFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { WASI } from "node:wasi";

const [wasm, input, native, nativeRows, output] = process.argv.slice(2);
if (!wasm || !input || !native || !nativeRows || !output) {
  throw new Error("使い方: node compare-cad.mjs <cad.wasm> <図面> <native のシーン> <native の JSONL> <WASI の出力先>");
}
// ネイティブのシーンを上書きして同じファイル同士を比べないよう、新しいフォルダーを使う。
mkdirSync(output);
const log = join(output, "measure.jsonl");
const fd = openSync(log, "w");
try {
  const wasi = new WASI({
    version: "preview1",
    args: ["cad", "/input/" + basename(input), "/output"],
    env: {},
    preopens: { "/input": resolve(dirname(input)), "/output": resolve(output) },
    stdout: fd,
    returnOnExit: true,
  });
  const { instance } = await WebAssembly.instantiate(readFileSync(wasm), wasi.getImportObject());
  assert.equal(wasi.start(instance), 0);
} finally {
  closeSync(fd);
}

const rows = (file) => readFileSync(file, "utf8").trim().split(/\r?\n/).map((line) => JSON.parse(line));
const measured = rows(nativeRows);
const other = rows(log);
const policies = ["provisional", "items-1024", "items-16384", "span-16384", "span-262144"];
assert.deepEqual(measured.map((row) => row.policy), policies);
assert.deepEqual(other.map((row) => row.policy), policies);
for (const [i, row] of measured.entries()) {
  const { policy, metrics, input_diagnostics } = row;
  assert.deepEqual(other[i].metrics, metrics);
  assert.equal(other[i].input_diagnostics, input_diagnostics);
  const bytes = readFileSync(join(native, policy + ".bin"));
  assert.ok(bytes.equals(readFileSync(join(output, policy + ".bin"))), policy);
  console.log(JSON.stringify({
    policy, metrics, input_diagnostics, native_ms: row.median_ms, wasi_ms: other[i].median_ms,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  }));
}
