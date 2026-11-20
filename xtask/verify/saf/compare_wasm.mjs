// make.py が検査した8組をネイティブと wasm で編集し、応答・保存・図面モデルを照合する。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { load } from "../../../bindings/js/src/index.js";
import { openProcess } from "../../../bindings/js/src/process.js";

const [kit, cli, wasm] = process.argv.slice(2);
if (!kit || !cli || !wasm) {
  throw new Error("使い方: node compare_wasm.mjs <検証キット> <CLI> <図面用の wasm>");
}
const module = await load(readFileSync(wasm));
const manifest = JSON.parse(readFileSync(join(kit, "manifest.json"), "utf8"));
assert.equal(manifest.cases.length, 8);

async function compare(folder, kind) {
  const read = (name) => readFileSync(join(folder, name));
  const input = `input.${kind}`;
  const edited = `edited.${kind}`;
  const repeat = `repeat.${kind}`;
  const records = JSON.parse(read("requests.json"));
  assert.deepEqual(records.map(({ request }) => request.op), ["open", "lift", "apply", "open", "lift", "open", "apply"]);
  const native = await openProcess(cli, { v: 1 });
  const session = module.open();
  try {
    for (const { request } of records) {
      if (request.op === "open") {
        assert.ok([input, edited, repeat].includes(request.input));
        const data = read(request.input === repeat ? input : request.input);
        await native.push(request.input, data);
        session.push(request.input, data);
      }
      const json = JSON.stringify({ v: 1, ...request });
      const reply = Buffer.from(await native.callBytes(json));
      assert.deepEqual(Buffer.from(session.callBytes(json)), reply, `${folder}: ${request.op}`);
      const { result, blobs, diagnostics } = JSON.parse(reply);
      assert.ok(result);
      assert.deepEqual(diagnostics, []);
      if (request.op === "open") {
        assert.equal(blobs.length, 0);
        continue;
      }
      assert.equal(blobs.length, 1);
      const bytes = Buffer.from(await native.take(blobs[0].id));
      assert.deepEqual(Buffer.from(session.take(blobs[0].id)), bytes);
      let expected = edited;
      if (request.op === "lift") {
        assert.ok([input, edited].includes(request.doc));
        expected = request.doc === input ? "input.zmdr" : "edited.zmdr";
      }
      assert.deepEqual(bytes, read(expected), `${folder}: ${expected}`);
    }
  } finally {
    session.close();
    await native.close();
  }
}

for (const kind of ["sfz", "p2z"]) {
  for (const naming of ["stem", "full"]) {
    for (const compression of [0, 8]) {
      const name = `${kind}-${naming}-${compression}`;
      await compare(join(kit, name), kind);
      console.log(`${name}: 応答7件・図面モデル2件・編集出力2件が一致`);
    }
  }
}
