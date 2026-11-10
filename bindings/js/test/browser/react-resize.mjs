// テスト内だけで、未処理エラーの直前の操作と寸法を記録する。
export function installResizeTrace() {
  let frame = 0, active = 0, serial = 0;
  const tick = () => { frame++; requestAnimationFrame(tick); };
  requestAnimationFrame(tick);
  const trace = window.resizeTrace = { records: [], callbacks: 0, writes: 0, phase: "initial" };
  const record = (kind, values = {}) => {
    trace.records.push({ kind, frame, phase: trace.phase, active, ...values });
    if (trace.records.length > 250) trace.records.shift();
  };
  const box = (target) => ({ client: [target.clientWidth, target.clientHeight], bitmap: [target.width, target.height],
    rect: [target.getBoundingClientRect().width, target.getBoundingClientRect().height],
    viewport: [innerWidth, innerHeight], document: [document.documentElement.clientWidth, document.documentElement.scrollHeight] });
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
    window.captureResizeFailure({ message: event.message, phase: trace.phase, frame, path: location.pathname, records: trace.records.slice(-30) });
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
          queueMicrotask(() => record("notify-microtask", { id, entries: entries.map(({ target }) => box(target)) }));
        }
      });
      record("construct", { id });
    }
    observe(target, options) { record("observe", box(target)); return super.observe(target, options); }
    disconnect() { record("disconnect"); return super.disconnect(); }
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
