// 決定 0199 の固定した Playwright を表示部品にも使う。外へ通信せず、合成入力だけを読む。
import { appendFileSync, existsSync, readFileSync } from "node:fs";
import { dirname, extname, join, normalize, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, firefox, webkit } from "playwright";

const browsers = { chromium, firefox, webkit };
const [name, wasmPath] = process.argv.slice(2);
if (!browsers[name] || !wasmPath) throw new Error("使い方: node bindings/js/test/browser/view2d.mjs <chromium|firefox|webkit> <zumen_abi1.wasm>");
const root = join(dirname(fileURLToPath(import.meta.url)), "../../../..");
const origin = "http://localhost:24191";
const pdfjs = existsSync(join(root, "xtask/verify/pdfjs/node_modules/pdfjs-dist/build/pdf.mjs"));
if (!pdfjs && process.env.ZUMEN_PDFJS_REQUIRED) throw new Error("PDF の検査には xtask/verify/pdfjs の npm ci が必要です");
const inputs = ["scene", "open_jww", "open_sxf"].map((test) =>
  JSON.parse(readFileSync(join(root, `bindings/conformance/cases/${test}.json`), "utf8")).steps.find((s) => s.push).push);
const browser = await browsers[name].launch();
try {
  const page = await browser.newPage({ hasTouch: true });
  const failures = [];
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== origin) { failures.push(`外への要求: ${url.origin}`); return route.abort(); }
    const path = normalize(decodeURIComponent(url.pathname).slice(1)).split(sep).join("/");
    if (url.pathname === "/") return route.fulfill({ contentType: "text/html", body: '<!doctype html><meta charset="utf-8"><title>view2d test</title><script type="importmap">{"imports":{"@zumen-jp/zumen":"/bindings/js/src/index.js"}}</script>' });
    if (["zumen_abi1.wasm", "target/wasm32-unknown-unknown/release/zumen_abi1.wasm"].includes(path)) return route.fulfill({ contentType: "application/wasm", body: readFileSync(wasmPath) });
    if (!["bindings/js/src/", "packages/view2d/src/", "packages/view2d/test/", "packages/view2d/example/", "xtask/verify/pdfjs/node_modules/pdfjs-dist/"].some((prefix) => path.startsWith(prefix)) && path !== "samples/pdf/pdfium-smask-blend.pdf") return route.fulfill({ status: 404, body: "" });
    const contentType = [".js", ".mjs"].includes(extname(path)) ? "text/javascript" : extname(path) === ".html" ? "text/html" : "application/octet-stream";
    return route.fulfill({ contentType, body: readFileSync(join(root, path)) });
  });
  page.on("pageerror", (error) => failures.push(error.message));
  await page.goto(origin);
  const result = await page.evaluate(async ({ inputs }) => {
    // 本体の下限を Safari 15.4 や Firefox 92 まで上げない。下限の実機の検査の代わりではない。
    const hasOwn = Object.hasOwn, at = Array.prototype.at;
    Object.hasOwn = undefined;
    Array.prototype.at = undefined;
    try {
      const { testView2D } = await import("/packages/view2d/test/browser.mjs");
      return await testView2D("/zumen_abi1.wasm", inputs);
    } finally {
      Object.hasOwn = hasOwn;
      Array.prototype.at = at;
    }
  }, { inputs });
  // 外部の pdf.js の下限と、表示部品自身の下限の検査を分ける。
  if (pdfjs) result.assertions += await page.evaluate(async () => {
    const { testPdfUnderlays } = await import("/packages/view2d/test/pdf.mjs");
    let assertions = 0;
    const assert = (ok, message) => { assertions++; if (!ok) throw new Error(message); };
    const near = (a, b, message) => assert(Math.abs(a - b) <= 1e-8, `${message}: ${a} / ${b}`);
    await testPdfUnderlays({ assert, near });
    return assertions;
  });
  // 実際のポインター捕捉と既定の動作は、合成したイベントだけでは確認できない。
  await page.evaluate(async () => {
    const { View2D } = await import("/packages/view2d/src/index.js");
    const { inputFixture } = await import("/packages/view2d/test/tools.mjs");
    const canvas = document.createElement("canvas");
    canvas.id = "native-input"; canvas.tabIndex = 0; canvas.style.cssText = "display:block;width:200px;height:200px;touch-action:none";
    document.body.append(canvas);
    const viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio: 2, view: { center: { x: 0, y: 0 }, scale: 2, rotation: 0 } });
    const input = inputFixture(); viewer.setScene(input); viewer.setTool("pan");
    const state = window.inputTest = { viewer, canvas, input, detach: viewer.attachInput(), pointerId: null, context: null, cleanup: 0, observed: 0 };
    canvas.addEventListener("pointerdown", (event) => { state.pointerId = event.pointerId; });
    viewer.addTool("custom", {
      onActivate(context) { state.context = context; context.subscribe(() => { state.observed++; }); return () => { state.cleanup++; }; },
      onInput: () => true,
    });
  });
  const native = async (message, fn) => { result.assertions++; if (!await page.evaluate(fn)) throw new Error(message); };
  await page.locator("#native-input").scrollIntoViewIfNeeded();
  const box = await page.locator("#native-input").boundingBox();
  await page.mouse.move(box.x + 100, box.y + 100); await page.mouse.down();
  await native("実イベントで捕捉する", () => inputTest.canvas.hasPointerCapture(inputTest.pointerId));
  await page.mouse.move(box.x + 250, box.y + 100); await page.mouse.up();
  await native("Canvas の外へのドラッグと pointerup", () => inputTest.viewer.getView().center.x === -75 && inputTest.viewer.getState().gesture === null && !inputTest.canvas.hasPointerCapture(inputTest.pointerId));
  await page.locator("#native-input").focus(); await page.keyboard.press("ArrowRight");
  await native("実キーで移動する", () => inputTest.viewer.getView().center.x === -55);
  await page.evaluate(() => inputTest.viewer.setInputOptions({ keyboard: false })); await page.keyboard.press("ArrowRight");
  await native("無効にした実キーで移動しない", () => inputTest.viewer.getView().center.x === -55);
  await page.evaluate(() => inputTest.viewer.setInputOptions({ keyboard: true }));
  await page.mouse.move(box.x + 100, box.y + 100); await page.keyboard.down("Control");
  try { await page.mouse.wheel(0, -100); await page.waitForFunction(() => inputTest.viewer.getView().scale > 2); }
  finally { await page.keyboard.up("Control"); }
  await native("実ホイールで修飾キーと固定点を使う", () => Math.abs(inputTest.viewer.getView().scale - 2 * Math.exp(.2)) < 1e-8 && inputTest.viewer.getView().center.x === -55);
  await page.evaluate(() => {
    const state = inputTest; state.viewer.setScene(null); state.viewer.setTool("select");
    state.beforeFailure = []; state.afterFailure = []; state.failures = []; state.failedNotifications = 0;
    state.first = new Error("native input notification"); state.second = new Error("native recovery notification");
    state.offBefore = state.viewer.subscribe((value) => state.beforeFailure.push(value));
    state.captureError = (event) => { if (event.error === state.first) { event.preventDefault(); state.failures.push(event.error); } };
    window.addEventListener("error", state.captureError);
  });
  await page.mouse.move(box.x + 20, box.y + 20); await page.mouse.down(); await page.mouse.move(box.x + 60, box.y + 60);
  await native("通知の失敗前には実ポインターを捕捉し範囲を描く", () => inputTest.canvas.hasPointerCapture(inputTest.pointerId) && inputTest.canvas.getContext("2d").getImageData(0, 0, 400, 400).data.some((value) => value !== 255));
  await page.evaluate(() => {
    inputTest.offFailure = inputTest.viewer.on("input", ({ state }) => { inputTest.failedNotifications++; throw state.gesture ? inputTest.first : inputTest.second; });
    inputTest.offAfter = inputTest.viewer.subscribe((state) => inputTest.afterFailure.push(state));
  });
  await page.mouse.move(box.x + 70, box.y + 70);
  await native("通知の失敗で捕捉を解放し、後始末の例外より元の例外を返す", () => !inputTest.canvas.hasPointerCapture(inputTest.pointerId) && inputTest.failures.length === 1 && inputTest.failures[0] === inputTest.first && inputTest.failedNotifications === 2);
  await native("通知の失敗後は両方の購読も中断後の状態を持つ", () => {
    const state = inputTest.viewer.getState();
    return state.gesture === null && state.measurement === null && inputTest.beforeFailure[inputTest.beforeFailure.length - 1] === state && inputTest.afterFailure[inputTest.afterFailure.length - 1] === state;
  });
  await native("通知の失敗後は Canvas の範囲も消す", () => inputTest.canvas.getContext("2d").getImageData(0, 0, 400, 400).data.every((value) => value === 255));
  await page.mouse.up();
  await page.evaluate(() => {
    inputTest.offBefore(); inputTest.offAfter(); inputTest.offFailure(); window.removeEventListener("error", inputTest.captureError);
    inputTest.viewer.setScene(inputTest.input);
  });
  for (const reason of ["tool", "cancel", "scene", "options", "detach", "lost", "destroy"]) {
    await page.evaluate(() => { inputTest.viewer.setTool("custom"); inputTest.detach(); inputTest.detach = inputTest.viewer.attachInput(); });
    await page.mouse.move(box.x + 100, box.y + 100); await page.mouse.down();
    await page.mouse.move(box.x + 100.5, box.y + 100);
    await native(`${reason} の前に捕捉する`, () => inputTest.canvas.hasPointerCapture(inputTest.pointerId));
    await page.evaluate((reason) => {
      const { viewer, canvas, pointerId } = inputTest;
      inputTest.active = inputTest.context;
      if (reason === "tool") viewer.setTool("select");
      if (reason === "cancel") viewer.cancelInput();
      if (reason === "scene") viewer.setScene(inputTest.input);
      if (reason === "options") viewer.setInputOptions({ wheel: !viewer.getState().input.wheel });
      if (reason === "detach") inputTest.detach();
      if (reason === "lost") canvas.releasePointerCapture(pointerId);
      if (reason === "destroy") viewer.destroy();
    }, reason);
    if (reason === "lost") await page.mouse.move(box.x + 101, box.y + 100);
    await native(`${reason} で捕捉とツールを片づける`, () => !inputTest.canvas.hasPointerCapture(inputTest.pointerId) && inputTest.active.signal.aborted);
    await page.mouse.up();
  }
  await page.evaluate(() => { inputTest.canvas.remove(); delete window.inputTest; });
  await page.goto(`${origin}/packages/view2d/example/index.html`);
  await page.locator("#file").setInputFiles({ name: "synthetic.dxf", mimeType: "application/octet-stream", buffer: Buffer.from(inputs[0].text) });
  await page.waitForFunction(() => document.querySelector("#status").value.startsWith("描画しました。"));
  await page.locator("#fit").click();
  const line = [0,"SECTION",2,"ENTITIES",0,"LINE",5,"A1",8,"0",10,-40,20,0,11,40,21,0,0,"ENDSEC",0,"EOF",""].join("\n");
  await page.locator("#file").setInputFiles({ name: "source.dxf", mimeType: "application/octet-stream", buffer: Buffer.from(line) });
  await page.waitForFunction(() => document.querySelector("#status").value.startsWith("描画しました。"));
  await page.locator("#drawing").click({ position: { x: 320, y: 200 } });
  await page.waitForFunction(() => document.querySelector("#selection").textContent.includes('"record"'));
  const source = JSON.parse(await page.locator("#selection").textContent());
  if (!Number.isInteger(source.source.record) || source.id !== "space/0/hA1" || source.path.length !== 0) throw new Error("利用例の選択と出所");
  await page.locator("#tool").selectOption("mark");
  await page.locator("#drawing").click({ position: { x: 320, y: 200 } });
  await page.waitForFunction(() => document.querySelector("#mark").textContent.startsWith("しるし:"));
  await page.locator("#drawing").press("Escape");
  if (await page.locator("#mark").textContent() !== "") throw new Error("独自ツールの中断");
  await page.locator("#tool").selectOption("select");
  await page.locator("#drawing").click({ position: { x: 10, y: 10 } });
  if (await page.locator("#selection").textContent() !== "") throw new Error("利用例の選択の解除");
  const drawing = await page.locator("#drawing").boundingBox();
  await page.touchscreen.tap(drawing.x + 320, drawing.y + 200);
  await page.waitForFunction(() => document.querySelector("#selection").textContent.includes('"record"'));
  if (failures.length) throw new Error(failures.join("\n"));
  const summary = `${name} ${browser.version()}: view2d の検査 ${result.assertions} 件、コアから ${result.formats.join("・")} を描画、利用例も成功`;
  const raster = `${name}: 巨大パスの Canvas 単独・View2D と近傍パスの画素: ${JSON.stringify(result.largeStrokeRaster)}`;
  const sat = `${name}: encoded f32 の stroke と独立 SAT の照合: ${JSON.stringify(result.strokeSat)}`;
  const tolerance = `${name}: 許容距離つき点選択と独立距離の照合: ${JSON.stringify(result.strokeTolerance)}`;
  const butt = `${name}: butt の両端と巨大な線の隙間: ${JSON.stringify(result.buttEnds)}`;
  const fills = `${name}: 巨大な閉輪と近傍の独立 Canvas の照合: ${JSON.stringify(result.largeFills)}`;
  const wide = `${name}: 巨大な線幅の cap・join と近傍の独立 Canvas の照合: ${JSON.stringify(result.wideStrokes)}`;
  console.log(summary);
  console.log(raster);
  console.log(sat);
  console.log(tolerance);
  console.log(butt);
  console.log(fills);
  console.log(wide);
  if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, `\n${summary}\n\n${raster}\n\n${sat}\n\n${tolerance}\n\n${butt}\n\n${fills}\n\n${wide}\n`);
} finally { await browser.close(); }
