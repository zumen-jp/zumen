// #520: MIT の固定ソースを target へ取得し、未改変の算術関数だけをコンパイルする。
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { harness } from "./harness.mjs";
import { pin, sourcePath, sourceUrl, sourceBlob, sourceSha256, artifactDirectory, executableName, signatures, sha256, gitBlob } from "./common.mjs";

// C++ のコメントと文字列内の括弧を除き、関数の元の byte span を切り出す。
function extract(bytes, signature) {
  const start = bytes.indexOf(signature);
  if (start < 0 || bytes.indexOf(signature, start + signature.length) >= 0) throw new Error(`関数が一意ではありません: ${signature}`);
  const opening = bytes.indexOf("{", start + signature.length);
  let depth = 0, state = "code";
  for (let i = opening; i < bytes.length; i++) {
    const c = bytes[i], next = bytes[i + 1];
    if (state === "line") { if (c === 10) state = "code"; continue; }
    if (state === "block") { if (c === 42 && next === 47) { state = "code"; i++; } continue; }
    if (state === "string" || state === "char") {
      if (c === 92) i++;
      else if ((state === "string" && c === 34) || (state === "char" && c === 39)) state = "code";
      continue;
    }
    if (c === 47 && next === 47) { state = "line"; i++; }
    else if (c === 47 && next === 42) { state = "block"; i++; }
    else if (c === 34) state = "string";
    else if (c === 39) state = "char";
    else if (c === 123) depth++;
    else if (c === 125 && --depth === 0) {
      return { signature, start, end: i + 1, sha256: sha256(bytes.subarray(start, i + 1)) };
    }
  }
  throw new Error(`関数の終わりが見つかりません: ${signature}`);
}

const quotePowerShell = text => `'${text.replaceAll("'", "''")}'`;
function compile() {
  const input = join(artifactDirectory, "harness.cpp");
  const output = join(artifactDirectory, executableName);
  if (process.platform === "win32") {
    const vswhere = join(process.env["ProgramFiles(x86)"] ?? "C:/Program Files (x86)", "Microsoft Visual Studio/Installer/vswhere.exe");
    const found = spawnSync(vswhere, ["-latest", "-products", "*", "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-property", "installationPath"], { encoding: "utf8", windowsHide: true });
    if (found.status !== 0 || !found.stdout.trim()) throw new Error("MSVC のある Visual Studio が見つかりません");
    const installation = found.stdout.trim();
    const module = join(installation, "Common7/Tools/Microsoft.VisualStudio.DevShell.dll");
    const args = ["/nologo", "/std:c++20", "/EHsc", "/O2", "/fp:strict", "/MT", "/W4", "/WX", "/utf-8", `/Fe${output}`, `/Fo${join(artifactDirectory, "harness.obj")}`, input];
    const command = `Import-Module ${quotePowerShell(module)}; Enter-VsDevShell -VsInstallPath ${quotePowerShell(installation)} -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null; & cl @(${args.map(quotePowerShell).join(",")}); exit $LASTEXITCODE`;
    const built = spawnSync("pwsh", ["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", command], { cwd: artifactDirectory, stdio: "inherit", timeout: 120_000, windowsHide: true });
    if (built.error || built.status !== 0) throw new Error("MSVC で照合プログラムを作れませんでした");
    return { family: "msvc", flags: args.slice(0, 9) };
  }
  const candidates = process.env.CXX ? [process.env.CXX] : ["c++", "g++", "clang++"];
  const compiler = candidates.find(command => spawnSync(command, ["--version"], { stdio: "ignore" }).status === 0);
  if (!compiler) throw new Error("C++20 のコンパイラーが見つかりません");
  const flags = ["-std=c++20", "-O2", "-fno-fast-math", "-ffp-contract=off", "-Wall", "-Wextra", "-Werror"];
  const built = spawnSync(compiler, [...flags, input, "-o", output], { cwd: artifactDirectory, stdio: "inherit", timeout: 120_000 });
  if (built.error || built.status !== 0) throw new Error("照合プログラムを作れませんでした");
  return { family: compiler, flags };
}

try {
  await mkdir(artifactDirectory, { recursive: true });
  let source;
  try { source = await readFile(join(artifactDirectory, "pdfmeasure.upstream.cpp")); }
  catch (error) { if (error.code !== "ENOENT") throw error; }
  if (!source) {
    const response = await fetch(sourceUrl, { signal: AbortSignal.timeout(30_000) });
    if (!response.ok) throw new Error(`固定ソースを取得できませんでした: HTTP ${response.status}`);
    source = Buffer.from(await response.arrayBuffer());
  }
  if (source.length > 64 * 1024 || sha256(source) !== sourceSha256 || gitBlob(source) !== sourceBlob) {
    throw new Error("取得した PDF4QT の固定ソースの hash が違います");
  }
  const headerEnd = source.indexOf("#include");
  const header = source.subarray(0, headerEnd);
  if (!header.toString("utf8").startsWith("// MIT License")) throw new Error("MIT の原文ヘッダーを確認できませんでした");
  const spans = signatures.map(signature => extract(source, signature));
  const definitions = Buffer.concat([
    header, Buffer.from("\nnamespace pdf {\n"),
    ...spans.flatMap(span => [source.subarray(span.start, span.end), Buffer.from("\n\n")]),
    Buffer.from("}\n"),
  ]);
  await writeFile(join(artifactDirectory, "pdfmeasure.upstream.cpp"), source);
  await writeFile(join(artifactDirectory, "upstream-functions.inc"), definitions);
  await writeFile(join(artifactDirectory, "harness.cpp"), harness);
  const compiler = compile();
  const files = {};
  for (const name of ["pdfmeasure.upstream.cpp", "upstream-functions.inc", "harness.cpp", executableName]) {
    files[name] = sha256(await readFile(join(artifactDirectory, name)));
  }
  const manifest = {
    protocol: 1, pin, platform: process.platform, arch: process.arch,
    source: { path: sourcePath, url: sourceUrl, blob: sourceBlob, sha256: sourceSha256, bytes: source.length },
    spans, compiler, files,
    setup_sha256: sha256(await readFile(new URL("setup.mjs", import.meta.url))),
    common_sha256: sha256(await readFile(new URL("common.mjs", import.meta.url))),
    harness_sha256: sha256(await readFile(new URL("harness.mjs", import.meta.url))),
  };
  await writeFile(join(artifactDirectory, "manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  process.stdout.write(`PDF4QT の算術照合を準備しました: ${artifactDirectory}\n`);
} catch (error) {
  process.stderr.write(`計測照合の準備: ${error.message}\n`);
  process.exitCode = 1;
}
