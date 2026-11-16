// Node.js の非同期の子プロセス API から全適合ケースを流す（#358）。
// node bindings/js/test/process-conformance.mjs <zumen の実行ファイル>
import { createHash } from "node:crypto";
import { compareHashes, loadCases, readSample } from "../../conformance/cases.mjs";
import { openProcess } from "../src/node.js";
import { runConformance } from "./run.mjs";

const [executable] = process.argv.slice(2);
if (!executable) {
  console.error("使い方: node bindings/js/test/process-conformance.mjs <zumen の実行ファイル>");
  process.exit(2);
}
const cases = loadCases();
const { hashes, scenes, failures } = await runConformance({
  zumen: { open: (options) => openProcess(executable, options, { timeoutMs: 120_000 }) },
  cases,
  readSample,
  sha256: (bytes) => createHash("sha256").update(bytes).digest("hex"),
});
compareHashes(hashes, failures);
console.log(`子プロセスのラッパー: ${cases.length} ケース、${hashes.length} ハッシュ、${scenes} シーン`);
if (failures.length) {
  console.error(failures.join("\n"));
  process.exitCode = 1;
} else console.log("ネイティブと同じ一覧になった");
