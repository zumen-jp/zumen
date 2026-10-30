// 適合試験を、TypeScript・JavaScript の包み（bindings/js）を通して、Node.js で流す（決定 0006・0194）。
//
// 使い方: node bindings/js/test/conformance.mjs <zumen_abi1.wasm>
//
// bindings/conformance/cases/*.json を、包みのセッションのメソッドで流し（本体は run.mjs）、状態と応答の値が期待どおりで、
// ハッシュの一覧が hashes.txt と一致することを確かめる。シーンの blob は、包みの読み手で読み、応答の数と合うかを確かめる。
// 非同期の load と同期の loadSync の両方を試す。ブラウザーで流すものは browser/run.mjs にある（決定 0199）。

import { createHash } from "node:crypto";
import { appendFileSync, readFileSync } from "node:fs";

import { compareHashes, loadCases, readSample } from "../../conformance/cases.mjs";
import { load, loadSync } from "../src/index.js";
import { runConformance } from "./run.mjs";

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) {
  console.error("使い方: node bindings/js/test/conformance.mjs <zumen_abi1.wasm>");
  process.exit(2);
}

const bytes = readFileSync(wasmPath);
const zumen = await load(bytes);
const cases = loadCases();
const { hashes, scenes, failures } = await runConformance({
  zumen,
  cases,
  readSample,
  sha256: (data) => createHash("sha256").update(data).digest("hex"),
});
if (loadSync(new WebAssembly.Module(bytes)).abiVersion() !== zumen.abiVersion()) {
  failures.push("loadSync と load の ABI の大版が違う");
}
compareHashes(hashes, failures);

const summary = `包みを通した試験の組 ${cases.length} 件、ハッシュ ${hashes.length} 件、読んだシーン ${scenes} 件`;
console.log(summary);
if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### TypeScript・JavaScript の包みの適合試験\n\n${summary}\n`);
}
if (failures.length > 0) {
  console.error(`期待と違った:\n${failures.join("\n")}`);
  process.exit(1);
}
console.log("ネイティブと同じ一覧になった");
