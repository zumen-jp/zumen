// Windows で全図を描き、対象ブラウザーの GPU プロセスと同じ物理 GPU のメモリを測る。
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { chromium } from "../../bindings/js/test/browser/node_modules/playwright/index.mjs";
import { chromiumLaunchOptions } from "../../bindings/js/test/browser/launch.mjs";

const dir = process.argv[2];
assert(process.platform === "win32" && dir, "measure-sfc-gpu-memory.mjs <シーンの保存先>");
const root = fileURLToPath(new URL("../../", import.meta.url));
const sampler = fileURLToPath(new URL("sample-gpu-memory.ps1", import.meta.url));
const origin = "http://127.0.0.1:24196";

for (const name of ["1-sheet", "1-space", "2-sheet", "2-space"]) {
  const browser = await chromium.launch(chromiumLaunchOptions(chromium));
  let observer;
  try {
    const page = await browser.newPage();
    await page.route(origin + "/**", async route => {
      const path = new URL(route.request().url()).pathname;
      if (path === "/") return route.fulfill({ contentType: "text/html", body:
        '<script type="importmap">{"imports":{"@zumen-jp/zumen":"/bindings/js/src/index.js"}}</script>' });
      if (/^\/(?:bindings\/js\/src|packages\/view2d\/src)\/[a-z0-9_-]+\.js$/.test(path)) {
        return route.fulfill({ contentType: "application/javascript", body: await readFile(resolve(root, "." + path)) });
      }
      if (path === "/scene.json" || path === "/scene.bin") {
        return route.fulfill({ body: await readFile(resolve(dir, name + path.slice(6))) });
      }
      return route.fulfill({ status: 404 });
    });
    await page.goto(origin);
    await page.evaluate(async () => {
      const { readScene } = await import("/bindings/js/src/index.js");
      const { View2D } = await import("/packages/view2d/src/index.js");
      const result = await (await fetch("/scene.json")).json();
      const scene = readScene(await (await fetch("/scene.bin")).arrayBuffer());
      const adapter = await navigator.gpu.requestAdapter();
      const device = await adapter.requestDevice();
      const canvas = document.body.appendChild(document.createElement("canvas"));
      const view = await View2D.create(canvas, { device, width: 960, height: 640, pixelRatio: 1,
        limits: { issues: result.counts.lines + result.counts.texts } });
      view.fitBounds(result.extent, 32);
      window.drawWholeMap = async () => {
        device.pushErrorScope("out-of-memory");
        device.pushErrorScope("validation");
        view.setScene({ result, scene, document: { doc: "map", format: "sxf", hash: "scene:map" } });
        const snapshots = [];
        for (let i = 0; i < 2; i++) {
          view.render();
          await device.queue.onSubmittedWorkDone();
          const { pixels } = await view.snapshot();
          let nonwhite = 0;
          for (let j = 0; j < pixels.length; j += 4) {
            if (pixels[j] !== 255 || pixels[j + 1] !== 255 || pixels[j + 2] !== 255) nonwhite++;
          }
          const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", pixels));
          snapshots.push({ nonwhite, hash: Array.from(hash, b => b.toString(16).padStart(2, "0")).join("") });
        }
        const validation = await device.popErrorScope(), memory = await device.popErrorScope();
        if (validation || memory) throw new Error((validation ?? memory).message);
        return snapshots;
      };
      window.disposeMap = () => { view.destroy(); device.destroy(); canvas.remove(); };
    });
    const cdp = await browser.newBrowserCDPSession();
    const { gpu } = await cdp.send("SystemInfo.getInfo");
    const gpuIds = async () => (await cdp.send("SystemInfo.getProcessInfo")).processInfo
      .filter(row => row.type.toLowerCase() === "gpu").map(row => row.id);
    const ids = await gpuIds();
    assert.equal(ids.length, 1, "GPU プロセスを特定できません");
    observer = spawn("pwsh", ["-NoProfile", "-File", sampler, "-GpuProcessId", String(ids[0])],
      { windowsHide: true, stdio: ["ignore", "pipe", "inherit"] });
    const reader = createInterface({ input: observer.stdout })[Symbol.asyncIterator]();
    const samples = [];
    async function sample() {
      const next = await reader.next();
      assert(!next.done, "カウンターの採取が中断しました");
      samples.push(JSON.parse(next.value));
      return samples.at(-1).time;
    }
    await sample();
    await sample();
    const before = samples.length - 1;
    const [first, second] = await page.evaluate(() => window.drawWholeMap());
    assert(first.nonwhite > 0);
    assert.deepEqual(first, second);
    const heldUntil = Date.now() + 1000;
    while (await sample() < heldUntil) { /* 破棄する前の標本を得る。 */ }
    const held = samples.length - 1;
    await page.evaluate(() => window.disposeMap());
    const afterUntil = Date.now() + 1000;
    while (await sample() < afterUntil) { /* 破棄した後の標本を得る。 */ }
    assert.deepEqual(await gpuIds(), ids, "GPU プロセスが途中で変わりました");
    // 複数のアダプターに割り当てられたときに別々の GPU の最大値を混ぜない。
    const adapters = new Set(samples.flatMap(s => s.rows.filter(r => r.processDedicated || r.processShared).map(r => r.adapter)));
    assert.equal(adapters.size, 1, "GPU プロセスが複数の物理 GPU を使用しています");
    const [luid] = adapters;
    const rows = samples.map(s => s.rows.find(r => r.adapter === luid));
    const max = {};
    for (const key of ["processDedicated", "processShared", "adapterDedicated", "adapterShared"]) {
      assert(rows.every(r => Number.isSafeInteger(r?.[key])));
      max[key] = Math.max(...rows.map(r => r[key]));
    }
    const gaps = samples.slice(1).map((s, i) => s.time - samples[i].time);
    console.log(JSON.stringify({ name, chromium: browser.version(), renderer: gpu.auxAttributes.glRenderer, luid, samples: samples.length,
      minIntervalMs: Math.min(...gaps), maxIntervalMs: Math.max(...gaps), before: rows[before], max,
      held: rows[held], after: rows.at(-1), ...first }));
  } finally {
    observer?.kill();
    await browser.close();
  }
}
