// 案内サイトの出力に JavaScript を混ぜず、公開に要る CSP・ヘッダー・リダイレクトをビルドのたびに確かめる。
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
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
assert.equal(existsSync(resolve(dist, "viewer")), false, "廃止した簡易ビューアー（dist/viewer）が残っている");

for (const path of output.filter((entry) => entry.endsWith(".html"))) {
  const html = readFileSync(path, "utf8");
  assert.doesNotMatch(html, /<script\b/i, `案内ページに script がある: ${path}`);
  // CSP3 §3.3。先に現れた link や script の読み込みは meta CSP では止められない。
  assert.match(
    html,
    /<head>\s*<meta charset="utf-8">\s*(?:<!--[\s\S]*?-->\s*)?<meta http-equiv="Content-Security-Policy" content="script-src 'none'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">/i,
    `案内ページの先頭にスクリプトを拒否する CSP がない: ${path}`,
  );
  assert.doesNotMatch(html, /href="\/viewer\//, `廃止した /viewer/ へのリンクが残っている: ${path}`);
}

const indexHtml = readFileSync(resolve(dist, "index.html"), "utf8");
assert.match(indexHtml, /href="https:\/\/editor\.zumen\.dev\/"/, "トップページに editor.zumen.dev へのリンクがない");

const headers = readFileSync(resolve(dist, "_headers"), "utf8");
assert.match(headers, /^\/\*\r?\n\s+Cache-Control: public, max-age=0, must-revalidate, no-transform/m);
assert.match(headers, /Referrer-Policy: no-referrer/);
assert.match(headers, /X-Content-Type-Options: nosniff/);

const redirects = readFileSync(resolve(dist, "_redirects"), "utf8");
assert.match(redirects, /^\/viewer https:\/\/editor\.zumen\.dev\/ 301$/m);
assert.match(redirects, /^\/viewer\/\* https:\/\/editor\.zumen\.dev\/ 301$/m);

console.log("サイトの出力: 全ページの script なし・先頭の CSP・editor.zumen.dev リンク・/viewer/ 転送・ヘッダーを確認");
