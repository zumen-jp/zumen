import { addExact, divideExact, dyadic, multiplyExact } from "./exact.js";

export const inside = (p, box) => p.x >= box.min_x && p.x <= box.max_x && p.y >= box.min_y && p.y <= box.max_y;
const zero = { integer: 0n, exponent: 0 }, one = { integer: 1n, exponent: 0 };
const compare = (a, b) => addExact(multiplyExact(a.n, b.d), multiplyExact(b.n, a.d), true).integer;

/** 原本の binary64 の線分と矩形の交点。比も座標も途中で丸めない。 */
export function clipSegment(a, b, box) {
  if (inside(a, box) && inside(b, box)) return [a, b];
  if (a.x < box.min_x && b.x < box.min_x || a.x > box.max_x && b.x > box.max_x ||
    a.y < box.min_y && b.y < box.min_y || a.y > box.max_y && b.y > box.max_y) return null;
  const start = { x: dyadic(a.x), y: dyadic(a.y) }, delta = {
    x: addExact(dyadic(b.x), start.x, true), y: addExact(dyadic(b.y), start.y, true),
  };
  let enter = { n: zero, d: one }, leave = { n: one, d: one };
  // Liang–Barsky の区間を有理数で更新する。平面は 4 つだけで、入力の長さによる反復はない。
  for (const axis of ["x", "y"]) {
    const d = delta[axis], low = box[`min_${axis}`], high = box[`max_${axis}`];
    if (d.integer === 0n) {
      if (a[axis] < low || a[axis] > high) return null;
      continue;
    }
    const negative = d.integer < 0n, denominator = negative ? { integer: -d.integer, exponent: d.exponent } : d;
    const bound = (value) => {
      const n = addExact(dyadic(value), start[axis], true);
      return { n: negative ? { integer: -n.integer, exponent: n.exponent } : n, d: denominator };
    };
    const first = bound(negative ? high : low), last = bound(negative ? low : high);
    if (compare(first, enter) > 0n) enter = first;
    if (compare(last, leave) < 0n) leave = last;
    if (compare(enter, leave) > 0n) return null;
  }
  const at = (t) => ({
    x: divideExact(addExact(multiplyExact(start.x, t.d), multiplyExact(delta.x, t.n)), t.d),
    y: divideExact(addExact(multiplyExact(start.y, t.d), multiplyExact(delta.y, t.n)), t.d),
  });
  return [enter.n.integer === 0n ? a : at(enter), compare(leave, { n: one, d: one }) === 0n ? b : at(leave)];
}

const negate = (a) => ({ integer: -a.integer, exponent: a.exponent });
const pointExact = (p) => ({ x: dyadic(p.x), y: dyadic(p.y), w: one });

/** 閉じた輪を矩形の境界で4回切る。元辺と境界辺を区別し、途中の交点を丸めない。 */
export function clipRing(points, box) {
  if (points.length === 0 || !Object.values(box).every(Number.isFinite) ||
    !points.every((p) => Number.isFinite(p.x) && Number.isFinite(p.y)) || points.every((p) => inside(p, box))) return points;
  let vertices = points.map((p, i) => ({ ...pointExact(p), incoming: { a: points[(i + points.length - 1) % points.length], b: p } }));
  for (const axis of ["x", "y"]) for (const minimum of [true, false]) {
    if (!vertices.length) return [];
    const value = box[`${minimum ? "min" : "max"}_${axis}`], boundary = { axis, value }, bound = dyadic(value);
    const within = (p) => {
      const delta = addExact(p[axis], multiplyExact(bound, p.w), true).integer;
      return minimum ? delta >= 0n : delta <= 0n;
    };
    const output = [];
    let previousInside = within(vertices[vertices.length - 1]);
    for (const current of vertices) {
      const currentInside = within(current);
      if (currentInside !== previousInside) {
        const point = ringIntersection(current.incoming, boundary);
        // 外から入る辺は、前の exit から今回の境界上を進む橋である。
        output.push({ ...point, incoming: previousInside ? current.incoming : boundary });
      }
      if (currentInside) output.push(current);
      previousInside = currentInside;
    }
    vertices = output;
  }
  return vertices.map((p) => ({ x: divideExact(p.x, p.w), y: divideExact(p.y, p.w) }));
}

function ringIntersection(edge, boundary) {
  const axis = boundary.axis, other = axis === "x" ? "y" : "x", bound = dyadic(boundary.value);
  if (edge.axis !== undefined) {
    // 作った境界辺との交点は矩形の角。平行な境界辺に内外の遷移はない。
    return { [axis]: bound, [other]: dyadic(edge.value), w: one };
  }
  const start = pointExact(edge.a), delta = {
    x: addExact(dyadic(edge.b.x), start.x, true), y: addExact(dyadic(edge.b.y), start.y, true),
  };
  const offset = addExact(bound, start[axis], true), denominator = delta[axis];
  let x = multiplyExact(bound, denominator);
  let y = addExact(multiplyExact(start[other], denominator), multiplyExact(delta[other], offset));
  // W を正にそろえるので、後の半平面は X-kW の符号だけで判定できる。
  if (denominator.integer < 0n) { x = negate(x); y = negate(y); }
  // 元辺の差と二積の和まで。四段を通しても次数は増えず、整数幅は約4,300bit以下。
  return { [axis]: x, [other]: y, w: denominator.integer < 0n ? negate(denominator) : denominator };
}
