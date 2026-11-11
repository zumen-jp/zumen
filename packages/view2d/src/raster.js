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

const squareExact = (a) => multiplyExact(a, a);
const dotExact = (ax, ay, bx, by) => addExact(multiplyExact(ax, bx), multiplyExact(ay, by));
const planeAt = (p, dx, dy, reverse = false) => {
  const x = reverse ? negate(dx) : dx, y = reverse ? negate(dy) : dy;
  return { x, y, c: negate(dotExact(x, y, dyadic(p.x), dyadic(p.y))) };
};
const planeValue = (plane, p) => addExact(dotExact(plane.x, plane.y, p.x, p.y), multiplyExact(plane.c, p.w));

/** 近傍矩形と二つの半平面の凸交差。辺の由来から交点を求め、途中では丸めない。 */
function clipPlanes(box, planes) {
  const points = [{ x: box.min_x, y: box.min_y }, { x: box.max_x, y: box.min_y },
    { x: box.max_x, y: box.max_y }, { x: box.min_x, y: box.max_y }];
  const edges = [
    { x: one, y: zero, c: negate(dyadic(box.min_x)) }, { x: zero, y: one, c: negate(dyadic(box.min_y)) },
    { x: one, y: zero, c: negate(dyadic(box.max_x)) }, { x: zero, y: one, c: negate(dyadic(box.max_y)) },
  ];
  // 凸輪の入出は各平面で二回まで。重複した境界頂点も含め、最後まで最大8頂点。
  // 元の境界係数から毎回交点を作るので、後の二乗検査まで最大8積、整数幅は約17,000bit以下。
  let vertices = points.map((p, i) => ({ ...pointExact(p), incoming: edges[i] }));
  for (const plane of planes) {
    if (!vertices.length) break;
    const output = [];
    let previousInside = planeValue(plane, vertices[vertices.length - 1]).integer >= 0n;
    for (const p of vertices) {
      const currentInside = planeValue(plane, p).integer >= 0n;
      if (currentInside !== previousInside) {
        const edge = p.incoming;
        let x = addExact(multiplyExact(edge.y, plane.c), multiplyExact(edge.c, plane.y), true);
        let y = addExact(multiplyExact(edge.c, plane.x), multiplyExact(edge.x, plane.c), true);
        let w = addExact(multiplyExact(edge.x, plane.y), multiplyExact(edge.y, plane.x), true);
        if (w.integer < 0n) { x = negate(x); y = negate(y); w = negate(w); }
        output.push({ x, y, w, incoming: previousInside ? edge : plane });
      }
      if (currentInside) output.push(p);
      previousInside = currentInside;
    }
    vertices = output;
  }
  return vertices;
}

const roundedPoints = (vertices) => vertices.map((p) => ({ x: divideExact(p.x, p.w), y: divideExact(p.y, p.w) }));
const finiteRegion = (box, radius, points) => Number.isFinite(radius) && radius > 0 &&
  Object.values(box).every(Number.isFinite) && points.every((p) => Number.isFinite(p.x) && Number.isFinite(p.y));
const relativePoint = (p, anchor) => ({ x: addExact(p.x, multiplyExact(dyadic(anchor.x), p.w), true),
  y: addExact(p.y, multiplyExact(dyadic(anchor.y), p.w), true) });

/** 明示的な閉輪の round stroke は、折れ線への距離<=radius の領域。全包する元頂点があれば枠全体を返す。 */
export function wideClosedStroke(points, radius, box) {
  if (!finiteRegion(box, radius, points) || points.length < 2 || !points.some((p) => p.x !== points[0].x || p.y !== points[0].y)) return null;
  const corners = [{ x: box.min_x, y: box.min_y }, { x: box.max_x, y: box.min_y },
    { x: box.max_x, y: box.max_y }, { x: box.min_x, y: box.max_y }];
  const squared = squareExact(dyadic(radius));
  for (const center of points) {
    if (corners.every((p) => {
      const x = addExact(dyadic(p.x), dyadic(center.x), true), y = addExact(dyadic(p.y), dyadic(center.y), true);
      return addExact(dotExact(x, y, x, y), squared, true).integer <= 0n;
    })) return corners;
  }
  return null;
}

/** 幅の側面が凸領域を全包すると証明できる場合だけ、butt の領域を近傍の塗りにする。 */
export function wideSegment(a, b, radius, box) {
  if (!finiteRegion(box, radius, [a, b])) return null;
  const dx = addExact(dyadic(b.x), dyadic(a.x), true), dy = addExact(dyadic(b.y), dyadic(a.y), true);
  if (dx.integer === 0n && dy.integer === 0n) return [];
  const vertices = clipPlanes(box, [planeAt(a, dx, dy), planeAt(b, dx, dy, true)]);
  const limit = multiplyExact(squareExact(dyadic(radius)), dotExact(dx, dy, dx, dy));
  for (const p of vertices) {
    const q = relativePoint(p, a), cross = addExact(multiplyExact(dx, q.y), multiplyExact(dy, q.x), true);
    if (addExact(squareExact(cross), multiplyExact(limit, squareExact(p.w)), true).integer > 0n) return null;
  }
  // |cross| は凸関数なので、頂点がすべて幅内なら領域全体も幅内である。
  return roundedPoints(vertices);
}

/** 円盤が扇形の近傍を全包すると証明できる round join。円弧が来る場合は従来経路に戻す。 */
export function wideJoin(previous, center, next, radius, box) {
  if (!finiteRegion(box, radius, [previous, center, next])) return null;
  const ix = addExact(dyadic(center.x), dyadic(previous.x), true), iy = addExact(dyadic(center.y), dyadic(previous.y), true);
  const ox = addExact(dyadic(next.x), dyadic(center.x), true), oy = addExact(dyadic(next.y), dyadic(center.y), true);
  if (ix.integer === 0n && iy.integer === 0n || ox.integer === 0n && oy.integer === 0n) return [];
  const cross = addExact(multiplyExact(ix, oy), multiplyExact(iy, ox), true);
  if (cross.integer === 0n && dotExact(ix, iy, ox, oy).integer > 0n) return [];
  const vertices = clipPlanes(box, [planeAt(center, ix, iy), planeAt(center, ox, oy, true)]);
  const squared = squareExact(dyadic(radius));
  for (const p of vertices) {
    const q = relativePoint(p, center);
    if (addExact(dotExact(q.x, q.y, q.x, q.y), multiplyExact(squared, squareExact(p.w)), true).integer > 0n) return null;
  }
  return roundedPoints(vertices);
}
