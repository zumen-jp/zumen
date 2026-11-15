import { addExact, divideExact, dyadic, multiplyExact } from "./exact.js";

const ONE = dyadic(1), TWO = dyadic(2);
const subtract = (a, b) => addExact(a, b, true);
const dot = (a, b) => addExact(multiplyExact(a.x, b.x), multiplyExact(a.y, b.y));
const cross = (a, b) => subtract(multiplyExact(a.x, b.y), multiplyExact(a.y, b.x));
const delta = (a, b) => ({ x: subtract(a.x, b.x), y: subtract(a.y, b.y) });
const exact = (p) => ({ x: dyadic(p.x), y: dyadic(p.y) });
const rounded = (p) => ({ x: divideExact(p.x, ONE), y: divideExact(p.y, ONE) });

/** 原点と相対座標を正確に加減し、最後の座標まで丸めない。 */
export function relativePosition(position, origin) {
  return Object.fromEntries(["x", "y"].map((axis) => [axis,
    addExact(subtract(dyadic(position.origin[axis]), dyadic(origin[axis])), dyadic(position.offset[axis]))]));
}

export function shiftedPosition(position, origin, dx = 0, dy = 0) {
  const p = relativePosition(position, origin);
  return rounded({ x: addExact(p.x, dyadic(dx)), y: addExact(p.y, dyadic(dy)) });
}

export function midpoint(a, b) {
  return { x: divideExact(addExact(dyadic(a.x), dyadic(b.x)), TWO),
    y: divideExact(addExact(dyadic(a.y), dyadic(b.y)), TWO) };
}

/** t と端点からの加算を有理数のまま行い、巨大な辺の途中の小さな差を保つ。 */
export function projectedPoint(a, b, p, clamp) {
  const start = exact(a), d = delta(exact(b), start), squared = dot(d, d);
  if (squared.integer === 0n) return null;
  const numerator = dot(delta(p, start), d);
  if (numerator.integer < 0n) return clamp ? a : null;
  if (subtract(numerator, squared).integer > 0n) return clamp ? b : null;
  return { x: divideExact(addExact(multiplyExact(start.x, squared), multiplyExact(d.x, numerator)), squared),
    y: divideExact(addExact(multiplyExact(start.y, squared), multiplyExact(d.y, numerator)), squared) };
}

/** 同一直線の重なりには一意の交点を作らない。別の塊の原点も途中で丸めない。 */
export function intersection(first, second) {
  const a = exact(first.a), b = exact(first.b);
  const c = relativePosition({ origin: second.origin, offset: second.a }, first.origin);
  const d = relativePosition({ origin: second.origin, offset: second.b }, first.origin);
  const r = delta(b, a), s = delta(d, c), q = delta(c, a), denominator = cross(r, s);
  if (denominator.integer === 0n) return null;
  const t = cross(q, s), u = cross(q, r);
  const within = (n) => denominator.integer > 0n ? n.integer >= 0n && subtract(n, denominator).integer <= 0n
    : n.integer <= 0n && subtract(n, denominator).integer >= 0n;
  if (!within(t) || !within(u)) return null;
  return { x: divideExact(addExact(multiplyExact(a.x, denominator), multiplyExact(r.x, t)), denominator),
    y: divideExact(addExact(multiplyExact(a.y, denominator), multiplyExact(r.y, t)), denominator) };
}

/** 四半円点だけを扱うので三角関数も、折れ線からの円の推定も使わない。 */
export function* quadrants(geometry) {
  const normalize = (angle) => ((angle % 360) + 360) % 360;
  const start = geometry.shape === "arc" ? normalize(geometry.startAngle) : 0;
  const turn = geometry.endAngle - geometry.startAngle;
  const sweep = geometry.shape === "arc" && Math.abs(turn) < 360 ? normalize(turn) : 0;
  const points = [[1, 0], [0, 1], [-1, 0], [0, -1]];
  for (let i = 0; i < points.length; i++) {
    if (geometry.shape === "arc" && sweep !== 0 && normalize(i * 90 - start) > sweep) continue;
    yield { x: points[i][0] * geometry.radius, y: points[i][1] * geometry.radius };
  }
}
