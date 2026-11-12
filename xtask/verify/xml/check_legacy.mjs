// 作業中の checkout を変えず、公開コミットの snapshot から旧文字コードの固定値を確かめる。
import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const commit = "e8d389a3b552b3a9fd2a047e2b7a25f77b1d2cec";
const label = "e8d389a3";
const root = fileURLToPath(new URL("../../../", import.meta.url));
const target = path.resolve(root, process.env.CARGO_TARGET_DIR ?? "target/xml-utf16-portability-468");
const snapshot = path.join(target, `main-${label}`);
const env = { ...process.env, CARGO_TARGET_DIR: path.join(target, `main-${label}-build`) };
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, env, maxBuffer: 32 * 1024 * 1024 });
  if (result.error || result.status !== 0) {
    const detail = `${result.error ?? result.stderr?.toString() ?? result.signal}`.replaceAll(root, "(workspace)");
    throw new Error(`旧コミットの検証が失敗: ${path.basename(command)}: ${detail}`);
  }
  return result.stdout;
}
await mkdir(snapshot, { recursive: true });
const archive = path.join(target, `main-${label}.tar`);
run("git", ["archive", "--format=tar", `--output=${archive}`, commit]);
run("tar", ["-xf", archive, "-C", snapshot]);
const exampleDir = path.join(snapshot, "crates/zumen-landxml/examples");
await mkdir(exampleDir, { recursive: true });
const probe = await readFile(new URL("./legacy_probe.rs", import.meta.url), "utf8");
const main = `\nfn main() -> Result<(), Box<dyn std::error::Error>> { use std::io::Write; let out = snapshot().map_err(std::io::Error::other)?; std::io::stdout().lock().write_all(out.as_bytes())?; Ok(()) }\n`;
await writeFile(path.join(exampleDir, "xml_legacy_probe.rs"), probe + main);
run("cargo", ["build", "--locked", "-p", "zumen-landxml", "--example", "xml_legacy_probe"], snapshot);
const executable = path.join(env.CARGO_TARGET_DIR, "debug/examples/xml_legacy_probe") + (process.platform === "win32" ? ".exe" : "");
const actual = run(executable, [], snapshot);
const expected = new URL("./legacy-e8d389a3.txt", import.meta.url);
if (process.argv.includes("--write")) {
  await writeFile(expected, actual);
} else {
  assert.deepEqual(actual, await readFile(expected), "公開コミットと固定値が違う");
}
console.log(`XML: main ${label} から UTF-8 / Shift_JIS の固定値 ${actual.length} バイトを確認`);
