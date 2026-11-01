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

/** `pattern` の後の位置。なければ -1 */
function afterPattern(head, at, pattern) {
  for (let i = at; i + pattern.length <= head.length; i++) {
    if (matchBytes(head, i, pattern)) {
      return i + pattern.length;
    }
  }
  return -1;
}

/** 文書型宣言の後の位置。なければ -1 */
function afterDoctype(head, at) {
  let depth = 0;
  while (at < head.length) {
    const byte = head[at];
    if (byte === 0x22 || byte === 0x27) {
      const end = head.indexOf(byte, at + 1);
      if (end === -1) {
        return -1;
      }
      at = end + 1;
      continue;
    }
    if (byte === 0x5b) {
      depth++;
    } else if (byte === 0x5d) {
      if (depth === 0) {
        return -1;
      }
      depth--;
    } else if (byte === 0x3e && depth === 0) {
      return at + 1;
    }
    at++;
  }
  return -1;
}

function isName(byte) {
  return !isSpace(byte) && ![0x3c, 0x3e, 0x2f, 0x3d, 0x21, 0x3f, 0x22, 0x27, 0x26, 0x3b, 0x5b, 0x5d].includes(byte);
}

function skipSpace(head, at) {
  while (at < head.length && isSpace(head[at])) {
    at++;
  }
  return at;
}

function nameEnd(head, at) {
  while (at < head.length && isName(head[at])) {
    at++;
  }
  return at < head.length ? at : -1;
}

/** XML の最初の要素の、接頭辞を除いた名前と名前空間の名前（バイト列）。読めなければ undefined */
function xmlRoot(head) {
  let at = matchBytes(head, 0, [0xef, 0xbb, 0xbf]) ? 3 : 0;
  for (;;) {
    at = skipSpace(head, at);
    if (matchBytes(head, at, encoder.encode("<?"))) {
      at = afterPattern(head, at + 2, encoder.encode("?>"));
    } else if (matchBytes(head, at, encoder.encode("<!--"))) {
      at = afterPattern(head, at + 4, encoder.encode("-->"));
    } else if (matchBytes(head, at, encoder.encode("<!DOCTYPE"))) {
      at = afterDoctype(head, at + 9);
    } else if (head[at] === 0x3c) {
      return startTag(head, at + 1);
    } else {
      return undefined;
    }
    if (at === -1) {
      return undefined;
    }
  }
}

function startTag(head, at) {
  const end = nameEnd(head, at);
  if (end === -1 || end === at) {
    return undefined;
  }
  const name = head.subarray(at, end);
  const colon = name.indexOf(0x3a);
  const prefix = colon === -1 ? undefined : name.subarray(0, colon);
  const local = colon === -1 ? name : name.subarray(colon + 1);
  if (local.length === 0) {
    return undefined;
  }
  let namespace;
  at = end;
  for (;;) {
    at = skipSpace(head, at);
    if (head[at] === 0x3e || (head[at] === 0x2f && head[at + 1] === 0x3e)) {
      return { local, namespace };
    }
    const attributeEnd = nameEnd(head, at);
    if (attributeEnd === -1 || attributeEnd === at) {
      return undefined;
    }
    const attribute = head.subarray(at, attributeEnd);
    at = skipSpace(head, attributeEnd);
    if (head[at] !== 0x3d) {
      return undefined;
    }
    at = skipSpace(head, at + 1);
    const quote = head[at];
    if (quote !== 0x22 && quote !== 0x27) {
      return undefined;
    }
    const valueEnd = head.indexOf(quote, at + 1);
    if (valueEnd === -1) {
      return undefined;
    }
    const value = head.subarray(at + 1, valueEnd);
    const expected = prefix === undefined ? encoder.encode("xmlns") : new Uint8Array([...encoder.encode("xmlns:"), ...prefix]);
    if (sameBytes(attribute, expected) && value.length > 0) {
      namespace = value;
    }
    at = valueEnd + 1;
  }
}

/** ZIP の書庫の中央ディレクトリの、項目の名前のどれかが pred に合うか。zumen-sdk の zip と同じ読み方 */
function anyZipName(bytes, pred) {
  const u16 = (at) => (at + 2 <= bytes.length ? bytes[at] | (bytes[at + 1] << 8) : undefined);
  const u32 = (at) => (at + 4 <= bytes.length ? u16(at) + u16(at + 2) * 0x10000 : undefined);
  const last = bytes.length - 22;
  if (last < 0) {
    return false;
  }
  let end = -1;
  for (let at = last; at >= Math.max(0, last - 0xffff); at--) {
    if (bytes[at] === 0x50 && bytes[at + 1] === 0x4b && bytes[at + 2] === 0x05 && bytes[at + 3] === 0x06) {
      end = at;
      break;
    }
  }
  if (end < 0) {
    return false;
  }
  const count = u16(end + 10);
  let at = u32(end + 16);
  for (let i = 0; i < count; i++) {
    if (!(at + 4 <= bytes.length && bytes[at] === 0x50 && bytes[at + 1] === 0x4b && bytes[at + 2] === 0x01 && bytes[at + 3] === 0x02)) {
      return false;
    }
    const name = u16(at + 28);
    const extra = u16(at + 30);
    const comment = u16(at + 32);
    const start = at + 46;
    if (comment === undefined || start + name > bytes.length) {
      return false;
    }
    if (pred(bytes.subarray(start, start + name))) {
      return true;
    }
    at = start + name + extra + comment;
  }
  return false;
}

/** name が、ASCII の大文字と小文字を区別せずに suffix で終わるか */
function endsWithIgnoreCase(name, suffix) {
  return name.length >= suffix.length && equalsIgnoreCase(name, name.length - suffix.length, name.length, suffix);
}

function sameBytes(a, b) {
  return a.length === b.length && a.every((byte, i) => byte === b[i]);
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
    case "xml_root": {
      const root = xmlRoot(head);
      return (
        root !== undefined &&
        sameBytes(root.local, encoder.encode(rule.name)) &&
        (rule.namespace === null || (root.namespace !== undefined && sameBytes(root.namespace, encoder.encode(rule.namespace))))
      );
    }
    case "zip_entry":
      return anyZipName(head, (name) => name[name.length - 1] !== 0x2f && rule.suffixes.some((suffix) => endsWithIgnoreCase(name, suffix)));
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
