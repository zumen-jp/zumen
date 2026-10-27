// ブラウザーで測るための静的な配信。使い方: node serve.mjs <encode が書いたディレクトリ>
// http://localhost:24180/browser.html を開く。
import { createServer } from "node:http";
import { createReadStream, existsSync } from "node:fs";
import { extname, join, normalize } from "node:path";

const data = process.argv[2] ?? "out";
const types = { ".html": "text/html", ".mjs": "text/javascript", ".js": "text/javascript", ".bin": "application/octet-stream" };
createServer((request, response) => {
  const path = normalize(decodeURIComponent(new URL(request.url, "http://localhost").pathname)).replace(/^(\.\.[/\\])+/, "");
  const file = path.startsWith("/data/") ? join(data, path.slice(6)) : join(import.meta.dirname, path);
  if (!existsSync(file)) {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream" });
  createReadStream(file).pipe(response);
}).listen(24180);
