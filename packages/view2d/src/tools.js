import { finite, valid, VIEW_SCALE_LIMITS } from "./validate.js";

export const DEFAULT_INPUT = Object.freeze({ pointer: true, touch: true, drag: true, pinch: true, wheel: true, keyboard: true, doubleClick: true, wheelModifier: "ctrlOrMeta" });
const BUILTINS = ["select", "pan", "measure"];
const POINTERS = ["pointerdown", "pointermove", "pointerup"];
const TYPES = [...POINTERS, "cancel", "wheel", "keydown", "dblclick"];
const position = (value) => {
  valid(value && typeof value === "object", "input.point");
  return Object.freeze({ x: finite(value.x, "input.point.x"), y: finite(value.y, "input.point.y") });
};

export function inputOptions(options = {}, previous = DEFAULT_INPUT) {
  valid(options && typeof options === "object", "input.options");
  for (const [key, value] of Object.entries(options)) {
    valid(Object.prototype.hasOwnProperty.call(DEFAULT_INPUT, key), "input.option");
    valid(key === "wheelModifier" ? ["ctrlOrMeta", "none"].includes(value) : typeof value === "boolean", `input.${key}`);
  }
  return Object.freeze({ ...previous, ...options });
}

export function normalizeInput(value) {
  valid(value && TYPES.includes(value.type), "input.type");
  const input = { type: value.type };
  for (const key of ["shiftKey", "ctrlKey", "altKey", "metaKey"]) {
    valid(value[key] === undefined || typeof value[key] === "boolean", `input.${key}`); input[key] = value[key] ?? false;
  }
  if (value.type === "cancel") {
    valid(value.reason === undefined || (typeof value.reason === "string" && value.reason.length <= 256), "input.reason");
    input.reason = value.reason ?? "cancel";
  } else if (value.type === "keydown") {
    valid(typeof value.key === "string" && value.key.length <= 128, "input.key"); input.key = value.key;
  } else {
    input.point = position(value.point);
    if (value.type === "wheel") input.deltaY = finite(value.deltaY, "input.deltaY");
    if (POINTERS.includes(value.type)) {
      valid(Number.isSafeInteger(value.pointerId) && value.pointerId >= 0, "input.pointerId"); input.pointerId = value.pointerId;
      valid(["mouse", "pen", "touch"].includes(value.pointerType ?? "mouse"), "input.pointerType"); input.pointerType = value.pointerType ?? "mouse";
      valid(Number.isInteger(value.button ?? 0) && (value.button ?? 0) >= -1 && (value.button ?? 0) <= 5, "input.button"); input.button = value.button ?? 0;
    }
  }
  return Object.freeze(input);
}

/** DOM に依らない入力と、ツールの有効期間。購読はツールの中断時にまとめて外す。 */
export class Tools {
  constructor(viewer, limits, release) {
    this.viewer = viewer; this.limits = limits; this.release = release;
    this.name = "select"; this.options = DEFAULT_INPUT; this.gesture = null;
    this.definitions = new Map(); this.pointers = new Map(); this.drag = null; this.session = null;
    this.busy = false; this.revision = 0;
  }
  idle() { valid(!this.busy, "input.reentrant"); }
  changed(gesture = null) { this.gesture = gesture && Object.freeze(gesture); this.revision++; }
  register(name, definition) {
    this.idle();
    valid(typeof name === "string" && name.length > 0 && name.length <= this.limits.textLength && !BUILTINS.includes(name) && !this.definitions.has(name), "tool.name");
    valid(definition && typeof definition.onInput === "function" && ["onActivate", "onCancel"].every((key) => definition[key] === undefined || typeof definition[key] === "function"), "tool.definition");
    valid(this.definitions.size < this.limits.stateItems, "limits.stateItems");
    this.definitions.set(name, Object.freeze({ onInput: definition.onInput, onActivate: definition.onActivate, onCancel: definition.onCancel }));
  }
  sessionFor() {
    if (this.session || !this.definitions.has(this.name)) return this.session;
    const controller = new AbortController(), cleanups = new Set(), definition = this.definitions.get(this.name);
    const retain = (cleanup) => { cleanups.add(cleanup); return () => { cleanups.delete(cleanup); cleanup(); }; };
    const context = Object.freeze({ viewer: this.viewer, signal: controller.signal,
      subscribe: (listener) => { valid(!controller.signal.aborted, "tool.inactive"); return retain(this.viewer.subscribe(listener)); },
      on: (type, listener) => { valid(!controller.signal.aborted, "tool.inactive"); return retain(this.viewer.on(type, listener)); },
      addOverlay: (draw) => { valid(!controller.signal.aborted, "tool.inactive"); return retain(this.viewer.addOverlay(draw)); },
    });
    this.session = { controller, cleanups, definition, context };
    const cleanup = definition.onActivate?.(context);
    valid(cleanup === undefined || typeof cleanup === "function", "tool.cleanup");
    if (cleanup) cleanups.add(cleanup);
    return this.session;
  }
  stop(reason) {
    const session = this.session; this.session = null;
    let failed = false, error;
    const run = (callback) => { try { callback(); } catch (value) { if (!failed) { failed = true; error = value; } } };
    if (session) {
      run(() => session.controller.abort());
      run(() => session.definition.onCancel?.(reason, session.context));
      for (const cleanup of session.cleanups) run(cleanup);
      session.cleanups.clear();
    }
    return { failed, error };
  }
  reset(reason) {
    this.pointers.clear(); this.drag = null; this.changed();
    const previous = this.busy; this.busy = true;
    try {
      let failed = false, error;
      try { this.release(); } catch (value) { failed = true; error = value; }
      const stopped = this.stop(reason);
      return failed ? { failed, error } : stopped;
    } finally { this.busy = previous; }
  }
  zoom(factor, anchor) {
    const scale = this.viewer.getView().scale, next = Math.max(VIEW_SCALE_LIMITS.min, Math.min(VIEW_SCALE_LIMITS.max, scale * factor));
    this.viewer.zoomBy(next / scale, anchor);
  }
  location(point) { return this.viewer.snapAt(point)?.position ?? this.viewer.positionAt(point); }
  navigate(input) {
    if (input.type === "wheel") {
      if (this.options.wheelModifier === "ctrlOrMeta" && !(input.ctrlKey || input.metaKey)) return false;
      this.zoom(Math.exp(-Math.max(-500, Math.min(500, input.deltaY)) * 0.002), input.point); return true;
    }
    if (input.type === "dblclick") { this.zoom(input.shiftKey ? 0.5 : 2, input.point); return true; }
    if (input.type === "keydown") {
      const step = input.shiftKey ? 200 : 40;
      if (input.key === "Escape") {
        const stopped = this.reset("escape"); this.viewer.setMeasurement(null);
        if (stopped.failed) throw stopped.error;
        return true;
      }
      if (input.ctrlKey || input.metaKey || input.altKey) return false;
      if (input.key === "+" || input.key === "=") { this.zoom(1.25); return true; }
      if (input.key === "-") { this.zoom(0.8); return true; }
      if (input.key === "Home" || input.key === "0") { this.viewer.fit(); return true; }
      const deltas = { ArrowLeft: [step, 0], ArrowRight: [-step, 0], ArrowUp: [0, step], ArrowDown: [0, -step] };
      if (Object.prototype.hasOwnProperty.call(deltas, input.key)) { this.viewer.panBy(...deltas[input.key]); return true; }
    }
    return false;
  }
  builtin(input) {
    const viewer = this.viewer;
    if (input.type === "pointerdown") {
      const mode = input.button === 1 ? "pan" : this.name;
      if (mode === "measure" && !viewer.getScene()) return false;
      this.drag = { id: input.pointerId, start: input.point, last: input.point, mode,
        position: mode === "measure" ? this.location(input.point) : null };
      this.changed({ type: mode, start: input.point, end: input.point });
      if (mode === "measure") viewer.setMeasurement({ start: this.drag.position, end: this.drag.position });
      return true;
    }
    const drag = this.drag;
    if (input.type === "pointermove" && !drag && this.name === "select") {
      viewer.setHovered(viewer.hitTest(input.point)[0] ?? null); return false;
    }
    if (!drag || drag.id !== input.pointerId) return false;
    if (input.type === "pointermove") {
      if (!this.options.drag) return true;
      const before = drag.last; drag.last = input.point;
      this.changed({ type: drag.mode, start: drag.start, end: input.point });
      if (drag.mode === "pan") viewer.panBy(input.point.x - before.x, input.point.y - before.y);
      else if (drag.mode === "measure") viewer.setMeasurement({ start: drag.position, end: this.location(input.point) });
      return true;
    }
    if (input.type === "pointerup") {
      this.drag = null; this.changed();
      if (drag.mode === "select") {
        const moved = this.options.drag && Math.hypot(input.point.x - drag.start.x, input.point.y - drag.start.y) >= 3;
        const refs = moved ? viewer.hitTestRect({ min_x: Math.min(drag.start.x, input.point.x), min_y: Math.min(drag.start.y, input.point.y), max_x: Math.max(drag.start.x, input.point.x), max_y: Math.max(drag.start.y, input.point.y) }) : viewer.hitTest(input.point).slice(0, 1);
        viewer.select(refs, input.ctrlKey || input.metaKey ? "toggle" : input.shiftKey ? "add" : "replace");
      } else if (drag.mode === "measure") viewer.setMeasurement({ start: drag.position, end: this.options.drag ? this.location(input.point) : drag.position });
      return true;
    }
    return false;
  }
  handle(input) {
    this.idle();
    if (input.type === "cancel") { const result = this.reset(input.reason); this.viewer.setMeasurement(null); if (result.failed) throw result.error; return true; }
    if (POINTERS.includes(input.type) && !(input.pointerType === "touch" ? this.options.touch : this.options.pointer)) return false;
    if (input.type === "wheel" && !this.options.wheel || input.type === "keydown" && !this.options.keyboard || input.type === "dblclick" && !this.options.doubleClick) return false;
    this.busy = true;
    try {
      // 中断は独自ツールが入力を処理するかどうかに依らない。
      if (input.type === "keydown" && input.key === "Escape") return this.navigate(input);
      if (input.type === "pointerdown") {
        if (input.button !== 0 && input.button !== 1 || this.pointers.has(input.pointerId) || this.pointers.size >= 2) return false;
        if (this.pointers.size && (input.pointerType !== "touch" || !this.options.pinch || [...this.pointers.values()].some((p) => p.pointerType !== "touch"))) return false;
        this.pointers.set(input.pointerId, input);
        if (this.pointers.size === 2) {
          this.drag = null;
          const [a, b] = [...this.pointers.values()]; this.changed({ type: "pinch", start: a.point, end: b.point });
          const stopped = this.stop("pinch");
          this.viewer.setMeasurement(null); if (stopped.failed) throw stopped.error; return true;
        }
      }
      if (input.type === "pointermove" && this.pointers.has(input.pointerId)) {
        if (this.pointers.size === 2) {
          const before = [...this.pointers.values()].map((p) => p.point);
          this.pointers.set(input.pointerId, input); const after = [...this.pointers.values()].map((p) => p.point);
          this.changed({ type: "pinch", start: after[0], end: after[1] });
          const center = (points) => ({ x: points[0].x / 2 + points[1].x / 2, y: points[0].y / 2 + points[1].y / 2 });
          const distance = (points) => Math.hypot(points[0].x - points[1].x, points[0].y - points[1].y);
          const a = center(before), b = center(after), previous = distance(before), current = distance(after);
          this.viewer.panBy(b.x - a.x, b.y - a.y);
          if (previous > 0 && current > 0) this.zoom(current / previous, b);
          return true;
        }
        if (!this.options.drag) return true;
        this.pointers.set(input.pointerId, input);
      }
      if (input.type === "pointerup") {
        if (!this.pointers.has(input.pointerId)) return false;
        if (this.pointers.size === 2) { const stopped = this.reset("pinch-end"); if (stopped.failed) throw stopped.error; return true; }
        this.pointers.delete(input.pointerId);
      }
      const session = this.sessionFor();
      if (session) {
        if (input.type === "pointerup") this.changed();
        else if (POINTERS.includes(input.type) && this.pointers.has(input.pointerId)) this.changed({ type: "custom", start: this.gesture?.start ?? input.point, end: input.point });
        const handled = session.definition.onInput(input, session.context);
        valid(handled === undefined || typeof handled === "boolean", "tool.result");
        if (handled) return true;
      } else if (this.builtin(input)) return true;
      if (input.type === "pointerdown") { this.pointers.delete(input.pointerId); this.changed(); }
      return this.navigate(input);
    } finally { this.busy = false; }
  }
}
