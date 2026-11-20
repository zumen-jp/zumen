// 決定 0199 の固定した Playwright と、lockfile で固定した three.js で 3D の表示部品を検査する。外へ通信せず、合成入力だけを読む。
import { appendFileSync, readFileSync } from "node:fs";
import { dirname, extname, join, normalize, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "playwright";
import { chromiumLaunchOptions } from "./launch.mjs";

const browsers = { chromium, firefox, webkit };
const [name, wasmPath] = process.argv.slice(2);
if (!browsers[name] || !wasmPath) throw new Error("使い方: node bindings/js/test/browser/view3d.mjs <chromium|firefox|webkit> <zumen_abi1.wasm>");
const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "../../../..");
const origin = `http://localhost:${process.env.ZUMEN_TEST_PORT ?? 24121}`;
const three = JSON.parse(readFileSync(join(here, "node_modules/three/package.json"), "utf8")).version;
const ifc = JSON.parse(readFileSync(join(root, "bindings/conformance/cases/scene3d.json"), "utf8")).steps.find((s) => s.push).push;
const pushed = (name) => JSON.parse(readFileSync(join(root, `bindings/conformance/cases/${name}.json`), "utf8")).steps.find((s) => s.push).push;
const layers = { las: pushed("scene3d_pointcloud"), gml: pushed("geo_scene3d") };
const importmap = JSON.stringify({
  imports: {
    "@zumen-jp/zumen": "/bindings/js/src/index.js",
    three: "/three/build/three.module.js",
    "three/webgpu": "/three/build/three.webgpu.js",
    "three/tsl": "/three/build/three.tsl.js",
  },
});
const launchOptions = name === "chromium" ? chromiumLaunchOptions(chromium) : {};
const browser = await browsers[name].launch(launchOptions);
try {
  const page = await browser.newPage();
  const failures = [];
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin) { failures.push(`外への要求: ${url.origin}`); return route.abort(); }
    const path = normalize(decodeURIComponent(url.pathname).slice(1)).split(sep).join("/");
    if (url.pathname === "/") return route.fulfill({ contentType: "text/html", body: `<!doctype html><meta charset="utf-8"><title>view3d test</title><script type="importmap">${importmap}</script>` });
    if (path === "zumen_abi1.wasm") return route.fulfill({ contentType: "application/wasm", body: readFileSync(wasmPath) });
    let file = null;
    if (path.startsWith("three/build/")) file = join(here, "node_modules", path);
    else if (["bindings/js/src/", "packages/view3d/src/", "packages/view3d/test/", "packages/view2d/src/", "packages/view2d/test/fixtures.mjs"].some((prefix) => path.startsWith(prefix))) file = join(root, path);
    if (file === null) return route.fulfill({ status: 404, body: "" });
    const contentType = [".js", ".mjs"].includes(extname(path)) ? "text/javascript" : "application/octet-stream";
    return route.fulfill({ contentType, body: readFileSync(file) });
  });
  page.on("pageerror", (error) => failures.push(error.message));
  if (process.env.ZUMEN_VIEW3D_TRACE) page.on("console", (message) => console.log(message.text()));
  await page.goto(origin);
  const result = await page.evaluate(async ({ ifc, layers, requireWebGPU }) => {
    const adapter = (await globalThis.navigator.gpu?.requestAdapter())
      ?? (await globalThis.navigator.gpu?.requestAdapter({ fallbackAdapter: true }));
    if (!adapter && !requireWebGPU) {
      const { View3D, View3DError } = await import("/packages/view3d/src/index.js");
      let caught = null;
      try { await View3D.create(globalThis.document.createElement("canvas"), { width: 50, height: 50 }); } catch (error) { caught = error; }
      if (!(caught instanceof View3DError && caught.reason === "webgpu-unavailable")) {
        throw new Error(`WebGPU がない環境での例外: ${caught?.reason ?? caught}`);
      }
      return { skippedNoWebGPU: true, assertions: 1 };
    }
    const { testView3D } = await import("/packages/view3d/test/browser.mjs");
    return await testView3D("/zumen_abi1.wasm", ifc, layers);
  }, { ifc, layers, requireWebGPU: name === "chromium" });
  if (failures.length) throw new Error(failures.join("\n"));
  if (result.skippedNoWebGPU) {
    const line = `${name} ${browser.version()}・three.js ${three}: WebGPU 非対応環境のため webgpu-unavailable の拒否（1 件）を確認`;
    console.log(line);
    if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `\n${line}\n`);
  } else {
    const lines = [
      `${name} ${browser.version()}・three.js ${three}: view3d の検査 ${result.assertions} 件、デバイス喪失は ${result.contextLoss === "checked" ? "確認" : "未確認"}`,
      `${name}: 合成シーンの CPU の時間（ms）: ${JSON.stringify(result.timings)}。描画の完了は snapshot までの時間で、GPU の時間の計測ではない`,
      `${name}: 確定したバイト数: ${JSON.stringify(result.bytes)}。メモリの観測（JS ヒープ、取れる環境だけ）: ${result.memory ?? "なし"}`,
      `${name}: コアの ZMS3 との列の一致と描画: ${JSON.stringify(result.core)}`,
      `${name}: 合成の点群（100 万点・64 塊）の CPU の時間（ms）: ${JSON.stringify(result.pointcloud)}。実データの大きな点群は未確認`,
    ];
    for (const line of lines) console.log(line);
    if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `\n${lines.join("\n\n")}\n`);
  }
} finally { await browser.close(); }
