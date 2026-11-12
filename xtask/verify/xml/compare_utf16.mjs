// XML の原本範囲・意味値・診断・全 Kind の予算を、ネイティブと wasm で比較する。
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
  const target = path.resolve(root, process.env.CARGO_TARGET_DIR ?? "target/xml-utf16-portability-468");
  const env = { ...process.env, CARGO_TARGET_DIR: target };
  function run(command, args) {
    const result = spawnSync(command, args, { cwd: root, env, maxBuffer: 32 * 1024 * 1024 });
    if (result.error || result.status !== 0) {
      const detail = `${result.error ?? result.stderr?.toString() ?? result.signal}`.replaceAll(root, "(workspace)");
      throw new Error(`検証プロセスが失敗: ${path.basename(command)}: ${detail}`);
    }
    return result.stdout;
  }
  const host = /^host: (.+)$/m.exec(run("rustc", ["-vV"]).toString())?.[1];
  assert.ok(host, "rustc の host がない");
  for (const targetName of [host, "wasm32-wasip1"]) {
    run("cargo", ["build", "--locked", "-p", "zumen-landxml", "--example", "utf16_probe", "--target", targetName]);
  }
  const executable = path.join(target, host, "debug/examples/utf16_probe") + (process.platform === "win32" ? ".exe" : "");
  const native = run(executable, []);
  assert.ok(native.length > 0, "probe の出力が空");
  assert.deepEqual(run(executable, []), native, "ネイティブの繰り返しが違う");
  const wasm = path.join(target, "wasm32-wasip1/debug/examples/utf16_probe.wasm");
  for (let repeat = 0; repeat < 2; repeat++) {
    const actual = run(process.execPath, ["--disable-warning=ExperimentalWarning", fileURLToPath(import.meta.url), "--wasm", wasm]);
    assert.deepEqual(actual, native, "ネイティブと wasm の原本範囲・値・診断・予算が違う");
  }
  const python = process.env.ZUMEN_XML_EXPAT_PYTHON ?? (process.platform === "win32" ? "py" : "python3.14");
  const pythonArgs = process.env.ZUMEN_XML_EXPAT_PYTHON || process.platform !== "win32" ? [] : ["-3.14"];
  const external = run(python, [...pythonArgs, "xtask/verify/xml/check_expat.py", "--probe", executable]);
  process.stdout.write(external);
  const legacy = await readFile(new URL("./legacy-e8d389a3.txt", import.meta.url));
  assert.deepEqual(run(executable, ["--legacy"]), legacy, "UTF-8 / Shift_JIS の公開値か予算が main e8d389a3 と違う");
  console.log(`XML UTF-16: native / wasm32-wasip1 が一致（各 2 回、${native.length} バイト）、旧文字コードの固定値も一致`);
}
