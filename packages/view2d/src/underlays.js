import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { end, rows, valid } from "./validate.js";
import { project, stringAt } from "./geometry.js";
import { unitVisible } from "./interaction.js";

const EMPTY = Object.freeze([]);
const safe = (value) => Number.isFinite(value) && Math.abs(value) <= 1e150;
const samePlan = (a, b) => a && b && a.width === b.width && a.height === b.height &&
  ["min_x", "min_y", "max_x", "max_y"].every((key) => a.region[key] === b.region[key]);

/** 左下原点の内容から CSS px へ。原点を足して引く丸めを避け、辺は別に変換する。 */
function frameOf(c, row, chunk, extent, view, size) {
  const p = project(view, size.width, size.height, c.image_xy[row * 2], c.image_xy[row * 2 + 1],
    c.chunk_origins[chunk * 2], c.chunk_origins[chunk * 2 + 1]);
  const cos = Math.cos(view.rotation), sin = Math.sin(view.rotation);
  const vector = (x, y, scale) => ({ x: view.scale * (cos * x - sin * y) * scale, y: -view.scale * (sin * x + cos * y) * scale });
  const unit = c.image_frames[row] === V.image_frame.unit;
  const u = vector(c.image_edges[row * 4], c.image_edges[row * 4 + 1], unit ? extent.width : 1);
  const v = vector(c.image_edges[row * 4 + 2], c.image_edges[row * 4 + 3], unit ? extent.height : 1);
  if (![p.x, p.y, u.x, u.y, v.x, v.y].every(safe)) return null;
  const scale = Math.max(Math.abs(u.x), Math.abs(u.y), Math.abs(v.x), Math.abs(v.y));
  const ux = u.x / scale, uy = u.y / scale, vx = v.x / scale, vy = v.y / scale, det = ux * vy - uy * vx;
  if (!Number.isFinite(det) || det === 0) return null;
  const inverse = ({ x, y }) => ({ x: (((x - p.x) / scale) * vy - ((y - p.y) / scale) * vx) / det,
    y: (ux * ((y - p.y) / scale) - uy * ((x - p.x) / scale)) / det });
  return { p, u, v, inverse };
}

// 画面と、内側を残す切り抜きの外形まで要求を絞る。反転と正確な共通部分は Canvas が切る。
function windowOf(c, unit, view, size) {
  const box = { min_x: 0, min_y: 0, max_x: size.width, max_y: size.height }, clip = c.unit_clips[unit];
  if (clip === SCENE_NONE) return box;
  for (let p = c.clip_polygons[clip]; p < end(c.clip_polygons, clip, c.polygon_vertices.length); p++) {
    if (c.polygon_flags[p] > 1) return "clip-unsupported";
    if (c.polygon_flags[p] === 1) continue;
    const b = { min_x: Infinity, min_y: Infinity, max_x: -Infinity, max_y: -Infinity };
    for (let n = c.polygon_vertices[p]; n < end(c.polygon_vertices, p, rows(c, "clip_xy")); n++) {
      const at = project(view, size.width, size.height, c.clip_xy[n * 2], c.clip_xy[n * 2 + 1]);
      b.min_x = Math.min(b.min_x, at.x); b.min_y = Math.min(b.min_y, at.y);
      b.max_x = Math.max(b.max_x, at.x); b.max_y = Math.max(b.max_y, at.y);
    }
    box.min_x = Math.max(box.min_x, b.min_x); box.min_y = Math.max(box.min_y, b.min_y);
    box.max_x = Math.min(box.max_x, b.max_x); box.max_y = Math.min(box.max_y, b.max_y);
  }
  return box;
}

function planOf(frame, extent, box, size, limits) {
  if (box.min_x >= box.max_x || box.min_y >= box.max_y) return "resource-outside-view";
  const { p, u, v, inverse } = frame;
  let polygon = [p, { x: p.x + u.x, y: p.y + u.y }, { x: p.x + u.x + v.x, y: p.y + u.y + v.y }, { x: p.x + v.x, y: p.y + v.y }];
  // 凸な四角を画面の四辺で切る。各段は最大 8 頂点で、回数を固定する。
  for (const [axis, bound, positive] of [["x", box.min_x, true], ["x", box.max_x, false], ["y", box.min_y, true], ["y", box.max_y, false]]) {
    const next = [], inside = (q) => positive ? q[axis] >= bound : q[axis] <= bound;
    for (let i = 0; i < polygon.length; i++) {
      const a = polygon[i], b = polygon[(i + 1) % polygon.length], ai = inside(a), bi = inside(b);
      if (ai) next.push(a);
      if (ai !== bi) { const t = (bound - a[axis]) / (b[axis] - a[axis]); next.push({ x: a.x + t * (b.x - a.x), y: a.y + t * (b.y - a.y) }); }
    }
    polygon = next;
  }
  if (polygon.length < 3) return "resource-outside-view";
  const points = polygon.map(inverse);
  const clamp = (x) => Math.max(0, Math.min(1, x));
  const left = clamp(Math.min(...points.map((q) => q.x))), right = clamp(Math.max(...points.map((q) => q.x)));
  const bottom = clamp(Math.min(...points.map((q) => q.y))), top = clamp(Math.max(...points.map((q) => q.y)));
  if (![left, right, bottom, top].every(Number.isFinite)) return "resource-frame-invalid";
  if (left >= right || bottom >= top) return "resource-outside-view";
  const width = Math.max(1, Math.ceil(Math.hypot(u.x, u.y) * (right - left) * size.pixelRatio));
  const height = Math.max(1, Math.ceil(Math.hypot(v.x, v.y) * (top - bottom) * size.pixelRatio));
  if (width > limits.underlaySide || height > limits.underlaySide || width * height > limits.underlayPixels) return "resource-limit";
  const region = Object.freeze({ min_x: left * extent.width, min_y: bottom * extent.height, max_x: right * extent.width, max_y: top * extent.height });
  if (!Object.values(region).every(safe)) return "resource-frame-invalid";
  return { frame, left, right, bottom, top, region, width, height };
}

/** 利用側の関数には描画先を渡さない。現行の要求だけを後から合成する（決定 0278）。 */
export class Underlays {
  constructor(limits, changed) {
    this.limits = limits; this.changed = changed; this.renderer = null; this.records = []; this.state = EMPTY;
    this.leases = new Set(); this.jobs = 0; this.pixels = 0; this.queued = false; this.closed = false; this.paused = false;
    this.calling = false;
    this.requestLimits = Object.freeze({ maxBytes: limits.underlayBytes, maxPixels: limits.underlayPixels, maxSide: limits.underlaySide });
  }

  invoke(fn) { this.calling = true; try { return fn(); } finally { this.calling = false; } }

  notify() {
    if (this.closed || this.queued) return;
    this.queued = true;
    queueMicrotask(() => { this.queued = false; if (!this.closed) this.changed(); });
  }

  release(value, lease, rasterOwner) {
    const { record } = lease;
    const failed = () => {
      if (this.records[record.row] !== record || record.owner !== lease.owner) return;
      if (rasterOwner !== undefined && lease.rasterOwner !== rasterOwner) return;
      record.failure = "resource-release-failed";
      this.notify();
    };
    try {
      const result = this.invoke(() => typeof value?.dispose === "function" ? value.dispose() : undefined);
      if (result && typeof result.then === "function") return Promise.resolve(result).catch(failed);
    } catch { failed(); }
  }

  dropPixels(record) {
    if (!record.raster) return;
    const raster = record.raster; record.raster = null;
    const lease = record.lease;
    lease.rasterReleases++;
    const done = () => { this.pixels -= raster.pixels; lease.rasterReleases--; this.finishLease(lease); };
    const result = this.release(raster.value, lease, raster.owner);
    if (result) result.then(() => { done(); this.notify(); }); else done();
  }

  finishLease(lease) {
    if (!lease.retired || lease.busy || lease.releasing || lease.rasterReleases) return;
    const done = () => { if (this.leases.delete(lease)) this.notify(); };
    if (lease.resource) {
      const resource = lease.resource; lease.resource = null; lease.releasing = true;
      const result = this.release(resource, lease);
      if (result) { result.then(done); return; }
    }
    done();
  }

  stop(record) {
    this.dropPixels(record);
    const lease = record.lease; record.lease = null;
    if (!lease) return;
    lease.retired = true; this.invoke(() => { lease.control.abort(); lease.operation?.control.abort(); });
    this.finishLease(lease);
  }

  reset() {
    for (const record of this.records) this.stop(record);
    this.records = []; this.state = EMPTY; this.columns = null; this.document = null; this.paused = false;
  }

  scene(prepared, index) {
    this.reset();
    if (!prepared) return;
    const c = prepared.columns;
    this.columns = c; this.document = prepared.metadata.document;
    for (let chunk = 0; chunk < prepared.order.length; chunk++) {
      for (let row = c.chunk_images[chunk]; row < end(c.chunk_images, chunk, c.image_units.length); row++) {
        const unit = c.image_units[row];
        const descriptor = Object.freeze({ index: row, element: index.elements[c.unit_entities[unit]], document: this.document,
          kind: ["image", "pdf", "dwf", "dgn"][c.image_kinds[row]] ?? "unknown", name: stringAt(c, "image_names", row), page: stringAt(c, "image_pages", row) });
        this.records.push({ row, chunk, unit, descriptor, extent: null, lease: null, raster: null, plan: null, failure: null, owner: null });
      }
    }
  }

  setRenderer(renderer) { valid(renderer === null || typeof renderer === "function", "underlayRenderer"); this.renderer = renderer; this.refresh(); }
  refresh() { this.paused = false; for (const record of this.records) { record.owner = null; this.stop(record); record.extent = null; record.failure = null; } }
  cancel() { this.paused = true; for (const record of this.records) this.stop(record); }
  destroy() { this.closed = true; this.reset(); this.renderer = null; this.changed = null; }

  resolve(record) {
    const lease = { record, owner: Symbol(), rasterOwner: null, control: new AbortController(), retired: false, busy: true, resource: null, operation: null, rasterReleases: 0, releasing: false };
    record.owner = lease.owner;
    record.lease = lease; this.leases.add(lease); this.jobs++;
    const renderer = this.renderer;
    const request = Object.freeze({ ...record.descriptor, signal: lease.control.signal, limits: this.requestLimits });
    Promise.resolve().then(() => lease.retired ? null : this.invoke(() => renderer(request))).then((value) => {
      if (lease.retired) return this.release(value, lease);
      try {
        const width = value?.width, height = value?.height;
        if (!value || !safe(width) || !safe(height) || width <= 0 || height <= 0 || typeof value.render !== "function" || typeof value.dispose !== "function") throw new Error("underlay.resource");
        record.extent = Object.freeze({ width, height });
        lease.resource = value;
      } catch {
        record.failure = "resource-invalid"; return this.release(value, lease);
      }
    }).catch(() => { if (!lease.retired) record.failure = "resource-resolve-failed"; }).finally(() => {
      lease.busy = false; this.jobs--; this.finishLease(lease); this.notify();
    });
  }

  rasterize(record, plan) {
    const lease = record.lease, pixels = plan.width * plan.height;
    const operation = { owner: Symbol(), control: new AbortController(), plan, pixels };
    lease.rasterOwner = operation.owner;
    lease.operation = operation; lease.busy = true; this.jobs++; this.pixels += pixels;
    const request = Object.freeze({ ...record.descriptor, signal: operation.control.signal, limits: this.requestLimits,
      region: plan.region, width: plan.width, height: plan.height });
    Promise.resolve().then(() => operation.control.signal.aborted ? null : this.invoke(() => lease.resource.render(request))).then((value) => {
      if (lease.retired || operation.control.signal.aborted) return this.release(value, lease, operation.owner);
      try {
        const source = value?.source, tag = Object.prototype.toString.call(source);
        if (!["[object ImageBitmap]", "[object HTMLCanvasElement]", "[object OffscreenCanvas]"].includes(tag) || source.width !== plan.width || source.height !== plan.height || typeof value.dispose !== "function") throw new Error("underlay.raster");
        record.raster = { value, pixels, plan, owner: operation.owner }; operation.pixels = 0;
      } catch {
        record.failure = "resource-invalid"; return this.release(value, lease, operation.owner);
      }
    }).catch(() => { if (!lease.retired && !operation.control.signal.aborted) record.failure = "resource-render-failed"; }).finally(() => {
      this.pixels -= operation.pixels; lease.busy = false; lease.operation = null; this.jobs--; this.finishLease(lease); this.notify();
    });
  }

  update(view, size, display) {
    const c = this.columns;
    for (const record of this.records) {
      const { row, unit, chunk } = record;
      let reason = record.failure;
      if (!unitVisible(c, unit, display)) reason = "resource-outside-view";
      else if (!reason && ![V.image_kind.image, V.image_kind.pdf].includes(c.image_kinds[row])) reason = "resource-kind-unsupported";
      else if (!reason && ![V.image_frame.corners, V.image_frame.unit].includes(c.image_frames[row])) reason = "resource-frame-invalid";
      let box;
      if (!reason) {
        box = windowOf(c, unit, view, size);
        if (typeof box === "string") reason = box;
        else if (box.min_x >= box.max_x || box.min_y >= box.max_y) reason = "resource-outside-view";
      }
      if (!reason && this.paused) reason = "resource-cancelled";
      if (!reason && !this.renderer) reason = "resource-unresolved";
      let plan = null;
      if (!reason) {
        if (record.extent || c.image_frames[row] === V.image_frame.corners) {
          const extent = record.extent ?? { width: 1, height: 1 }, frame = frameOf(c, row, chunk, extent, view, size);
          const result = frame ? planOf(frame, extent, box, size, this.limits) : "resource-frame-invalid";
          if (typeof result === "string") reason = result; else plan = result;
        }
      }
      record.plan = plan;
      if (reason) { this.stop(record); record.reason = record.failure ?? reason; continue; }
      const lease = record.lease;
      if (record.raster && !samePlan(record.raster.plan, plan)) {
        lease.rasterOwner = null;
        this.dropPixels(record);
      }
      if (lease?.operation && !samePlan(lease.operation.plan, plan)) {
        lease.rasterOwner = null;
        this.invoke(() => lease.operation.control.abort());
      }
      if (!lease) {
        // 枠の空きを待つ場合も、以前の表示の解放失敗を次の資源へ持ち越さない。
        record.owner = null;
        if (this.leases.size >= this.limits.underlayResources) reason = "resource-limit";
        else if (this.jobs < this.limits.underlayJobs) this.resolve(record);
      } else if (lease.resource && !lease.busy && !record.raster && plan) {
        if (this.pixels + plan.width * plan.height > this.limits.underlayPixels) reason = "resource-limit";
        else if (this.jobs < this.limits.underlayJobs) this.rasterize(record, plan);
      }
      record.reason = record.failure ?? reason ?? (record.raster ? undefined : "resource-pending");
    }
    this.snapshot();
  }

  snapshot() {
    const next = this.records.map((record, i) => {
      const value = { ...record.descriptor, status: record.reason ?? "ready", ...(record.extent ?? {}) };
      const old = this.state[i];
      return old && Object.keys(value).every((key) => old[key] === value[key]) && Object.keys(old).length === Object.keys(value).length ? old : Object.freeze(value);
    });
    if (next.length !== this.state.length || next.some((value, i) => value !== this.state[i])) this.state = Object.freeze(next);
  }

  reason(row) { return this.records[row]?.reason ?? (this.records[row]?.raster ? undefined : "resource-unresolved"); }

  draw(ctx, row) {
    const record = this.records[row], raster = record?.raster, plan = record?.plan;
    if (!raster || !plan) return;
    const { p, u, v } = plan.frame, dx = plan.right - plan.left, dy = plan.top - plan.bottom;
    try {
      const source = raster.value.source;
      if (source.width !== plan.width || source.height !== plan.height) throw new Error("underlay.source");
      // Canvas の画像は左上原点。内容の Y の上向きをここで反転する（HTML §4.12.5.1.15）。
      ctx.transform(u.x * dx / plan.width, u.y * dx / plan.width, -v.x * dy / plan.height, -v.y * dy / plan.height,
        p.x + u.x * plan.left + v.x * plan.top, p.y + u.y * plan.left + v.y * plan.top);
      ctx.drawImage(source, 0, 0);
    } catch { record.failure = record.reason = "resource-render-failed"; this.stop(record); this.snapshot(); this.notify(); }
  }
}
