import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { createServer } from "node:http";
import { cpus, platform, release, arch } from "node:os";
import { resolve, extname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { loadSync, readScene } from "../../bindings/js/src/index.js";
import { createSceneJob } from "../../bindings/js/examples/worker/job.mjs";

const root = fileURLToPath(new URL("../../", import.meta.url));
const out = resolve(process.argv[2] ?? resolve(root, "bench/chunks/out"));
const wasmPath = resolve(process.argv[3] ?? resolve(root, "target/wasm32-unknown-unknown/release/zumen_abi1.wasm"));
const repeats = Number(process.argv[4] ?? 7);
assert(Number.isInteger(repeats) && repeats >= 3 && repeats <= 99 && repeats % 2 === 1, "repeats must be odd, 3..99");
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const source = JSON.parse(await readFile(resolve(out, "results.json"), "utf8"));
assert.equal(source.repeats, repeats, "producer/consumer repeat counts differ");
assert.equal(source.results.length, 25);
// 公開入力は仮値のみ。合成入力は件数・範囲が効く両端を足す。
const rows = source.results.filter(r => r.policy === "provisional" ||
  r.case === "synthetic-10k" && ["items-1024", "items-16384"].includes(r.policy) ||
  r.case === "synthetic-100k" && ["span-16384", "span-262144"].includes(r.policy));
assert.equal(rows.length, 9);
for (const row of rows) assert.equal(sha256(await readFile(resolve(out, `${row.case}-${row.policy}.bin`))), row.scene_sha256);
const wasm = await readFile(wasmPath), module = await WebAssembly.compile(wasm);
const node_worker = [];
for (const row of rows.filter(r => r.policy === "provisional")) {
  const inputBytes = await readFile(resolve(out, `${row.case}-input.bin`));
  assert.equal(inputBytes.length, row.metrics.input_bytes);
  const ledgerInput = source.inputs.find(r => r.path.endsWith(row.case === "layout-dwg" ? "zumen-layout-r2000.dwg" : row.case === "arcdim-dxf" ? "zumen-arcdim-r2004_ascii.dxf" : row.case === "text-dxf" ? "text_alignments.dxf" : "\0"));
  if (!row.case.startsWith("synthetic")) assert(ledgerInput, "public input metadata missing");
  if (ledgerInput) assert.equal(sha256(inputBytes), ledgerInput.sha256);
  const session = loadSync(module).open();
  let expected, response;
  try {
    session.push("input", inputBytes);
    const opened = session.call({ v: 1, op: "open", input: "input" });
    response = session.call({ v: 1, op: "scene", doc: opened.result.doc, from: "space/0" });
    expected = session.take(response.result.blob);
  } finally { session.close(); }
  const memory_before = process.memoryUsage(), times = [];
  for (let i = -1; i < repeats; i++) {
    const input = Uint8Array.from(inputBytes).buffer;
    const start = performance.now();
    const job = createSceneJob(module, input, { signal: AbortSignal.timeout(120_000) });
    assert.equal(input.byteLength, 0);
    const result = await job.result;
    if (i >= 0) times.push(performance.now() - start);
    assert.equal(job.worker.threadId, -1);
    assert.deepEqual(result.blob, expected);
    assert.deepEqual(result.response, response);
    assert.equal(readScene(result.blob).chunks, response.result.counts.chunks);
  }
  node_worker.push({ case: row.case, input_bytes: inputBytes.length, input_sha256: sha256(inputBytes),
    scene_bytes: expected.length, scene_sha256: sha256(expected), counts: response.result.counts,
    tolerance: response.result.tolerance, job_to_exit_ms: median(times), memory_before, memory_after: process.memoryUsage() });
  console.log(`consumer worker: ${row.case}`);
}

const { chromium } = await import(pathToFileURL(resolve(root, "bindings/js/test/browser/node_modules/playwright/index.mjs")));
const port = 24192, origin = `http://127.0.0.1:${port}`, failures = [];
const files = new Map(rows.map(r => [`/data/${r.case}-${r.policy}.bin`, resolve(out, `${r.case}-${r.policy}.bin`)]));
const server = createServer(async (request, reply) => {
  try {
    const path = new URL(request.url, origin).pathname;
    if (request.method !== "GET") { reply.writeHead(405).end(); return; }
    if (path === "/") { reply.writeHead(200, { "Content-Type": "text/html" }).end('<!doctype html><meta charset="utf-8"><title>scene consumer bench</title><script type="importmap">{"imports":{"@zumen-jp/zumen":"/bindings/js/src/index.js"}}</script>'); return; }
    const sourcePath = /^\/(?:bindings\/js\/src|packages\/view2d\/src)\/[a-z0-9_-]+\.js$/.test(path) || ["/bench/chunks/consumer-page.mjs", "/bench/chunks/consumer-echo.mjs", "/packages/view2d/test/fixtures.mjs"].includes(path);
    const file = files.get(path) ?? (sourcePath ? resolve(root, `.${path}`) : null);
    if (!file) { reply.writeHead(404).end(); return; }
    reply.writeHead(200, { "Content-Type": [".js", ".mjs"].includes(extname(file)) ? "text/javascript" : "application/octet-stream", "Cache-Control": "no-store" }).end(await readFile(file));
  } catch { failures.push("local response failed"); reply.writeHead(500).end(); }
});
await new Promise((resolve, reject) => { server.once("error", reject); server.listen(port, "127.0.0.1", resolve); });
let browser;
const browser_results = [];
try {
  browser = await chromium.launch({ headless: true });
  const browserCdp = await browser.newBrowserCDPSession();
  const system = await browserCdp.send("SystemInfo.getInfo");
  const gpu = { devices: system.gpu.devices.map(({ vendorId, deviceId, vendorString, deviceString, driverVendor, driverVersion }) => ({ vendorId, deviceId, vendorString, deviceString, driverVendor, driverVersion })),
    gl_renderer: system.gpu.auxAttributes?.glRenderer, gl_vendor: system.gpu.auxAttributes?.glVendor,
    gpu_compositing: system.gpu.featureStatus?.gpu_compositing };
  for (const row of rows) {
    const context = await browser.newContext({ viewport: { width: 1000, height: 700 }, deviceScaleFactor: 1 });
    try {
      await context.route("**/*", route => {
        if (new URL(route.request().url()).origin !== origin) { failures.push("external request blocked"); return route.abort(); }
        return route.continue();
      });
      const page = await context.newPage();
      page.on("pageerror", () => failures.push("page error"));
      await page.goto(origin);
      await page.evaluate(async () => { window.bench = await import("/bench/chunks/consumer-page.mjs"); });
      const cdp = await context.newCDPSession(page);
      const heap = async () => { await cdp.send("HeapProfiler.collectGarbage"); return cdp.send("Runtime.getHeapUsage"); };
      const before = await heap();
      const setup = await page.evaluate(({ row, repeats }) => bench.setup(row, repeats), { row, repeats });
      const after_setup = await heap();
      const result = await page.evaluate(() => bench.measure());
      const after_measure = await heap();
      const baseline = browser_results.find(r => r.case === row.case);
      if (baseline) {
        assert.deepEqual(result.drawn, baseline.drawn);
        assert.deepEqual(result.issue_counts, baseline.issue_counts);
      }
      result.provisional_selection_equal = !baseline || result.selection_sha256 === baseline.selection_sha256;
      result.provisional_full_selection_equal = !baseline || result.full_selection_sha256 === baseline.full_selection_sha256;
      delete result.selected_keys; delete result.full_keys;
      await page.evaluate(() => bench.dispose());
      const after_dispose = await heap();
      browser_results.push({ case: row.case, policy: row.policy, scene_sha256: row.scene_sha256, source_metrics: row.metrics, ...setup, ...result,
        retained_heap_observations: { before, after_setup, after_measure, after_dispose } });
      console.log(`consumer browser: ${row.case}/${row.policy}`);
    } finally { await context.close(); }
  }
  assert.deepEqual(failures, []);
  const output = { schema: 1, measured_at: new Date().toISOString(), source_commit: execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim(),
    repeats, warmups: 1, environment: { cpu: cpus()[0]?.model, logical_cpus: cpus().length, os: platform(), release: release(), arch: arch(), node: process.version,
      rustc: execFileSync("rustc", ["--version"], { cwd: root, encoding: "utf8" }).trim(),
      chromium: browser.version(), playwright: JSON.parse(await readFile(resolve(root, "bindings/js/test/browser/node_modules/playwright/package.json"), "utf8")).version,
      headless: true, width: 960, height: 640, pixel_ratio: 1, gpu_completion_measured: false, gpu },
    producer_results_sha256: sha256(await readFile(resolve(out, "results.json"))), wasm_bytes: wasm.length, wasm_sha256: sha256(wasm),
    producer_harness_sha256: source.harness_sha256, producer_inputs: source.inputs, harness_sha256: {},
    validation: { producer_native_wasm_exact_pairs: source.results.length, node_worker_exact_cases: node_worker.length,
      synthetic_selection_exact: browser_results.filter(r => r.independent_selection).every(r => r.independent_selection.exact && r.independent_selection.full_exact),
      policy_selection_exact: browser_results.every(r => r.provisional_selection_equal && r.provisional_full_selection_equal) }, node_worker, browser_results };
  for (const path of ["bench/chunks/consumer.mjs", "bench/chunks/consumer-page.mjs", "bench/chunks/consumer-echo.mjs", "packages/view2d/test/fixtures.mjs", "bindings/js/src/scene.js", "bindings/js/src/index.js", "bindings/js/examples/worker/job.mjs", "bindings/js/examples/worker/worker.mjs", "bindings/js/test/browser/package-lock.json"]) output.harness_sha256[path] = sha256(await readFile(resolve(root, path)));
  await writeFile(resolve(out, "consumer-results.json"), JSON.stringify(output, null, 2) + "\n");
  console.log(JSON.stringify(output.validation));
} finally {
  await browser?.close();
  await new Promise(resolve => server.close(resolve));
}
