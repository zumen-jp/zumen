// 適合試験の組を、JavaScript で流すものが共有する読み方（決定 0012）。
//
// cases/*.json の手順の読み方と、応答の確かめ方と、ハッシュの一覧の比べ方を、Rust の側（src/lib.rs）と同じ規則で書く。
// wasm.mjs（モジュールを直接呼ぶ）と bindings/js/test/conformance.mjs（TypeScript・JavaScript の包みを通す）が使う。
// ファイルを読まない部分は steps.mjs にあり、ここから出し直す。

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { pushOf as pushWith } from "./steps.mjs";

export { ACTIONS, bytesOf, checkResponse, decodeHex, pointer, requestOf, same, samplePath } from "./steps.mjs";

const here = dirname(fileURLToPath(import.meta.url));
export const repository = join(here, "..", "..");

/** 公開サンプル（決定 0171）を読む */
export function readSample(file) {
  return new Uint8Array(readFileSync(join(repository, file)));
}

/** push の手順の、入力の名前と中身 */
export function pushOf(input) {
  return pushWith(input, readSample);
}

/** 試験の組を、名前の順に返す */
export function loadCases() {
  return readdirSync(join(here, "cases"))
    .filter((file) => file.endsWith(".json"))
    .sort()
    .map((file) => ({
      name: file.slice(0, -".json".length),
      steps: JSON.parse(readFileSync(join(here, "cases", file), "utf8")).steps,
    }));
}

/** ハッシュの一覧が hashes.txt と一致するかを確かめる */
export function compareHashes(hashes, failures) {
  const expected = readFileSync(join(here, "hashes.txt"), "utf8")
    .split("\n")
    .filter((line) => line !== "" && !line.startsWith("#"));
  if (JSON.stringify(hashes) !== JSON.stringify(expected)) {
    const missing = expected.filter((line) => !hashes.includes(line)).map((line) => `- ${line}`);
    const extra = hashes.filter((line) => !expected.includes(line)).map((line) => `+ ${line}`);
    failures.push(`ハッシュの一覧が hashes.txt と合わない:\n${[...missing, ...extra].join("\n")}`);
  }
}
