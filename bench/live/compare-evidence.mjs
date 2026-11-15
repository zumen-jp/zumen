// core-live が保存したバイト列・ID とハッシュ・診断・予算を、環境間で直接比べる。
import { closeSync, openSync, readSync, readdirSync } from "node:fs";
import { join } from "node:path";

const [reference, ...others] = process.argv.slice(2);
if (!reference || others.length === 0) {
  throw new Error("使い方: node bench/live/compare-evidence.mjs <基準のフォルダー> <比べるフォルダー> [...]");
}
function files(root, path = "") {
  return readdirSync(join(root, path), { withFileTypes: true }).flatMap((entry) => {
    const next = join(path, entry.name);
    if (entry.isDirectory()) return files(root, next);
    if (!entry.isFile()) throw new Error(`通常のファイルではない: ${next}`);
    return [next];
  }).sort();
}
function equal(left, right) {
  const handles = [];
  try {
    handles.push(openSync(left, "r"));
    handles.push(openSync(right, "r"));
    const buffers = [Buffer.alloc(1024 * 1024), Buffer.alloc(1024 * 1024)];
    let bytes = 0;
    for (;;) {
      const a = readSync(handles[0], buffers[0]);
      const b = readSync(handles[1], buffers[1]);
      if (a !== b || !buffers[0].subarray(0, a).equals(buffers[1].subarray(0, b))) return null;
      bytes += a;
      if (a === 0) return bytes;
    }
  } finally {
    handles.forEach(closeSync);
  }
}
const names = files(reference);
if (names.length === 0) throw new Error("基準のフォルダーに検証記録がない");
let failed = false;
for (const other of others) {
  const found = files(other);
  if (names.length !== found.length || names.some((name, index) => name !== found[index])) {
    console.error(`${other}: ファイルの一覧が不一致`);
    failed = true;
    continue;
  }
  let bytes = 0;
  let differences = 0;
  for (const name of names) {
    const count = equal(join(reference, name), join(other, name));
    if (count === null) {
      console.error(`${name}: 不一致`);
      differences += 1;
    } else {
      bytes += count;
    }
  }
  console.log(`${other}: ${names.length} ファイル、${bytes} バイト一致、不一致 ${differences}`);
  failed ||= differences !== 0;
}
process.exitCode = failed ? 1 : 0;
