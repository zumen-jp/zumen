import { createHash } from 'node:crypto';
import { execFileSync, spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, readFile, rename, rm, stat, writeFile } from 'node:fs/promises';
import { homedir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const release = JSON.parse(await readFile(new URL('./release.json', import.meta.url), 'utf8'));

const platforms = { win32: 'win', darwin: 'osx', linux: 'linux' };
const rid = `${platforms[process.platform]}-${process.arch === 'ia32' ? 'x86' : process.arch}`;
const executable = process.platform === 'win32' ? 'zumen.exe' : 'zumen';
const checksums = release.targets[rid];

function verify(bytes, expected) {
  if (createHash('sha256').update(bytes).digest('hex') !== expected) {
    throw new Error('zumen の配布物の SHA-256 が一致しません。');
  }
}

async function install(cache, name, destination) {
  const archive = name + (process.platform === 'win32' ? '.zip' : '.tar.gz');
  const url = `https://github.com/zumen-jp/zumen/releases/download/v${release.version}/${archive}`;
  const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
  if (!response.ok) throw new Error(`zumen の取得に失敗しました (HTTP ${response.status})。`);
  const bytes = Buffer.from(await response.arrayBuffer());
  verify(bytes, checksums.archive);

  const staging = await mkdtemp(path.join(cache, '.install-'));
  try {
    const download = path.join(staging, archive);
    await writeFile(download, bytes);
    const tar = process.platform === 'win32' ? path.join(process.env.SystemRoot, 'System32', 'tar.exe') : '/usr/bin/tar';
    execFileSync(tar, ['-xf', download, '-C', staging], { cwd: staging, windowsHide: true });
    const extracted = path.join(staging, name);
    verify(await readFile(path.join(extracted, executable)), checksums.executable);
    try {
      await rename(extracted, destination);
    } catch (error) {
      // 別のクライアントが同じ版を先に配置した場合は、そのファイルを呼び出し側で検査する。
      if (!existsSync(path.join(destination, executable))) throw error;
    }
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
}

async function locate() {
  if (!checksums) throw new Error(`zumen が対応していない環境です (${process.platform}/${process.arch})。`);
  const bundled = fileURLToPath(new URL('./native/', import.meta.url));
  if (existsSync(bundled)) return path.join(bundled, rid, executable);
  const cache = path.join(homedir(), '.cache', 'zumen', 'plugins');
  const name = `zumen-${release.version}-${rid}`;
  const destination = path.join(cache, name);
  const program = path.join(destination, executable);
  if (!existsSync(program)) {
    await mkdir(cache, { recursive: true, mode: 0o700 });
    await install(cache, name, destination);
  }
  return program;
}

async function drawingsRoot(args) {
  if (args[0] !== '--data-dir') return args[0] || process.env.ZUMEN_DRAWINGS_ROOT;
  const data = args[1];
  if (!data || !path.isAbsolute(data)) throw new Error('プラグインのデータフォルダーを取得できません。');
  const config = path.join(data, 'drawings-root.txt');
  try {
    const root = (await readFile(config, 'utf8')).trim();
    if (!root) throw new Error('drawings-root.txt に図面フォルダーの絶対パスを記入してください。');
    return root;
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const root = path.join(data, 'drawings');
  await mkdir(root, { recursive: true, mode: 0o700 });
  return root;
}

async function main() {
  const root = await drawingsRoot(process.argv.slice(2));
  if (root && (!path.isAbsolute(root) || !(await stat(root)).isDirectory())) {
    throw new Error('図面の作業フォルダーには既存のフォルダーの絶対パスを指定してください。');
  }
  const program = await locate();
  verify(await readFile(program), checksums.executable);
  const args = root ? ['mcp', '--root', root] : ['mcp'];
  const child = spawn(program, args, { stdio: 'inherit', windowsHide: true });
  process.on('SIGTERM', () => child.kill('SIGTERM'));
  process.on('SIGINT', () => child.kill('SIGINT'));
  child.on('error', error => { console.error(error.message); process.exitCode = 1; });
  child.on('exit', code => { process.exitCode = code ?? 1; });
}

main().catch(error => { console.error(error.message); process.exitCode = 1; });
