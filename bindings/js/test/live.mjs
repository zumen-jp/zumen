// node bindings/js/test/live.mjs <zumen の実行ファイル>
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { LiveProcess, ProcessError, ZumenError, openLiveProcess, openProcess } from "../src/node.js";
import { displayPacket, responseLength } from "../src/live-wire.js";

const executable = process.argv[2];
const fixture = fileURLToPath(new URL("./fixtures/live-child.mjs", import.meta.url));
const transport = (code) => (error) => error instanceof ProcessError && error.code === code;
async function fake(mode, limits = {}) {
  const child = spawn(process.execPath, [fixture, mode], { stdio: ["pipe", "pipe", "ignore"], windowsHide: true });
  const live = await LiveProcess.create(child, { timeoutMs: 2000, maxMessageBytes: 1024 * 1024, ...limits });
  try {
    await live.open(Uint8Array.of(0, 255, 254), { display: false });
    return { live, child };
  } catch (error) { await live.close(); throw error; }
}
const move = [1, 0, 5, 0, 1, -2];
function dxf(count = 130) {
  let text = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n";
  for (let i = 0; i < count; i++) text += `0\nLINE\n5\n${(16 + i).toString(16)}\n8\n0\n10\n${i * 2}\n20\n0\n11\n${i * 2 + 1}\n21\n0\n`;
  return Buffer.from(text + "0\nENDSEC\n0\nEOF\n");
}
function mirror(reply, old) {
  assert.equal(reply.status, "ok");
  assert.equal(reply.displayStatus, "ok");
  assert(reply.display);
  if (!reply.display.full) assert.equal(reply.base, old.revision);
  return { revision: reply.revision, columns: reply.display.columns.map((column) => {
    const out = Buffer.alloc(column.rows * column.stride);
    if (!reply.display.full) old.columns[column.id].copy(out, 0, 0, out.length);
    for (const block of column.blocks()) out.set(block.bytes, block.start * column.stride);
    return out;
  }) };
}

test("実プロセスの 64 行差分・古い版・保存・undo / redo", { skip: !executable, timeout: 30000 }, async () => {
  const live = await openLiveProcess(executable);
  try {
    const input = dxf();
    const opened = await live.open(input);
    assert.equal(opened.result, 130n);
    assert.equal(opened.revision, 0n);
    const initial = mirror(opened);
    live.acknowledge(opened.revision);
    const target = await live.target(65, opened.revision);
    assert.equal(target.target.id, "space/0/h51");
    assert.match(target.target.hash, /^v2:[0-9a-f]{32}$/);
    const layers = await live.layers(opened.revision);
    assert.equal(layers.result, 1n);
    assert.equal(layers.layers[0].name, "0");
    assert.equal((await live.layers(opened.revision + 1n)).status, "stale");
    const order = await live.order(opened.revision);
    assert.deepEqual([...order.order.rows()], Array.from({ length: 130 }, (_, i) => i));
    const changed = await live.transform({ revision: opened.revision, rows: [65], matrix: move });
    assert.equal(changed.revision, 1n);
    assert.equal(changed.result, 1n);
    assert.deepEqual([...changed.display.columns[0].blocks()].map(({ start, rows }) => [start, rows]), [[64, 64]]);
    const updated = mirror(changed, initial);
    assert.notDeepEqual(updated.columns[0], initial.columns[0]);
    const old = await live.transform({ revision: 0n, rows: [65], matrix: move });
    assert.equal(old.status, "stale");
    assert.equal(old.revision, 1n);
    const repeat = await live.display();
    assert.deepEqual(repeat.data, changed.data);
    live.acknowledge(repeat.revision);
    const empty = await live.display();
    assert(empty.display.columns.every((column) => [...column.blocks()].length === 0));
    const saved = await live.save();
    assert.notDeepEqual(saved.data, input);
    const undone = await live.undo(live.revision);
    assert.deepEqual(mirror(undone, updated).columns, initial.columns);
    assert.deepEqual((await live.save()).data, input);
    live.acknowledge(undone.revision);
    const redone = await live.redo(live.revision);
    assert.deepEqual(mirror(redone, { ...initial, revision: undone.revision }).columns, updated.columns);
    assert.deepEqual((await live.save()).data, saved.data);
    assert.equal((await live.open(input)).status, "alreadyOpen");
    const restyled = await live.restyle({ revision: live.revision, rows: [3], color: { code: { system: "aci", value: 3 } }, lineTypeScale: 2 });
    assert.equal(restyled.status, "ok");
    assert.equal(restyled.result, 1n);
    assert.notEqual(restyled.display, null);
    assert.equal((await live.restyle({ revision: restyled.revision, rows: [3], layer: 999 })).status, "invalid");
    await live.undo(live.revision);
    const copied = await live.copy({ revision: live.revision, rows: [65, 3], matrix: move });
    assert.equal(copied.status, "ok");
    assert.equal(copied.display, null);
    assert.deepEqual([...copied.selection.rows()], [130, 131]);
    const removed = await live.remove({ revision: copied.revision, rows: [130] });
    assert.equal(removed.result, 1n);
    assert.notEqual(removed.display, null);
    assert.equal((await live.remove({ revision: removed.revision, rows: [130] })).status, "missing");
    assert.equal((await live.target(130, removed.revision)).status, "missing");
    assert.equal((await live.target(131, removed.revision)).status, "ok");
    await live.undo(live.revision);
    await live.undo(live.revision);
    assert.deepEqual((await live.save()).data, saved.data);
  } finally { await live.close(); }
  assert(live.closed);
});

test("予算停止後の診断を独立して取得し、文書を再利用する", { skip: !executable, timeout: 30000 }, async () => {
  const live = await openLiveProcess(executable);
  try {
    assert.equal((await live.save()).status, "noDocument");
    const input = dxf(2);
    const stop = await live.open(input, { budget: { work: 0 } });
    assert.equal(stop.status, "exceeded");
    assert.equal(stop.revision, null);
    assert.equal(stop.used.decodedBytes, 0n);
    assert.equal((await live.diagnostics(stop.requestId, { budget: { decodedBytes: 0 } })).status, "exceeded");
    const diagnostics = await live.diagnostics(stop.requestId);
    assert.equal(diagnostics.status, "ok");
    assert.equal(diagnostics.diagnosticCount, stop.diagnosticCount);
    assert.match(new TextDecoder().decode(diagnostics.data), /budget\.work\.exceeded/);
    const opened = await live.open(input, { display: false, threads: 4 });
    assert.equal(opened.status, "ok");
    assert.equal(opened.display, null);
    const failed = await live.transform({ revision: 0n, rows: [0], matrix: move }, { budget: { decodedBytes: 0 } });
    assert.equal(failed.status, "exceeded");
    assert.equal(failed.revision, 0n);
    assert.deepEqual((await live.save()).data, input);
    assert.equal((await live.diagnostics(failed.requestId)).status, "noDiagnostics");
    assert.equal((await live.transform({ revision: 0n, rows: [0, 0xffffffff], matrix: move })).status, "missing");
    assert.deepEqual((await live.save()).data, input);
  } finally { await live.close(); }
});

test("矩形選択は版・編集・取り消しに追従し、予算停止で部分結果を返さない", { skip: !executable, timeout: 30000 }, async () => {
  let baseline;
  for (const threads of [1, 4]) {
    const live = await openLiveProcess(executable);
    try {
      const input = dxf();
      await live.open(input, { threads });
      const rect = [130.25, -0.1, 130.75, 0.1];
      const first = await live.select(rect, 0n);
      assert.equal(first.status, "ok");
      assert.equal(first.selection.length, 1);
      assert.equal(first.result, 1n);
      assert.deepEqual([...first.selection.rows()], [65]);
      const all = await live.select([-Infinity, -Infinity, Infinity, Infinity], 0n);
      assert.deepEqual([...all.selection.rows()], Array.from({ length: 130 }, (_, i) => i));
      const record = { data: all.data, used: all.used, diagnosticCount: all.diagnosticCount };
      if (baseline) assert.deepEqual(record, baseline); else baseline = record;
      for (const kind of ["decodedBytes", "elements", "work"]) {
        const stopped = await live.select([-Infinity, -Infinity, Infinity, Infinity], 0n, { budget: { [kind]: all.used[kind] - 1n } });
        assert.equal(stopped.status, "exceeded");
        assert.equal(stopped.revision, 0n);
        assert.equal(stopped.selection, null);
        assert.equal(stopped.data.length, 0);
        assert.equal(stopped.stop.kind, kind);
        assert.equal(stopped.diagnosticCount, 1);
      }
      assert.deepEqual((await live.save()).data, input);
      const changed = await live.transform({ revision: first.revision, rows: [...first.selection.rows()], matrix: move });
      assert.equal(changed.status, "ok");
      const stale = await live.select(rect, first.revision);
      assert.equal(stale.status, "stale");
      assert.equal(stale.selection, null);
      assert.equal((await live.select(rect, changed.revision)).selection.length, 0);
      assert.deepEqual([...(await live.select([135.25, -2.1, 135.75, -1.9], changed.revision)).selection.rows()], [65]);
      await live.undo(live.revision);
      assert.deepEqual([...(await live.select(rect, live.revision)).selection.rows()], [65]);
      await live.redo(live.revision);
      assert.equal((await live.select(rect, live.revision)).selection.length, 0);
      for (const empty of [[NaN, 0, 1, 1], [2, 0, 1, 1]]) {
        const result = await live.select(empty, live.revision);
        assert.equal(result.status, "ok");
        assert.deepEqual([...result.selection.rows()], []);
      }
      assert.deepEqual([...first.selection.rows()], [65]);
    } finally { await live.close(); }
  }
});

test("矩形選択の要求をコピーし、分割応答の検査後に行を借りる", async () => {
  const { live } = await fake("selection-fragmented");
  try {
    const rect = new Float64Array([1, 2, 3, 4]);
    const pending = live.select(rect, 7n);
    rect.fill(99);
    const selected = await pending;
    assert.equal(selected.revision, 7n);
    assert.equal(selected.selection.length, 2);
    assert.deepEqual([...selected.selection.rows()], [2, 65]);
    await live.save();
    assert.deepEqual([...selected.selection.rows()], [2, 65]);
    for (const bad of [[], [1, 2, 3], [1, 2, 3, "4"], null]) await assert.rejects(live.select(bad, 7n), TypeError);
    await assert.rejects(live.select([1, 2, 3, 4], undefined), RangeError);
    assert(!live.broken);
  } finally { await live.close(); }
});

for (const mode of ["selection-count", "selection-result", "selection-truncated", "selection-extra", "selection-no-revision", "selection-duplicate", "selection-descending", "selection-virtual"]) {
  test(`不正な選択結果を公開せず接続を終了する: ${mode}`, async () => {
    const { live } = await fake(mode);
    try {
      await assert.rejects(live.select([1, 2, 3, 4], 0n), transport("protocol"));
      assert(live.broken);
    } finally { await live.close(); }
  });
}

test("画層の表を検査してから凍結して公開する", async () => {
  const { live } = await fake("layers");
  try {
    const reply = await live.layers(7n);
    assert.equal(reply.revision, 7n);
    assert.equal(reply.result, 2n);
    assert.deepEqual(reply.layers, [
      { value: 7, name: "0", state: { visible: true, printable: false, editable: true, frozen: false }, rgb: [0x12, 0x34, 0x56], code: null, followsPlacement: true },
      { value: 3, name: "壁", state: null, rgb: null, code: { system: "jww", value: 5 }, followsPlacement: false },
    ]);
    assert(Object.isFrozen(reply.layers) && Object.isFrozen(reply.layers[0]) && Object.isFrozen(reply.layers[0].state));
    assert.equal((await live.save()).layers, null);
    await assert.rejects(live.layers(undefined), RangeError);
    assert(!live.broken);
  } finally { await live.close(); }
});

for (const mode of ["layers-duplicate", "layers-flags", "layers-state", "layers-rgb", "layers-system", "layers-padding", "layers-name", "layers-count", "layers-truncated", "layers-extra", "layers-result"]) {
  test(`不正な画層の表を公開せず接続を終了する: ${mode}`, async () => {
    const { live } = await fake(mode);
    try {
      await assert.rejects(live.layers(0n), transport("protocol"));
      assert(live.broken);
    } finally { await live.close(); }
  });
}

test("描く順を検査してから借りて公開する", async () => {
  const { live } = await fake("order");
  try {
    const reply = await live.order(4n);
    assert.equal(reply.revision, 4n);
    assert.equal(reply.order.length, 3);
    assert.deepEqual([...reply.order.rows()], [5, 1, 9]);
    assert(Object.isFrozen(reply.order));
    assert.equal((await live.save()).order, null);
    assert.deepEqual([...reply.order.rows()], [5, 1, 9]);
    await assert.rejects(live.order(-1), RangeError);
    assert(!live.broken);
  } finally { await live.close(); }
});

test("copy・remove・restyle の引数を送る前に検査する", async () => {
  const { live } = await fake("order");
  try {
    await assert.rejects(live.copy({ revision: 0n, rows: [0], matrix: move }, { display: true }), TypeError);
    await assert.rejects(live.copy({ revision: 0n, rows: [0], matrix: [1, 0, 0] }), TypeError);
    await assert.rejects(live.copy({ revision: 0n, rows: [-1], matrix: move }), RangeError);
    await assert.rejects(live.remove({ revision: 0n, rows: new Uint32Array(65537) }), RangeError);
    await assert.rejects(live.remove({ revision: -1, rows: [0] }), RangeError);
    for (const bad of [{}, { color: "red" }, { color: {} }, { color: { rgb: [1, 2] } }, { color: { code: { system: "x", value: 1 } } },
      { lineWeight: -1 }, { lineWeight: "none" }, { lineTypeScale: 0 }]) {
      await assert.rejects(live.restyle({ revision: 0n, rows: [0], ...bad }), TypeError);
    }
    await assert.rejects(live.restyle({ revision: 0n, rows: [0], layer: -1 }), RangeError);
    assert(!live.broken);
  } finally { await live.close(); }
});

for (const mode of ["order-duplicate", "order-virtual", "order-truncated", "order-result"]) {
  test(`不正な描く順を公開せず接続を終了する: ${mode}`, async () => {
    const { live } = await fake(mode);
    try {
      await assert.rejects(live.order(0n), transport("protocol"));
      assert(live.broken);
    } finally { await live.close(); }
  });
}

test("既存 Session の逐次 apply と保存・現在の対象を 1・4 スレッドで照合する", { skip: !executable, timeout: 30000 }, async () => {
  const records = [];
  for (const threads of [1, 4]) {
    const live = await openLiveProcess(executable);
    let reference;
    try {
      reference = await openProcess(executable);
      const input = dxf();
      await reference.push("native.dxf", input);
      const doc = (await reference.call({ v: 1, op: "open", input: "native.dxf" })).result.doc;
      const opened = await live.open(input, { threads });
      const initial = mirror(opened);
      live.acknowledge(opened.revision);
      const trace = [];
      for (const row of [65, 1]) {
        const entity = (await reference.call({ v: 1, op: "entities", doc, offset: row, limit: 1, fields: ["id", "hash"] })).result.items[0];
        const target = await live.target(row, live.revision);
        assert.deepEqual(target.target, { id: entity.id, hash: entity.hash });
        const changed = await live.transform({ revision: live.revision, rows: [row], matrix: move });
        assert.equal(changed.status, "ok");
        const applied = await reference.call({ v: 1, op: "apply", doc, edits: [{ kind: "transform", id: entity.id, hash: entity.hash, matrix: move }] });
        const bytes = await reference.take(applied.blobs.find((blob) => blob.kind === "file").id);
        assert.deepEqual([...(await live.save()).data], [...bytes]);
        const reread = (await reference.call({ v: 1, op: "entities", doc, offset: row, limit: 1, fields: ["id", "hash"] })).result.items[0];
        assert.deepEqual((await live.target(row, live.revision)).target, { id: reread.id, hash: reread.hash });
        trace.push({ data: changed.data.slice(), used: changed.used, diagnostics: changed.diagnosticCount });
      }
      await live.undo(live.revision);
      const undone = await live.undo(live.revision);
      assert.deepEqual((await live.save()).data, input);
      assert.deepEqual(mirror(undone, initial).columns, initial.columns);
      records.push(trace);
    } finally { await live.close(); await reference?.close(); }
  }
  assert.deepEqual(records[0], records[1]);
});

test("実プロセスで開く途中に中断し、同じ接続で再試行する", { skip: !executable, timeout: 30000 }, async () => {
  const live = await openLiveProcess(executable);
  try {
    const opening = live.open(dxf(50000), { display: false });
    await live.cancel();
    assert.equal((await opening).status, "cancelled");
    assert.equal(live.revision, null);
    assert.equal((await live.open(dxf(2))).status, "ok");
    assert(!live.broken);
  } finally { await live.close(); }
});

test("公開 DWG の 2 世代でも対象・保存・undo を既存 Session と照合する", { skip: !executable, timeout: 30000 }, async () => {
  for (const version of ["r2000", "r2018"]) {
    // 自作・CC0 の公開入力。samples/ledger.toml の出所と許諾に従う。
    const input = await readFile(new URL(`../../../samples/dwg/zumen-points-a-${version}.dwg`, import.meta.url));
    let baseline;
    for (const threads of [1, 4]) {
      const live = await openLiveProcess(executable);
      let reference;
      try {
        reference = await openProcess(executable);
        await reference.push("input.dwg", input);
        const doc = (await reference.call({ v: 1, op: "open", input: "input.dwg" })).result.doc;
        const opened = await live.open(input, { threads });
        assert.equal(opened.status, "ok");
        assert.equal(opened.displayStatus, "ok");
        live.acknowledge(opened.revision);
        const initial = (await live.save()).data;
        assert.deepEqual(initial, input);
        const target = await live.target(0, live.revision);
        assert.equal(target.status, "ok");
        const entities = (await reference.call({ v: 1, op: "entities", doc, fields: ["id", "hash"] })).result.items;
        const entity = entities.find((item) => item.id === target.target.id);
        assert(entity);
        assert.equal(entity.hash, target.target.hash);
        const changed = await live.transform({ revision: live.revision, rows: [0], matrix: move });
        assert.equal(changed.status, "ok");
        const applied = await reference.call({ v: 1, op: "apply", doc, edits: [{ kind: "transform", id: entity.id, hash: entity.hash, matrix: move }] });
        const expected = await reference.take(applied.blobs.find((blob) => blob.kind === "file").id);
        const saved = (await live.save()).data;
        assert.deepEqual([...saved], [...expected]);
        const after = (await live.target(0, live.revision)).target;
        const reread = (await reference.call({ v: 1, op: "entities", doc, fields: ["id", "hash"] })).result.items.find((item) => item.id === after.id);
        assert.equal(after.hash, reread.hash);
        assert.equal((await live.undo(live.revision)).status, "ok");
        assert.deepEqual((await live.save()).data, initial);
        const record = { opened: opened.data, changed: changed.data, used: changed.used, saved, after };
        if (baseline) assert.deepEqual(record, baseline); else baseline = record;
      } finally { await live.close(); await reference?.close(); }
    }
  }
});

test("分割された枠と独立した応答の寿命、入力ビューの範囲を保つ", async () => {
  const { live, child } = await fake("fragmented");
  try {
    const original = Uint8Array.of(0, 4, 255, 3, 0);
    const opened = live.open(original.subarray(1, 4), { display: false });
    original.fill(1);
    await opened;
    const first = await live.save();
    assert.deepEqual([...first.data], [4, 255, 3]);
    assert.equal(first.data.byteOffset, 136);
    assert.equal(first.data.buffer.byteLength, 139);
    const copy = first.data.slice();
    await live.save();
    assert.deepEqual(first.data, copy);
    assert.throws(() => live.acknowledge(0), RangeError);
    await assert.rejects(live.transform({ revision: -1, rows: [0], matrix: move }), RangeError);
    await assert.rejects(live.save({ budget: { unknown: 1 } }), TypeError);
    await assert.rejects(live.open(new Uint8Array(1024 * 1024), { display: false }), transport("limit"));
    assert(!live.broken);
    assert.deepEqual((await live.save()).data, copy);
  } finally { await live.close(); }
  assert(child.exitCode !== null || child.signalCode !== null);
  await live.close();
  await assert.rejects(live.save(), transport("closed"));
});

for (const mode of ["wrong-id", "wrong-magic", "wrong-command", "wrong-flags", "short-prefix", "unknown-status", "reserved", "bad-length", "unexpected-display", "unexpected-stop", "oversized", "truncated", "exit"]) {
  test(`壊れた応答を途中で公開せず接続を終了する: ${mode}`, async () => {
    const { live, child } = await fake(mode);
    try {
      const code = mode === "oversized" ? "limit" : ["exit", "truncated"].includes(mode) ? "io" : "protocol";
      await assert.rejects(live.save(), transport(code));
      assert(live.broken);
    } finally { await live.close(); }
    assert(child.exitCode !== null || child.signalCode !== null);
  });
}

test("通常要求の重複を拒否し、中断は対象の応答を待つ", async () => {
  const { live } = await fake("hold");
  try {
    const saving = live.save();
    await assert.rejects(live.save(), (error) => error instanceof ZumenError && error.status === "busy");
    await Promise.all([live.cancel(), live.cancel()]);
    const result = await saving;
    assert.equal(result.status, "cancelled");
    assert.deepEqual(result.stop, { kind: "cancelled" });
    assert(!live.broken);
  } finally { await live.close(); }
});

test("中断に応答しない子を終了し、終了を待つ", async () => {
  const { live, child } = await fake("silent");
  try {
    const saving = assert.rejects(live.save(), transport("timeout"));
    await assert.rejects(live.cancel(), transport("timeout"));
    await saving;
    assert(live.broken);
  } finally { await live.close(); }
  assert(child.exitCode !== null || child.signalCode !== null);
});

test("期限と処理中の close のどちらも子を残さない", async () => {
  for (const close of [false, true]) {
    const { live, child } = await fake("silent");
    const saving = assert.rejects(live.save(), transport(close ? "closed" : "timeout"));
    if (close) await live.close();
    await saving;
    await live.close();
    assert(child.exitCode !== null || child.signalCode !== null);
  }
  await assert.rejects(openLiveProcess("zumen-intentionally-missing-executable"), transport("spawn"));
});

test("13 列の後半の破損・最終容量・長さの不一致も反映前に拒否する", () => {
  const empty = Buffer.alloc(224);
  empty.write("ZLDP"); empty.writeUInt16LE(1, 4); empty.writeUInt16LE(1, 6);
  empty.writeUInt32LE(224, 8); empty.writeUInt32LE(13, 12);
  const widths = [8, 1, 2, 7, 1, 2, 8, 1, 2, 9, 3, 1, 2];
  widths.forEach((width, id) => { empty.writeUInt32LE(id, 16 + id * 16); empty.writeUInt32LE(width, 20 + id * 16); });
  assert.equal(displayPacket(empty).columns.length, 13);
  for (let position = 0; position < 4; position++) {
    const damaged = Buffer.from(empty);
    damaged[position] |= 0x80;
    assert.throws(() => displayPacket(damaged), transport("protocol"));
  }
  for (const [offset, value] of [[208, 0], [216, 1], [220, 1], [24, 0xffffffff], [8, 225], [12, 12]]) {
    const damaged = Buffer.from(empty);
    damaged.writeUInt32LE(value, offset);
    assert.throws(() => displayPacket(damaged), transport("protocol"));
  }
});

test("応答 magic の上位ビットも固定バイト列として照合する", () => {
  const header = Buffer.alloc(24);
  header.write("ZLIR"); header.writeUInt16LE(1, 4); header.writeUInt16LE(6, 6);
  header.writeBigUInt64LE(1n, 8); header.writeUInt32LE(136, 16);
  const pending = { command: 6, id: 1n, flags: 0 };
  assert.equal(responseLength(header, pending, 1024), 136);
  for (let position = 0; position < 4; position++) {
    const damaged = Buffer.from(header);
    damaged[position] |= 0x80;
    assert.throws(() => responseLength(damaged, pending, 1024), transport("protocol"));
  }
});
