// 実際の入口で、偶数の指定と native/wasm の回数の食い違いを検査する。
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, mkdir, readFile, rm, stat } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { after, test } from "node:test";

const folder = fileURLToPath(new URL("./",import.meta.url));
const outputRoot = resolve(folder,"out");
await mkdir(outputRoot,{recursive:true});
const temporary = await mkdtemp(join(outputRoot,"regression-"));
const native = resolve(folder,"target/release",process.platform === "win32" ? "measure.exe" : "measure");
const runner = resolve(folder,"run.mjs");
const missingModule = join(temporary,"missing.wasm");
after(async () => {
  // 消す先は、この workspace の out に今回作ったフォルダーだけ。
  assert.equal(dirname(resolve(temporary)),outputRoot);
  assert(basename(temporary).startsWith("regression-"));
  await rm(temporary,{recursive:true,force:true});
});

test("native rejects an even count before making output",async () => {
  const output = join(temporary,"even-native");
  const result = spawnSync(native,[output,"4"],{encoding:"utf8"});
  assert.notEqual(result.status,0);
  assert(result.stderr.includes("repeats must be odd and between 3 and 99"),"native must reject the repeat count");
  await assert.rejects(stat(output),{code:"ENOENT"});
});

test("wasm runner rejects an even count before reading input",() => {
  const result = spawnSync(process.execPath,[runner,join(temporary,"missing-input"),missingModule,"4"],{encoding:"utf8"});
  assert.notEqual(result.status,0);
  assert(result.stderr.includes("repeats must be an odd integer between 3 and 99"),"wasm runner must reject the repeat count");
});

test("native records its count and the wasm runner rejects a different count",async () => {
  const output = join(temporary,"native-three");
  const nativeResult = spawnSync(native,[output,"3"],{encoding:"utf8"});
  assert.equal(nativeResult.status,0,"native measurement with three repeats must succeed");
  const metadata = JSON.parse(await readFile(join(output,"native.json"),"utf8"));
  assert.equal(metadata.schema,1);
  assert.equal(metadata.repeats,3);
  assert.equal(metadata.warmups,1);
  assert.equal(metadata.results.length,25);
  const result = spawnSync(process.execPath,[runner,output,missingModule,"7"],{encoding:"utf8"});
  assert.notEqual(result.status,0);
  assert(result.stderr.includes("native/wasm repeat counts differ"),"wasm runner must reject different repeat counts before reading the module");
});
