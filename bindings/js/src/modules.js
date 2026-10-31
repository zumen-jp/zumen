// wasm のモジュールを分けて読み込む（決定 0013・0221）。
//
// 入力の形式を、コアと同じ順と規則（constants.js の DETECTION、元は schema/boundary.toml）で判定し、その形式を持つ
// モジュールだけを読み込む。規則の読み方は、zumen-sdk の Signature と同じにする。

import { DETECTION, MODULES } from "./constants.js";

const encoder = new TextEncoder();

function view(bytes) {
  if (bytes instanceof Uint8Array) {
    return bytes;
  }
  if (ArrayBuffer.isView(bytes)) {
    return new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }
  return new Uint8Array(bytes);
}

function isSpace(byte) {
  return byte === 0x20 || (byte >= 0x09 && byte <= 0x0d);
}

function isAlpha(byte) {
  return (byte >= 0x41 && byte <= 0x5a) || (byte >= 0x61 && byte <= 0x7a);
}

function isWord(byte) {
  return isAlpha(byte) || (byte >= 0x30 && byte <= 0x39) || byte === 0x5f || byte === 0x2d;
}

function upper(byte) {
  return byte >= 0x61 && byte <= 0x7a ? byte - 0x20 : byte;
}

function equalsIgnoreCase(head, start, end, text) {
  if (end - start !== text.length) {
    return false;
  }
  for (let i = 0; i < text.length; i++) {
    if (upper(head[start + i]) !== upper(text.charCodeAt(i))) {
      return false;
    }
  }
  return true;
}

/** `offset` から `bytes` が続く */
function matchBytes(head, offset, bytes) {
  if (offset + bytes.length > head.length) {
    return false;
  }
  return bytes.every((byte, i) => head[offset + i] === byte);
}

/** 先頭の行が、前後の ASCII の空白を除いて `lines` と一致する。空の行は飛ばさない */
function matchLines(head, lines) {
  let start = 0;
  for (const expected of lines) {
    if (start > head.length) {
      return false;
    }
    let end = head.indexOf(0x0a, start);
    const next = end === -1 ? head.length + 1 : end + 1;
    if (end === -1) {
      end = head.length;
    }
    let from = start;
    let to = end;
    while (from < to && isSpace(head[from])) {
      from++;
    }
    while (to > from && isSpace(head[to - 1])) {
      to--;
    }
    const want = encoder.encode(expected);
    if (to - from !== want.length || !want.every((byte, i) => head[from + i] === byte)) {
      return false;
    }
    start = next;
  }
  return true;
}

/** 空白とコメントを飛ばした位置。コメントが終わらなければ -1 */
function skip(head, at) {
  for (;;) {
    while (at < head.length && isSpace(head[at])) {
      at++;
    }
    if (head[at] !== 0x2f || head[at + 1] !== 0x2a) {
      return at;
    }
    let end = -1;
    for (let i = at + 2; i + 1 < head.length; i++) {
      if (head[i] === 0x2a && head[i + 1] === 0x2f) {
        end = i;
        break;
      }
    }
    if (end === -1) {
      return -1;
    }
    at = end + 2;
  }
}

/** 文字列の残りを飛ばした位置。`''` は引用符 1 つ。終わらなければ -1 */
function afterString(head, at) {
  for (;;) {
    const end = head.indexOf(0x27, at);
    if (end === -1) {
      return -1;
    }
    at = end + 1;
    if (head[at] !== 0x27) {
      return at;
    }
    at++;
  }
}

/** ISO 10303-21 の見出しの FILE_SCHEMA の最初の名前。読めなければ undefined */
function stepSchema(head) {
  const magic = encoder.encode("ISO-10303-21;");
  let at = 0;
  while (at < head.length && isSpace(head[at])) {
    at++;
  }
  if (!matchBytes(head, at, magic)) {
    return undefined;
  }
  at += magic.length;
  for (;;) {
    at = skip(head, at);
    if (at === -1 || at >= head.length) {
      return undefined;
    }
    const byte = head[at];
    if (byte === 0x27) {
      at = afterString(head, at + 1);
      if (at === -1) {
        return undefined;
      }
    } else if (isAlpha(byte) || byte === 0x5f) {
      let end = at;
      while (end < head.length && isWord(head[end])) {
        end++;
      }
      if (equalsIgnoreCase(head, at, end, "FILE_SCHEMA")) {
        return firstName(head, end);
      }
      if (equalsIgnoreCase(head, at, end, "DATA") || equalsIgnoreCase(head, at, end, "ENDSEC")) {
        return undefined;
      }
      at = end;
    } else {
      at++;
    }
  }
}

/** `(('NAME' …` の NAME */
function firstName(head, at) {
  for (const open of [0x28, 0x28, 0x27]) {
    at = skip(head, at);
    if (at === -1 || head[at] !== open) {
      return undefined;
    }
    at++;
  }
  const end = head.indexOf(0x27, at);
  return end === -1 ? undefined : head.subarray(at, end);
}

/** 規則に合うか */
function matches(head, rule) {
  switch (rule.kind) {
    case "bytes":
      return matchBytes(head, rule.offset, rule.bytes);
    case "lines":
      return matchLines(head, rule.lines);
    case "step_schema": {
      const name = stepSchema(head);
      return name !== undefined && name.length >= rule.prefix.length && equalsIgnoreCase(name, 0, rule.prefix.length, rule.prefix);
    }
    default:
      return false;
  }
}

/** 入力の形式を、コアと同じ順と規則で判定する。どの形式でもなければ undefined */
export function detect(bytes) {
  const head = view(bytes);
  return DETECTION.find((detection) => detection.rules.some((rule) => matches(head, rule)));
}

/** op を持つモジュールの名前 */
export function modulesWith(op) {
  return Object.keys(MODULES).filter((name) => MODULES[name].ops.includes(op));
}

/**
 * 分けた wasm のモジュールを、要るときに 1 度だけ読み込む。
 *
 * `loaders` は、モジュールの名前から、読み込む関数（`load` に渡せるものを返す）を引く表。持たないモジュールは使えない。
 * `load` は、包みの `load`。
 */
export class Modules {
  #loaders;
  #load;
  #loaded = new Map();

  constructor(loaders, load) {
    this.#loaders = loaders;
    this.#load = load;
  }

  /** 名前のモジュールを読み込む */
  get(name) {
    let loaded = this.#loaded.get(name);
    if (loaded === undefined) {
      const loader = this.#loaders[name];
      if (MODULES[name] === undefined || typeof loader !== "function") {
        return Promise.reject(new Error(`wasm のモジュール ${name} を読み込めない`));
      }
      loaded = this.#load(loader());
      this.#loaded.set(name, loaded);
    }
    return loaded;
  }

  /** 入力の形式を判定し、その形式を持つモジュールを読み込む。どの形式でもなければ undefined */
  forInput(bytes) {
    const detection = detect(bytes);
    return detection === undefined ? Promise.resolve(undefined) : this.get(detection.module);
  }

  /** op を持つモジュールのうち、読み込めるものの最初を読み込む */
  forOp(op) {
    const name = modulesWith(op).find((candidate) => typeof this.#loaders[candidate] === "function");
    return name === undefined ? Promise.reject(new Error(`op ${op} を持つモジュールがない`)) : this.get(name);
  }
}
