// 組んだ出力か公開したサイトを開き、全案内ページと 404 の CSP・通信・表示を確かめる。
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
    const path = resolve(folder, entry.name);
    const url = prefix + "/" + entry.name;
    if (entry.isDirectory()) collect(path, url);
    else {
      files.set(url, path);
      if (entry.name === "index.html") files.set(prefix + "/", path);
    }
  }
}
collect(dist);

const missingPath = "/no-such-path-zumen-csp-test/";
const staticCsp = "script-src 'none'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";
const headers = Object.fromEntries(
  readFileSync(resolve(dist, "_headers"), "utf8")
    .split(/\r?\n/)
    .filter((line) => /^\s+\S+:/.test(line))
    .map((line) => {
      const colon = line.indexOf(":");
      return [line.slice(0, colon).trim(), line.slice(colon + 1).trim()];
    }),
);
const mime = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".css": "text/css",
  ".svg": "image/svg+xml",
  ".md": "text/plain; charset=utf-8",
  ".mp4": "video/mp4",
  ".webp": "image/webp",
  ".woff2": "font/woff2",
};

const requests = [];
const errors = [];
const blocked = [];
let server;
let browser;

try {
  let origin = publicOrigin && new URL(publicOrigin).origin;
  if (!origin) {
    server = createServer((request, response) => {
      const url = new URL(request.url, "http://localhost");
      if (request.method !== "GET" || url.search || request.headers["content-length"] || request.headers["transfer-encoding"]) {
        response.writeHead(404).end();
        return;
      }
      if (url.pathname === "/viewer" || url.pathname.startsWith("/viewer/")) {
        response.writeHead(301, { Location: "https://editor.zumen.dev/" }).end();
        return;
      }
      const path = files.get(url.pathname);
      if (!path) {
        // Pages の実配信で観測した 404。_headers の CSP と no-transform がなくても、HTML で拒否する。
        const html = readFileSync(resolve(dist, "404.html"), "utf8").replace(
          "</body>",
          '<script>window.staticScriptRan=true</script><script defer src="https://example.invalid/parser-classic.js"></script><script type="module" src="https://example.invalid/parser-module.js"></script></body>',
        );
        response.writeHead(404, { "Content-Type": mime[".html"], "Cache-Control": "no-store" }).end(html);
        return;
      }
      const body = readFileSync(path);
      const baseHeaders = {
        "Content-Type": mime[extname(path)] ?? "application/octet-stream",
        "Accept-Ranges": "bytes",
        ...headers,
      };
      const range = request.headers.range;
      const match = range && /^bytes=(\d+)-(\d*)$/.exec(range);
      if (match) {
        const start = Number(match[1]);
        const end = match[2] ? Math.min(Number(match[2]), body.length - 1) : body.length - 1;
        if (start < body.length && start <= end) {
          response
            .writeHead(206, {
              ...baseHeaders,
              "Content-Range": `bytes ${start}-${end}/${body.length}`,
              "Content-Length": end - start + 1,
            })
            .end(body.subarray(start, end + 1));
          return;
        }
      }
      response.writeHead(200, { ...baseHeaders, "Content-Length": body.length }).end(body);
    });
    const port = Number(process.env.ZUMEN_TEST_PORT ?? 24189);
    await new Promise((ok, fail) => server.once("error", fail).listen(port, "127.0.0.1", ok));
    origin = `http://127.0.0.1:${port}`;
  }

  const isChromium = ["chromium", "chrome", "msedge"].includes(product);
  const launchOptions = isChromium && product !== "chromium" ? { channel: product } : {};
  browser = await launchers[product].launch(launchOptions);
  const context = await browser.newContext({ viewport: { width: 1360, height: 980 }, serviceWorkers: "block" });
  const allowed = new Set([...files.keys(), missingPath]);
  await context.route("**/*", (route) => {
    const request = route.request();
    const url = new URL(request.url());
    requests.push({ origin: url.origin, path: url.pathname, method: request.method(), body: request.postDataBuffer() !== null });
    if (url.origin !== origin || !allowed.has(url.pathname) || url.search || request.method() !== "GET" || request.postDataBuffer() !== null) {
      blocked.push({ method: request.method(), path: url.origin === origin ? url.pathname : "(external)", type: request.resourceType(), query: Boolean(url.search), body: request.postDataBuffer() !== null });
      return route.abort();
    }
    return route.continue();
  });
  context.on("weberror", () => errors.push("pageerror"));
  context.setDefaultTimeout(30_000);

  // ローカル検証時は /viewer/ が editor.zumen.dev へ 301 転送されることも確かめる。
  if (!publicOrigin) {
    const redirectRes = await fetch(origin + "/viewer/", { redirect: "manual" });
    assert.equal(redirectRes.status, 301);
    assert.equal(redirectRes.headers.get("location"), "https://editor.zumen.dev/");
  }

  const staticPage = await context.newPage();
  await staticPage.addInitScript(() => {
    window.staticCspFailures = [];
    document.addEventListener("securitypolicyviolation", (event) =>
      window.staticCspFailures.push({ directive: event.effectiveDirective, uri: event.blockedURI }),
    );
  });
  staticPage.on("websocket", () => errors.push("websocket"));

  const staticPaths = [...files.keys()].filter((path) => path.endsWith("/"));
  for (const path of [...staticPaths, missingPath]) {
    const result = await staticPage.goto(origin + path);
    assert.equal(result.status(), path === missingPath ? 404 : 200);
    assert.equal(await staticPage.locator('meta[http-equiv="Content-Security-Policy"]').getAttribute("content"), staticCsp);
    if (path !== missingPath) {
      assert.equal(await staticPage.locator("script").count(), 0, "通常の案内ページに script が注入されていない");
      assert.deepEqual(await staticPage.evaluate(() => window.staticCspFailures), []);
      if (path === "/") {
        const navHref = await staticPage.getByRole("link", { name: "図面を試す" }).first().getAttribute("href");
        assert.equal(navHref, "https://editor.zumen.dev/");
      }
    } else {
      await staticPage.getByRole("heading", { name: "ページが見つかりません", exact: true }).waitFor();
      if (!publicOrigin) {
        assert.equal(result.headers()["content-security-policy"], undefined);
        assert.equal(result.headers()["cache-control"], "no-store");
        await staticPage.waitForFunction(
          () => window.staticCspFailures.filter(({ directive }) => directive === "script-src-elem").length === 3,
        );
      }
      console.log(product + ": 404 の HTML の script タグ " + (await staticPage.locator("script").count()) + " 件（注入と通信の拒否を別に確認）");
    }
    // 外部・同一オリジン・インラインの実行を拒否する。DevTools 経由の evaluate 自体は CSP の対象外。
    await staticPage.evaluate(() => {
      for (const [src, type] of [
        ["https://example.invalid/dynamic-classic.js", ""],
        ["/csp-dynamic-module.js", "module"],
        ["", ""],
      ]) {
        const script = document.createElement("script");
        script.type = type;
        if (src) script.src = src;
        else script.textContent = "window.staticScriptRan=true";
        document.body.append(script);
      }
      fetch("/cdn-cgi/rum", { method: "POST", body: "csp-probe" }).catch(() => {});
    });
    await staticPage.waitForFunction(
      () =>
        ["https://example.invalid/dynamic-classic.js", location.origin + "/csp-dynamic-module.js", "inline"].every((uri) =>
          window.staticCspFailures.some((failure) => failure.directive === "script-src-elem" && failure.uri === uri),
        ) &&
        window.staticCspFailures.some(
          (failure) => failure.directive === "connect-src" && failure.uri === location.origin + "/cdn-cgi/rum",
        ),
    );
    assert.equal(await staticPage.evaluate(() => window.staticScriptRan), undefined);
  }

  // 狭い画面での横はみ出し確認
  await staticPage.setViewportSize({ width: 390, height: 900 });
  for (const path of staticPaths) {
    await staticPage.goto(origin + path);
    assert.ok(
      await staticPage.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
      `狭い画面で ${path} の本文が横にはみ出さない`,
    );
  }

  assert.deepEqual(blocked, [], "CSP が通信前に止める。route で止めたものを成功と数えない");
  assert.deepEqual(errors, []);
  assert.equal((await context.cookies()).length, 0);
  assert.ok(requests.every((request) => request.origin === origin && request.method === "GET" && !request.body));
  console.log(
    product +
      ": 案内 " +
      staticPaths.length +
      " ページ・404・editor.zumen.dev リンク・スクリプトと解析 POST の CSP による拒否・同一オリジンの資産への GET のみを確認（全要求 " +
      requests.length +
      " 件）",
  );
} finally {
  await browser?.close();
  if (server) await new Promise((ok) => server.close(ok));
}
