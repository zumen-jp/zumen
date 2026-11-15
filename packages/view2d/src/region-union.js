import { addExact, divideExact, dyadic, multiplyExact } from "./exact.js";
import { valid } from "./validate.js";

const ZERO = dyadic(0), ONE = dyadic(1), TWO = dyadic(2);
const sub = (a, b) => addExact(a, b, true);
const negate = (a) => ({ integer: -a.integer, exponent: a.exponent });
const cross = (a, b) => sub(multiplyExact(a.x, b.y), multiplyExact(a.y, b.x));
const dot = (a, b) => addExact(multiplyExact(a.x, b.x), multiplyExact(a.y, b.y));
const delta = (a, b) => ({ x: sub(a.x, b.x), y: sub(a.y, b.y) });
const exact = (p) => ({ x: dyadic(p.x), y: dyadic(p.y) });
const same = (a, b) => a.x === b.x && a.y === b.y;
const fraction = (n, d) => d.integer < 0n ? { n: negate(n), d: negate(d) } : { n, d };
const compare = (a, b) => sub(multiplyExact(a.n, b.d), multiplyExact(b.n, a.d)).integer;
const sign = (value) => value < 0n ? -1 : value > 0n ? 1 : 0;
const within = (t) => t.n.integer >= 0n && sub(t.n, t.d).integer <= 0n;
const at = (edge, t) => ({ x: addExact(multiplyExact(edge.a.x, t.d), multiplyExact(edge.vector.x, t.n)),
  y: addExact(multiplyExact(edge.a.y, t.d), multiplyExact(edge.vector.y, t.n)), w: t.d });
const rounded = (p) => ({ x: divideExact(p.x, p.w), y: divideExact(p.y, p.w) });
const key = (p) => `${p.x},${p.y}`;

/** 近傍の凸片の和から、内部の辺を除いた輪だけを返す。穴の片は weight=-1。
 * 同一 part を一度の nonzero fill にしても、内部辺を残すと Canvas の AA の筋が出る。
 * 交点で辺を分け、中点の両側の被覆数を整数で比較して外周だけをつなぐ。
 */
export function unionRegions(regions, charge) {
  const polygons = [], edges = [];
  let cuts = 0;
  for (const region of regions) {
    charge(1 + region.points.length);
    valid(region.weight === undefined || region.weight === 1 || region.weight === -1, "geometry.union.weight");
    const points = region.points.filter((p, i, all) => i === 0 || !same(p, all[i - 1]));
    if (points.length > 1 && same(points[0], points[points.length - 1])) points.pop();
    if (points.length < 3) continue;
    let area = ZERO;
    for (let i = 0; i < points.length; i++) area = addExact(area, cross(exact(points[i]), exact(points[(i + 1) % points.length])));
    if (area.integer === 0n) continue;
    if (area.integer < 0n) points.reverse();
    const polygon = { edges: [], weight: region.weight ?? 1, min_x: Infinity, min_y: Infinity, max_x: -Infinity, max_y: -Infinity };
    for (let i = 0; i < points.length; i++) {
      valid(edges.length < 16384, "geometry.union.edges");
      const p = points[i], q = points[(i + 1) % points.length], a = exact(p), b = exact(q);
      const edge = { id: edges.length, a, b, vector: delta(b, a), cuts: [{ n: ZERO, d: ONE }, { n: ONE, d: ONE }],
        min_x: Math.min(p.x, q.x), min_y: Math.min(p.y, q.y), max_x: Math.max(p.x, q.x), max_y: Math.max(p.y, q.y) };
      edges.push(edge); polygon.edges.push(edge);
      polygon.min_x = Math.min(polygon.min_x, p.x); polygon.min_y = Math.min(polygon.min_y, p.y);
      polygon.max_x = Math.max(polygon.max_x, p.x); polygon.max_y = Math.max(polygon.max_y, p.y);
    }
    polygons.push(polygon);
  }
  const addCut = (edge, t) => {
    if (!within(t)) return;
    charge(1); valid(++cuts <= 32768, "geometry.union.cuts"); edge.cuts.push(t);
  };
  for (let i = 0; i < edges.length; i++) for (let j = 0; j < i; j++) {
    charge(1);
    const a = edges[i], b = edges[j];
    if (a.min_x > b.max_x || b.min_x > a.max_x || a.min_y > b.max_y || b.min_y > a.max_y) continue;
    const q = delta(b.a, a.a), denominator = cross(a.vector, b.vector);
    if (denominator.integer !== 0n) {
      const t = fraction(cross(q, b.vector), denominator), u = fraction(cross(q, a.vector), denominator);
      if (within(t) && within(u)) { addCut(a, t); addCut(b, u); }
    } else if (cross(q, a.vector).integer === 0n) {
      const axis = a.vector.x.integer === 0n ? "y" : "x";
      for (const p of [b.a, b.b]) addCut(a, fraction(sub(p[axis], a.a[axis]), a.vector[axis]));
      for (const p of [a.a, a.b]) addCut(b, fraction(sub(p[axis], b.a[axis]), b.vector[axis]));
    }
  }
  const boundary = [];
  for (const edge of edges) {
    charge(edge.cuts.length * Math.max(1, Math.ceil(Math.log2(edge.cuts.length + 1))));
    edge.cuts.sort((a, b) => sign(compare(a, b)));
    const parameters = edge.cuts.filter((t, i, all) => i === 0 || compare(t, all[i - 1]) !== 0n);
    for (let i = 1; i < parameters.length; i++) {
      charge(1);
      const a = parameters[i - 1], b = parameters[i];
      const middle = at(edge, { n: addExact(multiplyExact(a.n, b.d), multiplyExact(b.n, a.d)), d: multiplyExact(TWO, multiplyExact(a.d, b.d)) });
      const position = rounded(middle);
      let left = 0, right = 0, canonical = edge.id;
      for (const polygon of polygons) {
        charge(1);
        if (position.x < polygon.min_x || position.x > polygon.max_x || position.y < polygon.min_y || position.y > polygon.max_y) continue;
        let insideLeft = true, insideRight = true, polygonCanonical = edge.id;
        for (const other of polygon.edges) {
          charge(1);
          const q = { x: sub(middle.x, multiplyExact(other.a.x, middle.w)), y: sub(middle.y, multiplyExact(other.a.y, middle.w)) };
          const side = cross(other.vector, q).integer;
          if (side < 0n) { insideLeft = false; insideRight = false; break; }
          if (side === 0n) {
            const toward = dot(other.vector, edge.vector).integer;
            insideLeft &&= toward > 0n; insideRight &&= toward < 0n;
            if (cross(other.vector, edge.vector).integer === 0n) polygonCanonical = Math.min(polygonCanonical, other.id);
          }
        }
        if (insideLeft) left += polygon.weight;
        if (insideRight) right += polygon.weight;
        if (insideLeft || insideRight) canonical = Math.min(canonical, polygonCanonical);
      }
      if (canonical !== edge.id || (left > 0) === (right > 0)) continue;
      const first = rounded(at(edge, a)), last = rounded(at(edge, b));
      if (same(first, last)) continue;
      boundary.push(left > 0 ? { a: first, b: last } : { a: last, b: first });
    }
  }
  const outgoing = new Map();
  for (let i = 0; i < boundary.length; i++) {
    charge(1); const start = key(boundary[i].a);
    if (!outgoing.has(start)) outgoing.set(start, []);
    outgoing.get(start).push(i);
  }
  const result = [], visited = new Set();
  for (let first = 0; first < boundary.length; first++) {
    if (visited.has(first)) continue;
    const points = [], start = key(boundary[first].a);
    let current = first;
    for (;;) {
      charge(1); valid(!visited.has(current), "geometry.union.cycle"); visited.add(current);
      const edge = boundary[current]; points.push(edge.a);
      const finish = key(edge.b);
      if (finish === start) break;
      current = undefined;
      for (const candidate of outgoing.get(finish) ?? []) { charge(1); if (!visited.has(candidate)) { current = candidate; break; } }
      valid(current !== undefined, "geometry.union.closed");
    }
    if (points.length >= 3) result.push(points);
  }
  return result;
}
