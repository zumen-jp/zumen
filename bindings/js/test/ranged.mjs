// open の needs に包みが答え、入力の範囲を読んで渡すことを確かめる（決定 0232・#474）。
// node bindings/js/test/ranged.mjs <wasm>
//
// 適合試験の 3 点の LAS（形式 0）の頭と VLR に点を足し、全体を渡したときと、初めの部分から範囲で渡したときの
// pointcloud の応答を比べる。Python の側（bindings/python/tests/ranged.py）と同じ入力を作る。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";
import { Zumen, ZumenError, openRanged } from "../src/index.js";

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) throw new Error("wasm のパスが必要");
const module = new WebAssembly.Module(readFileSync(wasmPath));
const zumen = new Zumen(new WebAssembly.Instance(module, {}));

/** 適合試験の LAS に、点を count 個まで足したもの */
function sampleLas(count) {
  const cases = JSON.parse(readFileSync(fileURLToPath(new URL("../../conformance/cases/pointcloud.json", import.meta.url)), "utf8"));
  const step = cases.steps.find((s) => s.push?.name === "b.las#0-365");
  const base = Uint8Array.from(Buffer.from(step.push.hex, "hex"));
  const view = new DataView(base.buffer);
  const offset = view.getUint32(96, true), length = view.getUint16(105, true);
  assert.equal(length, 20);
  const out = new Uint8Array(offset + count * length);
  out.set(base.subarray(0, offset));
  const data = new DataView(out.buffer);
  data.setUint32(107, count, true);
  data.setUint32(111, count, true);
  for (let i = 1; i < 5; i++) data.setUint32(111 + 4 * i, 0, true);
  let low = [Infinity, Infinity, Infinity], high = [-Infinity, -Infinity, -Infinity];
  const scale = [0, 1, 2].map((i) => data.getFloat64(131 + 8 * i, true));
  const shift = [0, 1, 2].map((i) => data.getFloat64(155 + 8 * i, true));
  for (let i = 0; i < count; i++) {
    const at = offset + i * length;
    // 決まった並びの整数座標。範囲の継ぎ目をまたぐ点も出る
    const xyz = [(i * 37) % 1000, (i * 53) % 1000, (i * 11) % 300];
    xyz.forEach((value, k) => {
      data.setInt32(at + 4 * k, value, true);
      const real = value * scale[k] + shift[k];
      low[k] = Math.min(low[k], real);
      high[k] = Math.max(high[k], real);
    });
    data.setUint16(at + 12, i % 65536, true);
    out[at + 14] = 0x09;
    out[at + 15] = 2 + (i % 3);
  }
  [0, 1, 2].forEach((k) => {
    data.setFloat64(179 + 16 * k, high[k], true);
    data.setFloat64(187 + 16 * k, low[k], true);
  });
  return out;
}

/** 開いた文書の点を、すべて返させる */
function points(session, doc, count) {
  const response = session.call({ v: 1, op: "pointcloud", doc, limit: count });
  const blobs = (response.blobs ?? []).map((blob) => [...session.take(blob)]);
  return { result: response.result, blobs };
}

const COUNT = 3000;
const las = sampleLas(COUNT);
const whole = zumen.open();
whole.push("a.las", las);
assert.equal(whole.call({ v: 1, op: "open", input: "a.las" }).result.format, "las");
const expected = points(whole, "a.las", COUNT);
assert.equal(expected.result.points, COUNT);
whole.close();

/** 範囲で開き、読んだ範囲と結果を返す */
async function viaRanges(source, options, request = {}) {
  const session = zumen.open();
  try {
    const response = await openRanged(session, { v: 1, input: "a.las", ...request }, source, options);
    assert.deepEqual(response.result.needs ?? [], []);
    const got = points(session, "a.las", COUNT);
    // 読み終えた範囲は捨ててある。名指ししたものがなければ close は失敗する
    for (const range of response.result.done ?? []) {
      assert.throws(() => session.call({ v: 1, op: "close", inputs: [range.input] }), ZumenError, range.input);
    }
    return got;
  } finally { session.close(); }
}

// 関数の読み口。初めの部分の後は、コアが求めた範囲だけを読む
const reads = [];
const read = (start, end) => { reads.push([start, end]); return las.slice(start, end); };
assert.deepEqual(await viaRanges(read, { size: las.length, head: 512 }), expected);
assert.deepEqual(reads[0], [0, 512]);
assert.ok(reads.length >= 2, "needs に答えて範囲を読む");
assert.ok(reads.slice(1).every(([start, end]) => end > start && end <= las.length));

// Blob（File）の読み口
assert.deepEqual(await viaRanges(new Blob([las]), { head: 1024 }), expected);
// 全体が初めの部分に入るときは、範囲を求めない
assert.deepEqual(await viaRanges(new Blob([las]), { head: las.length + 1 }), expected);

// URL の読み口。Range に 206 で答える
let served = 0;
const server = createServer((request, response) => {
  served++;
  const match = /^bytes=(\d+)-(\d+)$/.exec(request.headers.range ?? "");
  if (!match) { response.writeHead(200, { "Content-Length": las.length }); response.end(las); return; }
  const start = Number(match[1]), end = Math.min(Number(match[2]) + 1, las.length);
  response.writeHead(206, { "Content-Range": `bytes ${start}-${end - 1}/${las.length}`, "Content-Length": end - start });
  response.end(las.subarray(start, end));
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
try {
  const url = `http://127.0.0.1:${server.address().port}/a.las`;
  assert.deepEqual(await viaRanges(url, { head: 700 }), expected);
  assert.ok(served >= 2);
} finally { server.close(); }

// 予算の input_bytes を超える範囲は読まない。失敗したら、渡した入力を捨てる
{
  const session = zumen.open();
  const before = reads.length;
  await assert.rejects(openRanged(session, { v: 1, input: "a.las", budget: { input_bytes: 600 } }, read, { size: las.length, head: 512 }), /input_bytes/);
  assert.equal(reads.length, before + 1, "初めの部分のほかは読まない");
  assert.throws(() => session.call({ v: 1, op: "close", inputs: ["a.las"] }), ZumenError, "初めの部分も捨てた");
  session.push("a.las", las);
  assert.equal(session.call({ v: 1, op: "open", input: "a.las" }).result.format, "las", "同じ名前で渡し直せる");
  session.close();
}
// 読み口が短いバイト列を返したら止め、渡した範囲を捨てる
{
  const session = zumen.open();
  let pushedName;
  const short = (start, end) => start === 0 ? las.slice(0, end) : las.slice(start, end - 1);
  const push = session.push.bind(session);
  session.push = (name, data) => { if (name.includes("#")) pushedName = name; push(name, data); };
  await assert.rejects(openRanged(session, { v: 1, input: "a.las" }, short, { size: las.length, head: 512 }), /長さ/);
  assert.equal(pushedName, undefined, "短い範囲は渡さない");
  assert.throws(() => session.call({ v: 1, op: "close", inputs: ["a.las"] }), ZumenError);
  session.close();
}
// 範囲で読めない入力は、初めの部分を全体として扱う
{
  const session = zumen.open();
  const dxf = new TextEncoder().encode("0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n1\n21\n2\n0\nENDSEC\n0\nEOF\n");
  const response = await openRanged(session, { v: 1, input: "a.dxf" }, new Blob([dxf]));
  assert.equal(response.result.format, "dxf");
  session.close();
}
console.log(`open の needs に、関数・Blob・URL の読み口で答えた（${COUNT} 点、範囲の読み出し ${reads.length - 1} 回）`);
