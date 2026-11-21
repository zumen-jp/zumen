// 神戸市が配布する JWW の用紙と主な実寸空間を、共通 API で照らす。
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { load } from "../../bindings/js/src/index.js";
import { openProcess } from "../../bindings/js/src/process.js";

const [cli, wasm, ...files] = process.argv.slice(2);
assert(cli && wasm && files.length, "compare-jww.mjs <CLI> <wasm> <公開 JWW> ...");
const inputs = new Map([
  ["d492ca46b236ec731acd1c25f18bdaff37f79c5ea3c2be43aea9f0905d190d47", "W1"],
  ["232003ea92da87751d4066e32d57e6a8886cd28491ca7652ed354c5d2f3c5071", "W2"],
  ["bc35f33c8d9c9ef84525db334bae895e35a6b71e3b5a9d54ee1b8fb9e6749a52", "T1"],
  ["0316a17778fed792b006ee493aa521eddc7498eaf3f9b518e7e6739e7c43f4d4", "T2"],
]);
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const sum = values => Object.values(values).reduce((a, b) => a + b, 0);
const module = await load(await readFile(wasm));
for (const file of files) {
  const bytes = await readFile(file);
  const sample = inputs.get(hash(bytes));
  assert(sample, "対象の公開 JWW を指定してください");
  // 実寸空間を 0.01 mm で近似したシーンは Base64 応答で 64 MiB を超える。
  const native = await openProcess(cli, { v: 1 }, { maxMessageBytes: 256 * 1024 * 1024, timeoutMs: 300_000 });
  const session = module.open();
  try {
    await native.push("plan.jww", bytes);
    session.push("plan.jww", bytes);
    async function call(op, fields) {
      const request = JSON.stringify({ v: 1, op, ...fields });
      const data = Buffer.from(await native.callBytes(request));
      assert(data.equals(Buffer.from(session.callBytes(request))), op);
      return JSON.parse(data);
    }
    await call("open", { input: "plan.jww" });
    const { result } = await call("drawing", { doc: "plan.jww" });
    // 同数なら入力で先の空間を採る。
    const largest = result.spaces.reduce((a, b) => sum(b.elements) > sum(a.elements) ? b : a);
    for (const from of [result.sheets[0].id, largest.id]) {
      for (const options of [{}, { tolerance: 0.01 }]) {
        const scene = await call("scene", { doc: "plan.jww", from, ...options });
        const data = Buffer.from(await native.take(scene.result.blob));
        assert(data.equals(Buffer.from(session.take(scene.result.blob))));
        assert.equal(data.length, scene.blobs[0].size);
        console.log(JSON.stringify({
          sample, from, tolerance: scene.result.tolerance, views: result.sheets[0].views,
          counts: scene.result.counts, bytes: data.length, sha256: hash(data),
          diagnostics: scene.diagnostics.map(({ code, count }) => ({ code, count })),
        }));
      }
    }
  } finally {
    session.close();
    await native.close();
  }
}
