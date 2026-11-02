// 分けた wasm のモジュールで、適合試験を流す（決定 0013・0221）。
//
// 使い方: node bindings/js/test/modules.mjs <zumen_abi1.wasm> <drawing.wasm> <bim.wasm>
//
// 包みの loadModules でモジュールを読み込み、組ごとに、組が呼ぶ op を持つモジュール（steps.mjs の moduleOf）で流す
// （本体は run.mjs）。すべてのモジュールを入れたコアでだけ流す組（module が all）を除き、ハッシュの一覧が hashes.txt
// の同じ組の行と一致することを確かめる。あわせて、次を確かめる。
//
// - capabilities が、モジュールの持つ op だけを返し、持たない op には call.op.unsupported を返す
// - 包みの detect が、組が足すどの入力でも、すべてを入れたコア（1 つ目の引数）の open と同じ形式を返す

import { createHash } from "node:crypto";
import { appendFileSync, readFileSync } from "node:fs";
import { brotliCompressSync } from "node:zlib";

import { compareHashes, loadCases, moduleOf, pushOf, readSample } from "../../conformance/cases.mjs";
import { MODULES, ZumenError, detect, load, loadModules } from "../src/index.js";
import { runConformance } from "./run.mjs";

const [fullPath, ...paths] = process.argv.slice(2);
const names = Object.keys(MODULES);
if (!fullPath || paths.length !== names.length) {
  console.error(`使い方: node bindings/js/test/modules.mjs <zumen_abi1.wasm> ${names.map((name) => `<${name}.wasm>`).join(" ")}`);
  process.exit(2);
}

const failures = [];
const loaders = Object.fromEntries(names.map((name, index) => [name, () => readFileSync(paths[index])]));
const modules = loadModules(loaders);
const loaded = Object.fromEntries(await Promise.all(names.map(async (name) => [name, await modules.get(name)])));
if ((await modules.get(names[0])) !== loaded[names[0]]) {
  failures.push("loadModules が、同じモジュールを 2 度読み込んだ");
}

// モジュールが持つ op
for (const name of names) {
  const session = loaded[name].open();
  const { ops } = session.call({ v: 1, op: "capabilities" }).result;
  if (JSON.stringify([...ops].sort()) !== JSON.stringify([...MODULES[name].ops].sort())) {
    failures.push(`${name} の capabilities の op が MODULES と違う: ${ops.join(",")}`);
  }
  for (const op of names.flatMap((other) => MODULES[other].ops).filter((op) => !MODULES[name].ops.includes(op))) {
    try {
      session.call({ v: 1, op });
      failures.push(`${name} が、持たない op ${op} を断らない`);
    } catch (error) {
      const code = error instanceof ZumenError ? error.response?.diagnostics?.[0]?.code : String(error);
      if (code !== "call.op.unsupported") {
        failures.push(`${name} の、持たない op ${op} の診断が ${code}`);
      }
    }
  }
  session.close();
}

// 包みの判定と、コアの open
const cases = loadCases();
const full = (await load(readFileSync(fullPath))).open();
let inputs = 0;
for (const { name, steps } of cases) {
  for (const step of steps.filter((s) => s.push)) {
    const [, data] = pushOf(step.push);
    const input = `${name}-${inputs++}`;
    full.push(input, data);
    let format;
    try {
      format = full.call({ v: 1, op: "open", input, doc: input }).result.format;
    } catch (error) {
      if (!(error instanceof ZumenError) || error.response?.diagnostics?.[0]?.code !== "call.format.unsupported") {
        continue;
      }
    }
    // 図面モデルのバイナリ（決定 0246）の open は、元の文書の形式を返す。包みの判定は、まだバイナリを読まない（#483）
    if (String.fromCharCode(...data.subarray(0, 4)) === "ZMDR") {
      continue;
    }
    const found = detect(data);
    if (found?.format !== format) {
      failures.push(`${name} の入力 ${step.push.name ?? ""}: detect が ${found?.format}（open は ${format}）`);
    }
    if (format !== undefined && found !== undefined && (await modules.forInput(data)) !== loaded[found.module]) {
      failures.push(`${name} の入力 ${step.push.name ?? ""}: forInput が ${found.module} を返さない`);
    }
  }
}
full.close();

// 組を、組に合うモジュールで流す
const chosen = [];
for (const testCase of cases) {
  const name = moduleOf(testCase, MODULES);
  if (name === "all") {
    continue;
  }
  if (loaded[name] === undefined) {
    failures.push(`${testCase.name}: 組の op をすべて持つモジュールがない。module を書く`);
    continue;
  }
  chosen.push({ ...testCase, module: name });
}
const { hashes, scenes, failures: found } = await runConformance({
  zumen: loaded.drawing,
  cases: chosen,
  readSample,
  sha256: (data) => createHash("sha256").update(data).digest("hex"),
  pick: (testCase) => loaded[testCase.module],
});
failures.push(...found);
compareHashes(hashes, failures, chosen.map((testCase) => testCase.name));

// モジュールごとの大きさ（決定 0013 の「CI で測って記録する」）
const sizes = [["すべて", fullPath], ...names.map((name, index) => [name, paths[index]])]
  .map(([label, path]) => {
    const bytes = readFileSync(path);
    return `| ${label} | ${bytes.length} | ${brotliCompressSync(bytes).length} |`;
  })
  .join("\n");
const table = `| モジュール | バイト | brotli で |\n| --- | --- | --- |\n${sizes}`;
console.log(table);
const counts = names.map((name) => `${name} ${chosen.filter((testCase) => testCase.module === name).length} 件`).join("・");
const summary = `分けたモジュールで流した組 ${chosen.length} 件（${counts}）、ハッシュ ${hashes.length} 件、読んだシーン ${scenes} 件、判定した入力 ${inputs} 件`;
console.log(summary);
if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### 分けた wasm のモジュールの適合試験\n\n${summary}\n\n${table}\n`);
}
if (failures.length > 0) {
  console.error(`期待と違った:\n${failures.join("\n")}`);
  process.exit(1);
}
console.log("ネイティブと同じ一覧になった");
