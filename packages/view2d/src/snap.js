import { end, finite, rows, valid } from "./validate.js";
import { project, unproject } from "./geometry.js";
import { visiblePointTest } from "./hit.js";
import { elementKey, references } from "./interaction.js";
import { addExact, divideExact, dyadic } from "./exact.js";

const UNITS = new Set(["millimeter", "centimeter", "meter", "inch", "foot"]);

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

/** シーンの線・面の頂点と点の中心を、CSS px の距離と逆の描画順で選ぶ。 */
export function snapPoint(ctx, prepared, settings, view, size, display, index, limits, screen, tolerance) {
  const target = point(screen, "screen");
  finite(tolerance, "tolerance"); valid(tolerance >= 0, "tolerance");
  if (!prepared) return null;
  let work = 0, nearest = null;
  const charge = (count) => { work += count; valid(Number.isSafeInteger(work) && work <= limits.work, "limits.work"); };
  const { columns: c, order } = prepared;
  ctx.save();
  try {
    for (let chunk = order.length - 1; chunk >= 0; chunk--) {
      const group = order[chunk], ox = c.chunk_origins[chunk * 2], oy = c.chunk_origins[chunk * 2 + 1];
      for (let ordinal = group.indices.length - 1; ordinal >= 0; ordinal--) {
        charge(1);
        const [kind, i] = group.locate(group.indices[ordinal]);
        if (kind !== "line" && kind !== "face" && kind !== "point") continue;
        const visible = visiblePointTest(ctx, prepared, settings, view, size, display, kind, i, chunk, charge);
        if (!visible) continue;
        for (const offset of vertices(c, kind, i, charge)) {
          const screen = project(view, size.width, size.height, offset.x, offset.y, ox, oy);
          const distance = Math.hypot(screen.x - target.x, screen.y - target.y);
          if (distance > tolerance || (nearest && distance >= nearest.distance) || !visible(screen)) continue;
          const unit = c[`${kind}_units`][i], element = index.elements[c.unit_entities[unit]];
          nearest = Object.freeze({
            position: Object.freeze({ origin: Object.freeze({ x: ox, y: oy }), offset: Object.freeze(offset), element }),
            screen: Object.freeze(screen), kind: kind === "point" ? "point" : "vertex", distance, tolerance,
          });
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
