import { unionRegions } from "../src/region-union.js";

const box = (x1, y1, x2, y2) => [{ x: x1, y: y1 }, { x: x2, y: y1 }, { x: x2, y: y2 }, { x: x1, y: y2 }];
function pathOf(contours) {
  const path = new Path2D();
  for (const points of contours) { for (let i = 0; i < points.length; i++) path[i ? "lineTo" : "moveTo"](points[i].x, points[i].y); path.closePath(); }
  return path;
}

export function testRegionUnion(assert) {
  const outer = box(20, 20, 180, 180), hole = box(60, 60, 140, 140), left = box(20, 20, 100, 180), right = box(100, 20, 180, 180);
  const items = [
    { regions: [{ points: left }, { points: right }], expected: [outer] },
    { regions: [{ points: outer }, { points: [...outer].reverse() }], expected: [outer] },
    { regions: [{ points: outer }, { points: hole, weight: -1 }], expected: [outer, [...hole].reverse()] },
    { regions: [{ points: outer }, { points: hole, weight: -1 }, { points: box(80, 40, 120, 160) }],
      expected: [outer, box(60, 60, 80, 140).reverse(), box(120, 60, 140, 140).reverse()] },
    { regions: [{ points: outer }, { points: [...outer].reverse(), weight: -1 }], expected: [] },
    { regions: [{ points: box(20, 20, 100, 100) }, { points: box(100, 100, 180, 180) }], expected: [box(20, 20, 100, 100), box(100, 100, 180, 180)] },
    { regions: [{ points: box(30, 80, 120, 130) }, { points: box(80, 40, 150, 160) }], expected: [[
      { x: 30, y: 80 }, { x: 80, y: 80 }, { x: 80, y: 40 }, { x: 150, y: 40 }, { x: 150, y: 160 }, { x: 80, y: 160 }, { x: 80, y: 130 }, { x: 30, y: 130 },
    ]] },
  ];
  let pixels = 0, work = 0;
  for (const item of items) for (const alpha of [0.5, 1]) {
    const canvas = document.createElement("canvas"), reference = document.createElement("canvas");
    canvas.width = reference.width = 200; canvas.height = reference.height = 200;
    const contours = unionRegions(item.regions, (count) => { work += count; });
    for (const [target, rings] of [[canvas, contours], [reference, item.expected]]) {
      const ctx = target.getContext("2d"); ctx.fillStyle = "white"; ctx.fillRect(0, 0, 200, 200); ctx.fillStyle = `rgba(0,0,0,${alpha})`; ctx.fill(pathOf(rings), "nonzero");
    }
    const a = canvas.getContext("2d").getImageData(0, 0, 200, 200).data, b = reference.getContext("2d").getImageData(0, 0, 200, 200).data;
    let mismatch = 0;
    for (let i = 0; i < a.length; i += 4) { if (a[i] !== b[i] || a[i + 3] !== 255) mismatch++; pixels++; }
    assert(mismatch === 0, `内部辺を残さない和・穴・共有辺・接点: mismatch=${mismatch}`);
  }
  let seed = 828, samples = 0, boundaryExcluded = 0;
  const next = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 2 ** 32; };
  for (let i = 0; i < 20; i++) {
    const regions = [];
    for (let shape = 0; shape < 3; shape++) {
      const cx = 60 + next() * 80, cy = 60 + next() * 80, radius = 20 + next() * 50, rotation = next() * 2 * Math.PI, count = shape === 1 ? 4 : 3;
      regions.push({ points: Array.from({ length: count }, (_, j) => ({ x: cx + radius * Math.cos(rotation + j * 2 * Math.PI / count), y: cy + radius * Math.sin(rotation + j * 2 * Math.PI / count) })) });
    }
    const ctx = document.createElement("canvas").getContext("2d"), paths = regions.map((r) => pathOf([r.points]));
    const actual = pathOf(unionRegions(regions, (count) => { work += count; }));
    const inside = (x, y) => paths.some((p) => ctx.isPointInPath(p, x, y));
    for (let y = 0.5; y < 200; y += 5) for (let x = 0.5; x < 200; x += 5) {
      const expected = inside(x, y);
      if ([[1e-6, 0], [-1e-6, 0], [0, 1e-6], [0, -1e-6]].some(([dx, dy]) => inside(x + dx, y + dy) !== expected)) { boundaryExcluded++; continue; }
      assert(ctx.isPointInPath(actual, x, y) === expected, "斜めの凸片の和を独立した Canvas の内部判定と照合する"); samples++;
    }
  }
  assert(work > 0 && work < 1_000_000, "凸片の和の作業量を数える");
  let stopped = false;
  try { let remaining = 20; unionRegions(items[0].regions, (count) => { remaining -= count; if (remaining < 0) throw new RangeError("work"); }); }
  catch (error) { stopped = error instanceof RangeError; }
  assert(stopped, "領域の交差・分割・接続を同じ予算で中断できる");
  return { cases: items.length + 20, pixels, samples, boundaryExcluded, work };
}
