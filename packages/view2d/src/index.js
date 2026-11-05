// シーンの列を直接描く表示部品（決定 0267）。図面の読み取り・字形の配置・曲線の展開はコアに任せる。
import { bounds, finite, limitsOf, prepare, valid } from "./validate.js";
import { paint, project, reportOf, unproject } from "./render.js";
import { appearance, emptyInteraction, hiddenLayers, indexElements, references } from "./interaction.js";
import { hitPoint, hitRect } from "./hit.js";

const EMPTY_REPORT = Object.freeze({ drawn: Object.freeze({ lines: 0, faces: 0, texts: 0, points: 0, images: 0, glyphs: 0 }), issues: Object.freeze([]) });
const EMPTY = Object.freeze([]);
const EVENTS = ["scene", "view", "size", "layers", "selection", "hover", "highlight"];
const equalList = (a, b) => a.length === b.length && a.every((value, i) => value === b[i]);

function point(value, name) {
  valid(value && typeof value === "object", name);
  return Object.freeze({ x: finite(value.x, `${name}.x`), y: finite(value.y, `${name}.y`) });
}

function camera(value) {
  valid(value && typeof value === "object", "view");
  const center = point(value.center, "view.center");
  const scale = finite(value.scale, "view.scale");
  valid(scale >= 1e-12 && scale <= 1e12, "view.scale");
  const rotation = finite(value.rotation, "view.rotation") % (2 * Math.PI);
  return Object.freeze({ center, scale, rotation });
}

function color(ctx, value, field) {
  valid(typeof value === "string", field);
  // 無効な CSS 色を Canvas が黙って無視することに頼らない。
  ctx.fillStyle = "#010203";
  ctx.fillStyle = value;
  const first = ctx.fillStyle;
  ctx.fillStyle = "#040506";
  ctx.fillStyle = value;
  valid(ctx.fillStyle === first, field);
  return first;
}

/** 呼び出し側が所有する Canvas に描く。CSS・イベント・外部資源の取得は持たない。 */
export class View2D {
  #canvas;
  #ctx;
  #settings;
  #limits;
  #prepared = null;
  #view;
  #size;
  #index = null;
  #interaction = emptyInteraction();
  #display = { hidden: new Set(), colors: new Map() };
  #snapshot;
  #listeners = new Set();
  #notifying = false;

  constructor(canvas, options = {}) {
    valid(canvas && typeof canvas.getContext === "function", "canvas");
    this.#limits = limitsOf(options.limits);
    this.#view = camera(options.view ?? { center: { x: 0, y: 0 }, scale: 1, rotation: 0 });
    const ctx = canvas.getContext("2d");
    valid(ctx !== null, "canvas.context2d");
    const settings = {
      background: color(ctx, options.background ?? "#ffffff", "background"),
      foreground: color(ctx, options.foreground ?? "#111111", "foreground"),
      unresolvedColor: color(ctx, options.unresolvedColor ?? "#777777", "unresolvedColor"),
      selectionColor: color(ctx, options.selectionColor ?? "#ff8800", "selectionColor"),
      hoverColor: color(ctx, options.hoverColor ?? "#00aacc", "hoverColor"),
      pixelsPerMillimeter: finite(options.pixelsPerMillimeter ?? 96 / 25.4, "pixelsPerMillimeter"),
      fallbackFont: options.fallbackFont ?? "sans-serif",
    };
    valid(settings.pixelsPerMillimeter > 0 && settings.pixelsPerMillimeter <= 1e6, "pixelsPerMillimeter");
    valid(["sans-serif", "serif", "monospace"].includes(settings.fallbackFont), "fallbackFont");
    this.#settings = Object.freeze(settings);
    this.#canvas = canvas;
    this.#ctx = ctx;
    this.resize(options.width ?? canvas.clientWidth ?? canvas.width, options.height ?? canvas.clientHeight ?? canvas.height, options.pixelRatio ?? 1);
  }

  #live() { if (!this.#ctx) throw new Error("View2D は破棄済み"); }
  #mutable() { this.#live(); valid(!this.#notifying, "state.notification"); }

  #publish(type) {
    this.#snapshot = Object.freeze({ scene: this.getScene(), view: this.#view, size: this.#size, layers: this.getLayers(), ...this.#interaction });
    const event = Object.freeze({ type, state: this.#snapshot });
    let failed = false, first;
    this.#notifying = true;
    try {
      // 通知中の解除は有効にし、追加は次の変更から通知する。
      for (const entry of [...this.#listeners]) {
        if (!this.#listeners.has(entry) || (entry.type && entry.type !== type)) continue;
        try { entry.listener(entry.type ? event : this.#snapshot); }
        catch (error) { if (!failed) { failed = true; first = error; } }
      }
    } finally { this.#notifying = false; }
    if (failed) throw first;
  }

  #listen(type, listener) {
    this.#live();
    valid(typeof listener === "function", "listener");
    valid(this.#listeners.size < this.#limits.stateItems, "limits.stateItems");
    const entry = { type, listener };
    this.#listeners.add(entry);
    return () => { this.#listeners.delete(entry); };
  }

  subscribe(listener) { return this.#listen(null, listener); }
  on(type, listener) { this.#live(); valid(EVENTS.includes(type), "event.type"); return this.#listen(type, listener); }

  /** Scene・応答・文書識別を一括で検査し、成功したときだけ差し替える。null は消去。 */
  setScene(input) {
    this.#mutable();
    let next = null, index = null;
    const display = { hidden: new Set(), colors: new Map() };
    if (input !== null) {
      next = prepare(input, this.#limits);
      index = indexElements(next);
      Object.assign(next, reportOf(next, this.#limits, display));
    }
    this.#prepared = next;
    this.#index = index;
    this.#interaction = emptyInteraction();
    this.#display = display;
    const report = this.render();
    this.#publish("scene");
    return report;
  }

  getScene() { this.#live(); return this.#prepared?.metadata ?? null; }
  getView() { this.#live(); return this.#view; }
  getSize() { this.#live(); return this.#size; }
  getReport() { this.#live(); return this.#prepared?.report ?? EMPTY_REPORT; }
  getLayers() { this.#live(); return this.#index?.layers ?? EMPTY; }
  getState() { this.#live(); return this.#snapshot; }

  #interact(next, type) {
    const previous = this.#interaction;
    if (equalList(previous.hiddenLayers, next.hiddenLayers) && equalList(previous.selection, next.selection) &&
      previous.hovered === next.hovered && equalList(previous.highlights, next.highlights)) return this.getReport();
    const display = appearance(next, this.#index, this.#settings, this.#limits);
    let prepared = this.#prepared;
    if (prepared && !equalList(previous.hiddenLayers, next.hiddenLayers)) prepared = { ...prepared, ...reportOf(prepared, this.#limits, display) };
    this.#prepared = prepared;
    this.#interaction = Object.freeze(next);
    this.#display = display;
    const report = this.render();
    this.#publish(type);
    return report;
  }

  setHiddenLayers(ids) {
    this.#mutable();
    return this.#interact({ ...this.#interaction, hiddenLayers: hiddenLayers(ids, this.#index, this.#limits) }, "layers");
  }

  setLayersVisible(ids, visible) {
    this.#mutable(); valid(typeof visible === "boolean", "visible");
    const changed = hiddenLayers(ids, this.#index, this.#limits);
    const next = new Set(this.#interaction.hiddenLayers);
    for (const id of changed) { if (visible) next.delete(id); else next.add(id); }
    return this.setHiddenLayers([...next]);
  }

  select(elements, mode = "replace") {
    this.#mutable(); valid(["replace", "add", "remove", "toggle"].includes(mode), "selection.mode");
    const refs = references(elements, this.#index, this.#limits);
    const next = new Set(mode === "replace" ? [] : this.#interaction.selection);
    for (const ref of refs) {
      if (mode === "remove" || (mode === "toggle" && next.has(ref))) next.delete(ref);
      else next.add(ref);
    }
    return this.#interact({ ...this.#interaction, selection: Object.freeze([...next]) }, "selection");
  }

  setHovered(element) {
    this.#mutable();
    const hovered = element === null ? null : references([element], this.#index, this.#limits)[0];
    return this.#interact({ ...this.#interaction, hovered }, "hover");
  }

  setHighlight(name, elements, value = "#cc00cc") {
    this.#mutable(); valid(typeof name === "string" && name.length > 0 && name.length <= this.#limits.textLength, "highlight.name");
    const refs = references(elements, this.#index, this.#limits);
    const style = color(this.#ctx, value, "highlight.color");
    const highlights = this.#interaction.highlights.filter((group) => group.name !== name);
    if (refs.length) {
      const previous = this.#interaction.highlights.find((group) => group.name === name);
      highlights.push(previous?.color === style && equalList(previous.elements, refs) ? previous : Object.freeze({ name, color: style, elements: refs }));
    }
    return this.#interact({ ...this.#interaction, highlights: Object.freeze(highlights) }, "highlight");
  }

  hitTest(value, tolerance = 3) {
    this.#live(); const p = point(value, "point"); finite(tolerance, "tolerance"); valid(tolerance >= 0, "tolerance");
    return this.#prepared ? hitPoint(this.#ctx, this.#prepared, this.#settings, this.#view, this.#size, this.#display, this.#index, this.#limits, p, tolerance) : EMPTY;
  }

  hitTestRect(value) {
    this.#live(); const rect = bounds(value);
    return this.#prepared ? hitRect(this.#ctx, this.#prepared, this.#settings, this.#view, this.#size, this.#display, this.#index, this.#limits, rect) : EMPTY;
  }

  render() {
    this.#live();
    paint(this.#ctx, this.#prepared, this.#settings, this.#view, this.#size, this.#display);
    return this.getReport();
  }

  jumpTo(view) {
    this.#mutable(); const next = camera(view), previous = this.#view;
    if (next.center.x === previous.center.x && next.center.y === previous.center.y && next.scale === previous.scale && next.rotation === previous.rotation) return this.getReport();
    this.#view = next;
    const report = this.render(); this.#publish("view"); return report;
  }

  /** 回転を保ち、起点の座標の範囲を CSS px の余白を空けて収める。 */
  fitBounds(value, padding = 16) {
    this.#live();
    const box = bounds(value);
    finite(padding, "padding");
    valid(padding >= 0 && 2 * padding < Math.min(this.#size.width, this.#size.height), "padding");
    const w = box.max_x - box.min_x, h = box.max_y - box.min_y;
    const cos = Math.abs(Math.cos(this.#view.rotation)), sin = Math.abs(Math.sin(this.#view.rotation));
    const dx = cos * w + sin * h, dy = sin * w + cos * h;
    const scale = dx === 0 && dy === 0 ? this.#view.scale : Math.min(
      dx ? (this.#size.width - 2 * padding) / dx : Infinity,
      dy ? (this.#size.height - 2 * padding) / dy : Infinity,
    );
    return this.jumpTo({ center: { x: box.min_x + w / 2, y: box.min_y + h / 2 }, scale: Math.max(1e-12, Math.min(1e12, scale)), rotation: this.#view.rotation });
  }

  fit(padding = 16) {
    this.#live();
    const extent = this.#prepared?.metadata.result.extent;
    return extent ? this.fitBounds(extent, padding) : this.getReport();
  }

  #around(scale, rotation, anchor) {
    const p = point(anchor ?? { x: this.#size.width / 2, y: this.#size.height / 2 }, "anchor");
    const next = camera({ ...this.#view, scale, rotation });
    // 固定点の差も中心からの相対座標で求め、大きい中心を二度足して丸めない。
    const center = { x: 0, y: 0 };
    const before = unproject({ ...this.#view, center }, this.#size.width, this.#size.height, p.x, p.y);
    const after = unproject({ ...next, center }, this.#size.width, this.#size.height, p.x, p.y);
    return this.jumpTo({ ...next, center: { x: next.center.x + (before.x - after.x), y: next.center.y + (before.y - after.y) } });
  }

  zoomBy(factor, anchor) {
    this.#live(); finite(factor, "factor"); valid(factor > 0, "factor");
    return this.#around(this.#view.scale * factor, this.#view.rotation, anchor);
  }

  rotateBy(radians, anchor) {
    this.#live(); finite(radians, "radians");
    return this.#around(this.#view.scale, this.#view.rotation + radians, anchor);
  }

  /** 描画が画面上で dx・dy CSS px 動く向きに移動する。 */
  panBy(dx, dy) {
    this.#live(); finite(dx, "dx"); finite(dy, "dy");
    const cos = Math.cos(this.#view.rotation), sin = Math.sin(this.#view.rotation);
    return this.jumpTo({ ...this.#view, center: {
      x: this.#view.center.x - (cos * dx - sin * dy) / this.#view.scale,
      y: this.#view.center.y + (sin * dx + cos * dy) / this.#view.scale,
    } });
  }

  toScreen(value) { this.#live(); const p = point(value, "point"); return project(this.#view, this.#size.width, this.#size.height, p.x, p.y); }
  toScene(value) { this.#live(); const p = point(value, "point"); return unproject(this.#view, this.#size.width, this.#size.height, p.x, p.y); }

  resize(width, height, pixelRatio = this.#size?.pixelRatio ?? 1) {
    this.#mutable(); finite(width, "width"); finite(height, "height"); finite(pixelRatio, "pixelRatio");
    valid(width > 0 && height > 0 && pixelRatio > 0 && pixelRatio <= 8, "size");
    const pixelsX = Math.ceil(width * pixelRatio), pixelsY = Math.ceil(height * pixelRatio);
    valid(pixelsX <= 16384 && pixelsY <= 16384 && pixelsX * pixelsY <= this.#limits.canvasPixels, "limits.canvasPixels");
    if (this.#size?.width === width && this.#size?.height === height && this.#size?.pixelRatio === pixelRatio) return this.getReport();
    this.#canvas.width = pixelsX;
    this.#canvas.height = pixelsY;
    this.#size = Object.freeze({ width, height, pixelRatio });
    const report = this.render(); this.#publish("size"); return report;
  }

  /** Canvas の所有権は呼び出し側に残す。繰り返して呼べる。 */
  destroy() {
    if (!this.#ctx) return;
    this.#mutable();
    this.#ctx.setTransform(1, 0, 0, 1, 0, 0);
    this.#ctx.clearRect(0, 0, this.#canvas.width, this.#canvas.height);
    this.#prepared = null;
    this.#index = null;
    this.#interaction = emptyInteraction();
    this.#display = { hidden: new Set(), colors: new Map() };
    this.#snapshot = null;
    this.#listeners.clear();
    this.#ctx = null;
    this.#canvas = null;
  }
}
