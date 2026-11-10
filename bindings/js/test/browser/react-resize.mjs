import assert from "node:assert/strict";

// テスト内だけで、未処理エラーの直前の操作と寸法を記録する。
export function installResizeTrace() {
  let active = 0, serial = 0;
  const nativeFrame = requestAnimationFrame.bind(window), nativeCancel = cancelAnimationFrame.bind(window);
  const pending = new Set(), observers = new Map();
  const trace = window.resizeTrace = { records: [], callbacks: 0, writes: 0, phase: "initial", scheduled: 0, executed: 0, canceled: 0,
    nextFrame: () => new Promise(nativeFrame), get pending() { return pending.size; }, get observers() { return observers.size; } };
  window.requestAnimationFrame = (callback) => {
    const id = nativeFrame((time) => { pending.delete(id); trace.executed++; callback.call(window, time); });
    pending.add(id); trace.scheduled++; return id;
  };
  window.cancelAnimationFrame = (id) => { if (pending.delete(id)) trace.canceled++; nativeCancel(id); };
  const record = (kind, values = {}) => {
    trace.records.push({ kind, time: performance.now(), phase: trace.phase, active, ...values });
    if (trace.records.length > 250) trace.records.shift();
  };
  // 通知中の観測で layout を早めない。CSS の実測は、失敗が発生した後だけにする。
  const box = (target) => ({ bitmap: [target.width, target.height] });
  document.addEventListener("change", (event) => {
    const target = event.target;
    trace.phase = target.id === "drawing-file" ? "file:" + target.files?.[0]?.name : "change:" + (target.id || target.tagName) + ":" + target.value;
    record("operation");
  }, true);
  document.addEventListener("click", (event) => {
    const button = event.target.closest?.("button");
    if (button) { trace.phase = "click:" + button.textContent; record("operation"); }
  }, true);
  document.addEventListener("keydown", (event) => { trace.phase = "key:" + event.key + ":" + event.target.tagName; record("operation"); }, true);
  addEventListener("resize", () => { trace.phase = "viewport:" + innerWidth + "x" + innerHeight; record("viewport"); });
  addEventListener("pagehide", () => record("pagehide"));
  addEventListener("error", (event) => {
    const canvas = document.querySelector("canvas"), rect = canvas?.getBoundingClientRect();
    window.captureResizeFailure({ message: event.message, phase: trace.phase, path: location.pathname, records: trace.records.slice(-30),
      current: canvas && { client: [canvas.clientWidth, canvas.clientHeight], rect: [rect.width, rect.height], ...box(canvas) },
      viewport: [innerWidth, innerHeight], document: [document.documentElement.clientWidth, document.documentElement.scrollHeight] });
  });
  const NativeObserver = ResizeObserver;
  window.ResizeObserver = class extends NativeObserver {
    constructor(callback) {
      const id = ++serial;
      super((entries, observer) => {
        trace.callbacks++; active = id;
        record("notify", { id, entries: entries.map(({ target, contentRect }) => ({ reported: [contentRect.width, contentRect.height], ...box(target) })) });
        try { callback(entries, observer); }
        finally {
          record("notify-end", { id, entries: entries.map(({ target }) => box(target)) }); active = 0;
          const after = trace.afterNextCallback; trace.afterNextCallback = null; after?.();
          queueMicrotask(() => record("notify-microtask", { id, entries: entries.map(({ target }) => box(target)) }));
        }
      });
      record("construct", { id });
    }
    observe(target, options) {
      super.observe(target, options);
      if (!observers.has(this)) observers.set(this, new Set()); observers.get(this).add(target);
      record("observe", box(target));
    }
    unobserve(target) { super.unobserve(target); const targets = observers.get(this); targets?.delete(target); if (!targets?.size) observers.delete(this); }
    disconnect() { super.disconnect(); observers.delete(this); record("disconnect"); }
  };
  for (const axis of ["width", "height"]) {
    const property = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, axis);
    Object.defineProperty(HTMLCanvasElement.prototype, axis, { ...property, set(value) {
      trace.writes++; record("write-before", { axis, value, ...box(this) });
      property.set.call(this, value);
      record("write-after", { axis, value, ...box(this) });
    } });
  }
}

export async function checkPlaygroundResize(page, input, mode) {
  const original = page.viewportSize();
  await page.evaluate(() => resizeFixture.mount());
  const canvas = page.getByLabel("図面の表示", { exact: true });
  const sample = () => page.evaluate(async () => {
    const rows = [], canvas = document.querySelector('canvas[aria-label="図面の表示"]');
    for (let frame = 0; frame < 8; frame++) {
      await resizeTrace.nextFrame();
      const data = canvas.getContext("2d").getImageData(0, Math.floor(canvas.height / 2), canvas.width, 1).data;
      let hash = 2166136261, groups = 0, previous = false;
      for (let i = 0; i < data.length; i++) hash = Math.imul(hash ^ data[i], 16777619) >>> 0;
      for (let i = 0; i < data.length; i += 4) {
        const ink = data[i] < 225 || data[i + 1] < 225 || data[i + 2] < 225;
        if (ink && !previous) groups++; previous = ink;
      }
      rows.push({ client: [canvas.clientWidth, canvas.clientHeight], bitmap: [canvas.width, canvas.height], hash, groups,
        callbacks: resizeTrace.callbacks, pending: resizeTrace.pending, ratio: Math.min(devicePixelRatio || 1, 2) });
    }
    return rows;
  });
  const settled = async () => {
    const rows = await sample(), last = rows.at(-1);
    assert.ok(rows.slice(-4).every((row) => JSON.stringify(row) === JSON.stringify(last)), mode + ": 寸法・描画・通知が8フレーム内に収束する: " + JSON.stringify(rows));
    assert.deepEqual(last.bitmap, last.client.map((value) => Math.ceil(Math.max(1, value) * last.ratio)), mode + ": 最後のCSS寸法とbacking寸法が一致する");
    assert.equal(last.pending, 0, mode + ": 安定後に寸法更新を予約し続けない");
    return last;
  };
  await page.getByLabel("図面ファイル").setInputFiles({ name: "resize.dxf", buffer: input });
  await page.waitForFunction(() => document.querySelector(".status")?.textContent.startsWith("表示しました。"));
  assert.equal((await settled()).groups, 3, mode + ": 詳細パネルが並んだ初回fitで左右の線も表示する");
  assert.deepEqual(await page.evaluate(() => [probe.counts.workers, probe.counts.inputs, resizeTrace.observers]), [1, 1, 1]);
  for (const size of [{ width: 760, height: 620 }, { width: 1101, height: 801 }, original]) {
    await page.setViewportSize(size); await settled();
  }
  assert.equal(await page.evaluate(() => resizeTrace.records.filter((row) => row.kind === "write-before" && row.active).length), 0,
    mode + ": ROの配送中にCanvas寸法を書き換えず、状態通知とレイアウト変更を持ち越す");
  await canvas.focus();
  const before = (await settled()).hash;
  await canvas.press("ArrowRight"); assert.notEqual((await settled()).hash, before, mode + ": リサイズ後も移動できる");
  await canvas.press("Home"); assert.equal((await settled()).groups, 3, mode + ": リサイズ後も全体を表示できる");
  const worker = page.workers()[0], exited = worker.waitForEvent("close", { timeout: 10000 });
  const prior = await page.evaluate(() => {
    resizeTrace.afterNextCallback = () => resizeFixture.unmount();
    return { canceled: resizeTrace.canceled, executed: resizeTrace.executed };
  });
  await page.setViewportSize({ width: original.width - 64, height: original.height - 32 });
  await exited;
  await page.evaluate(async () => { for (let i = 0; i < 3; i++) await resizeTrace.nextFrame(); });
  const closed = await page.evaluate(() => ({ pending: resizeTrace.pending, observers: resizeTrace.observers, canceled: resizeTrace.canceled,
    executed: resizeTrace.executed, workers: probe.counts.workers, inputs: probe.counts.inputs, state: probe.counts.state }));
  assert.equal(closed.canceled, prior.canceled + 1, mode + ": 通知直後のunmountで予約を取り消す");
  assert.equal(closed.executed, prior.executed, mode + ": 破棄したviewerへ予約済みcallbackを実行しない");
  assert.deepEqual([closed.pending, closed.observers, closed.workers, closed.inputs, closed.state], [0, 0, 0, 0, 0], mode + ": observer・入力・Worker・購読を片づける");
  await page.setViewportSize(original);
  await page.evaluate(() => resizeFixture.mount()); await canvas.waitFor(); await settled();
  await page.evaluate(() => resizeFixture.unmount());
  assert.deepEqual(await page.evaluate(() => [resizeTrace.pending, resizeTrace.observers, probe.counts.inputs, probe.counts.state]), [0, 0, 0, 0], mode + ": 再接続後も後始末できる");
}
