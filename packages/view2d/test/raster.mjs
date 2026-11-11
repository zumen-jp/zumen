import { clipRing, clipSegment } from "../src/raster.js";

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
