import { clipSegment } from "../src/raster.js";

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
}
