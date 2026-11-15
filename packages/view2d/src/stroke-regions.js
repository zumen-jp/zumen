import { addExact, divideExact, dyadic, multiplyExact, sqrtExact } from "./exact.js";
import { circleArcs } from "./circle-path.js";
import { circleSide } from "./circle.js";

const ONE = dyadic(1), sub = (a, b) => addExact(a, b, true);
const negate = (a) => ({ integer: -a.integer, exponent: a.exponent });
const exact = (p) => ({ x: p.exact?.x ?? dyadic(p.x), y: p.exact?.y ?? dyadic(p.y), w: ONE });
const delta = (a, b) => ({ x: sub(a.x, b.x), y: sub(a.y, b.y) });
const dot = (a, b) => addExact(multiplyExact(a.x, b.x), multiplyExact(a.y, b.y));
const cross = (a, b) => sub(multiplyExact(a.x, b.y), multiplyExact(a.y, b.x));
const same = (a, b) => a.x === b.x && a.y === b.y;
const corners = (box) => [{ x: box.min_x, y: box.min_y }, { x: box.max_x, y: box.min_y },
  { x: box.max_x, y: box.max_y }, { x: box.min_x, y: box.max_y }];
const planeAt = (p, normal, reverse = false) => {
  const n = reverse ? { x: negate(normal.x), y: negate(normal.y) } : normal;
  return { ...n, c: negate(dot(n, p)) };
};
const value = (plane, p) => addExact(dot(plane, p), multiplyExact(plane.c, p.w));

/** 凸輪を半平面で切る。交点は元の辺の係数から求め、出力時だけ丸める。 */
function clip(points, planes, charge) {
  if (points.length < 3) return [];
  charge(points.length);
  let vertices = points.map((p, i) => {
    const a = exact(points[(i + points.length - 1) % points.length]), b = exact(p), d = delta(b, a);
    return { ...b, incoming: planeAt(a, { x: negate(d.y), y: d.x }) };
  });
  for (const plane of planes) {
    if (!vertices.length) break;
    const output = [];
    let previousInside = value(plane, vertices[vertices.length - 1]).integer >= 0n;
    for (const p of vertices) {
      charge(1);
      const currentInside = value(plane, p).integer >= 0n;
      if (currentInside !== previousInside) {
        const edge = p.incoming;
        let x = sub(multiplyExact(edge.y, plane.c), multiplyExact(edge.c, plane.y));
        let y = sub(multiplyExact(edge.c, plane.x), multiplyExact(edge.x, plane.c));
        let w = sub(multiplyExact(edge.x, plane.y), multiplyExact(edge.y, plane.x));
        if (w.integer < 0n) { x = negate(x); y = negate(y); w = negate(w); }
        output.push({ x, y, w, incoming: previousInside ? edge : plane });
      }
      if (currentInside) output.push(p);
      previousInside = currentInside;
    }
    vertices = output;
  }
  return vertices.map((p) => ({ x: divideExact(p.x, p.w), y: divideExact(p.y, p.w) }));
}

/** butt の四つの境界を保つ。側面の係数へ線幅を足してから座標へ丸めない。 */
export function segmentRegion(a, b, radius, box, charge) {
  charge(1);
  const first = exact(a), last = exact(b), d = delta(last, first);
  if (d.x.integer === 0n && d.y.integer === 0n) return [];
  const distance = multiplyExact(dyadic(radius), sqrtExact(dot(d, d)));
  const normal = { x: negate(d.y), y: d.x }, left = planeAt(first, normal), right = planeAt(first, normal, true);
  left.c = addExact(left.c, distance); right.c = addExact(right.c, distance);
  return clip(corners(box), [planeAt(first, d), planeAt(last, d, true), left, right], charge);
}

function hull(points, charge) {
  charge(points.length * Math.max(1, Math.ceil(Math.log2(points.length + 1))));
  points.sort((a, b) => a.x - b.x || a.y - b.y);
  const unique = points.filter((p, i) => i === 0 || !same(p, points[i - 1]));
  if (unique.length < 3) return [];
  const half = (items) => {
    const result = [];
    for (const p of items) {
      while (result.length >= 2) {
        charge(1);
        const a = exact(result[result.length - 2]), b = exact(result[result.length - 1]);
        if (cross(delta(b, a), delta(exact(p), b)).integer > 0n) break;
        result.pop();
      }
      result.push(p);
    }
    result.pop(); return result;
  };
  return [...half(unique), ...half(unique.toReversed())];
}

/** 円盤と画面近傍の凸交差。円弧だけを画素誤差以内の弦にし、枠内の角を合わせる。 */
export function diskRegion(circle, box, tolerance, charge) {
  charge(4);
  // 描画の誤差以下の穴は消してよい。選択は元の円を使い、この近似を使わない。
  if (circle.radius <= tolerance) return [];
  const points = corners(box).filter((p) => circleSide(p, circle) <= 0);
  if (points.length === 4) return points;
  const arcs = circleArcs(circle, circle.radius, box, tolerance, charge, circle.exactRadius);
  for (const arc of arcs) { charge(arc.length); points.push(...arc); }
  return hull(points, charge);
}

/** round join は二つの cap の外側にある円盤の扇形。短い元線の端を延ばさない。 */
export function joinRegion(previous, center, next, radius, box, tolerance, charge) {
  charge(1);
  const p = exact(center), incoming = delta(p, exact(previous)), outgoing = delta(exact(next), p);
  if (incoming.x.integer === 0n && incoming.y.integer === 0n || outgoing.x.integer === 0n && outgoing.y.integer === 0n) return [];
  if (cross(incoming, outgoing).integer === 0n && dot(incoming, outgoing).integer > 0n) return [];
  const disk = diskRegion({ ...center, radius }, box, tolerance, charge);
  return clip(disk, [planeAt(p, incoming), planeAt(p, outgoing, true)], charge);
}
