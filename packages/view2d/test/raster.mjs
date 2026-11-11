import { clipRing, clipSegment, wideClosedStroke, wideJoin, wideSegment } from "../src/raster.js";

/** 原本の binary64 の線分から、桁の相殺なしに画面近傍へ切る。 */
export function testRasterClipping(assert) {
  const box = { min_x: -2.5, min_y: -2.5, max_x: 202.5, max_y: 202.5 };
  const cases = [
    [[-(2 ** 50), -(2 ** 50) + 100], [2 ** 50, 2 ** 50 + 100], [[-2.5, 97.5], [102.5, 202.5]]],
    [[-(2 ** 100), -(2 ** 100)], [2 ** 100, 2 ** 100], [[-2.5, -2.5], [202.5, 202.5]]],
    [[-(2 ** 500), 0], [2 ** 500, 1], [[-2.5, .5], [202.5, .5]]],
    [[-1e308, 1e308], [1e308, -1e308], [[-2.5, 2.5], [2.5, -2.5]]],
    [[0, 100], [200, 100], [[0, 100], [200, 100]]],
    [[-4, -4], [-3, 201], null],
    [[-3, 210], [210, 210], null],
    [[-3, 0], [-3, 200], null],
    [[0, 0], [0, 0], [[0, 0], [0, 0]]],
    [[0, -3], [0, -3], null],
    [[-3, -3], [-2.5, -2.5], [[-2.5, -2.5], [-2.5, -2.5]]],
    [[-Number.MIN_VALUE, 0], [Number.MIN_VALUE, 0], [[-Number.MIN_VALUE, 0], [Number.MIN_VALUE, 0]]],
  ];
  for (const [first, last, expected] of cases) for (const reverse of [false, true]) {
    const points = reverse ? [last, first] : [first, last];
    const actual = clipSegment(...points.map(([x, y]) => ({ x, y })), box);
    const wanted = reverse && expected ? [...expected].reverse() : expected;
    assert(wanted ? actual && actual.every((p, i) => p.x === wanted[i][0] && p.y === wanted[i][1]) : actual === null,
      `画面近傍の線分の交点: ${JSON.stringify({ points, actual, wanted })}`);
  }
  testRingClipping(assert);
  testWideGeometry(assert);
}

function testWideGeometry(assert) {
  const box = { min_x: 0, min_y: 0, max_x: 200, max_y: 200 }, a = { x: 98, y: 98 }, b = { x: 102, y: 102 };
  const signedArea = (points) => points.reduce((sum, p, i) => {
    const q = points[(i + 1) % points.length]; return sum + p.x * q.y - p.y * q.x;
  }, 0);
  for (const radius of [1e9, 1e15, 1e38]) for (const [start, finish] of [[a, b], [b, a]]) {
    const polygon = wideSegment(start, finish, radius, box), sums = polygon.map((p) => p.x + p.y);
    assert(polygon.length <= 8 && Math.min(...sums) === 196 && Math.max(...sums) === 204 && signedArea(polygon) > 0,
      "巨大な法線 offset を作らず、向きを反転しても斜めの両 cap と凸輪の向きを保つ");
  }
  assert(wideSegment(a, b, 1, box) === null, "幅の側面が近傍を横切るときは証明せず従来経路へ戻す");
  assert(wideSegment(a, a, 1e38, box).length === 0, "ゼロ長の butt から領域を作らない");
  assert(wideSegment({ x: 400, y: 100 }, { x: 500, y: 100 }, 1, box).length === 0,
    "cap だけで共通部分が空なら、幅の側面を作る必要がない");
  assert(wideSegment(a, b, Infinity, box) === null && wideSegment(a, b, 1e38, { ...box, min_x: -Infinity }) === null,
    "有限でない派生値は新しい領域へ変えず、従来経路を保つ");
  const fold = wideJoin(a, b, a, 1e38, box);
  assert(fold.length <= 8 && Math.min(...fold.map((p) => p.x + p.y)) === 204 && signedArea(fold) > 0,
    "全包する円の折返し join は、前方の半平面だけを残す");
  assert(wideJoin(a, b, a, 1, box) === null, "join の円弧が近傍に来るときは従来経路へ戻す");
  assert(wideJoin(a, { x: 100, y: 100 }, b, 1e38, box).length === 0,
    "同じ向きの共線に円全体を足さない");
  const distant = wideSegment({ x: -1e150, y: 100 }, { x: 1e150, y: 100 }, 1e150, box);
  assert(distant.length === 4 && signedArea(distant) === 80000, "遠い両端でも近傍の矩形をそのまま覆う");
  const closed = [a, { x: 102, y: 98 }, b, { x: 98, y: 102 }];
  assert(signedArea(wideClosedStroke(closed, 1e38, box)) === 80000,
    "明示的な閉輪の元頂点の円盤が全包するときは単一矩形にする");
  assert(wideClosedStroke(closed, 1, box) === null && wideClosedStroke([a, a], 1e38, box) === null,
    "円盤の全包が不成立、または非ゼロ辺がない閉輪は従来経路を保つ");
}

function testRingClipping(assert) {
  const box = { min_x: 0, min_y: 0, max_x: 200, max_y: 200 };
  const rectangle = [[0, 0], [200, 0], [200, 200], [0, 200]];
  const cases = [
    [[], []],
    [[[-1e308, -1e308], [1e308, -1e308], [1e308, 1e308], [-1e308, 1e308]], rectangle],
    [[[-1e308, -1e308], [1e308, 1e308], [1e308, -1e308]], [[0, 0], [200, 200], [200, 0]]],
    [[[-10, -10], [-1, -10], [-1, 210], [-10, 210]], []],
    [[[-10, -10], [10, -10], [10, 210], [-10, 210]], [[0, 0], [10, 0], [10, 200], [0, 200]]],
    // 先に x=0 で作る遠方の交点を丸めても、最後の x=99.5 を動かしてはいけない。
    ...[2 ** 100, 2 ** 500, 1e308].map((h) => [[[-1, -h], [200, h], [h, -h]], [[99.5, 0], [99.5, 200], [200, 200], [200, 0]]]),
  ];
  const canonical = (points) => [...new Set(points.map(([x, y]) => `${x},${y}`))].sort().join(";");
  const area = (points) => points.reduce((sum, [x, y], i) => {
    const next = points[(i + 1) % points.length]; return sum + x * next[1] - y * next[0];
  }, 0);
  for (const [input, expected] of cases) for (const reverse of [false, true]) {
    const source = (reverse ? [...input].reverse() : input).map(([x, y]) => ({ x, y }));
    const original = JSON.stringify(source), output = clipRing(source, box);
    assert(canonical(output.map(({ x, y }) => [x, y])) === canonical(expected),
      `閉輪の交点を最後に一度だけ丸める: ${JSON.stringify({ input, output, expected, reverse })}`);
    assert(JSON.stringify(source) === original && output.length <= source.length * 81 / 16, "元の輪を変えず、四段の頂点数の上限を保つ");
    if (expected.length) assert(Math.sign(area(output.map(({ x, y }) => [x, y]))) === Math.sign(area(expected)) * (reverse ? -1 : 1),
      "非零の塗り規則に使う輪の向きを保つ");
  }
  const small = [{ x: 1, y: 1 }, { x: 2, y: 1 }, { x: 2, y: 2 }];
  assert(clipRing(small, box) === small, "近傍の輪は exact の一時配列を作らない");
  assert(clipRing(small, { ...box, min_x: -Infinity }) === small, "有限でない近傍の枠では従来の輪を保つ");
}
