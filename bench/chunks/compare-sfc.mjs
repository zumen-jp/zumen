// 松浦市の公開 SFC について、用紙ビューと実寸の空間をネイティブと wasm で照らす。
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { parseArgs } from "node:util";
import { load, readScene } from "../../bindings/js/src/index.js";
import { openProcess } from "../../bindings/js/src/process.js";

const { positionals, values } = parseArgs({ allowPositionals: true, options: { font: { type: "string" } } });
const [cli, wasm, ...files] = positionals;
assert(cli && wasm && files.length > 0, "compare-sfc.mjs [--font <OTF>] <CLI> <wasm> <SFC> ...");
const module = await load(await readFile(wasm));
const inputs = new Map([
  ["46d58faf3b00c80a21c7e01631336647f99d9bf4b5fbaf7510752416d6d8591e", 1],
  ["0d74c3b7b7337a4413b51e5f24be70fc4603859426c87becd5927857a81c7ea1", 2],
]);
const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const font = values.font ? await readFile(values.font) : undefined;
const fontHash = font ? digest(font) : null;

// 図面の文字列を出さず、漢字・ひらがな・カタカナを含む文字要素と、字形を持つ行を数える。
function textCounts(bytes, counts) {
  const scene = readScene(bytes);
  const { text_strings, text_glyphs, glyph_shapes } = scene.columns;
  assert.equal(scene.rows("text_strings"), counts.texts);
  assert.equal(glyph_shapes.length, counts.glyphs);
  let japaneseTexts = 0;
  let textsWithGlyphs = 0;
  let japaneseWithGlyphs = 0;
  for (let i = 0; i < counts.texts; i++) {
    const label = scene.text(text_strings[2 * i], text_strings[2 * i + 1]);
    const japanese = /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]/u.test(label);
    if (japanese) japaneseTexts++;
    const first = text_glyphs[i] ?? 0;
    const end = text_glyphs[i + 1] ?? glyph_shapes.length;
    if (end > first) {
      textsWithGlyphs++;
      if (japanese) japaneseWithGlyphs++;
    }
  }
  return {
    japaneseTexts, textsWithGlyphs, japaneseWithGlyphs,
    shapes: scene.rows("shape_rings"), rings: scene.rows("outline_vertices"), outlineVertices: scene.rows("outline_xy"),
  };
}

for (const file of files) {
  const input = await readFile(file);
  const sheet = inputs.get(digest(input));
  assert(sheet, "松浦市が配布する SFC の図郭 1・2 を指定してください");
  // 大きい図郭は Base64 にした入力が 64 MiB を超える。
  const native = await openProcess(cli, { v: 1 }, { maxMessageBytes: 128 * 1024 * 1024, timeoutMs: 120_000 });
  const session = module.open();
  try {
    await native.push("sheet.sfc", input);
    session.push("sheet.sfc", input);
    if (font) {
      await native.push("face.otf", font);
      session.push("face.otf", font);
    }
    async function call(op, fields) {
      const request = JSON.stringify({ v: 1, op, ...fields });
      const reply = Buffer.from(await native.callBytes(request));
      assert(reply.equals(Buffer.from(session.callBytes(request))), op);
      const parsed = JSON.parse(reply);
      assert(parsed.result, op);
      return parsed;
    }
    await call("open", { input: "sheet.sfc" });
    const { result } = await call("drawing", { doc: "sheet.sfc" });
    assert.equal(result.sheets.length, 1);
    assert.equal(result.sheets[0].views, 1);
    assert.equal(result.spaces.length, 1);
    for (const { id: from } of [...result.sheets, ...result.spaces]) {
      const checkFont = font && from === result.sheets[0].id;
      const options = [{}, { tolerance: 0.01 }];
      if (checkFont) options.push({ fonts: [{ input: "face.otf" }] });
      for (const fields of options) {
        const scene = await call("scene", { doc: "sheet.sfc", from, ...fields });
        const bytes = Buffer.from(await native.take(scene.result.blob));
        assert(bytes.equals(Buffer.from(session.take(scene.result.blob))));
        assert.equal(bytes.length, scene.blobs[0].size);
        console.log(JSON.stringify({
          sheet, from, tolerance: scene.result.tolerance, counts: scene.result.counts,
          bytes: bytes.length, sha256: digest(bytes),
          diagnostics: scene.diagnostics.map(({ code, count }) => ({ code, count })),
          ...(checkFont && fields.tolerance === undefined ? {
            fontSha256: fields.fonts ? fontHash : null, ...textCounts(bytes, scene.result.counts),
          } : {}),
        }));
      }
    }
  } finally {
    session.close();
    await native.close();
  }
}
