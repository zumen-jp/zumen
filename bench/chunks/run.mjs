// Node 標準ライブラリだけで時間・SHA-256・ネイティブとの完全一致を確認する。
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { cpus, platform, release, arch } from "node:os";
import { resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const out = resolve(process.argv[2]);
const modulePath = resolve(process.argv[3]);
const repeats = Number(process.argv[4] ?? 7);
assert(Number.isInteger(repeats) && repeats >= 3 && repeats <= 99);
const native = JSON.parse(await readFile(resolve(out,"native.json"),"utf8"));
const moduleBytes = await readFile(modulePath);
const { instance } = await WebAssembly.instantiate(moduleBytes, {});
const e = instance.exports;
const names = [...new Set(native.map(r => r.case))];
const policies = [...new Set(native.map(r => r.policy))];
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const fromWasm = (ptr,len) => Buffer.from(new Uint8Array(e.memory.buffer,ptr,len));
const report = () => JSON.parse(fromWasm(e.report_ptr(),e.report_len()).toString("utf8"));
const results = [];
for (const [ci,name] of names.entries()) {
  e.bench_prepare(ci);
  for (const [pi,policy] of policies.entries()) {
    const before = native.find(r => r.case === name && r.policy === policy);
    const expected = await readFile(resolve(out,`${name}-${policy}.bin`));
    e.bench_render(pi);
    e.bench_audit();
    assert.deepEqual(report(),before.metrics);
    assert(expected.equals(fromWasm(e.scene_ptr(),e.scene_len())),"native/wasm scene bytes differ");
    const times = [];
    for(let i=0;i<repeats;i++) {
      const start=performance.now();
      e.bench_render(pi);
      times.push(performance.now()-start);
      e.bench_audit();
      assert.deepEqual(report(),before.metrics);
      assert(expected.equals(fromWasm(e.scene_ptr(),e.scene_len())),"repeated wasm scene bytes differ");
    }
    times.sort((a,b)=>a-b);
    results.push({...before,wasm_median_ms:times[Math.floor(times.length/2)],scene_sha256:sha256(expected)});
  }
}

const samplePaths=["dwg/zumen-layout-r2000.dwg","dwg/zumen-arcdim-r2004_ascii.dxf","dxf/text_alignments.dxf"];
const ledger=await readFile(resolve(root,"samples/ledger.toml"),"utf8");
const inputs=[];
for(const path of samplePaths) {
  const entry=ledger.split("[[samples]]").find(block=>block.includes(`path = "${path}"`));
  assert(entry,"sample is absent from the ledger");
  const bytes=await readFile(resolve(root,"samples",path));
  const hash=sha256(bytes);
  assert.equal(hash,entry.match(/^sha256 = "([0-9a-f]+)"/m)[1]);
  assert.equal(bytes.length,Number(entry.match(/^bytes = (\d+)/m)[1]));
  inputs.push({path:`samples/${path}`,bytes:bytes.length,sha256:hash});
}
const result={
  schema:1,
  measured_at:new Date().toISOString(),
  source_commit:execFileSync("git",["rev-parse","HEAD"],{cwd:root,encoding:"utf8"}).trim(),
  harness_sha256: Object.fromEntries(await Promise.all(["Cargo.toml","Cargo.lock","src/lib.rs","src/main.rs","run.mjs"].map(async path=>[path,sha256(await readFile(new URL(path,import.meta.url)))]))),
  environment:{cpu:cpus()[0].model.trim(),logical_cpus:cpus().length,os:platform(),os_release:release(),arch:arch(),rustc:execFileSync("rustc",["-V"],{cwd:root,encoding:"utf8"}).trim(),cargo:execFileSync("cargo",["-V"],{cwd:root,encoding:"utf8"}).trim(),node:process.version},
  repeats,warmups:1,wasm_bytes:moduleBytes.length,wasm_sha256:sha256(moduleBytes),inputs,
  settings:{tolerance:0.01,origin:"model space",fonts:"none",region:null},
  results,
};
await writeFile(resolve(out,"results.json"),JSON.stringify(result,null,2)+"\n");
console.log(JSON.stringify({cases:names.length,policies:policies.length,repeats,exact_native_wasm:results.length}));
