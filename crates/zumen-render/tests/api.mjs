// 製品 wasm の open → render → take で合成 DXF を描き、図形から求めた画素と照らす。
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { load } from "../../../bindings/js/src/index.js";
import { decode } from "./decode.mjs";

const core = await load(readFileSync(process.argv[2]));

function render(entities, width, height) {
  const data = [0, "SECTION", 2, "HEADER", 9, "$INSUNITS", 70, 4, 0, "ENDSEC",
    0, "SECTION", 2, "ENTITIES", ...entities, 0, "ENDSEC", 0, "EOF", ""].join("\n");
  const session = core.open();
  try {
    session.push("a.dxf", Buffer.from(data));
    const doc = session.call({ v: 1, op: "open", input: "a.dxf" }).result.doc;
    const reply = session.call({ v: 1, op: "render", doc, from: "space/0", format: "png", width, height,
      region: [0, 0, width, height], dpi: 25.4, background: [0, 0, 0, 0], foreground: [255, 0, 0, 255] });
    assert.deepEqual(reply.diagnostics, []);
    return decode(Buffer.from(session.take(reply.result.blob)), width, height);
  } finally { session.close(); }
}

function renderSolid(points, width, height) {
  // DXF SOLID の 4 頂点は境界を 1,2,4,3 の順にたどる。三角形は最後の 2 点を重ねる。
  const [a, b, c, d = c] = points;
  const solid = [0, "SOLID", 5, "1", 8, "0", 420, 0xff0000];
  [a, b, d, c].forEach(([x, y], i) => solid.push(10 + i, x, 20 + i, y));
  return render(solid, width, height);
}

for (const size of [7, 4]) {
  // 4×4 では端点の一つを範囲外に置く。どちらの斜辺も画面上で x=y になる。
  const a = [size - 7, 7], b = [size, 0], c = [size - 7, 0];
  for (const [name, vertices] of [["正順", [a, b, c, c]], ["逆順", [b, a, c, c]], ["面積 0", [a, b, a, a]]]) {
    test(`${size}×${size} の三角形・${name}`, () => {
      const rgba = renderSolid(vertices, size, size);
      for (let y = 0; y < size; y++) {
        for (let x = 0; x < size; x++) {
          // 左端のサンプルを含み、右端を除く。斜辺上の画素は 16 個中 6 個。
          const alpha = name === "面積 0" || x > y ? 0 : x === y ? 96 : 255;
          const at = (y * size + x) * 4;
          assert.deepEqual([...rgba.subarray(at, at + 4)], [alpha ? 255 : 0, 0, 0, alpha], `(${x}, ${y})`);
        }
      }
    });
  }
}

// 整数の不等式でサンプルを分類する。X, Y は画面上の座標の 8 倍。
for (const { width, height, a, b, mid, d, inside } of [
  { width: 21, height: 9, a: [21, 9], b: [0, 0], mid: [14, 6], d: [0, 9], inside: (X, Y) => 3 * X + 7 * Y < 63 * 8 },
  { width: 4, height: 4, a: [11, 11], b: [-7, -3], mid: [2, 4], d: [11, -3], inside: (X, Y) => 7 * X + 9 * Y >= 14 * 8 },
]) {
  const expected = Buffer.alloc(width * height * 4);
  const transparent = Buffer.alloc(expected.length);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let samples = 0;
      for (const dy of [1, 3, 5, 7]) {
        for (const dx of [1, 3, 5, 7]) {
          if (inside(8 * x + dx, 8 * y + dy)) samples++;
        }
      }
      if (samples) expected.set([255, 0, 0, Math.round(255 * samples / 16)], 4 * (y * width + x));
    }
  }
  for (const [name, ring, pixels] of [
    ["長辺", [a, b, d], expected], ["長辺の逆順", [d, b, a], expected],
    ["分割", [a, mid, b, d], expected], ["分割の逆順", [d, b, mid, a], expected],
    ["往復", [a, b, mid], transparent], ["往復の逆順", [mid, b, a], transparent],
  ]) {
    test(`${width}×${height} の斜辺・${name}`, () => {
      assert.deepEqual(renderSolid(ring, width, height), pixels);
    });
  }
}

for (const [width, height, count] of [[1024, 1024, 600], [1920, 1080, 256]]) {
  test(`${width}×${height} の縦線 ${count} 本が既定予算に収まる`, () => {
    const lines = [];
    const expected = Buffer.alloc(width * 4);
    for (let i = 0; i < count; i++) {
      const column = Math.floor((i + 0.5) * width / count);
      const x = column + 0.5;
      lines.push(0, "LINE", 5, (i + 1).toString(16), 8, "0", 420, 0xff0000, 370, 0,
        10, x, 20, 0, 11, x, 21, height);
      expected.set([255, 0, 0, 255], column * 4);
    }
    const rgba = render(lines, width, height);
    for (let row = 0; row < height; row++) {
      assert.ok(rgba.subarray(row * expected.length, (row + 1) * expected.length).equals(expected), `行 ${row}`);
    }
  });
}
