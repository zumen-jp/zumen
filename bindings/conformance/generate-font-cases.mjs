// #564 の適合テストと fuzz の種のために、最小の合成フォントを組み立てる。
// 実在のフォントは使わない。node bindings/conformance/generate-font-cases.mjs [--check]
// CFF: Adobe #5176 の INDEX・DICT・CharStrings、#5177 の Type 2 の命令。
// CFF2: https://learn.microsoft.com/en-us/typography/opentype/spec/cff2
// SFNT・TTC: https://learn.microsoft.com/en-us/typography/opentype/spec/otff
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";

const bytes = (...values) => Buffer.from(values.flat());
const join = (...values) => Buffer.concat(values.flat());
const u16 = (value) => {
  const out = Buffer.alloc(2);
  out.writeUInt16BE(value & 0xffff);
  return out;
};
const u32 = (value) => {
  const out = Buffer.alloc(4);
  out.writeUInt32BE(value >>> 0);
  return out;
};
const padded = (data) => join(data, Buffer.alloc((4 - data.length % 4) % 4));
const checksum = (data) => {
  const words = padded(data);
  let sum = 0;
  for (let i = 0; i < words.length; i += 4) sum = (sum + words.readUInt32BE(i)) >>> 0;
  return sum;
};

// INDEX の count は CFF で u16、CFF2 で u32。位置は 1 から数える。
function index(objects, cff2) {
  const count = cff2 ? u32(objects.length) : u16(objects.length);
  if (objects.length === 0) return count;
  const offsets = [1];
  for (const object of objects) offsets.push(offsets.at(-1) + object.length);
  const width = offsets.at(-1) <= 255 ? 1 : 2;
  assert(offsets.at(-1) <= 65535);
  return join(count, bytes(width), offsets.map((n) => u32(n).subarray(4 - width)), objects);
}

// DICT の整数を固定長にして、位置を埋め直しても長さが変わらないようにする。
const dict = (...entries) => join(entries.map(([operands, op]) =>
  join(operands.map((n) => join(bytes(29), u32(n))), bytes(op))));
const operators = { rmoveto: 21, hlineto: 6, rrcurveto: 8, callsubr: 10, callgsubr: 29, return: 11, endchar: 14 };
const program = (text) => join(text.split(/\s+/).filter(Boolean).map((token) => {
  if (token in operators) return bytes(operators[token]);
  const n = Number(token);
  assert(Number.isInteger(n) && n >= -32768 && n <= 32767);
  return n >= -107 && n <= 107 ? bytes(n + 139) : join(bytes(28), u16(n));
}));

function cffTable(cff2) {
  const end = cff2 ? "" : "endchar";
  const ret = cff2 ? "" : "return";
  // .notdef・H・g の 3 字形。H は CFF が幅 400、CFF2 が幅 700 の長方形。
  // g は局所と全体の副ルーチンを通る曲線で、外形は [100, 0, 400, 300]。
  const width = cff2 ? 700 : 400;
  // CFF の字形の幅は hmtx と揃える。CFF2 は幅を hmtx だけに持つ。
  const advance = (n) => cff2 ? "" : String(n);
  const chars = index([
    program(`${advance(500)} 50 0 rmoveto 400 700 -400 hlineto ${end}`),
    program(`${advance(800)} 0 0 rmoveto ${width} 700 ${-width} hlineto ${end}`),
    program(`${advance(600)} -107 callsubr -107 callgsubr ${end}`),
  ], cff2);
  const globals = index([program(`0 400 300 0 0 -400 rrcurveto ${ret}`)], cff2);
  const locals = index([program(`100 0 rmoveto ${ret}`)], cff2);
  const privateDict = dict([[6], [19]]);
  if (!cff2) {
    const names = index([Buffer.from("ZumenSyntheticCFF")], false);
    const strings = index([], false);
    const top = (at) => dict([[at], [17]], [[privateDict.length, at + chars.length], [18]]);
    const at = 4 + names.length + index([top(0)], false).length + strings.length + globals.length;
    return join(bytes(1, 0, 4, 4), names, index([top(at)], false), strings, globals, chars, privateDict, locals);
  }
  // Font DICT が 1 つなので、FDSelect は省く。CFF2 は endchar・return を持たない。
  const top = (charsAt, fontsAt) => dict([[charsAt], [17]], [[fontsAt], [12, 36]]);
  const fonts = (privateAt) => index([dict([[privateDict.length, privateAt], [18]])], true);
  const charsAt = 5 + top(0, 0).length + globals.length;
  const fontsAt = charsAt + chars.length;
  const privateAt = fontsAt + fonts(0).length;
  const topDict = top(charsAt, fontsAt);
  return join(bytes(2, 0, 5), u16(topDict.length), topDict, globals, chars, fonts(privateAt), privateDict, locals);
}

function font(cff2) {
  const head = Buffer.alloc(54);
  head.writeUInt32BE(0x00010000, 0);
  head.writeUInt32BE(0x5f0f3cf5, 12);
  head.writeUInt16BE(1000, 18);
  head.writeInt16BE(700, 40);
  head.writeInt16BE(700, 42);
  const hhea = Buffer.alloc(36);
  hhea.writeUInt32BE(0x00010000, 0);
  hhea.writeInt16BE(900, 4);
  hhea.writeInt16BE(-250, 6);
  hhea.writeUInt16BE(800, 10);
  hhea.writeInt16BE(1, 18);
  hhea.writeUInt16BE(3, 34);
  const os2 = Buffer.alloc(96);
  os2.writeUInt16BE(4, 0);
  os2.writeUInt16BE(400, 4);
  os2.writeUInt16BE(5, 6);
  os2.writeUInt16BE(0x40, 62);
  os2.writeUInt16BE(0x48, 64);
  os2.writeUInt16BE(0x67, 66);
  os2.writeInt16BE(900, 68);
  os2.writeInt16BE(-250, 70);
  os2.writeUInt16BE(900, 74);
  os2.writeUInt16BE(250, 76);
  os2.writeInt16BE(700, 88);
  const cmap = join(u16(0), u16(1), u16(3), u16(10), u32(12),
    u16(12), u16(0), u32(40), u32(0), u32(2),
    u32(0x48), u32(0x48), u32(1), u32(0x67), u32(0x67), u32(2));
  const nameText = Buffer.from(cff2 ? "ZumenSyntheticCFF2" : "ZumenSyntheticCFF", "utf16le").swap16();
  const name = join(u16(0), u16(1), u16(18), u16(3), u16(1), u16(0x409), u16(6), u16(nameText.length), u16(0), nameText);
  const post = join(u32(0x00030000), Buffer.alloc(28));
  const tables = [
    [cff2 ? "CFF2" : "CFF ", cffTable(cff2)], ["OS/2", os2], ["cmap", cmap],
    ["head", head], ["hhea", hhea], ["hmtx", join([500, 800, 600].map((n) => join(u16(n), u16(0))))],
    ["maxp", join(u32(0x00005000), u16(3))], ["name", name], ["post", post],
  ].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0);
  const entrySelector = Math.floor(Math.log2(tables.length));
  const searchRange = 16 * 2 ** entrySelector;
  const directory = [Buffer.from("OTTO"), u16(tables.length), u16(searchRange), u16(entrySelector), u16(16 * tables.length - searchRange)];
  let offset = 12 + tables.length * 16;
  let headAt;
  for (const [tag, data] of tables) {
    if (tag === "head") headAt = offset;
    directory.push(Buffer.from(tag), u32(checksum(data)), u32(offset), u32(data.length));
    offset += padded(data).length;
  }
  const out = join(directory, tables.map(([, data]) => padded(data)));
  out.writeUInt32BE((0xb1b0afba - checksum(out)) >>> 0, headAt + 8);
  return out;
}

function collection(fonts) {
  let offset = 12 + fonts.length * 4;
  const offsets = [];
  const bodies = fonts.map((data) => {
    const out = Buffer.from(data);
    offsets.push(offset);
    for (let i = 0; i < out.readUInt16BE(4); i++) {
      const record = 12 + i * 16;
      const tableAt = out.readUInt32BE(record + 8);
      // コレクションの表の位置はファイルの先頭から数える。head の調整値は使わない。
      if (out.toString("ascii", record, record + 4) === "head") out.writeUInt32BE(0, tableAt + 8);
      out.writeUInt32BE(tableAt + offset, record + 8);
    }
    offset += out.length;
    return out;
  });
  return join(Buffer.from("ttcf"), u32(0x00010000), u32(fonts.length), offsets.map(u32), bodies);
}

const cff = font(false);
const cff2 = font(true);
const fonts = [["cff.otf", cff], ["cff2.otf", cff2], ["collection.otc", collection([cff, cff2])]];
const steps = [{ push: { name: "a.dxf", text: "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nTEXT\n5\nA1\n8\n0\n10\n0\n20\n0\n40\n7\n1\nHg\n0\nENDSEC\n0\nEOF\n" } },
  ...fonts.map(([name, data]) => ({ push: { name, hex: data.toString("hex") } })),
  { call: { v: 1, op: "open", input: "a.dxf" }, expect: { "/result/format": "dxf" } }];
for (const [input, index] of [["cff.otf", 0], ["cff2.otf", 0], ["collection.otc", 0], ["collection.otc", 1]]) {
  steps.push({ call: { v: 1, op: "scene", doc: "a.dxf", from: "space/0", tolerance: 0.01,
    fonts: [{ input, index }], heights: { "dxf.TEXT.40": "cap_height" } },
    expect: { "/result/counts/texts": 1, "/result/counts/glyphs": 2 },
  }, { take: 0 });
}
const output = JSON.stringify({ description: "合成の CFF・CFF2 の直線と 3 次曲線を字形にし、TTC の 2 つ目を選べることを確かめる（#564）。generate-font-cases.mjs で作る。H の幅は CFF が 400、CFF2 が 700。g は局所と全体の副ルーチンを通り、外形は [100, 0, 400, 300]", steps }, null, 2) + "\n";
const destination = new URL("cases/scene_fonts_cff.json", import.meta.url);
assert(process.argv.length === 2 || process.argv.length === 3 && process.argv[2] === "--check");
if (process.argv[2] === "--check") assert.equal(readFileSync(destination, "utf8"), output);
else writeFileSync(destination, output);
console.log(`合成フォント ${fonts.length} 件、描画 ${4} 件`);
