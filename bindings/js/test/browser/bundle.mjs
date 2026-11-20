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
const origin = `http://localhost:${process.env.ZUMEN_TEST_PORT ?? 24128}`;
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

/** 表示部品と React フックの配布物を作り、ライセンス同梱と余分なファイル混入の不在を確かめる（#909） */
function packUiPackage(work, slug, dir) {
  const staged = join(work, `pkg-${slug}`);
  cpSync(join(root, dir, "src"), join(staged, "src"), { recursive: true });
  for (const name of ["package.json", "README.md"]) {
    copyFileSync(join(root, dir, name), join(staged, name));
  }
  for (const name of ["LICENSE-MIT", "LICENSE-APACHE"]) {
    copyFileSync(join(root, name), join(staged, name));
  }
  const [packed] = Object.values(JSON.parse(npm(["pack", "--json", "--ignore-scripts"], staged)));
  const paths = packed.files.map(({ path }) => path);
  assert.ok(paths.includes("LICENSE-MIT") && paths.includes("LICENSE-APACHE"), `${slug}: ライセンス本文を含む`);
  assert.ok(!paths.some((p) => p.startsWith("test/") || p.startsWith("example/")), `${slug}: テストや例を含まない`);
  return { path: join(staged, packed.filename), size: packed.size };
}

/** 通信なしで peerDependencies の解決と Vite バンドルを確かめるための最小の React 19.3.1 パッケージ */
function packPeerReact(work) {
  const staged = join(work, "peer-react");
  mkdirSync(staged);
  writeFileSync(join(staged, "package.json"), JSON.stringify({
    name: "react",
    version: "19.3.1",
    type: "module",
    exports: { ".": "./index.js" },
  }));
  writeFileSync(join(staged, "index.js"), [
    "export function useCallback(fn) { return fn; }",
    "export function useEffect() {}",
    "export function useLayoutEffect() {}",
    "export function useMemo(fn) { return fn(); }",
    "export function useRef(v) { return { current: v }; }",
    "export function useState(v) { return [v, () => {}]; }",
    "export function useSyncExternalStore(_s, get) { return get(); }",
  ].join("\n"));
  const [packed] = Object.values(JSON.parse(npm(["pack", "--json", "--ignore-scripts"], staged)));
  return join(staged, packed.filename);
}

/** npm pack した three 0.187.0 を通信なしで入れるための tarball を作る */
function packPeerThree(work) {
  const [packed] = Object.values(JSON.parse(npm(["pack", "--json", "--ignore-scripts", "--pack-destination", work], join(here, "node_modules/three"))));
  return join(work, packed.filename);
}

/** 2D のみを使う利用側は three と @zumen-jp/view3d なしで npm install と Vite ビルドが通ることを確かめる */
async function verifyUiBundles(work, coreTgz) {
  const view2d = packUiPackage(work, "view2d", "packages/view2d");
  const view3d = packUiPackage(work, "view3d", "packages/view3d");
  const reactPkg = packUiPackage(work, "react", "packages/react");
  const reactPeer = packPeerReact(work);
  const threePeer = packPeerThree(work);

  const app2d = join(work, "app-2d");
  mkdirSync(app2d);
  writeFileSync(join(app2d, "package.json"), JSON.stringify({ private: true, type: "module" }));
  copyFileSync(coreTgz, join(app2d, "zumen.tgz"));
  copyFileSync(view2d.path, join(app2d, "view2d.tgz"));
  copyFileSync(reactPkg.path, join(app2d, "react-pkg.tgz"));
  copyFileSync(reactPeer, join(app2d, "react-peer.tgz"));
  npm(["install", "./zumen.tgz", "./view2d.tgz", "./react-pkg.tgz", "./react-peer.tgz",
    "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--package-lock=false"], app2d);
  const req2d = createRequire(join(app2d, "package.json"));
  assert.throws(() => req2d.resolve("three"), "2D のみの環境に three は入らない");
  assert.throws(() => req2d.resolve("@zumen-jp/view3d"), "2D のみの環境に view3d は入らない");
  writeFileSync(join(app2d, "index.html"), '<!doctype html><script type="module" src="./entry.js"></script>');
  writeFileSync(join(app2d, "entry.js"), [
    'import { View2D, ReviewStore, ReviewLayer } from "@zumen-jp/view2d";',
    'import { useDrawing, useDrawingViewer, useReviewLayer, useViewerState } from "@zumen-jp/react";',
    "window.ui2d = { View2D, ReviewStore, ReviewLayer, useDrawing, useDrawingViewer, useReviewLayer, useViewerState };",
  ].join("\n"));
  await build({
    configFile: false,
    root: app2d,
    logLevel: "warn",
    build: { outDir: join(app2d, "dist"), assetsInlineLimit: 0 },
    worker: { format: "es" },
  });

  const app3d = join(work, "app-3d");
  mkdirSync(app3d);
  writeFileSync(join(app3d, "package.json"), JSON.stringify({ private: true, type: "module" }));
  copyFileSync(coreTgz, join(app3d, "zumen.tgz"));
  copyFileSync(view2d.path, join(app3d, "view2d.tgz"));
  copyFileSync(view3d.path, join(app3d, "view3d.tgz"));
  copyFileSync(reactPkg.path, join(app3d, "react-pkg.tgz"));
  copyFileSync(reactPeer, join(app3d, "react-peer.tgz"));
  copyFileSync(threePeer, join(app3d, "three-peer.tgz"));
  npm(["install", "./zumen.tgz", "./view2d.tgz", "./view3d.tgz", "./react-pkg.tgz", "./react-peer.tgz", "./three-peer.tgz",
    "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--package-lock=false"], app3d);
  writeFileSync(join(app3d, "index.html"), '<!doctype html><script type="module" src="./entry.js"></script>');
  writeFileSync(join(app3d, "entry.js"), [
    'import { View2D } from "@zumen-jp/view2d";',
    'import { View3D, THREE_REVISION } from "@zumen-jp/view3d";',
    'import { attachGround2D } from "@zumen-jp/view3d/ground2d";',
    'import { attachPointCloud } from "@zumen-jp/view3d/pointcloud";',
    'import { useDrawingViewer } from "@zumen-jp/react";',
    'import { useScene3D, useView3D } from "@zumen-jp/react/view3d";',
    "window.ui3d = { View2D, View3D, THREE_REVISION, attachGround2D, attachPointCloud, useDrawingViewer, useScene3D, useView3D };",
  ].join("\n"));
  await build({
    configFile: false,
    root: app3d,
    logLevel: "warn",
    build: { outDir: join(app3d, "dist"), assetsInlineLimit: 0 },
    worker: { format: "es" },
  });
  return { view2dSize: view2d.size, view3dSize: view3d.size, reactSize: reactPkg.size };
}

const work = mkdtempSync(join(tmpdir(), "zumen-js-bundle-"));
let browser;
try {
  const packed = pack(work);
  const uiPacked = await verifyUiBundles(work, packed.path);
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
  const summary = `Vite + Chromium ${browser.version()}: npm pack の 4 パッケージ（zumen・view2d・view3d・react）と wasm 5 件・exports・内容・URL・遅延取得・2D 単独（three 不要）と 3D バンドルを確認。zumen tgz ${packed.size} bytes（展開後 ${packed.unpackedSize} bytes）、view2d ${uiPacked.view2dSize} bytes、view3d ${uiPacked.view3dSize} bytes、react ${uiPacked.reactSize} bytes`;
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
