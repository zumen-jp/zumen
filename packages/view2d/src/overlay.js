// WebGPU 描画パスへ直接図形を発行する重ね描き用の 2D コンテキスト。
// addOverlay のコールバックが使う Canvas 2D 互換の操作（変換・線・破線・塗り・円弧・文字・画像）を提供する。
import { premultipliedRgba, tryParseCssColor } from "./color.js";
import { rasterizeTransformedText } from "./text-raster.js";

function multiplyMatrix([a1, b1, c1, d1, e1, f1], [a2, b2, c2, d2, e2, f2]) {
  return [
    a1 * a2 + c1 * b2,
    b1 * a2 + d1 * b2,
    a1 * c2 + c1 * d2,
    b1 * c2 + d1 * d2,
    a1 * e2 + c1 * f2 + e1,
    b1 * e2 + d1 * f2 + f1,
  ];
}

function dashSubpaths(subpaths, rawDash) {
  const dash = rawDash.filter((v) => Number.isFinite(v) && v >= 0);
  if (dash.length === 0 || dash.every((v) => v === 0)) return subpaths;
  const pattern = dash.length % 2 === 1 ? [...dash, ...dash] : dash;
  const out = [];
  for (const sub of subpaths) {
    const coords = sub.coords;
    const count = coords.length >> 1;
    if (count < 2) continue;
    const segCount = sub.closed ? count : count - 1;
    let patIdx = 0;
    let patRem = pattern[0];
    let current = null;
    for (let s = 0; s < segCount; s++) {
      let x0 = coords[s * 2], y0 = coords[s * 2 + 1];
      const next = (s + 1) % count;
      const x1 = coords[next * 2], y1 = coords[next * 2 + 1];
      const dx = x1 - x0, dy = y1 - y0;
      let rem = Math.hypot(dx, dy);
      if (!(rem > 0)) continue;
      const ux = dx / rem, uy = dy / rem;
      while (rem > 1e-6) {
        if (patRem <= 1e-6) {
          patIdx = (patIdx + 1) % pattern.length;
          patRem = pattern[patIdx];
          if (patRem <= 1e-6) continue;
        }
        const step = Math.min(rem, patRem);
        const nx = x0 + ux * step, ny = y0 + uy * step;
        if (patIdx % 2 === 0) {
          if (!current) {
            current = [x0, y0, nx, ny];
          } else {
            current.push(nx, ny);
          }
        } else if (current) {
          out.push({ coords: current, closed: false });
          current = null;
        }
        x0 = nx; y0 = ny;
        rem -= step;
        patRem -= step;
      }
    }
    if (current) out.push({ coords: current, closed: false });
  }
  return out;
}

export class OverlayContext2D {
  #surface;
  #size;
  #stack = [];
  #matrix;
  #strokeStyle = "#000000";
  #fillStyle = "#000000";
  #lineWidth = 1;
  #lineCap = "butt";
  #lineJoin = "miter";
  #globalAlpha = 1;
  #font = "10px sans-serif";
  #textAlign = "start";
  #textBaseline = "alphabetic";
  #lineDash = [];
  #subpaths = [];
  #current = null;

  constructor(surface, size) {
    this.#surface = surface;
    this.#size = size;
    this.#matrix = [size.pixelRatio, 0, 0, size.pixelRatio, 0, 0];
  }

  get strokeStyle() { return this.#strokeStyle; }
  set strokeStyle(v) { if (typeof v === "string" && tryParseCssColor(v)) this.#strokeStyle = v; }

  get fillStyle() { return this.#fillStyle; }
  set fillStyle(v) { if (typeof v === "string" && tryParseCssColor(v)) this.#fillStyle = v; }

  get lineWidth() { return this.#lineWidth; }
  set lineWidth(v) { if (typeof v === "number" && Number.isFinite(v) && v > 0) this.#lineWidth = v; }

  get lineCap() { return this.#lineCap; }
  set lineCap(v) { if (v === "butt" || v === "round" || v === "square") this.#lineCap = v; }

  get lineJoin() { return this.#lineJoin; }
  set lineJoin(v) { if (v === "round" || v === "bevel" || v === "miter") this.#lineJoin = v; }

  get globalAlpha() { return this.#globalAlpha; }
  set globalAlpha(v) { if (typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= 1) this.#globalAlpha = v; }

  get font() { return this.#font; }
  set font(v) { if (typeof v === "string" && v.trim() !== "") this.#font = v; }

  get textAlign() { return this.#textAlign; }
  set textAlign(v) { if (["left", "right", "center", "start", "end"].includes(v)) this.#textAlign = v; }

  get textBaseline() { return this.#textBaseline; }
  set textBaseline(v) { if (["top", "hanging", "middle", "alphabetic", "ideographic", "bottom"].includes(v)) this.#textBaseline = v; }

  save() {
    this.#stack.push({
      matrix: [...this.#matrix],
      strokeStyle: this.#strokeStyle,
      fillStyle: this.#fillStyle,
      lineWidth: this.#lineWidth,
      lineCap: this.#lineCap,
      lineJoin: this.#lineJoin,
      globalAlpha: this.#globalAlpha,
      font: this.#font,
      textAlign: this.#textAlign,
      textBaseline: this.#textBaseline,
      lineDash: [...this.#lineDash],
    });
  }

  restore() {
    const prev = this.#stack.pop();
    if (!prev) return;
    this.#matrix = prev.matrix;
    this.#strokeStyle = prev.strokeStyle;
    this.#fillStyle = prev.fillStyle;
    this.#lineWidth = prev.lineWidth;
    this.#lineCap = prev.lineCap;
    this.#lineJoin = prev.lineJoin;
    this.#globalAlpha = prev.globalAlpha;
    this.#font = prev.font;
    this.#textAlign = prev.textAlign;
    this.#textBaseline = prev.textBaseline;
    this.#lineDash = prev.lineDash;
  }

  setTransform(a, b, c, d, e, f) {
    if (typeof a === "object" && a !== null) {
      this.#matrix = [a.a ?? 1, a.b ?? 0, a.c ?? 0, a.d ?? 1, a.e ?? 0, a.f ?? 0];
    } else {
      this.#matrix = [a, b, c, d, e, f];
    }
  }

  resetTransform() {
    this.#matrix = [1, 0, 0, 1, 0, 0];
  }

  transform(a, b, c, d, e, f) {
    this.#matrix = multiplyMatrix(this.#matrix, [a, b, c, d, e, f]);
  }

  translate(x, y) {
    this.transform(1, 0, 0, 1, x, y);
  }

  rotate(angle) {
    const cos = Math.cos(angle), sin = Math.sin(angle);
    this.transform(cos, sin, -sin, cos, 0, 0);
  }

  scale(x, y) {
    this.transform(x, 0, 0, y, 0, 0);
  }

  getTransform() {
    const [a, b, c, d, e, f] = this.#matrix;
    if (typeof DOMMatrix !== "undefined") return new DOMMatrix([a, b, c, d, e, f]);
    return { a, b, c, d, e, f };
  }

  setLineDash(segments) {
    if (Array.isArray(segments)) this.#lineDash = segments.filter((n) => typeof n === "number" && Number.isFinite(n) && n >= 0);
  }

  getLineDash() {
    return [...this.#lineDash];
  }

  #toCss(x, y) {
    const [a, b, c, d, e, f] = this.#matrix;
    const pr = this.#size.pixelRatio;
    return [(a * x + c * y + e) / pr, (b * x + d * y + f) / pr];
  }

  #cssScale() {
    const [a, b, c, d] = this.#matrix;
    return (Math.hypot(a, b) + Math.hypot(c, d)) / (2 * this.#size.pixelRatio);
  }

  beginPath() {
    this.#subpaths = [];
    this.#current = null;
  }

  moveTo(x, y) {
    if (!Number.isFinite(x) || !Number.isFinite(y)) return;
    const [cx, cy] = this.#toCss(x, y);
    this.#current = { coords: [cx, cy], closed: false };
    this.#subpaths.push(this.#current);
  }

  lineTo(x, y) {
    if (!Number.isFinite(x) || !Number.isFinite(y)) return;
    const [cx, cy] = this.#toCss(x, y);
    if (!this.#current) {
      this.#current = { coords: [cx, cy], closed: false };
      this.#subpaths.push(this.#current);
    } else {
      this.#current.coords.push(cx, cy);
    }
  }

  closePath() {
    if (this.#current && this.#current.coords.length >= 2) {
      this.#current.closed = true;
      const [x0, y0] = this.#current.coords;
      this.#current = { coords: [x0, y0], closed: false };
      this.#subpaths.push(this.#current);
    }
  }

  rect(x, y, w, h) {
    if (![x, y, w, h].every(Number.isFinite)) return;
    const p0 = this.#toCss(x, y);
    const p1 = this.#toCss(x + w, y);
    const p2 = this.#toCss(x + w, y + h);
    const p3 = this.#toCss(x, y + h);
    this.#subpaths.push({ coords: [...p0, ...p1, ...p2, ...p3], closed: true });
    this.#current = { coords: [...p0], closed: false };
  }

  arc(x, y, radius, startAngle, endAngle, counterclockwise = false) {
    if (![x, y, radius, startAngle, endAngle].every(Number.isFinite) || radius < 0) return;
    let sweep = endAngle - startAngle;
    const tau = 2 * Math.PI;
    if (!counterclockwise) {
      if (sweep >= tau) sweep = tau;
      else if (sweep < 0) sweep = ((sweep % tau) + tau) % tau;
    } else {
      if (sweep <= -tau) sweep = -tau;
      else if (sweep > 0) sweep = ((sweep % tau) - tau) % tau;
    }
    const steps = Math.max(12, Math.min(512, Math.ceil(Math.abs(sweep) * Math.max(1, Math.sqrt(radius * this.#cssScale() * 4)))));
    for (let i = 0; i <= steps; i++) {
      const a = startAngle + (sweep * i) / steps;
      const px = x + radius * Math.cos(a);
      const py = y + radius * Math.sin(a);
      if (i === 0 && !this.#current) this.moveTo(px, py);
      else this.lineTo(px, py);
    }
  }

  stroke() {
    const color = premultipliedRgba(this.#strokeStyle, this.#globalAlpha);
    if (color[3] <= 0 || this.#subpaths.length === 0) return;
    const width = Math.max(1 / this.#size.pixelRatio, this.#lineWidth * this.#cssScale());
    const paths = this.#lineDash.length > 0 ? dashSubpaths(this.#subpaths, this.#lineDash.map((d) => d * this.#cssScale())) : this.#subpaths;
    this.#surface.drawStroke(paths, [], width, color);
  }

  fill(fillRule = "nonzero") {
    const color = premultipliedRgba(this.#fillStyle, this.#globalAlpha);
    if (color[3] <= 0 || this.#subpaths.length === 0) return;
    const rule = fillRule === "evenodd" ? "evenodd" : "nonzero";
    this.#surface.drawFill(this.#subpaths, [], rule, color);
  }

  fillRect(x, y, w, h) {
    if (![x, y, w, h].every(Number.isFinite) || w === 0 || h === 0) return;
    const color = premultipliedRgba(this.#fillStyle, this.#globalAlpha);
    if (color[3] <= 0) return;
    const p0 = this.#toCss(x, y);
    const p1 = this.#toCss(x + w, y);
    const p2 = this.#toCss(x + w, y + h);
    const p3 = this.#toCss(x, y + h);
    this.#surface.drawFill([{ coords: [...p0, ...p1, ...p2, ...p3], closed: true }], [], "nonzero", color);
  }

  strokeRect(x, y, w, h) {
    if (![x, y, w, h].every(Number.isFinite) || (w === 0 && h === 0)) return;
    const color = premultipliedRgba(this.#strokeStyle, this.#globalAlpha);
    if (color[3] <= 0) return;
    const p0 = this.#toCss(x, y);
    const p1 = this.#toCss(x + w, y);
    const p2 = this.#toCss(x + w, y + h);
    const p3 = this.#toCss(x, y + h);
    const width = Math.max(1 / this.#size.pixelRatio, this.#lineWidth * this.#cssScale());
    const raw = [{ coords: [...p0, ...p1, ...p2, ...p3], closed: true }];
    const paths = this.#lineDash.length > 0 ? dashSubpaths(raw, this.#lineDash.map((d) => d * this.#cssScale())) : raw;
    this.#surface.drawStroke(paths, [], width, color);
  }

  fillText(text, x, y) {
    if (!text || !Number.isFinite(x) || !Number.isFinite(y) || this.#globalAlpha <= 0) return;
    const m = multiplyMatrix(this.#matrix, [1, 0, 0, 1, x, y]);
    const pr = this.#size.pixelRatio;
    const cw = Math.ceil(this.#size.width * pr);
    const ch = Math.ceil(this.#size.height * pr);
    const align = this.#textAlign === "start" ? "left" : this.#textAlign === "end" ? "right" : this.#textAlign;
    const patch = rasterizeTransformedText({
      text: String(text),
      font: this.#font,
      textAlign: align,
      textBaseline: this.#textBaseline,
      fillStyle: this.#fillStyle,
      matrix: m,
      canvasWidth: cw,
      canvasHeight: ch,
    });
    if (!patch) return;
    this.#surface.drawImageQuad(
      patch.data,
      patch.width,
      patch.height,
      [patch.width / pr, 0, 0, patch.height / pr, patch.x0 / pr, patch.y0 / pr],
      this.#globalAlpha,
      true,
    );
  }

  drawImage(source, dx, dy, dw, dh) {
    if (!source || !Number.isFinite(dx) || !Number.isFinite(dy) || this.#globalAlpha <= 0) return;
    const sw = source.width, sh = source.height;
    if (!(sw > 0 && sh > 0)) return;
    const w = dw === undefined ? sw : dw;
    const h = dh === undefined ? sh : dh;
    if (!Number.isFinite(w) || !Number.isFinite(h) || w === 0 || h === 0) return;
    const p0 = this.#toCss(dx, dy);
    const px = this.#toCss(dx + w, dy);
    const py = this.#toCss(dx, dy + h);
    this.#surface.drawImageQuad(
      source,
      sw,
      sh,
      [px[0] - p0[0], px[1] - p0[1], py[0] - p0[0], py[1] - p0[1], p0[0], p0[1]],
      this.#globalAlpha,
      false,
    );
  }
}
