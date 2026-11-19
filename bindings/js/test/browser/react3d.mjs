// #818。3D のプレイグラウンドと React のフックを、ローカルの配布物だけを配るサーバーで確かめる。
import assert from "node:assert/strict";
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { basename, dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "playwright";
import { build } from "vite";

const [product, wasm] = process.argv.slice(2);
const browsers = { chromium, firefox, webkit, chrome: chromium, msedge: chromium };
if (!browsers[product] || !wasm) throw new Error("使い方: node react3d.mjs <chromium|firefox|webkit|chrome|msedge> <zumen_abi1.wasm（全部入り）>");
const root = fileURLToPath(new URL("../../../../", import.meta.url));
const app = join(root, "playground");
process.env.ZUMEN_SCENE3D_WASM = process.env.ZUMEN_DRAWING_WASM = resolve(wasm);
const target = resolve(root, "target");
mkdirSync(target, { recursive: true });
const work = mkdtempSync(join(target, "react3d-playground-"));
const assets = new Map(), failures = [], seen = new Set();
function collect(folder, prefix) {
  for (const entry of readdirSync(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name), url = prefix + "/" + entry.name;
    if (entry.isDirectory()) collect(path, url);
    else assets.set(url, path);
  }
}
function fixture(name, file) {
  const steps = JSON.parse(readFileSync(join(root, "bindings/conformance/cases", name + ".json"))).steps;
  const { push } = steps.find((step) => step.push && (!file || step.push.name === file));
  return push.file ? readFileSync(join(root, push.file)) : Buffer.from(push.hex ?? push.text, push.hex ? "hex" : "utf8");
}
// 合成の IFC（地図の変換を持つ 7 要素）・CityGML の面・LAS の 12 点・DXF の図面
const inputs = { ifc: fixture("scene3d", "scene.ifc"), gml: fixture("geo_scene3d"), las: fixture("scene3d_pointcloud"), dxf: fixture("scene") };
const port = Number(process.env.ZUMEN_TEST_PORT ?? 24125 + Object.keys(browsers).indexOf(product) % 3);
const origin = "http://127.0.0.1:" + port;
const mime = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm" };
let browser, server;
try {
  const options = { root: app, configFile: join(app, "vite.config.js"), logLevel: "error" };
  await build({ ...options, build: { outDir: join(work, "app") } });
  assert.ok(readFileSync(join(work, "app/licenses.md"), "utf8").includes("three"), "three のライセンスを出力する");
  collect(join(work, "app"), "/app");
  for (const [folder, mode] of [["production", "production"], ["strict", "development"]]) {
    await build({ ...options, define: { "process.env.NODE_ENV": JSON.stringify(mode) }, build: {
      outDir: join(work, folder), rolldownOptions: { input: join(app, "test/lifecycle3d.html") },
    } });
    collect(join(work, folder), "/" + folder);
  }
  server = createServer((request, response) => {
    const path = assets.get(request.url);
    if (request.method !== "GET" || request.headers["content-length"] || request.headers["transfer-encoding"] || !path) {
      failures.push("配布物以外の要求または送信"); response.writeHead(404).end(); return;
    }
    seen.add(request.url);
    response.writeHead(200, { "Content-Type": mime[extname(path)] ?? "application/octet-stream" }).end(readFileSync(path));
  });
  await new Promise((ok, fail) => server.once("error", fail).listen(port, "127.0.0.1", ok));
  const isChromium = ["chromium", "chrome", "msedge"].includes(product);
  const launchOptions = isChromium
    ? {
        ...(product === "chromium" ? {} : { channel: product }),
        ignoreDefaultArgs: ["--disable-gpu"],
        args: [
          "--enable-gpu",
          "--enable-unsafe-webgpu",
          "--ignore-gpu-blocklist",
          ...(process.platform === "win32" ? ["--use-webgpu-adapter=d3d11"] : []),
          ...(process.platform === "linux" ? ["--use-angle=vulkan", "--enable-features=Vulkan", "--enable-unsafe-swiftshader"] : []),
        ],
      }
    : {};
  browser = await browsers[product].launch(launchOptions);
  const context = await browser.newContext({ serviceWorkers: "block", acceptDownloads: true });
  await context.route("**/*", (route) => {
    const request = route.request(), url = new URL(request.url());
    if (url.origin !== origin || !assets.has(url.pathname) || url.search || request.method() !== "GET" || request.postDataBuffer() !== null) {
      failures.push("許可していない通信: " + request.method() + " " + url.pathname); return route.abort();
    }
    return route.continue();
  });
  // WebGPU が使えない環境でも未処理エラーを起こさず、表示できない理由を画面へ出すことを確かめる。
  const noGpuPage = await context.newPage();
  noGpuPage.on("pageerror", (error) => failures.push(error.message));
  noGpuPage.on("websocket", () => failures.push("WebSocket"));
  await noGpuPage.addInitScript(() => {
    Object.defineProperty(navigator, "gpu", { value: undefined, configurable: true });
  });
  await noGpuPage.goto(origin + "/app/view3d.html");
  await noGpuPage.locator(".status").getByText("表示できません: このブラウザーでは WebGPU を使えません", { exact: true }).waitFor();
  await noGpuPage.close();

  const page = await context.newPage();
  await page.addInitScript(() => {
    window.blockedRequests = [];
    document.addEventListener("securitypolicyviolation", (event) => {
      if (/^https?:|^wss?:/.test(event.blockedURI)) window.blockedRequests.push(event.violatedDirective);
    });
  });
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("websocket", () => failures.push("WebSocket"));
  const downloads = [];
  page.on("download", (download) => downloads.push(download));
  await page.goto(origin + "/app/view3d.html");
  const hasWebGPU = await page.evaluate(async () => Boolean(
    (await globalThis.navigator.gpu?.requestAdapter()) ?? (await globalThis.navigator.gpu?.requestAdapter({ fallbackAdapter: true })),
  ));
  if (!hasWebGPU && !isChromium) {
    assert.deepEqual(await page.evaluate(() => window.blockedRequests), []);
    assert.deepEqual(failures, []);
    const summary = `${product} ${browser.version()}: WebGPU 非対応環境のため 3D プレイグラウンドの配信・CSP のみ確認`;
    console.log(summary);
    if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, "\n" + summary + "\n");
  } else {

  const status = page.locator(".status");
  const canvas = page.getByLabel("3D の表示", { exact: true });
  const camera = page.getByLabel("視点", { exact: true });
  const notices = page.getByRole("region", { name: "表示の通知" });
  const choose = (label, name, buffer) => page.getByLabel(label, { exact: true }).setInputFiles({ name, mimeType: "application/octet-stream", buffer });
  async function ready(text) {
    await page.waitForFunction((value) => {
      const status = document.querySelector(".status")?.textContent ?? "";
      return status.includes("表示しました。 " + value) || status.includes("表示できません");
    }, text);
    assert.ok((await status.textContent()).includes(text), await page.locator("main").innerText());
  }
  const shot = () => canvas.screenshot();
  const notice = (pattern) => page.waitForFunction((source) => new RegExp(source).test(document.querySelector("[aria-labelledby=notices-title] ul")?.textContent ?? ""), pattern.source);

  assert.equal(await status.getAttribute("role"), "status");
  const empty = await shot();
  await page.getByText("表示の資源", { exact: true }).click();
  const resources = page.getByLabel("表示の資源", { exact: true });
  const usage = async () => (await resources.innerText()).replace(/\s+/g, " ").match(/geometry \d+ texture \d+/)[0];
  const baseline = await usage();
  // 建物（IFC）：一覧・選択・属性・鍵盤の操作・追加描画・BCF の保存と再読
  await choose("3D ファイル", "scene.ifc", inputs.ifc);
  await ready("IFC ／ 建物 ／ 要素 7 件");
  const drawn = await shot();
  assert.notDeepEqual(drawn, empty, "IFC の画素");
  const elements = page.getByRole("region", { name: "要素" }).getByRole("button");
  assert.equal(await elements.count(), 7);
  const first = elements.first();
  await first.focus(); await page.keyboard.press("Space");
  assert.equal(await first.getAttribute("aria-pressed"), "true");
  const inspector = page.getByRole("region", { name: "選んだ要素" });
  await inspector.getByText("建物モデルの属性", { exact: true }).waitFor();
  assert.ok((await inspector.innerText()).includes((await first.innerText()).split("\n")[0]), "選んだ要素の ID");
  await inspector.getByText("建物モデルの属性", { exact: true }).click();
  assert.match(await inspector.locator("pre").innerText(), /"class"/);
  await first.click();
  assert.equal(await first.getAttribute("aria-pressed"), "false", "もう一度押すと選択を外す");
  const home = await camera.innerText();
  await canvas.focus();
  for (const [key, label] of [["+", "拡大"], ["-", "縮小"], ["ArrowLeft", "移動"], ["ArrowUp", "移動"]]) {
    const before = await camera.innerText();
    await page.keyboard.press(key);
    assert.notEqual(await camera.innerText(), before, key + " で" + label);
  }
  await page.keyboard.press("Home");
  assert.equal(await camera.innerText(), home, "Home で全体");
  await page.getByRole("button", { name: "拡大", exact: true }).click();
  assert.notEqual(await camera.innerText(), home);
  await page.getByRole("button", { name: "全体を表示", exact: true }).click();
  assert.equal(await camera.innerText(), home);
  const plain = await shot();
  await page.getByLabel("幅の寸法を表示").check();
  await page.waitForFunction(() => true);
  assert.notDeepEqual(await shot(), plain, "寸法の追加描画");
  await page.getByLabel("幅の寸法を表示").uncheck();

  const saving = page.waitForEvent("download");
  await page.getByRole("button", { name: "BCF 3.0 で保存", exact: true }).click();
  const download = await saving;
  assert.equal(download.suggestedFilename(), "scene-3.0.bcf");
  const bcf = readFileSync(await download.path());
  assert.equal(bcf.subarray(0, 2).toString("latin1"), "PK", "BCF は ZIP");
  await notice(/BCF 3\.0 を保存しました/);
  const saved = await camera.innerText();
  const saving21 = page.waitForEvent("download");
  await page.getByRole("button", { name: "BCF 2.1 で保存", exact: true }).click();
  assert.equal((await saving21).suggestedFilename(), "scene-2.1.bcf");
  await canvas.focus(); await page.keyboard.press("+"); await page.keyboard.press("ArrowLeft");
  assert.notEqual(await camera.innerText(), saved);
  await choose("BCF を読む", "scene-3.0.bcf", bcf);
  await notice(/BCF の「プレイグラウンドの視点」の視点を当てました/);
  assert.equal(await camera.innerText(), saved, "BCF の視点を当て直す");
  assert.equal(downloads.length, 2, "保存は押したときだけ");

  // 地面の図面と重ねる点群は通知に状態か理由を出す。表示中の IFC は地図の変換を持つ
  await choose("地面の図面", "ground.dxf", inputs.dxf);
  await notice(/地面の図面: /);
  await choose("重ねる点群", "scan.las", inputs.las);
  await notice(/重ねた点群: /);
  const withLayers = await notices.innerText();

  // 地物（CityGML）：同じ EPSG:6677 の点群は重なり、高さの基準は片方だけなので未確認
  await choose("3D ファイル", "geo.gml", inputs.gml);
  await ready("CITYGML ／ 地物 ／ 要素 1 件");
  await notice(/重ねた点群: \d+ 点（高さの基準 未確認）/);
  assert.equal(await page.getByRole("button", { name: "BCF 3.0 で保存", exact: true }).isDisabled(), true, "BCF は IFC だけ");
  await elements.first().click();
  assert.match(await inspector.innerText(), /bldg:Building/);
  // 点群（LAS）を開くと、同じシーンに点を描く
  await choose("3D ファイル", "c.las", inputs.las);
  await ready("LAS ／ 点群 ／ 要素 0 件 ／ 点 12 点");
  await notice(/点群: 12 点を描きました/);
  // 3D でない形式は理由を示す
  await choose("3D ファイル", "drawing.dxf", inputs.dxf);
  await page.getByText("表示できません: dxf は 3D に対応していません", { exact: true }).waitFor();

  await page.getByRole("button", { name: "閉じる", exact: true }).click();
  assert.equal(await status.textContent(), "3D のファイル（IFC・CityGML・LAS など）を選んでください。");
  assert.equal(await usage(), baseline, "閉じると開く前と同じ geometry と texture の数に戻る");
  assert.equal(page.workers().length, 0, "閉じると Worker を終える");
  assert.deepEqual(await page.evaluate(() => window.blockedRequests), []);

  // 本番用 React と開発用 StrictMode：差し替え・中断・古い応答・アンマウント
  for (const folder of ["production", "strict"]) {
    await page.goto(origin + "/" + folder + "/test/lifecycle3d.html");
    await page.waitForFunction(() => Boolean(window.probe));
    await page.evaluate((bytes) => probe.render(bytes), [...inputs.ifc]);
    await page.waitForFunction(() => probe.status === "ready");
    assert.deepEqual(await page.evaluate(() => [probe.format, probe.module, probe.scene]), ["ifc", "bim", "ifc"]);
    assert.deepEqual(await page.evaluate(() => [probe.counts.workers, probe.counts.state, probe.counts.events, probe.counts.inputs]), [1, 1, 1, 1], folder);
    await page.evaluate((bytes) => probe.render(bytes, true), [...inputs.gml]);
    await page.waitForFunction(() => probe.delayed.length > 0);
    assert.equal(await page.evaluate(() => probe.delayed.at(-1).name), "civil");
    await page.evaluate((bytes) => probe.render(bytes), [...inputs.las]);
    await page.waitForFunction(() => probe.format === "las");
    await page.evaluate(() => probe.release());
    assert.ok(await page.evaluate(() => probe.delayed.every((task) => task.signal.aborted) && probe.format === "las" && probe.module === "pointcloud"), folder + ": 中断した読み込み");
    await page.evaluate((bytes) => { probe.holdNext(); probe.render(bytes); }, [...inputs.gml]);
    await page.waitForFunction(() => probe.held && probe.status === "loading");
    await page.evaluate((bytes) => probe.render(bytes), [...inputs.ifc]);
    await page.waitForFunction(() => probe.format === "ifc");
    await page.evaluate(() => probe.deliver());
    assert.deepEqual(await page.evaluate(() => [probe.format, probe.scene]), ["ifc", "ifc"], folder + ": 古い応答を表示しない");
    assert.deepEqual(await page.evaluate(() => [probe.counts.workers, probe.counts.state, probe.counts.events, probe.counts.inputs]), [1, 1, 1, 1]);
    assert.equal(page.workers().length, 1, "表示中のセッションを持つ実際の Worker は一つ");
    const exited = page.workers()[0].waitForEvent("close", { timeout: 10000 });
    assert.deepEqual(await page.evaluate(() => probe.finish()), ["AbortError", "AbortError"]);
    await exited;
    const counts = await page.evaluate(() => probe.counts);
    assert.deepEqual([counts.workers, counts.state, counts.events, counts.inputs], [0, 0, 0, 0], folder + ": " + JSON.stringify(counts));
    assert.equal(counts.starts, counts.ends);
    assert.ok(counts.transferred >= 4 && counts.destroyed === counts.created && counts.created >= 1, JSON.stringify(counts));
    assert.equal(page.workers().length, 0);
    // View3D.create の解決前にアンマウントしても、遅れて解決したインスタンスを直ちに破棄し GPU 資源を漏らさない。
    await page.evaluate(() => { probe.holdCreate(); probe.render(null); });
    await page.waitForFunction(() => probe.pendingCreates === 1);
    await page.evaluate(() => { probe.unmount(); probe.releaseCreate(); });
    await page.waitForFunction((prev) => probe.counts.created === prev.created + 1 && probe.counts.destroyed === probe.counts.created, counts, { timeout: 10000 });
    assert.deepEqual(await page.evaluate(() => [probe.counts.state, probe.counts.events, probe.counts.inputs, probe.counts.created - probe.counts.destroyed]), [0, 0, 0, 0]);
  }
  assert.ok([...seen].some((p) => p.includes("/app/assets/worker3d-")) && [...seen].some((p) => p.endsWith(".wasm")));
  assert.deepEqual(failures, []);
  const summary = product + " " + browser.version() + " で 3D のプレイグラウンドの IFC・CityGML・LAS の表示、要素の選択と属性、鍵盤の操作、"
    + "寸法の追加描画、地面の図面と点群の重ね合わせの通知、BCF 3.0・2.1 の明示の保存と再読、未対応の形式の理由、閉じたときの資源と Worker の解放、"
    + "本番用 React・StrictMode の差し替え・中断・古い応答・アンマウント、ローカルの配布物だけの GET を確認";
  console.log(summary);
  console.log("通知: " + withLayers.replace(/\s+/g, " "));
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, "\n" + summary + "\n");
  }
} catch (error) {
  console.error("失敗の時点の記録: " + JSON.stringify(failures));
  if (browser?.isConnected()) for (const page of browser.contexts().flatMap((context) => context.pages())) {
    console.error(await page.locator("body").innerText().catch(() => ""));
  }
  throw error;
} finally {
  await browser?.close();
  if (server?.listening) await new Promise((done) => server.close(done));
  assert.equal(dirname(resolve(work)), target); assert.ok(basename(work).startsWith("react3d-playground-"));
  rmSync(work, { recursive: true, force: true });
}
