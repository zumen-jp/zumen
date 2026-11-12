import { addExact, dyadic, multiplyExact } from "./exact.js";

const square = (value) => multiplyExact(value, value);
const radiusExact = (circle, offset = 0) => addExact(dyadic(circle.radius), dyadic(offset));
const distanceSquared = (p, circle) => addExact(square(addExact(dyadic(p.x), dyadic(circle.x), true)), square(addExact(dyadic(p.y), dyadic(circle.y), true)));
const compare = (a, b) => { const difference = addExact(a, b, true).integer; return difference < 0n ? -1 : difference > 0n ? 1 : 0; };

/** clip のない viewport 内部の問い合わせと、一つの円/円環の閉集合の交差。
 * 点の tolerance は Minkowski 和、矩形は中心からの最短・最長距離で判定する。
 * sqrt と交点の丸めを使わず、曲率も残す。可視領域との境界接触は従来の配置検査が担う。
 * 有限 binary64 の固定個数の差・和・二乗なので、整数の大きさも約 4,300 bit 以下に収まる。
 */
export function circleIntersects(circle, halfWidth, fill, query) {
  const outer = radiusExact(circle, halfWidth), inner = fill ? dyadic(0) : radiusExact(circle, -halfWidth);
  if (query.circle) {
    const tolerance = dyadic(query.circle.radius), far = addExact(outer, tolerance), near = addExact(inner, tolerance, true);
    const squared = distanceSquared(query.center, circle);
    return compare(squared, square(far)) <= 0 && (near.integer <= 0n || compare(squared, square(near)) >= 0);
  }
  const rect = query.bounds;
  const nearest = { x: Math.max(rect.min_x, Math.min(circle.x, rect.max_x)), y: Math.max(rect.min_y, Math.min(circle.y, rect.max_y)) };
  if (compare(distanceSquared(nearest, circle), square(outer)) > 0) return false;
  if (inner.integer <= 0n) return true;
  // 可変の頂点走査を増やさず、矩形の四隅だけを調べる。呼出側で元の円の work を課金する。
  for (const x of [rect.min_x, rect.max_x]) for (const y of [rect.min_y, rect.max_y]) {
    if (compare(distanceSquared({ x, y }, circle), square(inner)) >= 0) return true;
  }
  return false;
}
