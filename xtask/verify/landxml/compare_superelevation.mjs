// 合成入力の意味値・出所・診断・全 Kind の予算を、ネイティブと wasm で比較する。
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

if (process.argv[2] === "--wasm") {
  const { WASI } = await import("node:wasi");
  const wasi = new WASI({ version: "preview1", args: [], env: {}, preopens: {} });
  const module = await WebAssembly.compile(await readFile(process.argv[3]));
  const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
  process.exitCode = wasi.start(instance);
} else {
  const root = fileURLToPath(new URL("../../../", import.meta.url));
  const target = path.resolve(root, process.env.CARGO_TARGET_DIR ?? "target/landxml-superelevation-contract");
  const env = { ...process.env, CARGO_TARGET_DIR: target };
  function run(command, args) {
    const result = spawnSync(command, args, {
      cwd: root, env, maxBuffer: 16 * 1024 * 1024,
    });
    if (result.error || result.status !== 0) {
      throw new Error(`${command} ${args.join(" ")}: ${result.error ?? result.stderr?.toString() ?? result.signal}`);
    }
    return result.stdout;
  }
  const host = /^host: (.+)$/m.exec(run("rustc", ["-vV"]).toString())?.[1];
  assert.ok(host, "rustc の host がない");
  for (const targetName of [host, "wasm32-wasip1"]) {
    run("cargo", ["build", "--locked", "-p", "zumen-landxml", "--example", "superelevation_probe", "--target", targetName]);
  }
  const executable = path.join(target, host, "debug/examples/superelevation_probe") + (process.platform === "win32" ? ".exe" : "");
  const native = run(executable, []);
  assert.ok(native.length > 0, "probe の出力が空");
  assert.deepEqual(run(executable, []), native, "ネイティブの繰り返しが違う");
  const wasm = path.join(target, "wasm32-wasip1/debug/examples/superelevation_probe.wasm");
  for (let repeat = 0; repeat < 2; repeat++) {
    const actual = run(process.execPath, ["--disable-warning=ExperimentalWarning", fileURLToPath(import.meta.url), "--wasm", wasm]);
    assert.deepEqual(actual, native, "ネイティブと wasm のビット列・診断・予算が違う");
  }
  console.log(`J-LandXML Superelevation: native / wasm32-wasip1 が一致（各 2 回、${native.length} バイト）`);
}
