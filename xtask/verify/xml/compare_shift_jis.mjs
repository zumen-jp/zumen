// 全1バイトと全 lead/trail の組について、native / WASI と JS の decoder を比較する。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { compareShiftJisSignatures } from "../../../bindings/js/test/shift-jis-signatures.mjs";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const target = path.resolve(root, process.env.CARGO_TARGET_DIR ?? "target/xml-shift-jis-824");
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, env: { ...process.env, CARGO_TARGET_DIR: target }, maxBuffer: 32 * 1024 * 1024 });
  if (result.error || result.status !== 0) {
    throw new Error(`${path.basename(command)}: ${result.error ?? result.stderr?.toString().replaceAll(root, "(workspace)")}`);
  }
  return result.stdout;
}
const host = /^host: (.+)$/m.exec(run("rustc", ["-vV"]).toString())?.[1];
assert.ok(host);
for (const name of [host, "wasm32-wasip1"]) {
  run("cargo", ["build", "--locked", "-p", "zumen-xml", "--example", "shift_jis_probe", "--target", name]);
}
const native = path.join(target, host, "debug/examples/shift_jis_probe") + (process.platform === "win32" ? ".exe" : "");
const snapshot = run(native, []);
assert.deepEqual(run(native, []), snapshot);
const wasm = path.join(target, "wasm32-wasip1/debug/examples/shift_jis_probe.wasm");
for (let repeat = 0; repeat < 2; repeat++) {
  assert.deepEqual(run(process.execPath, ["--disable-warning=ExperimentalWarning", "xtask/verify/xml/compare_utf16.mjs", "--wasm", wasm]), snapshot);
}
const count = compareShiftJisSignatures(snapshot.toString());
console.log(`Shift_JIS: native / WASI 各2回と Node.js ${process.version} が一致（${count} 入力）`);

const browserName = process.argv[2];
if (browserName) {
  const { chromium, firefox, webkit } = await import("../../../bindings/js/test/browser/node_modules/playwright/index.mjs");
  const browser = await { chromium, firefox, webkit }[browserName].launch();
  try {
    const page = await browser.newPage();
    await page.route("http://localhost:24191/**", async (route) => {
      const name = new URL(route.request().url()).pathname;
      if (name === "/") { return route.fulfill({ contentType: "text/html", body: "<!doctype html><title>Shift_JIS</title>" }); }
      const allowed = ["/bindings/js/test/shift-jis-signatures.mjs", "/bindings/js/src/modules.js", "/bindings/js/src/constants.js"];
      if (!allowed.includes(name)) { return route.fulfill({ status: 404, body: "" }); }
      return route.fulfill({ contentType: "text/javascript", body: readFileSync(path.join(root, name.slice(1))) });
    });
    await page.goto("http://localhost:24191/");
    const actual = await page.evaluate(async (snapshot) => {
      const { compareShiftJisSignatures } = await import("/bindings/js/test/shift-jis-signatures.mjs");
      return compareShiftJisSignatures(snapshot);
    }, snapshot.toString());
    assert.equal(actual, count);
    console.log(`${browserName} ${browser.version()}: ${count} 入力が一致`);
  } finally {
    await browser.close();
  }
}
