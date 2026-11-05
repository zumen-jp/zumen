// npm pack したパッケージを Vite で組み、配るファイルだけで wasm を読み込む（決定 0268）。
// 使い方: node bindings/js/test/browser/bundle.mjs <all.wasm> <drawing.wasm> <bim.wasm> <civil.wasm> <pointcloud.wasm>

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { appendFileSync, copyFileSync, cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, extname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { chromium } from "playwright";
import { build } from "vite";

import { MODULES } from "../../src/constants.js";

const names = ["all", "drawing", "bim", "civil", "pointcloud"];
const wasmPaths = process.argv.slice(2);
if (wasmPaths.length !== names.length) {
  console.error("使い方: node bindings/js/test/browser/bundle.mjs <all.wasm> <drawing.wasm> <bim.wasm> <civil.wasm> <pointcloud.wasm>");
  process.exit(2);
}
const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../../..");
const filename = (name) => name === "all" ? "zumen_abi1.wasm" : `zumen_abi1_${name}.wasm`;
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const expectedHashes = Object.fromEntries(names.map((name, index) => [name, hash(readFileSync(wasmPaths[index]))]));
const origin = "http://localhost:24191";
// ルート以外に置いたアプリでも、Vite が書き換えた URL から読む。
const base = "/viewer/";
const types = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".wasm": "application/wasm" };

/** npm に渡す引数は、このテストの固定の語だけ。Windows の npm.cmd はシェルを通す */
function npm(args, cwd) {
  const result = spawnSync("npm", args, { cwd, encoding: "utf8", shell: process.platform === "win32" });
  assert.equal(result.status, 0, result.error?.message ?? result.stderr);
  return result.stdout;
}

/** wasm の同梱を試すためのパッケージを作る。依存のライセンスの生成は release.yml の licenses job が担う */
function pack(work) {
  const staged = join(work, "package");
  cpSync(join(root, "bindings/js/src"), join(staged, "src"), { recursive: true });
  for (const name of ["package.json", "README.md"]) {
    copyFileSync(join(root, "bindings/js", name), join(staged, name));
  }
  for (const name of ["LICENSE-MIT", "LICENSE-APACHE"]) {
    copyFileSync(join(root, name), join(staged, name));
  }
  for (let index = 0; index < names.length; index++) {
    copyFileSync(wasmPaths[index], join(staged, filename(names[index])));
  }
  // npm 10・11 は配列、npm 12 はパッケージ名をキーにした表を返す。
  const [packed] = Object.values(JSON.parse(npm(["pack", "--json", "--ignore-scripts"], staged)));
  assert.deepEqual(packed.files.filter(({ path }) => path.endsWith(".wasm")).map(({ path }) => path).sort(), names.map(filename).sort());
  return { path: join(staged, packed.filename), size: packed.size, unpackedSize: packed.unpackedSize };
}

const work = mkdtempSync(join(tmpdir(), "zumen-js-bundle-"));
let browser;
try {
  const packed = pack(work);
  const app = join(work, "app");
  mkdirSync(app);
  writeFileSync(join(app, "package.json"), JSON.stringify({ private: true, type: "module" }));
  copyFileSync(packed.path, join(app, "package.tgz"));
  npm(["install", "./package.tgz", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--package-lock=false"], app);

  // 配ったパッケージの exports で探せ、同じバイト列が取り出せる。
  const require = createRequire(join(app, "package.json"));
  const { wasmUrls } = await import(pathToFileURL(require.resolve("@zumen-jp/zumen/wasm")).href);
  assert.deepEqual(Object.keys(wasmUrls).sort(), [...names].sort());
  for (const name of names) {
    const exported = require.resolve(`@zumen-jp/zumen/${filename(name)}`);
    assert.equal(hash(readFileSync(exported)), expectedHashes[name], `${name}: exports の中身`);
    assert.equal(hash(readFileSync(new URL(wasmUrls[name]))), expectedHashes[name], `${name}: URL の中身`);
  }

  cpSync(join(here, "vite"), app, { recursive: true });
  const dist = join(app, "dist");
  await build({ configFile: false, root: app, base, logLevel: "warn", build: { outDir: dist, assetsInlineLimit: 0 } });

  browser = await chromium.launch();
  const context = await browser.newContext();
  const requested = [];
  const failures = [];
  // Vite の出力だけを返す。元のパッケージやリポジトリのファイルへは戻らない。
  await context.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin || !url.pathname.startsWith(base)) {
      return route.fulfill({ status: 404, body: "" });
    }
    const path = resolve(dist, decodeURIComponent(url.pathname.slice(base.length)) || "index.html");
    const within = relative(dist, path);
    if (isAbsolute(within) || within === ".." || within.startsWith(`..${sep}`)) {
      return route.fulfill({ status: 404, body: "" });
    }
    try {
      const body = readFileSync(path);
      if (extname(path) === ".wasm") {
        const name = names.find((name) => expectedHashes[name] === hash(body));
        assert.ok(name, "配る wasm のバイト列がビルドで変わらない");
        requested.push(name);
      }
      return route.fulfill({ contentType: types[extname(path)] ?? "application/octet-stream", body });
    } catch (error) {
      failures.push(error.message);
      return route.fulfill({ status: 404, body: "" });
    }
  });
  const page = await context.newPage();
  page.on("pageerror", (error) => failures.push(error.message));
  await page.goto(`${origin}${base}`);
  await page.waitForFunction(() => window.bundled !== undefined);
  assert.deepEqual(requested, [], "ラッパーと URL の import だけでは wasm を取得しない");
  const urls = await page.evaluate(() => window.bundled.urls);
  for (const name of names) {
    assert.ok(urls[name].startsWith(`${origin}${base}assets/`), `${name}: Vite の出力を指す`);
  }

  // 入力の判定から図面を開いても、図面のモジュールだけを取る。
  assert.equal(await page.evaluate(() => window.bundled.openDrawing()), "dxf");
  assert.deepEqual(requested, ["drawing"]);
  for (const name of names.slice(1)) {
    const ops = await page.evaluate((name) => window.bundled.ops(name), name);
    assert.deepEqual([...ops].sort(), [...MODULES[name].ops].sort(), `${name}: 分けたモジュールの op`);
    assert.deepEqual(requested, names.slice(1, names.indexOf(name) + 1), `${name}: 必要なモジュールだけを取得する`);
    await page.evaluate((name) => window.bundled.ops(name), name);
    assert.deepEqual(requested, names.slice(1, names.indexOf(name) + 1), `${name}: 2 回目は取得し直さない`);
  }
  const allOps = await page.evaluate(() => window.bundled.ops("all"));
  for (const name of names.slice(1)) {
    assert.ok(MODULES[name].ops.every((op) => allOps.includes(op)), `全体版でも ${name} の op が使える`);
  }
  assert.deepEqual(requested, [...names.slice(1), "all"]);
  assert.deepEqual(failures, []);
  const summary = `Vite + Chromium ${browser.version()}: npm pack の wasm 5 件・exports・内容・URL・遅延取得・再利用を確認。tgz ${packed.size} bytes、展開後 ${packed.unpackedSize} bytes`;
  console.log(summary);
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### 分けた wasm の配布\n\n${summary}\n`);
  }
} finally {
  await browser?.close();
  // この実行で作った一時フォルダーだけを消す。
  assert.equal(dirname(resolve(work)), resolve(tmpdir()));
  assert.ok(relative(tmpdir(), work).startsWith("zumen-js-bundle-"));
  rmSync(work, { recursive: true, force: true });
}
