// 適合試験を、TypeScript・JavaScript の包み（bindings/js）を通して、ブラウザーの中で流す（決定 0194・0199）。
//
// 使い方: node bindings/js/test/browser/run.mjs <chromium|firefox|webkit> <zumen_abi1.wasm> [<drawing.wasm> <bim.wasm> <plugin.wasm>]
//
// 分けた wasm のモジュール（決定 0221）を渡せば、包みの loadModules で読み込み、組ごとに組に合うモジュールでも流す
// （すべてのモジュールを入れたコアでだけ流す組を除く）。
// Playwright でブラウザーを動かし、ページの中で包みを読み込んで、Node.js と同じ本体（../run.mjs）で組を流す。
// wasm は fetch の応答から load で読み込み、loadSync も試す。ハッシュの一覧は、ページの外で hashes.txt と比べる。
// ページとファイルは、Playwright が要求を横取りして返す。待ち受けるサーバーは立てない。
// ブラウザーは `npx playwright install <ブラウザー>` で取っておく。

import { appendFileSync, readFileSync } from "node:fs";
import { extname, join, normalize, sep } from "node:path";

import { chromium, firefox, webkit } from "playwright";

import { compareHashes, loadCases, moduleOf, repository, samplePath } from "../../../conformance/cases.mjs";
import { MODULES } from "../../src/constants.js";

const BROWSERS = { chromium, firefox, webkit };
// 安全なコンテキスト（crypto.subtle が使える）になる localhost にする。番号は開発用の範囲（AGENTS.md）
const ORIGIN = "http://localhost:24190";
const WASM_PATH = "/zumen_abi1.wasm";
// ページから読めるのは、包み・組の本体・公開サンプルだけ
const SERVED = ["bindings/js/src/", "bindings/js/test/run.mjs", "bindings/conformance/steps.mjs", "samples/"];
const TYPES = { ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm" };

const [name, wasmPath, ...modulePaths] = process.argv.slice(2);
const moduleNames = Object.keys(MODULES);
if (!Object.hasOwn(BROWSERS, name ?? "") || !wasmPath || (modulePaths.length !== 0 && modulePaths.length !== moduleNames.length)) {
  console.error("使い方: node bindings/js/test/browser/run.mjs <chromium|firefox|webkit> <zumen_abi1.wasm> [<drawing.wasm> <bim.wasm> <plugin.wasm>]");
  process.exit(2);
}

const cases = loadCases();
const samples = [
  ...new Set(cases.flatMap(({ steps }) => steps.filter((step) => step.push?.file).map((step) => samplePath(step.push.file)))),
];

/** ページの要求に、リポジトリのファイルを返す */
async function serve(route) {
  const path = decodeURIComponent(new URL(route.request().url()).pathname);
  if (path === "/") {
    return route.fulfill({ contentType: "text/html; charset=utf-8", body: '<!doctype html><meta charset="utf-8"><title>zumen</title>' });
  }
  if (path === WASM_PATH) {
    return route.fulfill({ contentType: TYPES[".wasm"], body: readFileSync(wasmPath) });
  }
  const module = moduleNames.findIndex((module) => path === `/modules/${module}.wasm`);
  if (module !== -1 && modulePaths.length > 0) {
    return route.fulfill({ contentType: TYPES[".wasm"], body: readFileSync(modulePaths[module]) });
  }
  const relative = normalize(path.slice(1)).split(sep).join("/");
  if (!SERVED.some((prefix) => relative === prefix || (prefix.endsWith("/") && relative.startsWith(prefix)))) {
    return route.fulfill({ status: 404, body: "" });
  }
  return route.fulfill({
    contentType: TYPES[extname(relative)] ?? "application/octet-stream",
    body: readFileSync(join(repository, relative)),
  });
}

// 分けたモジュールで流す組と、そのモジュール
const split =
  modulePaths.length === 0
    ? []
    : cases
        .map((testCase, index) => ({ index, module: moduleOf(testCase, MODULES) }))
        .filter(({ module }) => module !== "all");
const failures = [];
for (const { index, module } of split) {
  if (!moduleNames.includes(module)) {
    failures.push(`${cases[index].name}: 組の op をすべて持つモジュールがない。module を書く`);
  }
}

const browser = await BROWSERS[name].launch();
const version = browser.version();
let result;
try {
  const context = await browser.newContext();
  await context.route(`${ORIGIN}/**`, serve);
  const page = await context.newPage();
  page.on("pageerror", (error) => failures.push(`ページのエラー: ${error.message}`));
  await page.goto(`${ORIGIN}/`);
  result = await page.evaluate(
    async ({ cases, samples, wasmPath, split }) => {
      const { load, loadModules, loadSync } = await import("/bindings/js/src/index.js");
      const { runConformance } = await import("/bindings/js/test/run.mjs");
      const fetchBytes = async (path) => {
        const response = await fetch(path);
        if (!response.ok) {
          throw new Error(`${path} を取得できない（${response.status}）`);
        }
        return new Uint8Array(await response.arrayBuffer());
      };
      const loaded = new Map();
      for (const file of samples) {
        loaded.set(file, await fetchBytes(`/${file}`));
      }
      const zumen = await load(fetch(wasmPath));
      const readSample = (file) => {
        if (!loaded.has(file)) {
          throw new Error(`取っていない公開サンプル: ${file}`);
        }
        return loaded.get(file);
      };
      const sha256 = async (data) =>
        Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", data)), (byte) =>
          byte.toString(16).padStart(2, "0"),
        ).join("");
      const run = await runConformance({ zumen, cases, readSample, sha256 });
      // 分けたモジュールで、組に合うものを fetch で読み込んで流す
      if (split.length > 0) {
        const modules = loadModules(Object.fromEntries(split.map(({ module }) => [module, () => fetch(`/modules/${module}.wasm`)])));
        const picked = {};
        for (const { module } of split) {
          picked[module] = await modules.get(module);
        }
        run.split = await runConformance({
          zumen: picked.drawing,
          cases: split.map(({ index, module }) => ({ ...cases[index], module })),
          readSample,
          sha256,
          pick: (testCase) => picked[testCase.module],
        });
      }
      // 主スレッドでは大きなモジュールを同期でコンパイルできないので、非同期でコンパイルしたものを loadSync に渡す
      const module = await WebAssembly.compile(await fetchBytes(wasmPath));
      if (loadSync(module).abiVersion() !== zumen.abiVersion()) {
        run.failures.push("loadSync と load の ABI の大版が違う");
      }
      return { ...run, userAgent: navigator.userAgent };
    },
    { cases, samples, wasmPath: WASM_PATH, split },
  );
} finally {
  await browser.close();
}

failures.push(...result.failures);
compareHashes(result.hashes, failures);
if (result.split) {
  failures.push(...result.split.failures.map((failure) => `分けたモジュール: ${failure}`));
  compareHashes(result.split.hashes, failures, split.map(({ index }) => cases[index].name));
}

const splitSummary = result.split ? `。分けたモジュールで流した組 ${split.length} 件、ハッシュ ${result.split.hashes.length} 件` : "";
const summary = `${name} ${version}: 包みを通した試験の組 ${cases.length} 件、ハッシュ ${result.hashes.length} 件、読んだシーン ${result.scenes} 件${splitSummary}`;
console.log(summary);
console.log(result.userAgent);
if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### ブラウザーでの TypeScript・JavaScript の包みの適合試験\n\n${summary}\n`);
}
if (failures.length > 0) {
  console.error(`期待と違った:\n${failures.join("\n")}`);
  process.exit(1);
}
console.log("ネイティブと同じ一覧になった");
