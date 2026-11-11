// JavaScript を動かす範囲と、公開に要る資産をビルドのたびに確かめる（決定 0296）。
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
function files(folder) {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(folder, entry.name);
    return entry.isDirectory() ? files(path) : [path];
  });
}
const output = files(dist);
const viewer = readFileSync(resolve(dist, "viewer/index.html"), "utf8");
assert.match(viewer, /<script[^>]+src="\/viewer\/assets\//);
assert.match(viewer, /href="\/viewer\/viewer\.css"/);
const wasm = output.filter((path) => /[/\\]viewer[/\\].+\.wasm$/.test(path));
assert.equal(wasm.length, 1);
const bom = JSON.parse(readFileSync(resolve(dist, "viewer/sboms/zumen_abi1_drawing.wasm.cdx.json"), "utf8"));
assert.ok(bom.metadata.component.hashes.some(({ alg, content }) => alg === "SHA-256" && content === createHash("sha256").update(readFileSync(wasm[0])).digest("hex")));
for (const name of ["LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_LICENSES"]) assert.ok(readFileSync(resolve(dist, "viewer", name)).length > 0);
assert.ok(output.some((path) => /[/\\]viewer[/\\]assets[/\\]worker-[^/\\]+\.js$/.test(path)));
for (const path of output.filter((path) => path.endsWith(".html") && path !== resolve(dist, "viewer/index.html"))) {
  const html = readFileSync(path, "utf8");
  assert.doesNotMatch(html, /<script\b/i, "案内ページに script がある");
  // CSP3 §3.3。先に現れた link や script の読み込みは meta CSP では止められない。
  assert.match(html, /<head>\s*<meta charset="utf-8">\s*(?:<!--[\s\S]*?-->\s*)?<meta http-equiv="Content-Security-Policy" content="script-src 'none'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">/i,
    "案内ページの先頭にスクリプトを拒否する CSP がない");
}
// ビューアーのヘッダーは、案内のサイトのナビと同じ並びにする（site/src/data/site.ts の nav）。
const nav = [...readFileSync(new URL("../src/data/site.ts", import.meta.url), "utf8").matchAll(/\{ href: '([^']+)', label: '([^']+)' \}/g)];
assert.ok(nav.length > 0, "サイトのナビを読めない");
const bundle = output.filter((path) => /[/\\]viewer[/\\]assets[/\\][^/\\]+\.js$/.test(path)).map((path) => readFileSync(path, "utf8")).join("\n");
const viewerNav = [...bundle.matchAll(/\{\s*href:\s*(["`])([^"`]+)\1,\s*label:\s*(["`])([^"`]+)\3\s*\}/g)].map(([, , href, , label]) => href + " " + label);
assert.deepEqual(viewerNav, nav.map(([, href, label]) => href + " " + label), "ビューアーのナビが案内のサイトと違う");
const headers = readFileSync(resolve(dist, "_headers"), "utf8");
assert.match(headers, /^\/\*\r?\n\s+Cache-Control: public, max-age=0, must-revalidate, no-transform/m);
assert.match(headers, /\/viewer\/\*/);
assert.match(headers, /connect-src 'self'/);
assert.match(headers, /frame-ancestors 'none'/);
assert.ok(readFileSync(resolve(dist, "viewer/licenses.md"), "utf8").includes("react"));
const samples = readFileSync(resolve(dist, "viewer/samples.md"), "utf8");
assert.ok(samples.includes("zumen-layout-r2000.dwg"));
assert.doesNotMatch(samples, /zumen-(minsert-r2004|xclip-r2018)\.dwg/);
const pair = readFileSync(new URL("../../samples/dwg/zumen-layout-r2000_ascii.dxf", import.meta.url), "utf8").split(/\r?\n/);
const savedBy = pair.findIndex((line) => line.trim() === "$LASTSAVEDBY");
assert.ok(savedBy < 0 || !pair[savedBy + 2]?.trim(), "DWG の対の DXF に保存者のメタデータがある");
console.log("サイトの出力: 案内ページの script なし・先頭の CSP・ビューアーの wasm/Worker/CSS/CSP・ナビ・ライセンスを確認");
