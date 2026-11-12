// 決定 0296。組んだ出力か公開したサイトを開き、ページと Worker の通信・CSP・表示を確かめる。
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { createServer } from "node:http";
import { extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "playwright";

const [product = "chromium", publicOrigin] = process.argv.slice(2);
const launchers = { chromium, firefox, webkit, chrome: chromium, msedge: chromium };
assert.ok(launchers[product]);
const root = fileURLToPath(new URL("../../", import.meta.url));
const dist = resolve(root, "site/dist");
const files = new Map();
function collect(folder, prefix = "") {
  for (const entry of readdirSync(folder, { withFileTypes: true })) {
    const path = resolve(folder, entry.name), url = prefix + "/" + entry.name;
    if (entry.isDirectory()) collect(path, url);
    else { files.set(url, path); if (entry.name === "index.html") files.set(prefix + "/", path); }
  }
}
collect(dist);
const missingPath = "/no-such-path-zumen-csp-test/";
const staticCsp = "script-src 'none'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
const headers = Object.fromEntries(readFileSync(resolve(dist, "_headers"), "utf8").split(/\r?\n/)
  .filter((line) => /^\s+\S+:/.test(line)).map((line) => {
    const colon = line.indexOf(":"); return [line.slice(0, colon).trim(), line.slice(colon + 1).trim()];
  }));
const mime = { ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".md": "text/plain; charset=utf-8" };
const requests = [], errors = [], blocked = [];
let server, browser;
try {
  let origin = publicOrigin && new URL(publicOrigin).origin;
  if (!origin) {
    server = createServer((request, response) => {
      const url = new URL(request.url, "http://localhost"), path = files.get(url.pathname);
      if (request.method !== "GET" || url.search || request.headers["content-length"] || request.headers["transfer-encoding"]) {
        response.writeHead(404).end(); return;
      }
      if (!path) {
        // Pages の実配信で観測した 404。_headers の CSP と no-transform がなくても、HTML で拒否する。
        const html = readFileSync(resolve(dist, "404.html"), "utf8").replace("</body>",
          '<script>window.staticScriptRan=true</script><script defer src="https://example.invalid/parser-classic.js"></script><script type="module" src="https://example.invalid/parser-module.js"></script></body>');
        response.writeHead(404, { "Content-Type": mime[".html"], "Cache-Control": "no-store" }).end(html); return;
      }
      response.writeHead(200, { "Content-Type": mime[extname(path)] ?? "application/octet-stream", ...(url.pathname.startsWith("/viewer/") ? headers : { "Cache-Control": headers["Cache-Control"] }) }).end(readFileSync(path));
    });
    await new Promise((ok, fail) => server.once("error", fail).listen(24189, "127.0.0.1", ok));
    origin = "http://127.0.0.1:24189";
  }
  browser = await launchers[product].launch(["chrome", "msedge"].includes(product) ? { channel: product } : {});
  const context = await browser.newContext({ viewport: { width: 1360, height: 980 }, serviceWorkers: "block" });
  const allowed = new Set([...files.keys(), missingPath]);
  // リダイレクト先も含め、知らない要求は送信前に止める。失敗として数え、通信がなかったという結果にはしない。
  await context.route("**/*", (route) => {
    const request = route.request(), url = new URL(request.url());
    requests.push({ origin: url.origin, path: url.pathname, method: request.method(), body: request.postDataBuffer() !== null });
    if (url.origin !== origin || !allowed.has(url.pathname) || url.search || request.method() !== "GET" || request.postDataBuffer() !== null) {
      blocked.push("request"); return route.abort();
    }
    return route.continue();
  });
  context.on("weberror", () => errors.push("pageerror"));
  context.setDefaultTimeout(30_000);
  const page = await context.newPage();
  await page.addInitScript(() => {
    window.cspFailures = [];
    document.addEventListener("securitypolicyviolation", (event) => window.cspFailures.push(event.violatedDirective));
  });
  page.on("websocket", () => errors.push("websocket"));
  const response = await page.goto(origin + "/viewer/");
  assert.equal(response.status(), 200);
  const csp = response.headers()["content-security-policy"];
  assert.equal(csp, headers["Content-Security-Policy"]);
  assert.equal(response.headers()["referrer-policy"], "no-referrer");
  assert.match(response.headers()["cache-control"], /(?:^|,\s*)no-transform(?:,|$)/);
  assert.match(response.headers()["cache-control"], /(?:^|,\s*)max-age=0(?:,|$)/);
  const picker = page.getByLabel("図面ファイル"), samples = page.getByLabel("公開サンプル");
  async function ready(format) {
    await page.waitForFunction((value) => document.querySelector(".status")?.textContent.includes("表示しました。 " + value), format);
    await page.waitForFunction(() => {
      const node = document.querySelector("canvas");
      if (!node?.width || !node.height) return false;
      const data = node.getContext("2d").getImageData(0, 0, node.width, node.height).data;
      for (let i = 0; i < data.length; i += 4) if (data[i] < 225 || data[i + 1] < 225 || data[i + 2] < 225) return true;
      return false;
    });
  }
  for (const [path, format] of [["dxf/text_alignments.dxf", "DXF"], ["dxf/opendxfviewer-latin1.dxf", "DXF"], ["dwg/zumen-layout-r2000.dwg", "DWG"]]) {
    await samples.selectOption(path);
    if (path === "dxf/opendxfviewer-latin1.dxf") {
      // 公開済みの 0.2.0 は ANSI_1252 を扱わない。空の表示を描画できたと数えず、診断を確認する。
      await page.locator(".status").filter({ hasText: "表示できる要素がありません" }).waitFor();
      assert.match(await page.getByRole("region", { name: /^診断/ }).textContent(), /dxf.code-page.unsupported/);
    } else await ready(format);
    await page.getByRole("region", { name: "サンプルの提供元とライセンス" }).waitFor();
  }
  const fixture = (name) => {
    const { push } = JSON.parse(readFileSync(resolve(root, "bindings/conformance/cases", name + ".json"))).steps.find((step) => step.push);
    return Buffer.from(push.hex ?? push.text, push.hex ? "hex" : "utf8");
  };
  const inputs = [["scene", "dxf", "DXF"], ["open_jww", "jww", "JWW"], ["composite_sxf", "sfc", "SXF"]];
  for (const [name, extension, format] of inputs) {
    await picker.setInputFiles({ name: "local." + extension, mimeType: "application/octet-stream", buffer: fixture(name) }); await ready(format);
  }
  // 図面に書かれた外部のフォント・画像・PDF の URL を、読み込み先として使わない。
  const section = (name, pairs) => [0, "SECTION", 2, name, ...pairs, 0, "ENDSEC"];
  const remote = "https://example.invalid/";
  const references = Buffer.from([
    ...section("HEADER", [9, "$INSUNITS", 70, 4]),
    ...section("TABLES", [0, "TABLE", 2, "STYLE", 0, "STYLE", 2, "Remote", 3, remote + "font.ttf", 0, "ENDTAB"]),
    ...section("ENTITIES", [
      0, "LINE", 5, "A0", 8, "0", 10, 0, 20, 0, 11, 100, 21, 100,
      0, "TEXT", 5, "A1", 8, "0", 7, "Remote", 10, 0, 20, 0, 40, 3, 1, "Text",
      0, "IMAGE", 5, "A2", 8, "0", 100, "AcDbRasterImage", 90, 0, 10, 0, 20, 0, 11, 1, 21, 0, 12, 0, 22, 1, 13, 10, 23, 10, 340, "D1", 70, 3,
      0, "PDFUNDERLAY", 5, "A3", 8, "0", 340, "D2", 10, 20, 20, 10, 41, 1, 42, 1,
    ]),
    ...section("OBJECTS", [0, "IMAGEDEF", 5, "D1", 1, remote + "image.png", 10, 10, 20, 10, 0, "PDFDEFINITION", 5, "D2", 1, remote + "drawing.pdf", 2, "1"]),
    0, "EOF", "",
  ].join("\n"));
  await picker.setInputFiles({ name: "references.dxf", mimeType: "application/octet-stream", buffer: references }); await ready("DXF");
  // 手元で選ぶ DWG も確認する。公開サンプルを File の入力に渡し、アップロード用の通信がないことを見る。
  await picker.setInputFiles({ name: "local.dwg", mimeType: "application/octet-stream", buffer: readFileSync(resolve(root, "samples/dwg/zumen-layout-r2000.dwg")) });
  await ready("DWG");
  const first = page.getByRole("region", { name: /^要素/ }).getByRole("button").first();
  await first.click();
  await page.getByRole("region", { name: "出所" }).getByText("ハンドル", { exact: true }).waitFor();
  // 公開済み 0.2.0 は convert_estimate を持たないので、保存前の見積もりに対応するまでは保存を表示しない。
  assert.equal(await page.getByRole("region", { name: /^DXF で保存/ }).count(), 0);
  assert.equal(await page.getByRole("link", { name: "非公開の報告窓口" }).count(), 1);
  assert.deepEqual(await page.evaluate(() => window.cspFailures), []);
  assert.deepEqual(blocked, []);
  assert.deepEqual(errors, []);
  assert.ok(requests.some((request) => request.path.endsWith(".wasm")));
  assert.ok(requests.some((request) => /\/worker-[^/]+\.js$/.test(request.path)));
  assert.ok(requests.every((request) => request.origin === origin && request.method === "GET" && !request.body));
  assert.equal((await context.cookies()).length, 0);
  // CSP がページと Worker の両方で通信より先に止めることも確認する。route で止めただけなら失敗にする。
  await page.evaluate(() => fetch("https://example.invalid/csp-probe").catch(() => {}));
  await page.waitForFunction(() => window.cspFailures.includes("connect-src"));
  const worker = page.workers()[0];
  assert.ok(worker);
  assert.equal(await worker.evaluate(() => new Promise((ok) => {
    self.addEventListener("securitypolicyviolation", (event) => ok(event.effectiveDirective), { once: true });
    fetch("https://example.invalid/worker-csp-probe").catch(() => {});
    setTimeout(() => ok("missing"), 2000);
  })), "connect-src");
  assert.deepEqual(blocked, []);
  await page.setViewportSize({ width: 390, height: 900 });
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "狭い画面で本文が横にはみ出さない");
  if (process.env.ZUMEN_VIEWER_SCREENSHOT) await page.screenshot({ path: process.env.ZUMEN_VIEWER_SCREENSHOT, fullPage: true });
  console.log(product + ": 公開サンプル 3 件・手元の DXF/DWG/JWW/SXF・外部参照・出所・見積もり未対応時の保存の非表示・CSP の拒否・同一オリジンの資産への GET のみを確認（要求 " + requests.length + " 件）");
  await page.close();
  // 案内ページはビューアーと別の HTML。すべての通常ページと、実際の 404 応答を確かめる。
  const staticPage = await context.newPage();
  await staticPage.addInitScript(() => {
    window.staticCspFailures = [];
    document.addEventListener("securitypolicyviolation", (event) => window.staticCspFailures.push({ directive: event.effectiveDirective, uri: event.blockedURI }));
  });
  staticPage.on("websocket", () => errors.push("websocket"));
  const staticPaths = [...files.keys()].filter((path) => path.endsWith("/") && !path.startsWith("/viewer/"));
  for (const path of [...staticPaths, missingPath]) {
    const result = await staticPage.goto(origin + path);
    assert.equal(result.status(), path === missingPath ? 404 : 200);
    assert.equal(await staticPage.locator('meta[http-equiv="Content-Security-Policy"]').getAttribute("content"), staticCsp);
    if (path !== missingPath) {
      assert.equal(await staticPage.locator("script").count(), 0, "通常の案内ページに script が注入されていない");
      assert.deepEqual(await staticPage.evaluate(() => window.staticCspFailures), []);
    } else {
      await staticPage.getByRole("heading", { name: "ページが見つかりません", exact: true }).waitFor();
      if (!publicOrigin) {
        assert.equal(result.headers()["content-security-policy"], undefined);
        assert.equal(result.headers()["cache-control"], "no-store");
        await staticPage.waitForFunction(() => window.staticCspFailures.filter(({ directive }) => directive === "script-src-elem").length === 3);
      }
      console.log(product + ": 404 の HTML の script タグ " + await staticPage.locator("script").count() + " 件（注入と通信の拒否を別に確認）");
    }
    // 外部・同一オリジン・インラインの実行を拒否する。DevTools 経由の evaluate 自体は CSP の対象外。
    await staticPage.evaluate(() => {
      for (const [src, type] of [["https://example.invalid/dynamic-classic.js", ""], ["/csp-dynamic-module.js", "module"], ["", ""]]) {
        const script = document.createElement("script");
        script.type = type;
        if (src) script.src = src;
        else script.textContent = "window.staticScriptRan=true";
        document.body.append(script);
      }
      fetch("/cdn-cgi/rum", { method: "POST", body: "csp-probe" }).catch(() => {});
    });
    await staticPage.waitForFunction(() => ["https://example.invalid/dynamic-classic.js", location.origin + "/csp-dynamic-module.js", "inline"].every((uri) =>
      window.staticCspFailures.some((failure) => failure.directive === "script-src-elem" && failure.uri === uri)) &&
      window.staticCspFailures.some((failure) => failure.directive === "connect-src" && failure.uri === location.origin + "/cdn-cgi/rum"));
    assert.equal(await staticPage.evaluate(() => window.staticScriptRan), undefined);
  }
  assert.deepEqual(blocked, [], "CSP が通信前に止める。route で止めたものを成功と数えない");
  assert.deepEqual(errors, []);
  assert.equal((await context.cookies()).length, 0);
  assert.ok(requests.every((request) => request.origin === origin && request.method === "GET" && !request.body));
  console.log(product + ": 案内 " + staticPaths.length + " ページ・404・スクリプトと解析 POST の CSP による拒否・同一オリジンの資産への GET のみを確認（全要求 " + requests.length + " 件）");
} finally {
  await browser?.close();
  if (server) await new Promise((ok) => server.close(ok));
}
