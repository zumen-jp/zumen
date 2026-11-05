// 決定 0199 の固定した Playwright を表示部品にも使う。外へ通信せず、合成入力だけを読む。
import { appendFileSync, readFileSync } from "node:fs";
import { dirname, extname, join, normalize, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "playwright";

const browsers = { chromium, firefox, webkit };
const [name, wasmPath] = process.argv.slice(2);
if (!browsers[name] || !wasmPath) throw new Error("使い方: node bindings/js/test/browser/view2d.mjs <chromium|firefox|webkit> <zumen_abi1.wasm>");
const root = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
const origin = "http://localhost:24191";
const inputs = ["scene", "open_jww", "open_sxf"].map((test) =>
  JSON.parse(readFileSync(join(root, `bindings/conformance/cases/${test}.json`), "utf8")).steps.find((s) => s.push).push);
const browser = await browsers[name].launch();
try {
  const page = await browser.newPage();
  const failures = [];
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin) { failures.push(`外への要求: ${url.origin}`); return route.abort(); }
    const path = normalize(decodeURIComponent(url.pathname).slice(1)).split(sep).join("/");
    if (url.pathname === "/") return route.fulfill({ contentType: "text/html", body: '<!doctype html><meta charset="utf-8"><title>view2d test</title><script type="importmap">{"imports":{"@zumen-jp/zumen":"/bindings/js/src/index.js"}}</script>' });
    if (["zumen_abi1.wasm", "target/wasm32-unknown-unknown/release/zumen_abi1.wasm"].includes(path)) return route.fulfill({ contentType: "application/wasm", body: readFileSync(wasmPath) });
    if (!["bindings/js/src/", "packages/view2d/src/", "packages/view2d/test/", "packages/view2d/example/"].some((prefix) => path.startsWith(prefix))) return route.fulfill({ status: 404, body: "" });
    const contentType = [".js", ".mjs"].includes(extname(path)) ? "text/javascript" : extname(path) === ".html" ? "text/html" : "application/octet-stream";
    return route.fulfill({ contentType, body: readFileSync(join(root, path)) });
  });
  page.on("pageerror", (error) => failures.push(error.message));
  await page.goto(origin);
  const result = await page.evaluate(async ({ inputs }) => {
    const { testView2D } = await import("/packages/view2d/test/browser.mjs");
    return testView2D("/zumen_abi1.wasm", inputs);
  }, { inputs });
  await page.goto(`${origin}/packages/view2d/example/index.html`);
  await page.locator("#file").setInputFiles({ name: "synthetic.dxf", mimeType: "application/octet-stream", buffer: Buffer.from(inputs[0].text) });
  await page.waitForFunction(() => document.querySelector("#status").value.startsWith("描画しました。"));
  await page.locator("#fit").click();
  if (failures.length) throw new Error(failures.join("\n"));
  const summary = `${name} ${browser.version()}: view2d の検査 ${result.assertions} 件、コアから ${result.formats.join("・")} を描画、利用例も成功`;
  console.log(summary);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `\n${summary}\n`);
} finally { await browser.close(); }
