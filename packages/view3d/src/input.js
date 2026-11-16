// 入力の接続（docs/view3d.md#当たり判定と入力）。DOM の値を正規化してから、ビューを固定した操作として扱う。
import { cross, dot, invalid, isObject, length, normalize, scale, sub } from "./common.js";

const DEFAULTS = Object.freeze({ select: true, pan: true, orbit: true, zoom: true, touch: true, dragThreshold: 4, zoomSpeed: 0.002, rotateSpeed: 0.01 });
const TYPES = new Set(["pointerdown", "pointermove", "pointerup", "pointercancel", "wheel", "keydown"]);
const POINTER_TYPES = new Set(["mouse", "pen", "touch"]);
/** 操作を元のカメラへ戻す中断の理由。ビュー・シーン・設定の変更と例外では戻さない。 */
const RESTORE = new Set(["escape", "cancel", "pointercancel", "lostpointercapture", "blur", "hidden"]);
const MARGIN = 0.01;

export const INPUT_DEFAULTS = DEFAULTS;

export function readInputOptions(value, previous = DEFAULTS) {
  if (value === undefined) return previous;
  if (!isObject(value)) invalid("入力の設定はオブジェクト");
  const next = { ...previous };
  for (const [key, v] of Object.entries(value)) {
    if (!Object.prototype.hasOwnProperty.call(DEFAULTS, key)) invalid(`入力の未知の設定: ${key}`);
    if (typeof DEFAULTS[key] === "boolean") { if (typeof v !== "boolean") invalid(`入力の ${key} は真偽値`); }
    else if (typeof v !== "number" || !Number.isFinite(v) || v <= 0 || v > 1000) invalid(`入力の ${key} は 1000 以下の正の有限値`);
    next[key] = v;
  }
  return Object.freeze(next);
}

function readPoint(value) {
  if (!isObject(value) || typeof value.x !== "number" || typeof value.y !== "number" || !Number.isFinite(value.x) || !Number.isFinite(value.y)) invalid("入力の point は有限の { x, y }（CSS px）");
  return Object.freeze({ x: value.x, y: value.y });
}

/** 正規化した入力。DOM を使わない利用側と検査は、この形で handleInput に渡す。 */
export function normalizeInput(value) {
  if (!isObject(value) || !TYPES.has(value.type)) invalid(`入力の type は ${[...TYPES].join("・")}`);
  const out = { type: value.type };
  for (const key of ["shiftKey", "ctrlKey", "altKey", "metaKey"]) {
    if (value[key] !== undefined && typeof value[key] !== "boolean") invalid(`入力の ${key} は真偽値`);
    out[key] = value[key] === true;
  }
  if (value.type.startsWith("pointer")) {
    if (!Number.isSafeInteger(value.pointerId)) invalid("入力の pointerId は整数");
    out.pointerId = value.pointerId;
    out.pointerType = value.pointerType ?? "mouse";
    if (!POINTER_TYPES.has(out.pointerType)) invalid("入力の pointerType は mouse・pen・touch");
    out.button = value.button ?? (value.type === "pointermove" ? -1 : 0);
    if (!Number.isSafeInteger(out.button) || out.button < -1 || out.button > 31) invalid("入力の button は -1〜31 の整数");
    if (value.type !== "pointercancel") out.point = readPoint(value.point);
  } else if (value.type === "wheel") {
    out.point = readPoint(value.point);
    if (typeof value.deltaY !== "number" || !Number.isFinite(value.deltaY)) invalid("入力の deltaY は有限値（CSS px）");
    out.deltaY = value.deltaY;
  } else {
    if (typeof value.key !== "string" || value.key.length > 64) invalid("入力の key は 64 文字以下の文字列");
    out.key = value.key;
  }
  return Object.freeze(out);
}

/** u の周りに角 a だけ回す（Rodrigues）。u は単位ベクトル。 */
function rotate(v, u, a) {
  const c = Math.cos(a), s = Math.sin(a), k = dot(u, v), w = cross(u, v);
  return [0, 1, 2].map((i) => v[i] * c + w[i] * s + u[i] * k * (1 - c));
}

function basis(camera) {
  const forward = normalize(camera.direction), right = normalize(cross(forward, camera.up));
  return { forward, right, up: cross(right, forward) };
}

/** ビューの CSS px あたりの文書単位。透視投影は奥行き depth の位置で測る。 */
function perPixel(camera, rect, depth) {
  return camera.projection === "perspective" ? (2 * Math.tan((camera.fov * Math.PI) / 360) * depth) / rect.height : camera.width / rect.width;
}

export function panCamera(camera, rect, dx, dy, depth) {
  const { right, up } = basis(camera), k = perPixel(camera, rect, depth);
  return { ...camera, position: [0, 1, 2].map((i) => camera.position[i] - right[i] * dx * k + up[i] * dy * k) };
}

/** 注視点の周りに、上方向の軸で水平に、右方向の軸で上下に回す。方向が上方向と平行にならないよう上下の角を抑える。 */
export function orbitCamera(camera, pivot, yaw, pitch) {
  const axis = normalize(camera.up), forward = normalize(camera.direction);
  const angle = Math.acos(Math.max(-1, Math.min(1, dot(forward, axis))));
  const limited = Math.max(angle - (Math.PI - MARGIN), Math.min(angle - MARGIN, pitch));
  const right = normalize(cross(forward, axis));
  let offset = sub(camera.position, pivot), direction = forward;
  offset = rotate(rotate(offset, right, limited), axis, yaw);
  direction = rotate(rotate(direction, right, limited), axis, yaw);
  return { ...camera, position: [0, 1, 2].map((i) => pivot[i] + offset[i]), direction };
}

/** 画面の点の下を保って拡大・縮小する。factor が 1 より大きいと離れる。 */
export function zoomCamera(camera, rect, point, factor, target) {
  if (camera.projection === "orthographic") {
    const { right, up } = basis(camera), aspect = rect.width / rect.height;
    const ndcX = ((point.x - rect.x) / rect.width) * 2 - 1, ndcY = 1 - ((point.y - rect.y) / rect.height) * 2;
    const half = camera.width / 2, offset = [0, 1, 2].map((i) => right[i] * ndcX * half + up[i] * ndcY * (half / aspect));
    return { ...camera, width: camera.width * factor, position: [0, 1, 2].map((i) => camera.position[i] + offset[i] * (1 - factor)) };
  }
  return { ...camera, position: [0, 1, 2].map((i) => target[i] + (camera.position[i] - target[i]) * factor) };
}

/**
 * 操作の状態。操作中はポインターを押したビューに固定し、枠の外へ出ても同じビューを動かす。
 * host は表示部品が渡す関数の組で、ビュー・当たり・カメラ・選択・通知を扱う。
 */
export class InputState {
  options = DEFAULTS;
  #host;
  #active = null;
  #touches = new Map();

  constructor(host) { this.#host = host; }

  get gesture() { return this.#active?.gesture ?? null; }

  /** 光線上の注視点（f64）。当たりがなければシーンの中心の奥行き、シーンもなければ 10 単位先。 */
  #pivot(view, at) {
    const { rect } = view;
    const point = { x: Math.min(Math.max(at.x, rect.x), rect.x + rect.width), y: Math.min(Math.max(at.y, rect.y), rect.y + rect.height) };
    const hit = this.#host.hitTest(view.id, point).hits[0];
    const ray = this.#host.getRay(view.id, point);
    let depth;
    if (hit !== undefined) depth = hit.distance;
    else {
      const center = this.#host.center();
      depth = center === null ? 10 : Math.max(dot(sub(center, ray.origin), ray.direction), 1e-6);
    }
    return { point: [0, 1, 2].map((i) => ray.origin[i] + ray.offset[i] + ray.direction[i] * depth), depth: Math.max(dot(scale(ray.direction, depth), normalize(view.camera.direction)), 1e-9) };
  }

  #start(gesture, view, input, extra = {}) {
    this.#active = { gesture, view: view.id, rect: view.rect, pointerId: input.pointerId, start: input.point, camera: view.camera, moved: false, ...extra };
    if (gesture !== "select" && gesture !== "tap") this.#host.emit("input", Object.freeze({ type: "input", action: "start", gesture, view: view.id }));
  }

  #end() {
    const active = this.#active;
    this.#active = null;
    if (active.gesture !== "select" && active.gesture !== "tap") this.#host.emit("input", Object.freeze({ type: "input", action: "end", gesture: active.gesture, view: active.view }));
  }

  #pick(viewId, point) {
    const { hits } = this.#host.hitTest(viewId, point);
    const first = hits[0];
    if (first === undefined) this.#host.select([]);
    else if (first.kind === "element") this.#host.select([first.id]);
    this.#host.emit("pick", Object.freeze({ type: "pick", view: viewId, point, hits }));
  }

  #moved(input) {
    const a = this.#active;
    if (!a.moved && Math.hypot(input.point.x - a.start.x, input.point.y - a.start.y) > this.options.dragThreshold) a.moved = true;
  }

  #twoFinger() {
    const [p, q] = [...this.#touches.values()];
    return { centroid: { x: (p.x + q.x) / 2, y: (p.y + q.y) / 2 }, distance: Math.max(Math.hypot(p.x - q.x, p.y - q.y), 1e-6) };
  }

  /** 入力を扱えば true。扱わない操作は DOM の既定の動作に残す。 */
  handle(input) {
    const o = this.options, host = this.#host;
    switch (input.type) {
      case "pointerdown": {
        if (input.pointerType === "touch") {
          if (!o.touch && !o.select) return false;
          if (this.#touches.size >= 2 || (this.#active !== null && this.#active.gesture !== "tap")) return false;
          if (this.#touches.size === 0) {
            const view = host.viewAt(input.point);
            if (view === null) return false;
            this.#touches.set(input.pointerId, input.point);
            this.#start("tap", view, input);
            return true;
          }
          if (!o.touch || this.#active === null) return false;
          this.#touches.set(input.pointerId, input.point);
          const view = host.view(this.#active.view);
          const { centroid, distance } = this.#twoFinger();
          this.#start("touch", view, input, { centroid, distance, pivot: this.#pivot(view, centroid) });
          return true;
        }
        if (this.#active !== null) return false;
        const view = host.viewAt(input.point);
        if (view === null) return false;
        if (input.button === 0 && o.select) { this.#start("select", view, input); return true; }
        if (input.button !== 1) return false;
        const gesture = input.shiftKey ? (o.orbit ? "orbit" : null) : (o.pan ? "pan" : null);
        if (gesture === null) return false;
        this.#start(gesture, view, input, { pivot: this.#pivot(view, input.point) });
        return true;
      }
      case "pointermove": {
        const a = this.#active;
        if (input.pointerType === "touch") {
          if (!this.#touches.has(input.pointerId) || a === null) return false;
          this.#touches.set(input.pointerId, input.point);
          if (a.gesture === "tap") { this.#moved(input); return true; }
          const { centroid, distance } = this.#twoFinger();
          const panned = panCamera(a.camera, a.rect, centroid.x - a.centroid.x, centroid.y - a.centroid.y, a.pivot.depth);
          const shift = sub(panned.position, a.camera.position);
          const target = [0, 1, 2].map((i) => a.pivot.point[i] + shift[i]);
          host.setCamera(a.view, zoomCamera(panned, a.rect, centroid, a.distance / distance, target));
          return true;
        }
        if (a === null || a.pointerId !== input.pointerId) return false;
        this.#moved(input);
        const dx = input.point.x - a.start.x, dy = input.point.y - a.start.y;
        if (a.gesture === "pan") host.setCamera(a.view, panCamera(a.camera, a.rect, dx, dy, a.pivot.depth));
        else if (a.gesture === "orbit") host.setCamera(a.view, orbitCamera(a.camera, a.pivot.point, -dx * o.rotateSpeed, -dy * o.rotateSpeed));
        return true;
      }
      case "pointerup": {
        const a = this.#active;
        if (input.pointerType === "touch") {
          if (!this.#touches.delete(input.pointerId)) return false;
          if (a === null) return true;
          if (a.gesture === "tap") {
            this.#active = null;
            if (!a.moved && o.select) this.#pick(a.view, input.point);
          } else if (this.#touches.size < 2) this.#end();
          return true;
        }
        if (a === null || a.pointerId !== input.pointerId) return false;
        if (a.gesture === "select") {
          this.#active = null;
          if (!a.moved) this.#pick(a.view, a.start);
        } else this.#end();
        return true;
      }
      case "pointercancel": {
        const involved = this.#touches.has(input.pointerId) || this.#active?.pointerId === input.pointerId;
        if (!involved) return false;
        const cleanup = this.reset("pointercancel");
        if (cleanup.failed) throw cleanup.error;
        return true;
      }
      case "wheel": {
        if (!o.zoom || !(input.ctrlKey || input.metaKey) || (this.#active !== null && this.#active.gesture !== "select")) return false;
        const view = host.viewAt(input.point);
        if (view === null) return false;
        const factor = Math.exp(input.deltaY * o.zoomSpeed);
        const target = view.camera.projection === "perspective" ? this.#pivot(view, input.point).point : null;
        host.setCamera(view.id, zoomCamera(view.camera, view.rect, input.point, factor, target));
        return true;
      }
      default: {
        if (input.key !== "Escape" || this.#active === null) return false;
        const cleanup = this.reset("escape");
        if (cleanup.failed) throw cleanup.error;
        return true;
      }
    }
  }

  /** 操作を中断する。中断の理由によっては元のカメラへ戻す。後始末の失敗は最初の例外として返す。 */
  reset(reason) {
    const active = this.#active;
    this.#active = null;
    this.#touches.clear();
    if (active === null) return { failed: false };
    let failed = false, error;
    const run = (fn) => { try { fn(); } catch (e) { if (!failed) { failed = true; error = e; } } };
    if (RESTORE.has(reason) && ["pan", "orbit", "touch"].includes(active.gesture)) {
      run(() => { if (this.#host.view(active.view) !== null) this.#host.setCamera(active.view, active.camera); });
    }
    if (active.gesture !== "select" && active.gesture !== "tap") {
      run(() => this.#host.emit("input", Object.freeze({ type: "input", action: "cancel", gesture: active.gesture, view: active.view, reason })));
    }
    return { failed, error };
  }
}

/** Pointer Events・UI Events を正規化して渡す。touch-action とフォーカスの指定は利用側の CSS/HTML に置く。 */
export function bindInput(viewer, element) {
  if (!element || typeof element.addEventListener !== "function" || typeof element.removeEventListener !== "function" || typeof element.getBoundingClientRect !== "function") invalid("入力の要素は DOM の要素");
  const listeners = [], captures = new Set(), document = element.ownerDocument, window = document?.defaultView;
  let connected = true;
  const on = (target, type, callback, options) => {
    if (!target) return;
    target.addEventListener(type, callback, options);
    listeners.push(() => target.removeEventListener(type, callback, options));
  };
  const modifiers = (event) => ({ shiftKey: event.shiftKey, ctrlKey: event.ctrlKey, altKey: event.altKey, metaKey: event.metaKey });
  const editable = (event) => {
    for (let target = event.target; target && target !== element; target = target.parentElement) {
      if (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(target.tagName)) return true;
    }
    return false;
  };
  // 要素の内容の箱を CSS px のビューの座標へ写す。枠線・余白と CSS の拡大を除く
  function point(event) {
    const rect = element.getBoundingClientRect(), size = viewer.getState().size, style = window?.getComputedStyle(element);
    const number = (name) => Number.parseFloat(style?.[name] ?? "0") || 0;
    const left = number("borderLeftWidth") + number("paddingLeft"), right = number("borderRightWidth") + number("paddingRight");
    const top = number("borderTopWidth") + number("paddingTop"), bottom = number("borderBottomWidth") + number("paddingBottom");
    const cssWidth = number("width"), cssHeight = number("height");
    const outerWidth = cssWidth > 0 ? cssWidth + (style.boxSizing === "border-box" ? 0 : left + right) : rect.width;
    const outerHeight = cssHeight > 0 ? cssHeight + (style.boxSizing === "border-box" ? 0 : top + bottom) : rect.height;
    const contentWidth = outerWidth - left - right, contentHeight = outerHeight - top - bottom;
    if (!(rect.width > 0 && rect.height > 0 && contentWidth > 0 && contentHeight > 0)) return null;
    return { x: (((event.clientX - rect.left) * outerWidth) / rect.width - left) * (size.width / contentWidth),
      y: (((event.clientY - rect.top) * outerHeight) / rect.height - top) * (size.height / contentHeight) };
  }
  function release(id) {
    if (!captures.delete(id)) return;
    try { if (element.hasPointerCapture?.(id)) element.releasePointerCapture(id); }
    catch (error) { if (!["NotFoundError", "InvalidStateError"].includes(error.name)) throw error; }
  }
  function cancel(reason) { if (connected) viewer.cancelInput(reason); }
  function send(event, input) {
    const handled = viewer.handleInput(input);
    if (handled && event.cancelable) event.preventDefault();
    return handled;
  }
  for (const type of ["pointerdown", "pointermove", "pointerup"]) on(element, type, (event) => {
    if (editable(event)) return;
    const at = point(event);
    if (at === null) return;
    const handled = send(event, { type, pointerId: event.pointerId, pointerType: event.pointerType || "mouse", button: event.button, point: at, ...modifiers(event) });
    if (type === "pointerdown" && handled && element.setPointerCapture) {
      try { element.setPointerCapture(event.pointerId); captures.add(event.pointerId); }
      catch (error) { if (!["NotFoundError", "InvalidStateError"].includes(error.name)) throw error; }
    }
    if (type === "pointerup") release(event.pointerId);
  }, { passive: false });
  on(element, "pointercancel", (event) => { release(event.pointerId); if (connected) viewer.handleInput({ type: "pointercancel", pointerId: event.pointerId, pointerType: event.pointerType || "mouse" }); });
  on(element, "lostpointercapture", (event) => { if (captures.delete(event.pointerId)) cancel("lostpointercapture"); });
  on(element, "wheel", (event) => {
    if (editable(event)) return;
    // UI Events §3.5: deltaMode は px・行・ページ。行は 16 CSS px とする
    const multiplier = event.deltaMode === 0 ? 1 : event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? viewer.getState().size.height : null;
    const at = point(event);
    if (multiplier !== null && at !== null) send(event, { type: "wheel", point: at, deltaY: event.deltaY * multiplier, ...modifiers(event) });
  }, { passive: false });
  on(element, "keydown", (event) => { if (!event.isComposing && !editable(event)) send(event, { type: "keydown", key: event.key, ...modifiers(event) }); });
  on(element, "blur", () => cancel("blur"));
  on(window, "blur", () => cancel("blur"));
  on(document, "visibilitychange", () => { if (document.hidden) cancel("hidden"); });
  return {
    disconnect() {
      if (!connected) return;
      connected = false;
      for (const off of listeners) off();
      listeners.length = 0;
      let failed = false, first;
      for (const id of [...captures]) {
        try { release(id); } catch (error) { if (!failed) { failed = true; first = error; } }
      }
      if (failed) throw first;
    },
  };
}
