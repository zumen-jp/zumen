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

for (const size of [7, 4]) {
  // 4×4 では端点の一つを範囲外に置く。どちらの斜辺も画面上で x=y になる。
  const a = [size - 7, 7], b = [size, 0], c = [size - 7, 0];
  for (const [name, vertices] of [["正順", [a, b, c, c]], ["逆順", [b, a, c, c]], ["面積 0", [a, b, a, a]]]) {
    test(`${size}×${size} の三角形・${name}`, () => {
      const solid = [0, "SOLID", 5, "1", 8, "0", 420, 0xff0000];
      vertices.forEach(([x, y], i) => solid.push(10 + i, x, 20 + i, y));
      const rgba = render(solid, size, size);
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
