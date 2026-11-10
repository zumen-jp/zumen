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
