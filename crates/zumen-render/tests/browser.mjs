// SVG と PNG を Chromium に読ませて画素を照らす。PNG は Node.js の zlib でも復号する。
// 入力の参照名は URL にせず、ブラウザーからのネットワーク要求をすべて拒否する。
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { decode } from "./decode.mjs";

const require = createRequire(new URL("../../../bindings/js/test/browser/package.json", import.meta.url));
const { chromium } = require("playwright");
const svg = readFileSync(process.argv[2], "utf8");
const regionSvg = readFileSync(process.argv[3], "utf8");
const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const requests = [];
  await page.route("**/*", (route) => { requests.push(route.request().url()); return route.abort(); });
  const result = await page.evaluate(async (text) => {
    const document = new DOMParser().parseFromString(text, "image/svg+xml");
    if (document.querySelector("parsererror")) throw new Error("SVG の XML を読めない");
    const image = new Image();
    const url = URL.createObjectURL(new Blob([text], { type: "image/svg+xml" }));
    try {
      image.src = url;
      await image.decode();
      const canvas = window.document.createElement("canvas");
      canvas.width = 100; canvas.height = 100;
      const context = canvas.getContext("2d");
      context.drawImage(image, 0, 0);
      const pixel = (x, y) => Array.from(context.getImageData(x, y, 1, 1).data);
      return { width: image.naturalWidth, height: image.naturalHeight,
        red: pixel(5, 5), hole: pixel(50, 45), line: pixel(50, 50), glyph: pixel(30, 75),
        clipped: pixel(70, 70), inverted: pixel(80, 80), outside: pixel(97, 80),
        scripts: document.querySelectorAll("script,text,image,foreignObject").length };
    } finally { URL.revokeObjectURL(url); }
  }, svg);
  assert.deepEqual(result, { width: 100, height: 100, red: [255, 0, 0, 255], hole: [255, 255, 255, 255],
    line: [0, 0, 255, 255], glyph: [0, 160, 0, 255], clipped: [255, 128, 0, 255],
    inverted: [255, 0, 0, 255], outside: [255, 0, 0, 255], scripts: 0 });
  assert.deepEqual(requests, []);
  const margin = await page.evaluate(async (text) => {
    const url = URL.createObjectURL(new Blob([text], { type: "image/svg+xml" }));
    try {
      const image = new Image(); image.src = url; await image.decode();
      const canvas = document.createElement("canvas"); canvas.width = 200; canvas.height = 100;
      const context = canvas.getContext("2d"); context.drawImage(image, 0, 0);
      return [25, 100, 175].map((x) => Array.from(context.getImageData(x, 50, 1, 1).data));
    } finally { URL.revokeObjectURL(url); }
  }, regionSvg);
  assert.deepEqual(margin, [[255, 255, 255, 255], [0, 0, 255, 255], [255, 255, 255, 255]]);
  assert.deepEqual(requests, []);
  for (const [name, width, height] of [["drawing", 100, 100], ["region", 200, 100], ["alpha", 257, 80], ["alpha-drawing", 100, 100]]) {
    const folder = dirname(process.argv[2]);
    const png = readFileSync(join(folder, `${name}.png`));
    const expected = readFileSync(join(folder, `${name}.rgba`));
    assert.deepEqual(decode(png, width, height), expected);
    const decoded = await page.evaluate(async (base64) => {
      const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
      const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      try {
        const image = new Image(); image.src = url; await image.decode();
        const canvas = document.createElement("canvas");
        canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
        const context = canvas.getContext("2d"); context.drawImage(image, 0, 0);
        return { width: canvas.width, height: canvas.height, rgba: Array.from(context.getImageData(0, 0, canvas.width, canvas.height).data) };
      } finally { URL.revokeObjectURL(url); }
    }, png.toString("base64"));
    assert.equal(decoded.width, width); assert.equal(decoded.height, height);
    assert.equal(decoded.rgba.length, expected.length);
    for (let i = 0; i < expected.length; i++) {
      // Canvas が内部で使う premultiplied RGBA の丸めだけ、半透明の RGB は 1 まで認める。
      const alpha = expected[i - i % 4 + 3];
      const tolerance = i % 4 !== 3 && alpha > 0 && alpha < 255 ? 1 : 0;
      assert.ok(Math.abs(decoded.rgba[i] - expected[i]) <= tolerance, `${name}: byte ${i}`);
    }
  }
  assert.deepEqual(requests, []);
  console.log(`Chromium ${browser.version()} と zlib ${process.versions.zlib}: PNG の寸法・RGBA・CRC・Adler-32 を確認`);
  console.log(`Chromium ${browser.version()}: SVG の寸法・描画順・穴・字形・切り抜きと外部要求なしを確認`);
} finally { await browser.close(); }
