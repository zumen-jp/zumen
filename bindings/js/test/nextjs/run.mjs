// TypeScript・JavaScript の包みを、Next.js のサーバー側で組んで使えることを確かめる（決定 0259）。
//
// 使い方: node bindings/js/test/nextjs/run.mjs <zumen_abi1.wasm>
//
// 先に、このフォルダーで `npm ci --ignore-scripts` を流しておく。包みに wasm を入れて npm pack し、このフォルダーに
// 入れる。Turbopack（設定なし）と webpack（serverExternalPackages）のそれぞれで standalone に組み、組んだサーバーを
// 起こして、API ルート（app/api/open/route.js）に公開サンプルの DXF を送り、開けることを確かめる。

import { spawn, spawnSync } from "node:child_process";
import { copyFileSync, cpSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) {
  console.error("使い方: node bindings/js/test/nextjs/run.mjs <zumen_abi1.wasm>");
  process.exit(2);
}

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../../..");
const sample = readFileSync(join(root, "samples", "dxf", "text_alignments.dxf"));
// 開発用のポートの範囲（AGENTS.md）
const port = 24131;
const env = { ...process.env, NEXT_TELEMETRY_DISABLED: "1" };
const next = join(here, "node_modules", "next", "dist", "bin", "next");

const variants = [
  { name: "Turbopack", args: [], config: {} },
  { name: "webpack", args: ["--webpack"], config: { serverExternalPackages: ["@zumen-jp/zumen"] } },
];

/** コマンドを流し、失敗したら止める。Windows の npm は npm.cmd なので、シェルを通す */
function run(command, args, cwd) {
  const shell = process.platform === "win32" && command === "npm";
  const result = spawnSync(command, args, { cwd, env, stdio: "inherit", shell });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} が ${result.status ?? result.signal} で終わった`);
  }
}

/** 包みに wasm とライセンスの本文を入れて npm pack し、tgz の道筋を返す */
function pack(work) {
  const staged = join(work, "package");
  cpSync(join(root, "bindings", "js", "src"), join(staged, "src"), { recursive: true });
  for (const name of ["package.json", "README.md"]) {
    copyFileSync(join(root, "bindings", "js", name), join(staged, name));
  }
  for (const name of ["LICENSE-MIT", "LICENSE-APACHE"]) {
    copyFileSync(join(root, name), join(staged, name));
  }
  copyFileSync(wasmPath, join(staged, "zumen_abi1.wasm"));
  run("npm", ["pack", "--pack-destination", work], staged);
  return join(work, readdirSync(work).find((name) => name.endsWith(".tgz")));
}

/** 組み方の設定を書き、standalone に組む */
function build(variant) {
  // リポジトリの直下にも package-lock.json があるので、根を明示する
  const config = { output: "standalone", outputFileTracingRoot: here, turbopack: { root: here }, ...variant.config };
  writeFileSync(join(here, "next.config.mjs"), `export default ${JSON.stringify(config, null, 2)};\n`);
  rmSync(join(here, ".next"), { recursive: true, force: true });
  run(process.execPath, [next, "build", ...variant.args], here);
}

/** サーバーが応えるまで待つ。API ルートは POST だけなので、GET には 405 が返る */
async function waitFor(url, server) {
  for (let attempt = 0; attempt < 150; attempt++) {
    if (server.exitCode !== null) {
      throw new Error(`サーバーが ${server.exitCode} で終わった`);
    }
    try {
      await fetch(url);
      return;
    } catch {
      await new Promise((done) => setTimeout(done, 200));
    }
  }
  throw new Error("サーバーが応えない");
}

/** 組んだサーバーを起こし、API ルートで図面を開く */
async function serve(variant) {
  const standalone = join(here, ".next", "standalone");
  const server = spawn(process.execPath, [join(standalone, "server.js")], {
    cwd: standalone,
    env: { ...env, PORT: String(port), HOSTNAME: "127.0.0.1" },
    stdio: ["ignore", "inherit", "inherit"],
  });
  const exited = new Promise((done) => server.once("exit", done));
  try {
    const url = `http://127.0.0.1:${port}/api/open`;
    await waitFor(url, server);
    const response = await fetch(url, { method: "POST", body: sample });
    const body = await response.json();
    if (response.status !== 200 || body.format !== "dxf" || !(body.total > 0)) {
      throw new Error(`${variant.name}: 期待と違う応答（${response.status}）: ${JSON.stringify(body)}`);
    }
    console.log(`${variant.name}: ${JSON.stringify(body)}`);
  } finally {
    server.kill();
    await exited;
  }
}

const work = mkdtempSync(join(tmpdir(), "zumen-nextjs-"));
try {
  const tgz = pack(work);
  run("npm", ["install", "--no-save", "--ignore-scripts", "--no-audit", "--no-fund", tgz], here);
  for (const variant of variants) {
    build(variant);
    await serve(variant);
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}
console.log("Next.js の Turbopack と webpack で組み、API ルートで図面を開けた");
