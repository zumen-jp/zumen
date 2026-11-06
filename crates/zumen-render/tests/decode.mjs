// 製品とは別の zlib で PNG を開き、チャンクの CRC・Adler-32・各行を検査する。
import assert from "node:assert/strict";
import { crc32, inflateSync } from "node:zlib";

export function decode(png, width, height) {
  assert.equal(png.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
  assert.equal(png.readUInt32BE(16), width);
  assert.equal(png.readUInt32BE(20), height);
  const chunks = [];
  const idat = [];
  for (let at = 8; at < png.length;) {
    const length = png.readUInt32BE(at);
    const type = png.toString("ascii", at + 4, at + 8);
    const body = png.subarray(at + 4, at + 8 + length);
    assert.equal(crc32(body), png.readUInt32BE(at + 8 + length));
    if (type === "IDAT") idat.push(body.subarray(4));
    chunks.push(type);
    at += length + 12;
  }
  assert.deepEqual(chunks.slice(0, 3), ["IHDR", "sRGB", "gAMA"]);
  assert.ok(chunks.slice(3, -1).every((type) => type === "IDAT"));
  assert.equal(chunks.at(-1), "IEND");
  const rows = inflateSync(Buffer.concat(idat));
  const stride = width * 4 + 1;
  assert.equal(rows.length, height * stride);
  const rgba = Buffer.alloc(width * height * 4);
  for (let row = 0; row < height; row++) {
    assert.equal(rows[row * stride], 0);
    rows.copy(rgba, row * width * 4, row * stride + 1, (row + 1) * stride);
  }
  return rgba;
}
