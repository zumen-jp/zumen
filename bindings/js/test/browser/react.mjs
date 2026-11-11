// 決定 0199・0267。ローカルのビルドを配り、ページと Worker が要求したものを確認する。
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { appendFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { basename, dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "playwright";
import { build } from "vite";
import { checkBoundaries, checkInteractions, installWorkerGate } from "./react-interactions.mjs";
import { checkPlaygroundResize, installResizeTrace } from "./react-resize.mjs";
import { loadSync } from "../../src/index.js";

const [product, wasm] = process.argv.slice(2);
const browsers = { chromium, firefox, webkit, chrome: chromium, msedge: chromium };
if (!browsers[product] || !wasm) throw new Error("使い方: node react.mjs <chromium|firefox|webkit|chrome|msedge> <drawing.wasm>");
const root = fileURLToPath(new URL("../../../../", import.meta.url));
const app = join(root, "playground");
process.env.ZUMEN_DRAWING_WASM = resolve(wasm);
const target = resolve(root, "target");
mkdirSync(target, { recursive: true });
const work = mkdtempSync(join(target, "react-playground-"));
const assets = new Map(), failures = [], seen = new Set();
function collect(folder, prefix) {
  for (const entry of readdirSync(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name), url = prefix + "/" + entry.name;
    if (entry.isDirectory()) collect(path, url);
    else assets.set(url, path);
  }
}
function fixture(name) {
  const value = JSON.parse(readFileSync(join(root, "bindings/conformance/cases", name + ".json"))).steps.find((s) => s.push).push;
  return Buffer.from(value.hex ?? value.text, value.hex ? "hex" : "utf8");
}
const inputs = { dxf: fixture("scene"), jww: fixture("open_jww"), sxf: fixture("composite_sxf") };
// 「DXF で保存」（#690）で変換する入力と、ネイティブの convert が書いた DXF の SHA-256（適合テストのハッシュの一覧の blob の行）
function conversions() {
  const steps = JSON.parse(readFileSync(join(root, "bindings/conformance/cases/convert_playground.json"))).steps;
  const pinned = new Map(readFileSync(join(root, "bindings/conformance/hashes.txt"), "utf8").split(/\r?\n/)
    .map((line) => line.trim().split(" ")).filter(([name, , kind]) => name === "convert_playground" && kind === "blob")
    .map(([, step, , hash]) => [Number(step), hash]));
  const bytes = new Map(steps.filter((step) => step.push).map(({ push }) => [push.name,
    push.file ? readFileSync(join(root, push.file)) : Buffer.from(push.hex ?? push.text, push.hex ? "hex" : "utf8")]));
  const found = [];
  let doc;
  steps.forEach((step, i) => {
    if (step.call?.op === "convert") doc = step.call.doc;
    if ("take" in step) found.push({ name: doc, bytes: bytes.get(doc), sha256: pinned.get(i) });
  });
  assert.equal(found.length, 7);
  assert.ok(found.every((item) => item.bytes && item.sha256));
  return found;
}
// 同じ入力を、Node.js で同じ wasm に convert させる。保存したファイルを開き直し、もう一度 DXF に変換する
function convertInNode(module, input, saved) {
  const session = loadSync(module).open();
  try {
    session.push("input", input);
    session.call({ v: 1, op: "open", input: "input" });
    const estimated = session.call({ v: 1, op: "convert_estimate", doc: "input", format: "dxf", to: "converted.dxf" });
    assert.deepEqual(estimated.blobs, [], "見積もりではファイルを返さない");
    const converted = session.call({ v: 1, op: "convert", doc: "input", format: "dxf", to: "converted.dxf" });
    assert.equal(estimated.result.estimate.hash, converted.result.hash, "見積もりと保存するバイト列のハッシュ");
    assert.equal(estimated.result.estimate.elements, converted.result.elements, "見積もりと保存する要素数");
    assert.deepEqual(estimated.diagnostics, converted.diagnostics, "見積もりと保存の診断");
    const written = session.take(0);
    session.push("saved", saved);
    const reopened = session.call({ v: 1, op: "open", input: "saved" });
    const drawing = session.call({ v: 1, op: "drawing", doc: reopened.result.doc });
    const again = session.call({ v: 1, op: "convert", doc: reopened.result.doc, format: "dxf", to: "again.dxf" });
    return { estimated, converted, written, reopened, drawing, again };
  } finally { session.close(); }
}
// 記録されたフォント・画像・PDF の参照名を、ブラウザーの要求先として使わない。
const section = (name, pairs) => [0, "SECTION", 2, name, ...pairs, 0, "ENDSEC"];
const external = "https://example.invalid/";
const references = Buffer.from([
  ...section("HEADER", [9, "$INSUNITS", 70, 4]),
  ...section("TABLES", [0, "TABLE", 2, "STYLE", 0, "STYLE", 2, "Remote", 3, external + "font.ttf", 0, "ENDTAB"]),
  ...section("ENTITIES", [
    0, "TEXT", 5, "A0", 8, "0", 7, "Remote", 10, 0, 20, 0, 40, 3, 1, "Text",
    0, "IMAGE", 5, "A1", 8, "0", 100, "AcDbRasterImage", 90, 0, 10, 0, 20, 0, 11, 1, 21, 0, 12, 0, 22, 1, 13, 10, 23, 10, 340, "D1", 70, 3,
    0, "PDFUNDERLAY", 5, "A2", 8, "0", 340, "D2", 10, 20, 20, 10, 41, 1, 42, 1, 50, 0, 280, 19, 11, 0, 21, 0, 11, 8, 21, 11,
  ]),
  ...section("OBJECTS", [0, "IMAGEDEF", 5, "D1", 1, external + "image.png", 10, 10, 20, 10,
    0, "PDFDEFINITION", 5, "D2", 1, external + "sheet.pdf", 2, "1"]),
  0, "EOF", "",
].join("\n"));
const port = 24182 + Object.keys(browsers).indexOf(product);
const origin = "http://127.0.0.1:" + port;
const mime = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm" };
let browser, server;
try {
  const options = { root: app, configFile: join(app, "vite.config.js"), logLevel: "error" };
  await build({ ...options, build: { outDir: join(work, "app") } });
  assert.deepEqual(readFileSync(join(work, "app/viewer.css")), readFileSync(join(app, "public/viewer.css")));
  // 同梱した公開サンプルの出所と、同梱を求めるライセンスの本文の一覧（#689）
  const listed = readFileSync(join(work, "app/samples.md"), "utf8");
  for (const name of ["dxf/text_alignments.dxf", "dxf/opendxfviewer-latin1.dxf", "dwg/zumen-layout-r2000.dwg", "dwg/zumen-minsert-r2004.dwg", "dwg/zumen-xclip-r2018.dwg"]) {
    assert.ok(listed.includes("## " + name), name);
  }
  assert.equal(listed.match(/Permission is hereby granted/g)?.length, 2, "MIT の本文 2 つ");
  collect(join(work, "app"), "/app");
  // 同じ境界の検査を本番用 React と開発用 StrictMode の両方で組む。検査用の入口は製品へ入れない。
  for (const [folder, mode] of [["production", "production"], ["strict", "development"]]) {
    await build({ ...options, define: { "process.env.NODE_ENV": JSON.stringify(mode) }, build: {
      outDir: join(work, folder), rolldownOptions: { input: join(app, "test/lifecycle.html") },
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
  browser = await browsers[product].launch(["chrome", "msedge"].includes(product) ? { channel: product } : {});
  const context = await browser.newContext({ serviceWorkers: "block" });
  await context.route("**/*", (route) => {
    const request = route.request(), url = new URL(request.url());
    if (url.origin !== origin || !assets.has(url.pathname) || url.search || request.method() !== "GET" || request.postDataBuffer() !== null) {
      failures.push("許可していない通信"); return route.abort();
    }
    return route.continue();
  });
  const page = await context.newPage();
  await page.exposeFunction("captureResizeFailure", (record) => console.log("ResizeObserver diagnostic: " + JSON.stringify(record)));
  await page.addInitScript(installResizeTrace);
  await page.addInitScript(installWorkerGate);
  await page.addInitScript(() => {
    window.blockedRequests = [];
    document.addEventListener("securitypolicyviolation", (event) => {
      // CSP がネットワークより先に止めた要求も数える。WebKit の select の生成時の inline style は通信ではない。
      if (/^https?:|^wss?:/.test(event.blockedURI)) window.blockedRequests.push(event.violatedDirective);
    });
  });
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("websocket", () => failures.push("WebSocket"));
  await page.goto(origin + "/app/index.html");
  const file = page.getByLabel("図面ファイル");
  const status = page.locator(".status");
  const source = page.getByRole("region", { name: "出所" });
  async function ready(text) {
    await page.waitForFunction((value) => {
      const status = document.querySelector(".status")?.textContent;
      return status?.includes("表示しました。 " + value) || (status?.includes("表示できる要素がありません") && status.includes(value)) || status?.includes("表示できません");
    }, text);
    assert.ok((await status.textContent()).includes(text), await page.locator("main").textContent());
  }
  const pixels = () => page.getByLabel("図面の表示", { exact: true }).evaluate((canvas) => {
    const data = canvas.getContext("2d").getImageData(0, 0, canvas.width, canvas.height).data;
    let count = 0;
    for (let i = 0; i < data.length; i += 4) if (data[i] < 225 || data[i + 1] < 225 || data[i + 2] < 225) count++;
    return count;
  });
  assert.equal(await status.getAttribute("role"), "status");
  // 最初の Tab は「本文へ移動」に止まる（WebKit は既定でリンクに止まらない）。サイトのナビとパンくずのあとに、ファイルの選択へ届く。
  await page.keyboard.press("Tab");
  const first = await page.evaluate(() => document.activeElement?.textContent);
  assert.ok(first === "本文へ移動" || await file.evaluate((el) => el === document.activeElement), "最初の Tab: " + first);
  for (let i = 0; i < 12 && !await file.evaluate((el) => el === document.activeElement); i++) await page.keyboard.press("Tab");
  assert.ok(await file.evaluate((el) => el === document.activeElement), "Tab でファイルの選択に届く");
  for (const [format, id] of [["dxf", "space/0/hA1"], ["jww", "space/0/r1"], ["sxf", "sheet/0/i30"]]) {
    await file.setInputFiles({ name: "local." + (format === "sxf" ? "sfc" : format), mimeType: "application/octet-stream", buffer: inputs[format] });
    await ready(format.toUpperCase());
    assert.ok(await pixels() > 0, format + ": 図形の画素");
    if (format === "jww") {
      const canvas = page.getByLabel("図面の表示", { exact: true });
      await canvas.click();
      await source.getByText("レコード", { exact: true }).waitFor();
      const before = await canvas.evaluate((node) => node.toDataURL());
      await canvas.press("ArrowRight");
      assert.notEqual(await canvas.evaluate((node) => node.toDataURL()), before, "矢印キーで描画が移動する");
      await canvas.press("Home");
    }
    const button = page.getByRole("button", { name: id, exact: true });
    await button.focus(); await page.keyboard.press("Space");
    await source.getByText("レコード", { exact: true }).waitFor();
    assert.equal(await button.getAttribute("aria-pressed"), "true");
    assert.equal(await source.getByRole("status").getAttribute("aria-live"), "polite");
    assert.match(await source.ariaSnapshot(), /レコード/);
    assert.match(await source.innerText(), new RegExp(id));
    const layers = page.getByRole("checkbox");
    for (const layer of await layers.all()) { await layer.focus(); await page.keyboard.press("Space"); assert.equal(await layer.isChecked(), false); }
    assert.equal(await pixels(), 0, "レイヤの非表示");
    for (const layer of await layers.all()) await layer.check();
    assert.ok(await pixels() > 0);
    if (format !== "sxf") {
      const place = page.getByLabel("用紙・空間");
      await page.evaluate(() => placeGate.defer());
      await place.focus(); await page.keyboard.press("ArrowUp");
      await page.waitForFunction(() => placeGate.waiting);
      assert.equal(await place.inputValue(), "sheet/0");
      assert.ok(await place.evaluate((node) => !node.disabled && node === document.activeElement), "用紙の読み込み中もフォーカスを保つ");
      await page.keyboard.press("ArrowDown"); await ready(format.toUpperCase() + " ／ space/0");
      await page.evaluate(() => placeGate.release());
      assert.equal(await place.inputValue(), "space/0");
      assert.ok(await place.evaluate((node) => node === document.activeElement));
      await place.selectOption("sheet/0"); await ready(format.toUpperCase() + " ／ sheet/0");
      const first = page.getByRole("region", { name: /^要素/ }).getByRole("button").first();
      await first.press("Enter"); await source.getByText("レコード", { exact: true }).waitFor();
    } else {
      assert.match(await page.getByRole("region", { name: /^診断/ }).innerText(), /sxf\.lift\.dropped/);
      assert.match(await source.innerText(), /#30/);
    }
  }
  // DXF で保存（#690）。合成の DWG・JWW・SFC・P21 と公開サンプルの DWG を開いて保存し、保存したバイト列が、
  // ネイティブの convert が書いたもの（適合テスト）と同じであることを確かめる。変換の診断の件数は、画面と応答で比べる
  const module = new WebAssembly.Module(readFileSync(wasm));
  const exportPanel = page.getByRole("region", { name: /^DXF で保存/ });
  const downloads = [];
  const collectDownload = (download) => downloads.push(download);
  page.on("download", collectDownload);
  for (const { name, bytes, sha256 } of conversions()) {
    await file.setInputFiles({ name, mimeType: "application/octet-stream", buffer: bytes });
    await ready(name.endsWith(".dwg") ? "DWG" : name.endsWith(".jww") ? "JWW" : "SXF");
    // 文書ごとに新しい欄になる（前の図面の結果を見ない）
    const shown = exportPanel.getByRole("status");
    assert.equal(await shown.innerText(), "", name);
    assert.equal(await exportPanel.getByRole("button", { name: "DXF で保存", exact: true }).count(), 0, "見積もり前には保存しない");
    const before = downloads.length;
    const estimate = exportPanel.getByRole("button", { name: "保存前の見積もりを確認", exact: true });
    await estimate.click();
    await shown.getByText("見積もりを確認して、DXF で保存してください。", { exact: true }).waitFor();
    assert.equal(downloads.length, before, "見積もりだけではダウンロードしない");
    const preview = exportPanel.getByRole("region", { name: "保存前の見積もり", exact: true });
    await preview.waitFor();
    const saving = page.waitForEvent("download");
    const save = exportPanel.getByRole("button", { name: "DXF で保存", exact: true });
    await save.focus();
    await page.keyboard.press("Enter");
    const download = await saving;
    assert.equal(download.suggestedFilename(), name.replace(/\.[^.]*$/, "") + ".dxf", name);
    const saved = readFileSync(await download.path());
    assert.equal(createHash("sha256").update(saved).digest("hex"), sha256, name + ": ネイティブの convert と同じバイト列");
    const node = convertInNode(module, bytes, saved);
    assert.deepEqual(Buffer.from(node.written), saved, name + ": Node.js の wasm と同じバイト列");
    const { elements } = node.converted.result;
    const report = node.estimated.result.estimate;
    assert.match(await preview.innerText(), new RegExp("対象 " + report.total + " 件から、DXF に要素 " + elements + " 件"));
    const labels = { native: "保持", equivalent: "等価", approximated: "近似", drawn: "見た目だけ", dropped: "欠落" };
    for (const [fidelity, label] of Object.entries(labels)) {
      const count = report.counts.filter((row) => row.fidelity === fidelity).reduce((sum, row) => sum + row.count, 0);
      assert.equal(await preview.getByText(label, { exact: true }).locator("xpath=following-sibling::dd[1]").innerText(), count + " 件", name + ": " + label);
    }
    await shown.getByText(/のダウンロードを始めました/).waitFor();
    assert.ok(await save.evaluate((el) => el === document.activeElement), name + ": 押したあともフォーカスを保つ");
    assert.match(await shown.innerText(), new RegExp("要素 " + elements + " 件・変換の診断 " + node.converted.diagnostics.length + " 件"), name);
    const listed = exportPanel.getByRole("region", { name: /^変換の診断/ });
    assert.equal(await listed.locator("h2 small").innerText(), String(node.converted.diagnostics.length), name);
    for (const diagnostic of node.converted.diagnostics) {
      assert.ok((await listed.innerText()).includes(diagnostic.code + " · " + diagnostic.count + " 件"), name + ": " + diagnostic.code);
    }
    // 保存した DXF を開き直してもう一度変換すると、同じ数の要素を書き、新しく落とすものはない
    assert.equal(node.reopened.result.format, "dxf", name);
    assert.equal(node.again.result.elements, elements, name + ": 開き直した要素の数");
    const known = new Set(node.converted.diagnostics.map(({ code }) => code));
    for (const { code } of [...node.reopened.diagnostics, ...node.drawing.diagnostics, ...node.again.diagnostics]) {
      assert.ok(known.has(code), name + ": 開き直して出た " + code);
    }
  }
  // DXF を開いて保存するときは、元のファイルと分かる名前にする
  await file.setInputFiles({ name: "local.dxf", mimeType: "application/octet-stream", buffer: inputs.dxf });
  await ready("DXF");
  assert.equal(await exportPanel.getByRole("region", { name: "保存前の見積もり", exact: true }).count(), 0, "切り替え前の見積もりは残さない");
  await exportPanel.getByRole("button", { name: "保存前の見積もりを確認", exact: true }).click();
  await exportPanel.getByRole("status").getByText("見積もりを確認して、DXF で保存してください。", { exact: true }).waitFor();
  const savingDxf = page.waitForEvent("download");
  await exportPanel.getByRole("button", { name: "DXF で保存", exact: true }).click();
  assert.equal((await savingDxf).suggestedFilename(), "local-zumen.dxf");
  // 見積もり・保存の応答待ちに入力を切り替えても、前の見積もりやダウンロードを採用しない。
  for (const op of ["convert_estimate", "convert"]) {
    await file.setInputFiles({ name: "changed.dxf", mimeType: "application/octet-stream", buffer: inputs.dxf });
    await ready("DXF");
    if (op === "convert") {
      await exportPanel.getByRole("button", { name: "保存前の見積もりを確認", exact: true }).click();
      await exportPanel.getByRole("status").getByText("見積もりを確認して、DXF で保存してください。", { exact: true }).waitFor();
    }
    await page.evaluate((op) => exportGate.defer(op), op);
    const before = downloads.length;
    await exportPanel.getByRole("button", { name: op === "convert" ? "DXF で保存" : "保存前の見積もりを確認", exact: true }).click();
    await page.waitForFunction(() => exportGate.waiting);
    await file.setInputFiles({ name: "changed.jww", mimeType: "application/octet-stream", buffer: inputs.jww });
    await ready("JWW");
    await page.evaluate(() => exportGate.release());
    await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
    assert.equal(downloads.length, before, op + ": 前の入力の結果を保存しない");
    assert.equal(await exportPanel.getByRole("region", { name: "保存前の見積もり", exact: true }).count(), 0, op + ": 前の入力の見積もりを表示しない");
    assert.equal(await exportPanel.getByRole("button", { name: "DXF で保存", exact: true }).count(), 0);
  }
  page.off("download", collectDownload);
  // 公開サンプル（#689）。DWG の R2000・R2004・R2018 を選んで開き、表示・レイヤ・選択・出所・診断を操作する。
  // サンプルは配布物の中のファイルで、取るのはローカルの GET だけ（下の failures で確かめる）
  const samplePicker = page.getByLabel("公開サンプル");
  const sampleSource = page.getByRole("region", { name: "サンプルの提供元とライセンス" });
  for (const [path, version] of [["dwg/zumen-layout-r2000.dwg", "R2000"], ["dwg/zumen-minsert-r2004.dwg", "R2004"], ["dwg/zumen-xclip-r2018.dwg", "R2018"]]) {
    await samplePicker.selectOption(path);
    await ready("DWG");
    assert.ok(await pixels() > 0, version + ": 図形の画素");
    const shown = await sampleSource.innerText();
    assert.match(shown, /CC0-1\.0/, version);
    assert.match(shown, /zumen-jp/, version);
    const layers = page.getByRole("checkbox");
    assert.ok(await layers.count() > 0, version + ": レイヤ");
    for (const layer of await layers.all()) await layer.uncheck();
    assert.equal(await pixels(), 0, version + ": レイヤの非表示");
    for (const layer of await layers.all()) await layer.check();
    assert.ok(await pixels() > 0, version);
    const first = page.getByRole("region", { name: /^要素/ }).getByRole("button").first();
    await first.press("Enter");
    // DWG の要素の出所は、ハンドルとバイト範囲
    await source.getByText("ハンドル", { exact: true }).waitFor();
    assert.match(await source.innerText(), /バイト範囲/, version);
    assert.equal(await first.getAttribute("aria-pressed"), "true", version);
    assert.equal(await page.getByRole("region", { name: /^診断/ }).count(), 1, version);
  }
  // 本文の同梱を求めるライセンス（MIT）のサンプルは、本文を配布物から開ける
  await samplePicker.selectOption("dxf/text_alignments.dxf");
  await ready("DXF");
  assert.match(await sampleSource.innerText(), /MIT/);
  const text = await sampleSource.getByRole("link", { name: "本文", exact: true }).getAttribute("href");
  assert.match(await page.evaluate((href) => fetch(href).then((r) => r.text()), text), /MIT License/);
  // 続けて選ぶと、最後に選んだものだけを開く（大きい DWG を取っている間に、小さい DXF を選ぶ）
  // DWG の要求を止めておき、DXF を開いてから流す
  const slow = "**/zumen-layout-r2000-*.dwg";
  let release, held = new Promise((done) => { release = done; });
  let arrived;
  const requested = new Promise((done) => { arrived = done; });
  await page.route(slow, async (route) => { arrived(); await held; await route.continue(); });
  await samplePicker.selectOption("dwg/zumen-layout-r2000.dwg");
  await requested;
  await samplePicker.selectOption("dxf/opendxfviewer-latin1.dxf");
  await ready("DXF");
  assert.equal(await samplePicker.inputValue(), "dxf/opendxfviewer-latin1.dxf");
  assert.match(await sampleSource.innerText(), /ANSI_1252/);
  assert.match(await status.textContent(), /表示できる要素がありません/);
  assert.match(await page.getByRole("region", { name: /^診断/ }).textContent(), /dxf.code-page.unsupported/);
  // 先に選んだ DWG が遅れて届いても、差し替えない
  const late = page.waitForResponse(slow);
  release();
  await late;
  await page.waitForTimeout(500);
  assert.match(await status.textContent(), /表示できる要素がありません.*DXF/);
  assert.equal(await samplePicker.inputValue(), "dxf/opendxfviewer-latin1.dxf");
  await page.unroute(slow);
  // 開いたサンプルを閉じる
  await page.getByRole("button", { name: "閉じる", exact: true }).click();
  assert.equal(await status.textContent(), "図面を選んでください。");
  assert.equal(await sampleSource.count(), 0);
  assert.equal(await samplePicker.inputValue(), "");
  // 取れなかったサンプルは、選択を空に戻し、同じものを選び直せる
  const broken = "**/zumen-xclip-r2018-*.dwg";
  await page.route(broken, (route) => route.fulfill({ status: 500, body: "" }));
  await samplePicker.selectOption("dwg/zumen-xclip-r2018.dwg");
  await page.getByText("サンプルを取れませんでした。", { exact: true }).waitFor();
  assert.equal(await samplePicker.inputValue(), "");
  await page.unroute(broken);
  await samplePicker.selectOption("dwg/zumen-xclip-r2018.dwg");
  await ready("DWG");
  // 手元のファイルを選ぶと、サンプルの出所を消す
  await file.setInputFiles({ name: "local.dxf", mimeType: "application/octet-stream", buffer: inputs.dxf });
  await ready("DXF");
  assert.equal(await sampleSource.count(), 0);
  assert.equal(await samplePicker.inputValue(), "");

  await file.setInputFiles({ name: "references.dxf", mimeType: "application/octet-stream", buffer: references });
  await ready("DXF");
  const report = page.getByRole("region", { name: /^診断/ });
  assert.equal(await report.getByText("resource-unresolved", { exact: true }).count(), 2, await report.innerText());
  assert.match(await report.innerText(), /browser-text/);
  await file.setInputFiles({ name: "bad.dxf", mimeType: "application/octet-stream", buffer: Buffer.from("invalid") });
  await page.getByText("図面を表示できませんでした。", { exact: true }).waitFor();
  assert.equal(await page.getByRole("complementary").count(), 0);
  await page.getByRole("button", { name: "閉じる", exact: true }).press("Enter");
  assert.equal(await status.textContent(), "図面を選んでください。");
  assert.ok(await file.evaluate((el) => el === document.activeElement));

  const resizeInput = await checkInteractions(page, ready);
  assert.deepEqual(await page.evaluate(() => window.blockedRequests), []);
  await page.goto(origin + "/production/test/lifecycle.html");
  await checkBoundaries(page, inputs.jww, "本番");
  await checkPlaygroundResize(page, resizeInput, "本番");
  await page.goto(origin + "/strict/test/lifecycle.html");
  await page.evaluate((bytes) => probe.render(bytes, true), [...inputs.dxf]);
  assert.ok(await page.evaluate(() => probe.delayed.length === 2 && probe.delayed[0].signal.aborted && !probe.delayed[1].signal.aborted));
  await page.evaluate((bytes) => probe.render(bytes), [...inputs.jww]);
  await page.waitForFunction(() => probe.format === "jww");
  await page.evaluate(() => probe.release());
  assert.ok(await page.evaluate(() => probe.delayed.every((task) => task.signal.aborted) && probe.format === "jww" && probe.counts.starts === 1));
  await page.evaluate(() => { probe.render(null, false, 7); probe.select(); });
  assert.equal(await page.evaluate(() => probe.eventVersion), 7);
  assert.equal(await page.evaluate(() => probe.selected), "space/0/r1");
  await page.evaluate((bytes) => { probe.holdNext(); probe.render(bytes); }, [...inputs.dxf]);
  await page.waitForFunction(() => probe.held && probe.status === "loading");
  await page.evaluate((bytes) => probe.render(bytes), [...inputs.sxf]);
  await page.waitForFunction(() => probe.format === "sxf");
  await page.evaluate(() => probe.deliver());
  assert.equal(await page.evaluate(() => probe.format), "sxf");
  assert.deepEqual(await page.evaluate(() => [probe.counts.workers, probe.counts.state, probe.counts.events]), [1, 1, 1]);
  const workers = page.workers();
  assert.equal(workers.length, 1, "表示中のセッションを持つ実際の Worker は一つ");
  const exited = workers[0].waitForEvent("close", { timeout: 10000 });
  assert.deepEqual(await page.evaluate(() => probe.finish()), ["AbortError", "AbortError"]);
  await exited;
  assert.equal(page.workers().length, 0);
  const counts = await page.evaluate(() => probe.counts);
  assert.deepEqual([counts.workers, counts.starts, counts.ends, counts.transferred, counts.state, counts.events, counts.destroyed], [0, 3, 3, 3, 0, 0, 2]);
  await checkBoundaries(page, inputs.jww, "StrictMode");
  await checkPlaygroundResize(page, resizeInput, "StrictMode");
  assert.ok([...seen].some((p) => p.includes("/app/assets/worker-")) && [...seen].some((p) => p.endsWith(".wasm")));
  assert.deepEqual(failures, []);
  const summary = product + " " + browser.version() + " で DXF・JWW・SXF と公開サンプルの DWG（R2000・R2004・R2018）、初回の表示範囲・倍率の両端、用紙の連続切り替え・レイヤ・40 要素の選択・配置別の診断、DXF で保存（合成の DWG・JWW・SFC・P21 と公開サンプルの DWG をネイティブと同じバイト列で）、本番用 React・StrictMode の再接続中の状態と出所、リサイズの8フレーム以内の収束と予約取消、ローカルの配布物だけの GET を確認";
  console.log(summary);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, "\n" + summary + "\n");
} finally {
  await browser?.close();
  if (server?.listening) await new Promise((done) => server.close(done));
  assert.equal(dirname(resolve(work)), target); assert.ok(basename(work).startsWith("react-playground-"));
  rmSync(work, { recursive: true, force: true });
}
