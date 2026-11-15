import { addExact, divideExact, dyadic, multiplyExact, sqrtExact } from "./exact.js";
import { valid } from "./validate.js";

const ONE = dyadic(1), TWO = dyadic(2), FOUR = dyadic(4);
const sub = (a, b) => addExact(a, b, true);
const square = (a) => multiplyExact(a, a);
const number = (a) => divideExact(a, ONE);
const same = (a, b) => a.x === b.x && a.y === b.y;

/** c±sqrt(q) の小さい側は (c²-q)/(c∓sqrt(q)) にして、相殺で境界を動かさない。 */
function roots(center, squared) {
  if (squared.integer < 0n) return null;
  const s = sqrtExact(squared), sum = addExact(center, s), difference = sub(center, s), product = sub(square(center), squared);
  if (center.integer >= 0n) return [sum.integer === 0n ? 0 : divideExact(product, sum), number(sum)];
  return [number(difference), divideExact(product, difference)];
}

/** 巨大な円のうち矩形内の弧だけを返す。弦の誤差は呼出側の画素数で指定する。 */
export function circleArcs(center, radius, box, tolerance, charge, exactRadius) {
  // 画面内で変化が大きい接線側の軸をパラメーターにする。円の上下端に
  // 近い弧を Y で分けると、微小な Y の差が丸められて左右の弧が途切れる。
  if (Math.abs(center.y - (box.min_y / 2 + box.max_y / 2)) > Math.abs(center.x - (box.min_x / 2 + box.max_x / 2))) {
    const swapped = { x: center.y, y: center.x, exact: center.exact ? { x: center.exact.y, y: center.exact.x } : undefined };
    const bounds = { min_x: box.min_y, min_y: box.min_x, max_x: box.max_y, max_y: box.max_x };
    return arcsAlongY(swapped, radius, bounds, tolerance, charge, exactRadius).map((points) => {
      charge(points.length); return points.map((p) => ({ x: p.y, y: p.x }));
    });
  }
  return arcsAlongY(center, radius, box, tolerance, charge, exactRadius);
}

function arcsAlongY(center, radius, box, tolerance, charge, exactRadius) {
  let work = 0;
  const spend = () => { valid(++work <= 4096, "circle.work"); charge(1); };
  const cx = center.exact?.x ?? dyadic(center.x), cy = center.exact?.y ?? dyadic(center.y), r = exactRadius ?? dyadic(radius), rr = square(r);
  const horizontal = (y) => roots(cx, sub(rr, square(sub(dyadic(y), cy))));
  const vertical = (x) => roots(cy, sub(rr, square(sub(dyadic(x), cx))));
  const nearestX = Math.max(box.min_x, Math.min(center.x, box.max_x));
  const range = vertical(nearestX);
  if (!range) return [];
  const low = Math.max(box.min_y, range[0]), high = Math.min(box.max_y, range[1]);
  if (!(low < high)) return [];
  const events = [low, high];
  for (const x of [box.min_x, box.max_x]) for (const y of vertical(x) ?? []) {
    if (y > low && y < high) events.push(y);
  }
  if (center.y > low && center.y < high) events.push(center.y);
  events.sort((a, b) => a - b);
  const epsilon = dyadic(tolerance);
  const chordLimit = multiplyExact(FOUR, multiplyExact(epsilon, sub(multiplyExact(TWO, r), epsilon)));
  const small = (a, b) => sub(addExact(square(sub(dyadic(a.x), dyadic(b.x))), square(sub(dyadic(a.y), dyadic(b.y)))), chordLimit).integer <= 0n;
  const chains = [];
  for (let side = 0; side < 2; side++) {
    let chain = null;
    const point = (y) => {
      const xs = horizontal(y);
      // 丸めた上下端が円の外へ出た場合は、接点の X を保つ。
      return { x: Math.max(box.min_x, Math.min(xs ? xs[side] : center.x, box.max_x)), y };
    };
    for (let i = 1; i < events.length; i++) {
      spend();
      const y0 = events[i - 1], y1 = events[i];
      if (y0 === y1) continue;
      const middle = horizontal(y0 / 2 + y1 / 2);
      if (!middle || middle[side] < box.min_x || middle[side] > box.max_x) { chain = null; continue; }
      const first = point(y0), last = point(y1);
      if (!chain || !same(chain[chain.length - 1], first)) { chain = [first]; chains.push(chain); }
      const stack = [[first, last, 0]];
      while (stack.length) {
        spend();
        const [a, b, depth] = stack.pop();
        const y = a.y / 2 + b.y / 2;
        if (small(a, b)) chain.push(b);
        else {
          valid(depth < 32 && y !== a.y && y !== b.y, "circle.subdivision");
          const p = point(y);
          stack.push([p, b, depth + 1], [a, p, depth + 1]);
        }
      }
    }
  }
  // 上下の接点で左右の弧をつなぎ、画面内へ人工の butt の端を作らない。
  for (let i = 0; i < chains.length; i++) {
    for (let j = i + 1; j < chains.length; j++) {
      const a = chains[i], b = chains[j];
      if (same(a[a.length - 1], b[b.length - 1])) b.reverse();
      if (same(a[0], b[0])) a.reverse();
      if (same(a[a.length - 1], b[0])) {
        a.push(...b.slice(1)); chains.splice(j, 1); j = i;
      }
    }
  }
  return chains;
}
