// 既存の Playwright で、SVG を外部の実装に読ませ、描画順・穴・字形・クリップの画素を照らす。
// 入力の参照名は URL にせず、ブラウザーからのネットワーク要求をすべて拒否する。
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import { createRequire } from "node:module";

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
  console.log(`Chromium ${browser.version()}: SVG の寸法・描画順・穴・字形・切り抜きと外部要求なしを確認`);
} finally { await browser.close(); }
