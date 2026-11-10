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
  assert.doesNotMatch(readFileSync(path, "utf8"), /<script\b/i, "案内ページに script がある");
}
const headers = readFileSync(resolve(dist, "_headers"), "utf8");
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
console.log("サイトの出力: 案内ページの script なし・ビューアーの wasm/Worker/CSS/CSP・ライセンスを確認");
