// 同じ DXF をネイティブと wasm で変換し、応答と P21 をバイト単位で照らす。
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { load, ZumenError } from "../../../bindings/js/src/index.js";
import { openProcess } from "../../../bindings/js/src/process.js";

const [folder, cli, wasm] = process.argv.slice(2);
const core = await load(readFileSync(wasm));

async function response(session, request) {
  try {
    return ["ok", Buffer.from(await session.callBytes(JSON.stringify({ v: 1, ...request })))];
  } catch (error) {
    if (!(error instanceof ZumenError) || !error.bytes) throw error;
    return [error.status, Buffer.from(error.bytes)];
  }
}

for (const name of ["measured", "override"]) {
  const native = await openProcess(cli, { v: 1 });
  const web = core.open();
  try {
    const bytes = readFileSync(join(folder, name + ".dxf"));
    const input = name + ".dxf";
    await native.push(input, bytes);
    web.push(input, bytes);
    async function call(request, expected = "ok") {
      const reply = await response(native, request);
      assert.deepEqual(await response(web, request), reply, name + " " + request.op);
      assert.equal(reply[0], expected, request.op);
      return JSON.parse(reply[1]);
    }
    await call({ op: "open", input });
    const source = (await call({ op: "drawing", doc: input })).result;
    assert.equal(source.spaces[0].unit, "millimeter");
    assert.deepEqual(source.sheets[0].view_details[0].matrix, [0.1, 0, 70, 0, 0.1, 90]);
    const { items } = (await call({ op: "entities", doc: input, fields: ["kind", "id", "geometry", "source"] })).result;
    assert.equal(items.length, 1);
    const dimension = items[0];
    assert.equal(dimension.kind, "dimension");
    assert.deepEqual(dimension.geometry.first, [0, 0]);
    assert.deepEqual(dimension.geometry.second, [1000, 0]);
    assert.equal(dimension.geometry.measurement, 1000);

    const target = { doc: input, format: "sxf", to: "plan" };
    const estimate = await call({ op: "convert_estimate", ...target, limit: 1000 });
    const full = estimate.result.estimate;
    assert.equal(full.details.length, full.total);
    const changes = full.details.filter((row) => row.kind === "dimension");
    assert.equal(changes.length, 1);
    assert.equal(changes[0].fidelity, "approximated");
    assert.equal(changes[0].output_elements, 1);
    assert.deepEqual(changes[0].source, dimension.source);
    assert.ok(changes[0].reasons.includes("dimension_appearance"));
    assert.ok(changes[0].reasons.includes("dimension_terminators"));
    assert.equal(changes[0].reasons.includes("dimension_label_fixed"), name === "measured");
    const terminators = full.details.filter((row) => row.kind === "dimension_terminator");
    assert.deepEqual(terminators.map((row) => row.reasons).sort(), [["first"], ["second"]]);
    for (const row of terminators) {
      assert.equal(row.fidelity, "dropped");
      assert.equal(row.output_elements, 0);
      assert.deepEqual(row.source, dimension.source);
    }
    const totals = full.counts.filter((row) => ["dimension", "dimension_terminator"].includes(row.kind));
    assert.deepEqual(totals, [
      { kind: "dimension", fidelity: "approximated", count: 1, output_elements: 1 },
      { kind: "dimension_terminator", fidelity: "dropped", count: 2, output_elements: 0 },
    ]);
    const failed = await call({ op: "convert", ...target, budget: { model_bytes: 0 } }, "failed");
    assert.ok(failed.diagnostics.some(({ code }) => code === "budget.model-bytes.exceeded"));
    assert.deepEqual(failed.blobs, []);
    assert.equal(failed.result, undefined);
    const converted = await call({ op: "convert", ...target });
    assert.equal(converted.result.elements, 1);
    assert.equal(converted.result.elements, full.elements);
    assert.equal(converted.result.hash, full.hash);
    assert.deepEqual(converted.diagnostics, estimate.diagnostics);
    const dropped = converted.diagnostics.filter((row) => row.params?.kind === "dimension_terminator");
    assert.equal(dropped.length, 1);
    assert.equal(dropped[0].code, "convert.element.unsupported");
    assert.equal(dropped[0].count, 2);
    const at = { record: dimension.source.record };
    assert.deepEqual(dropped[0].at, [at, at]);
    for (const reason of ["dimension_terminators", "dimension_appearance"]) {
      const warning = converted.diagnostics.find((row) => row.params?.kind === "dimension" && row.params.reason === reason);
      assert.equal(warning?.code, "convert.item.approximated");
      assert.equal(warning.count, 1);
      assert.deepEqual(warning.at, [at]);
    }
    const blob = converted.blobs[0].id;
    const p21 = Buffer.from(await native.take(blob));
    assert.deepEqual(Buffer.from(web.take(blob)), p21);
    writeFileSync(join(folder, name + ".p21"), p21, { flag: "wx" });
    writeFileSync(join(folder, name + ".json"), JSON.stringify({ source: dimension, estimate, converted }, null, 2), { flag: "wx" });
  } finally {
    web.close();
    await native.close();
  }
}
console.log("native/wasm: 2 conversions, matching responses and P21 bytes");
