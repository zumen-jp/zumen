import assert from "node:assert/strict";

// 表示を再開した利用側の effect が実際に受けた状態で確かめる。最後に描いた状態だけでは、古い ready を見落とす。
export async function checkBoundaries(page, bytes, mode) {
  for (const kind of ["Activity", "Suspense"]) {
    const initialDestroys = await page.evaluate(() => probe.counts.destroyed);
    await page.evaluate(({ bytes, kind }) => { boundary.begin(bytes); boundary.render(kind, true); }, { bytes: [...bytes], kind });
    await page.waitForFunction(() => ["layout", "passive"].every((phase) => boundary.calls.some((c) => c.phase === phase && c.record === 1)));
    assert.equal(await page.evaluate(() => probe.counts.destroyed), initialDestroys + (mode === "StrictMode" ? 1 : 0), "開発用 StrictMode だけが初回に effect を再実行する");
    const workers = page.workers();
    assert.equal(workers.length, 1);
    const [oldWorker] = workers;
    const exited = oldWorker.waitForEvent("close", { timeout: 10000 });
    await page.evaluate((kind) => { boundary.hold(); boundary.render(kind, false); }, kind);
    await exited;
    assert.equal(page.workers().length, 0);
    await page.evaluate((kind) => { boundary.mark(); boundary.render(kind, true); }, kind);
    assert.ok(await page.evaluate(() => boundary.fresh), kind + " から同じ Canvas に再接続");
    await page.waitForFunction(() => boundary.waiting > 0);
    const waiting = await page.evaluate(() => boundary.calls);
    assert.ok(["layout", "passive"].every((phase) => waiting.some((call) => call.phase === phase))
      && waiting.every((call) => call.status === "loading" && call.workers === 0), mode + " " + kind + " の再接続: " + JSON.stringify(waiting));
    await page.evaluate(() => boundary.release());
    await page.waitForFunction(() => ["layout", "passive"].every((phase) => boundary.calls.some((c) => c.phase === phase && c.record === 1)));
    const calls = await page.evaluate(() => boundary.calls.filter((c) => c.status === "ready"));
    assert.ok(calls.every((c) => c.workers === 1 && c.record === 1), mode + " " + kind + " の ready は生存する Worker に属する: " + JSON.stringify(calls));
    await page.evaluate(() => boundary.move());
    await page.waitForFunction(() => boundary.events === 1 && Number(document.querySelector("#camera")?.textContent) < 0);
    assert.deepEqual(await page.evaluate(() => [probe.counts.state, probe.counts.events]), [1, 1]);
    await page.evaluate(() => boundary.end());
    assert.deepEqual(await page.evaluate(() => [probe.counts.workers, probe.counts.state, probe.counts.events]), [0, 0, 0]);
  }
}

// ページに入れる検査用のゲート。応答を待つ間に選択が重なった場合を、入力の大きさや速さに頼らず作る。
export function installWorkerGate() {
  const held = [], seen = [];
  let hold = false, deferOpen = false, openReply, deferExport, exportReply, omitInput = false;
  const send = Worker.prototype.postMessage;
  Worker.prototype.postMessage = function (message, ...rest) {
    if (message.op === "convert_estimate" && omitInput) {
      omitInput = false;
      const deliver = this.onmessage;
      this.onmessage = (event) => {
        if (event.data.id === message.id && event.data.value?.result?.estimate) {
          delete event.data.value.result.estimate.input_diagnostics;
          delete event.data.value.inputDiagnostics;
        }
        deliver(event);
      };
    }
    if (message.op === deferExport) {
      deferExport = null;
      const deliver = this.onmessage;
      this.onmessage = (event) => {
        if (event.data.id === message.id) exportReply = () => deliver(event);
        else deliver(event);
      };
    }
    if (message.op === "open" && deferOpen) {
      deferOpen = false;
      const deliver = this.onmessage;
      this.onmessage = (event) => { openReply = () => deliver(event); };
    }
    if (message.op === "source") {
      seen.push(message.element.id);
      if (hold) { held.push(() => send.call(this, message, ...rest)); return; }
    }
    return send.call(this, message, ...rest);
  };
  window.sourceGate = { seen, start() { seen.length = 0; hold = true; }, release() { hold = false; for (const send of held.splice(0)) send(); } };
  window.placeGate = {
    defer() { deferOpen = true; }, get waiting() { return Boolean(openReply); },
    release() { openReply(); openReply = null; },
  };
  window.exportGate = {
    defer(op) { deferExport = op; }, get waiting() { return Boolean(exportReply); },
    release() { exportReply(); exportReply = null; },
    omitInputDiagnostics() { omitInput = true; },
  };
}

const section = (name, pairs) => [0, "SECTION", 2, name, ...pairs, 0, "ENDSEC"];
const dxf = (...sections) => Buffer.from([...section("HEADER", [9, "$INSUNITS", 70, 4]), ...sections.flat(), 0, "EOF", ""].join("\n"));
const line = (id, x1, y1, x2, y2) => [0, "LINE", 5, id, 8, "0", 10, x1, 20, y1, 11, x2, 21, y2];

export async function checkInteractions(page, ready) {
  const file = page.getByLabel("図面ファイル");
  const canvas = page.getByLabel("図面の表示", { exact: true });
  const source = page.getByRole("region", { name: "出所" });
  async function open(bytes, name) {
    await file.setInputFiles({ name: name + ".dxf", mimeType: "application/octet-stream", buffer: bytes });
    await ready("DXF");
  }
  const wide = dxf(section("ENTITIES", [...line("B0", -1000, 0, -1000, 40), ...line("B1", 0, 0, 0, 40), ...line("B2", 1000, 0, 1000, 40)]));
  await open(wide, "wide");
  const lineCount = () => canvas.evaluate((node) => {
    const row = node.getContext("2d").getImageData(0, Math.floor(node.height / 2), node.width, 1).data;
    let count = 0, previous = false;
    for (let i = 0; i < row.length; i += 4) {
      const ink = row[i] < 225 || row[i + 1] < 225 || row[i + 2] < 225;
      if (ink && !previous) count++;
      previous = ink;
    }
    return count;
  });
  assert.equal(await lineCount(), 3, "詳細パネルを並べた初回の表示に左右の線が収まる");
  await page.getByRole("button", { name: "全体を表示" }).click();
  assert.equal(await lineCount(), 3);

  const handles = Array.from({ length: 40 }, (_, i) => (0xD0 + i).toString(16).toUpperCase());
  await open(dxf(section("ENTITIES", handles.flatMap((id, i) => line(id, 0, i, 10, i)))), "selections");
  await page.evaluate(() => sourceGate.start());
  for (const [i, handle] of handles.entries()) {
    if (i === 25) await page.getByRole("button", { name: "次の要素" }).press("Enter");
    await page.getByRole("button", { name: "space/0/h" + handle, exact: true }).press("Space");
  }
  assert.deepEqual(await page.evaluate(() => sourceGate.seen), ["space/0/hD0"]);
  await page.evaluate(() => sourceGate.release());
  await source.getByText("F7", { exact: true }).waitFor();
  assert.deepEqual(await page.evaluate(() => sourceGate.seen), ["space/0/hD0", "space/0/hF7"]);

  const grid = dxf(
    section("BLOCKS", [0, "BLOCK", 5, "10", 2, "MARK", 70, 0, 10, 0, 20, 0, 0, "TEXT", 5, "11", 8, "0", 10, 0, 20, 0, 40, 2.5, 1, "X", 0, "ENDBLK"]),
    section("ENTITIES", [0, "INSERT", 5, "20", 8, "0", 2, "MARK", 10, 0, 20, 0, 70, 2, 71, 2, 44, 20, 45, 20]),
  );
  await open(grid, "grid");
  const rows = await page.getByRole("region", { name: /^診断/ }).getByRole("listitem").filter({ hasText: "browser-text" }).allTextContents();
  assert.equal(rows.length, 4);
  for (const column of [0, 1]) for (const row of [0, 1]) {
    assert.ok(rows.some((text) => text.includes("space/h10/h11") && text.includes("space/0/h20（列 " + column + "・行 " + row + "）")), "配置別の診断");
  }

  const bigger = page.getByRole("button", { name: "拡大", exact: true }), smaller = page.getByRole("button", { name: "縮小", exact: true });
  const width = await canvas.evaluate((node) => node.clientWidth - 32);
  for (const [scale, button, back, atLimit] of [[2e12, bigger, smaller, true], [0.9e12, bigger, smaller, false], [0.5e-12, smaller, bigger, true], [1.1e-12, smaller, bigger, false]]) {
    await open(dxf(section("ENTITIES", line("C1", 0, 0, width / scale, 0))), "scale");
    assert.equal(await button.isDisabled(), atLimit, "初回の倍率の限界");
    if (!atLimit) await button.press("Enter");
    assert.ok(await button.isDisabled(), "一回の操作でも上限・下限に収める");
    await back.press("Enter"); assert.ok(await button.isEnabled());
  }
  return wide;
}
