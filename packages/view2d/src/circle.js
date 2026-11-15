import { addExact, divideExact, dyadic, multiplyExact, sqrtExact } from "./exact.js";

const ONE = dyadic(1), TWO = dyadic(2), FOUR = dyadic(4);
const sub = (a, b) => addExact(a, b, true);
const square = (value) => multiplyExact(value, value);
const coordinate = (point, key) => point.exact?.[key] ?? (point.anchor ? addExact(dyadic(point.anchor[key]), dyadic(point[key === "x" ? "offsetX" : "offsetY"])) : dyadic(point[key]));
const radiusExact = (circle, offset = 0) => addExact(circle.exactRadius ?? dyadic(circle.radius), dyadic(offset));
const distanceSquared = (p, circle) => addExact(square(sub(coordinate(p, "x"), coordinate(circle, "x"))), square(sub(coordinate(p, "y"), coordinate(circle, "y"))));
const compare = (a, b) => { const difference = addExact(a, b, true).integer; return difference < 0n ? -1 : difference > 0n ? 1 : 0; };

/** 円環の二つの半径を加減算の途中で丸めず、境界と判定に同じ値を使う。 */
export function offsetCircle(circle, offset) {
  const exactRadius = radiusExact(circle, offset);
  return { ...circle, radius: divideExact(exactRadius, ONE), exactRadius };
}

/** 交点の最終座標だけを丸めた場合は、求めた元の境界上であることを保持する。 */
export function circleSide(point, circle) {
  for (const boundary of point.circleBoundaries ?? []) {
    if (boundary === circle || (compare(coordinate(boundary, "x"), coordinate(circle, "x")) === 0 &&
      compare(coordinate(boundary, "y"), coordinate(circle, "y")) === 0 && compare(radiusExact(boundary), radiusExact(circle)) === 0)) return 0;
  }
  return compare(distanceSquared(point, circle), square(radiusExact(circle)));
}

export function circleBounds(circle) {
  const r = radiusExact(circle), x = coordinate(circle, "x"), y = coordinate(circle, "y");
  return { min_x: divideExact(sub(x, r), ONE), min_y: divideExact(sub(y, r), ONE), max_x: divideExact(addExact(x, r), ONE), max_y: divideExact(addExact(y, r), ONE) };
}

export function circleCardinal(circle, dx, dy) {
  const r = radiusExact(circle);
  return { x: divideExact(addExact(coordinate(circle, "x"), multiplyExact(dyadic(dx), r)), ONE),
    y: divideExact(addExact(coordinate(circle, "y"), multiplyExact(dyadic(dy), r)), ONE), circleBoundaries: [circle] };
}

/** 線分と円の二次式の判別式を正確に作り、交点の座標まで差を丸めない。 */
export function intersectCircleLine(circle, edge, accept) {
  const ax = coordinate(edge.a, "x"), ay = coordinate(edge.a, "y"), dx = sub(coordinate(edge.b, "x"), ax), dy = sub(coordinate(edge.b, "y"), ay);
  const px = sub(ax, coordinate(circle, "x")), py = sub(ay, coordinate(circle, "y"));
  const a = addExact(square(dx), square(dy)), b = addExact(multiplyExact(px, dx), multiplyExact(py, dy));
  if (a.integer === 0n) return circleSide(edge.a, circle) === 0 && accept(edge.a);
  const c = sub(addExact(square(px), square(py)), square(radiusExact(circle))), discriminant = sub(square(b), multiplyExact(a, c));
  if (discriminant.integer < 0n) return false;
  const root = sqrtExact(discriminant), negativeB = sub(dyadic(0), b);
  const within = (t) => t.integer >= 0n && sub(t, a).integer <= 0n;
  const at = (t, boundary) => ({ x: divideExact(addExact(multiplyExact(ax, a), multiplyExact(dx, t)), a),
    y: divideExact(addExact(multiplyExact(ay, a), multiplyExact(dy, t)), a), circleBoundaries: boundary ? [circle] : [] });
  for (const t of [sub(negativeB, root), addExact(negativeB, root)]) if (within(t) && accept(at(t, true))) return true;
  return within(negativeB) && accept(at(negativeB, false));
}

/** 二円の共通弦。差・判別式・基点を整数のまま扱い、巨大な半径の相殺を避ける。 */
export function intersectCirclePair(first, second, accept) {
  const x = coordinate(first, "x"), y = coordinate(first, "y"), dx = sub(coordinate(second, "x"), x), dy = sub(coordinate(second, "y"), y);
  const distance = addExact(square(dx), square(dy));
  if (distance.integer === 0n) return false;
  const r = square(radiusExact(first)), s = square(radiusExact(second)), along = sub(addExact(distance, r), s);
  const discriminant = sub(multiplyExact(FOUR, multiplyExact(r, distance)), square(along));
  if (discriminant.integer < 0n) return false;
  const root = sqrtExact(discriminant), denominator = multiplyExact(TWO, distance);
  const baseX = addExact(multiplyExact(x, denominator), multiplyExact(dx, along));
  const baseY = addExact(multiplyExact(y, denominator), multiplyExact(dy, along));
  for (const reverse of [false, true]) {
    const point = { x: divideExact(addExact(baseX, multiplyExact(dy, root), reverse), denominator),
      y: divideExact(addExact(baseY, multiplyExact(dx, root), !reverse), denominator), circleBoundaries: [first, second] };
    if (accept(point)) return true;
  }
  return false;
}

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
