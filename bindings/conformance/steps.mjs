// 適合試験の手順の読み方と応答の確かめ方のうち、ファイルを読まない部分（決定 0012）。
//
// Node.js とブラウザーの両方で読み込む。ファイルを読む部分は cases.mjs にある。ブラウザーで流す試験
// （bindings/js/test/browser/run.mjs、決定 0199）は、ここだけを読み込む。

const encoder = new TextEncoder();

/** 手順の種類。手順は、どれか 1 つの欄を持つ */
export const ACTIONS = ["session", "push", "call", "call_text", "call_hex", "take"];

export function decodeHex(hex) {
  const digits = hex.replace(/\s+/g, "");
  if (!/^([0-9a-fA-F]{2})*$/.test(digits)) {
    throw new Error(`16 進として読めない: ${hex}`);
  }
  return Uint8Array.from(digits.match(/../g) ?? [], (pair) => parseInt(pair, 16));
}

export function pointer(value, path) {
  if (path === "") {
    return value;
  }
  let current = value;
  for (const raw of path.slice(1).split("/")) {
    const token = raw.replaceAll("~1", "/").replaceAll("~0", "~");
    if (Array.isArray(current)) {
      if (!/^(0|[1-9][0-9]*)$/.test(token) || Number(token) >= current.length) {
        return undefined;
      }
      current = current[Number(token)];
    } else if (current !== null && typeof current === "object" && Object.hasOwn(current, token)) {
      current = current[token];
    } else {
      return undefined;
    }
  }
  return current;
}

export function same(a, b) {
  if (Array.isArray(a) || Array.isArray(b)) {
    return Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((x, i) => same(x, b[i]));
  }
  if (a !== null && b !== null && typeof a === "object" && typeof b === "object") {
    const keys = Object.keys(a);
    return keys.length === Object.keys(b).length && keys.every((key) => Object.hasOwn(b, key) && same(a[key], b[key]));
  }
  return a === b;
}

/** session の設定か、call の要求のバイト列。値は JSON に直し、文字列はそのまま渡す */
export function bytesOf(input, valueKey, textKey) {
  if (input === null || input === undefined) {
    return new Uint8Array();
  }
  if (Object.hasOwn(input, valueKey)) {
    return encoder.encode(JSON.stringify(input[valueKey]));
  }
  if (Object.hasOwn(input, textKey)) {
    return encoder.encode(input[textKey]);
  }
  return new Uint8Array();
}

// 公開サンプル（決定 0171）の道筋を確かめる。道筋はリポジトリの根から書き、samples/ の中に限る
export function samplePath(file) {
  const parts = file.split("/");
  if (parts[0] !== "samples" || parts.length < 2 || parts.slice(1).some((p) => p === "" || p === "." || p === ".." || p.includes("\\"))) {
    throw new Error(`push の file は samples/ の中のファイルにする: ${file}`);
  }
  return file;
}

/** push の手順の、入力の名前と中身。readSample は、samples/ の中のファイルの中身を返す */
export function pushOf(input, readSample) {
  const name = Object.hasOwn(input, "name_hex") ? decodeHex(input.name_hex) : encoder.encode(input.name);
  const data = Object.hasOwn(input, "hex")
    ? decodeHex(input.hex)
    : Object.hasOwn(input, "file")
      ? readSample(samplePath(input.file))
      : encoder.encode(input.text ?? "");
  return [name, data];
}

/** call・call_text・call_hex の手順の、要求のバイト列 */
export function requestOf(action, input) {
  if (action === "call_text") {
    return encoder.encode(input);
  }
  if (action === "call_hex") {
    return decodeHex(input);
  }
  return encoder.encode(JSON.stringify(input));
}

/** 応答の値を、手順の expect と absent で確かめる */
export function checkResponse(at, step, response, failures) {
  for (const [path, value] of Object.entries(step.expect ?? {})) {
    const found = pointer(response, path);
    if (found === undefined) {
      failures.push(`${at}: ${path} がない`);
    } else if (!same(found, value)) {
      failures.push(`${at}: ${path} が ${JSON.stringify(found)}（期待は ${JSON.stringify(value)}）`);
    }
  }
  for (const path of step.absent ?? []) {
    if (pointer(response, path) !== undefined) {
      failures.push(`${at}: ${path} があってはいけない`);
    }
  }
}

/**
 * 組を流す wasm のモジュールの名前（決定 0221）。`modules` は包みの MODULES。組の `module` があればそれを、なければ
 * 組が呼ぶ op（どのモジュールにもない op は除く）をすべて持つ最初のモジュールを返す。`all` は、すべてのモジュールを
 * 入れたコアでだけ流す組。どのモジュールでも流せなければ undefined
 */
export function moduleOf(testCase, modules) {
  if (testCase.module !== undefined) {
    return testCase.module;
  }
  const names = Object.keys(modules);
  const ops = testCase.steps
    .map((step) => step.call?.op)
    .filter((op) => typeof op === "string" && names.some((name) => modules[name].ops.includes(op)));
  return names.find((name) => ops.every((op) => modules[name].ops.includes(op)));
}
