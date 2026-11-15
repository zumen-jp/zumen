import { end, finite, rows, valid } from "./validate.js";
import { project, unproject } from "./geometry.js";
import { visiblePointTest } from "./hit.js";
import { elementKey, references } from "./interaction.js";
import { addExact, divideExact, dyadic } from "./exact.js";
import { intersection, midpoint, projectedPoint, quadrants, relativePosition, shiftedPosition } from "./snap-geometry.js";

const UNITS = new Set(["millimeter", "centimeter", "meter", "inch", "foot"]);
const SNAP_KINDS = new Set(["vertex", "point", "midpoint", "center", "quadrant", "intersection", "perpendicular", "nearest", "insertion"]);

function point(value, name) {
  valid(value && typeof value === "object", name);
  return Object.freeze({ x: finite(value.x, `${name}.x`), y: finite(value.y, `${name}.y`) });
}

/** 大きな原点を足さず、カメラの中心と相対座標を分けて返す。 */
export function positionAt(view, size, screen) {
  const p = point(screen, "screen"), origin = point(view.center, "view.center");
  const offset = unproject({ ...view, center: { x: 0, y: 0 } }, size.width, size.height, p.x, p.y);
  return Object.freeze({ origin, offset: point(offset, "position.offset") });
}

function* vertices(c, kind, i, charge) {
  if (kind === "point") {
    charge(1);
    yield { x: c.point_xy[i * 2], y: c.point_xy[i * 2 + 1] };
  } else if (kind === "line") {
    for (let v = c.line_vertices[i]; v < end(c.line_vertices, i, rows(c, "line_xy")); v++) {
      charge(1);
      yield { x: c.line_xy[v * 2], y: c.line_xy[v * 2 + 1] };
    }
  } else if (kind === "face") {
    for (let r = c.face_rings[i]; r < end(c.face_rings, i, c.ring_vertices.length); r++) {
      charge(1);
      for (let v = c.ring_vertices[r]; v < end(c.ring_vertices, r, rows(c, "face_xy")); v++) {
        charge(1);
        yield { x: c.face_xy[v * 2], y: c.face_xy[v * 2 + 1] };
      }
    }
  }
}

function position(value, name) {
  valid(value && typeof value === "object", name);
  return { origin: point(value.origin, `${name}.origin`), offset: point(value.offset, `${name}.offset`) };
}

function geometryOptions(values, index, limits, charge) {
  valid(Array.isArray(values) && values.length <= limits.stateItems, "snap.geometry");
  const out = new Map();
  for (const value of values) {
    charge(1);
    valid(value && ["circle", "arc", "insertion"].includes(value.shape), "snap.geometry.shape");
    const element = references([value.element], index, limits, charge)[0];
    const geometry = { shape: value.shape, element };
    if (value.shape === "insertion") geometry.position = position(value.position, "snap.geometry.position");
    else {
      geometry.center = position(value.center, "snap.geometry.center");
      geometry.radius = finite(value.radius, "snap.geometry.radius");
      valid(geometry.radius > 0, "snap.geometry.radius");
      if (value.shape === "arc") {
        geometry.startAngle = finite(value.startAngle, "snap.geometry.startAngle");
        geometry.endAngle = finite(value.endAngle, "snap.geometry.endAngle");
      }
    }
    if (!out.has(element)) out.set(element, []);
    out.get(element).push(geometry);
  }
  return out;
}

/** 分割座標の候補を、CSS px の距離と逆の描画順で選ぶ。 */
export function snapPoint(ctx, prepared, settings, view, size, display, index, limits, screen, tolerance, options = {}) {
  const target = point(screen, "screen");
  finite(tolerance, "tolerance"); valid(tolerance >= 0, "tolerance");
  valid(options && typeof options === "object" && !Array.isArray(options), "snap.options");
  const kinds = options.kinds === undefined ? ["vertex", "point"] : options.kinds;
  const optionWork = options.kinds === undefined ? 0 : kinds?.length;
  valid(Array.isArray(kinds) && optionWork <= limits.stateItems && optionWork <= limits.work, "snap.kinds");
  for (const kind of kinds) valid(SNAP_KINDS.has(kind), "snap.kinds");
  const selected = new Set(kinds);
  let work = optionWork, nearest = null;
  const charge = (count) => { work += count; valid(Number.isSafeInteger(work) && work <= limits.work, "limits.work"); };
  const reference = options.reference === undefined ? null : position(options.reference, "snap.reference");
  valid(!selected.has("perpendicular") || reference, "snap.reference");
  const geometry = geometryOptions(options.geometry === undefined ? [] : options.geometry, index, limits, charge);
  if (!prepared || selected.size === 0) return null;
  const { columns: c, order } = prepared;
  const cursor = selected.has("nearest") ? positionAt(view, size, target) : null, segments = [];
  let rank = 0, nearestRank = Infinity;
  const consider = (owner, offset, snapKind, anchor = false, other) => {
    charge(1);
    const screen = project(view, size.width, size.height, offset.x, offset.y, owner.origin.x, owner.origin.y);
    const distance = Math.hypot(screen.x - target.x, screen.y - target.y);
    if (!Number.isFinite(distance) || distance > tolerance || (nearest && (distance > nearest.distance ||
      (distance === nearest.distance && owner.rank >= nearestRank)))) return;
    if (!(anchor ? owner.anchor : owner.visible)(screen) || (other && !other.visible(screen))) return;
    nearestRank = owner.rank;
    nearest = Object.freeze({
      position: Object.freeze({ origin: Object.freeze(owner.origin), offset: Object.freeze(offset), element: owner.element }),
      screen: Object.freeze(screen), kind: snapKind, distance, tolerance,
    });
  };
  ctx.save();
  try {
    for (let chunk = order.length - 1; chunk >= 0; chunk--) {
      const group = order[chunk], ox = c.chunk_origins[chunk * 2], oy = c.chunk_origins[chunk * 2 + 1];
      for (let ordinal = group.indices.length - 1; ordinal >= 0; ordinal--) {
        charge(1);
        const [kind, i] = group.locate(group.indices[ordinal]);
        const ordinalRank = rank++;
        const visible = visiblePointTest(ctx, prepared, settings, view, size, display, kind, i, chunk, charge);
        if (!visible) continue;
        const unit = c[`${kind}_units`][i], element = index.elements[c.unit_entities[unit]];
        const owner = { origin: { x: ox, y: oy }, element, rank: ordinalRank, visible,
          anchor: visiblePointTest(ctx, prepared, settings, view, size, display, kind, i, chunk, charge, true) };
        const vertexKind = kind === "point" ? "point" : "vertex";
        if (selected.has(vertexKind)) for (const offset of vertices(c, kind, i, charge)) consider(owner, offset, vertexKind);
        if (kind === "text" && selected.has("insertion")) {
          consider(owner, { x: c.text_xy[i * 2], y: c.text_xy[i * 2 + 1] }, "insertion", true);
        }
        if (kind === "line" && ["midpoint", "nearest", "perpendicular", "intersection"].some((value) => selected.has(value))) {
          const from = c.line_vertices[i], to = end(c.line_vertices, i, rows(c, "line_xy"));
          const query = cursor && relativePosition(cursor, owner.origin), base = reference && relativePosition(reference, owner.origin);
          for (let v = from + 1; v < to; v++) {
            charge(1);
            const a = { x: c.line_xy[(v - 1) * 2], y: c.line_xy[(v - 1) * 2 + 1] };
            const b = { x: c.line_xy[v * 2], y: c.line_xy[v * 2 + 1] };
            if (a.x === b.x && a.y === b.y) continue;
            if (selected.has("midpoint")) consider(owner, midpoint(a, b), "midpoint");
            if (selected.has("nearest")) consider(owner, projectedPoint(a, b, query, true), "nearest");
            if (selected.has("perpendicular")) {
              const foot = projectedPoint(a, b, base, false);
              if (foot) consider(owner, foot, "perpendicular");
            }
            if (selected.has("intersection")) {
              const current = { ...owner, a, b };
              for (const previous of segments) {
                charge(1);
                const at = intersection(previous, current);
                if (at) consider(previous, at, "intersection", false, current);
              }
              valid(segments.length < limits.stateItems, "limits.stateItems");
              segments.push(current);
            }
          }
        }
        for (const value of geometry.get(element) ?? []) {
          charge(1);
          if (value.shape === "insertion") {
            if (selected.has("insertion")) consider(owner, shiftedPosition(value.position, owner.origin), "insertion", true);
          } else {
            if (selected.has("center")) consider(owner, shiftedPosition(value.center, owner.origin), "center", true);
            if (selected.has("quadrant")) for (const delta of quadrants(value)) {
              consider(owner, shiftedPosition(value.center, owner.origin, delta.x, delta.y), "quadrant", true);
            }
          }
        }
      }
    }
  } finally { ctx.restore(); }
  return nearest;
}

/** 測定値はシーンの座標で返し、配置や用紙の尺度を列から復元しない。 */
export function measurementOf(prepared, start, end, index, limits) {
  valid(prepared, "measurement.scene");
  const coordinate = (value, name) => {
    valid(value && typeof value === "object", name);
    const { x, y } = value;
    valid([x, y].every((v) => typeof v === "number" && Number.isFinite(v)), name);
    return Object.freeze({ x, y });
  };
  const positions = [start, end].map((value, i) => {
    const name = i === 0 ? "measurement.start" : "measurement.end";
    valid(value && typeof value === "object", name);
    return { origin: coordinate(value.origin, `${name}.origin`), offset: coordinate(value.offset, `${name}.offset`),
      ...(value.element === undefined ? {} : { element: value.element }) };
  });
  // 二つの参照の検査に一つの作業量上限を使う。利用側の参照自体は保管しない。
  references(positions.filter((value) => value.element !== undefined).map((value) => value.element), index, limits);
  for (const value of positions) {
    if (value.element !== undefined) value.element = index.byKey.get(elementKey(value.element));
    Object.freeze(value);
  }
  const [first, second] = positions;
  // 有限な f64 の 4 項だけなので、整数幅は約 2100 bit、演算回数は固定。
  const difference = (axis) => divideExact(addExact(
    addExact(dyadic(second.origin[axis]), dyadic(second.offset[axis])),
    addExact(dyadic(first.origin[axis]), dyadic(first.offset[axis])), true), { integer: 1n, exponent: 0 });
  const delta = coordinate({ x: difference("x"), y: difference("y") }, "measurement.delta");
  const distance = Math.hypot(delta.x, delta.y);
  valid(Number.isFinite(distance), "measurement.distance");
  const { from, view, unit, tolerance } = prepared.metadata.result;
  const reason = from.startsWith("sheet/") || view !== undefined ? "paper" : !from.startsWith("space/") ? "definition" :
    !UNITS.has(unit) ? "unit-unknown" : positions.some((p) => p.element?.path.length) ? "placement-scale" :
      positions.some((p) => !p.element) ? "unbound" : null;
  const real = reason ? { status: "unknown", reason } : { status: "known", distance, unit };
  return Object.freeze({ start: first, end: second, delta, distance, from, ...(view === undefined ? {} : { view }), unit, tolerance, real: Object.freeze(real) });
}
