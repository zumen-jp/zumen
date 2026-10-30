// 適合試験を、TypeScript・JavaScript の包み（bindings/js）を通して、ブラウザーの中で流す（決定 0194・0199）。
//
// 使い方: node bindings/js/test/browser/run.mjs <chromium|firefox|webkit> <zumen_abi1.wasm>
//
// Playwright でブラウザーを動かし、ページの中で包みを読み込んで、Node.js と同じ本体（../run.mjs）で組を流す。
// wasm は fetch の応答から load で読み込み、loadSync も試す。ハッシュの一覧は、ページの外で hashes.txt と比べる。
// ページとファイルは、Playwright が要求を横取りして返す。待ち受けるサーバーは立てない。
// ブラウザーは `npx playwright install <ブラウザー>` で取っておく。

import { appendFileSync, readFileSync } from "node:fs";
import { extname, join, normalize, sep } from "node:path";

import { chromium, firefox, webkit } from "playwright";

import { compareHashes, loadCases, repository, samplePath } from "../../../conformance/cases.mjs";

const BROWSERS = { chromium, firefox, webkit };
// 安全なコンテキスト（crypto.subtle が使える）になる localhost にする。番号は開発用の範囲（AGENTS.md）
const ORIGIN = "http://localhost:24190";
const WASM_PATH = "/zumen_abi1.wasm";
// ページから読めるのは、包み・組の本体・公開サンプルだけ
const SERVED = ["bindings/js/src/", "bindings/js/test/run.mjs", "bindings/conformance/steps.mjs", "samples/"];
const TYPES = { ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm" };

const [name, wasmPath] = process.argv.slice(2);
if (!Object.hasOwn(BROWSERS, name ?? "") || !wasmPath) {
  console.error("使い方: node bindings/js/test/browser/run.mjs <chromium|firefox|webkit> <zumen_abi1.wasm>");
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
  const relative = normalize(path.slice(1)).split(sep).join("/");
  if (!SERVED.some((prefix) => relative === prefix || (prefix.endsWith("/") && relative.startsWith(prefix)))) {
    return route.fulfill({ status: 404, body: "" });
  }
  return route.fulfill({
    contentType: TYPES[extname(relative)] ?? "application/octet-stream",
    body: readFileSync(join(repository, relative)),
  });
}

const browser = await BROWSERS[name].launch();
const version = browser.version();
const failures = [];
let result;
try {
  const context = await browser.newContext();
  await context.route(`${ORIGIN}/**`, serve);
  const page = await context.newPage();
  page.on("pageerror", (error) => failures.push(`ページのエラー: ${error.message}`));
  await page.goto(`${ORIGIN}/`);
  result = await page.evaluate(
    async ({ cases, samples, wasmPath }) => {
      const { load, loadSync } = await import("/bindings/js/src/index.js");
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
      const run = await runConformance({
        zumen,
        cases,
        readSample: (file) => {
          if (!loaded.has(file)) {
            throw new Error(`取っていない公開サンプル: ${file}`);
          }
          return loaded.get(file);
        },
        sha256: async (data) =>
          Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", data)), (byte) =>
            byte.toString(16).padStart(2, "0"),
          ).join(""),
      });
      // 主スレッドでは大きなモジュールを同期でコンパイルできないので、非同期でコンパイルしたものを loadSync に渡す
      const module = await WebAssembly.compile(await fetchBytes(wasmPath));
      if (loadSync(module).abiVersion() !== zumen.abiVersion()) {
        run.failures.push("loadSync と load の ABI の大版が違う");
      }
      return { ...run, userAgent: navigator.userAgent };
    },
    { cases, samples, wasmPath: WASM_PATH },
  );
} finally {
  await browser.close();
}

failures.push(...result.failures);
compareHashes(result.hashes, failures);

const summary = `${name} ${version}: 包みを通した試験の組 ${cases.length} 件、ハッシュ ${result.hashes.length} 件、読んだシーン ${result.scenes} 件`;
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
