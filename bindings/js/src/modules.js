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

/** ISO 10303-21 のヘッダーの FILE_SCHEMA の最初の名前。読めなければ undefined */
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

/** XML の署名に従って、原本を 1 バイトか UTF-16 のコード単位でたどる。UTF-32 は受け取らない */
function xmlInput(head) {
  const four = [
    [0, 0, 0xfe, 0xff], [0xff, 0xfe, 0, 0], [0, 0, 0xff, 0xfe], [0xfe, 0xff, 0, 0],
    [0, 0, 0, 0x3c], [0x3c, 0, 0, 0], [0, 0, 0x3c, 0], [0, 0x3c, 0, 0],
  ];
  if (four.some((pattern) => matchBytes(head, 0, pattern))) {
    return undefined;
  }
  let width = 1;
  let little = false;
  let start = matchBytes(head, 0, [0xef, 0xbb, 0xbf]) ? 3 : 0;
  if (matchBytes(head, 0, [0xff, 0xfe]) || matchBytes(head, 0, [0xfe, 0xff])) {
    width = 2;
    little = head[0] === 0xff;
    start = 2;
  } else if (matchBytes(head, 0, [0x3c, 0, 0x3f, 0]) || matchBytes(head, 0, [0, 0x3c, 0, 0x3f])) {
    width = 2;
    little = head[0] === 0x3c;
  }
  const unit = (at) => {
    if (at + width > head.length) {
      return undefined;
    }
    return width === 1 ? head[at] : little ? head[at] | (head[at + 1] << 8) : (head[at] << 8) | head[at + 1];
  };
  const starts = (at, pattern) => {
    for (let i = 0; i < pattern.length; i++) {
      if (unit(at + i * width) !== pattern.charCodeAt(i)) {
        return false;
      }
    }
    return true;
  };
  const find = (at, value) => {
    for (; unit(at) !== undefined; at += width) {
      if (unit(at) === value) {
        return at;
      }
    }
    return -1;
  };
  return { head, width, start, unit, starts, find, shiftJis: false };
}

const shiftJisLabels = ["csshiftjis", "ms932", "ms_kanji", "shift-jis", "shift_jis", "sjis", "windows-31j", "x-sjis"];
let shiftJisDecoder;

// SDK の XML の署名と同じ空白。署名は XML の構文全体の検査を担当しない。
function isXmlSpace(unit) {
  return unit === 0x20 || unit === 0x09 || unit === 0x0a || unit === 0x0c || unit === 0x0d;
}

function shiftJisLead(unit) {
  return (unit >= 0x81 && unit <= 0x9f) || (unit >= 0xe0 && unit <= 0xfc);
}

/** WHATWG Shift_JIS 12.3.1 の文字境界。原本の [・] と2バイト目を区別する。 */
function afterXmlCharacter(input, at) {
  if (input.shiftJis && shiftJisLead(input.unit(at))) {
    const trail = input.unit(at + 1);
    return ((trail >= 0x40 && trail <= 0x7e) || (trail >= 0x80 && trail <= 0xfc)) ? at + 2 : -1;
  }
  return at + input.width;
}

function decodeShiftJis(bytes) {
  shiftJisDecoder ??= new TextDecoder("shift_jis", { fatal: true, ignoreBOM: true });
  let result = "";
  let start = 0;
  for (let at = 0; at < bytes.length; at++) {
    if (shiftJisLead(bytes[at])) { at++; }
    else if (bytes[at] === 0x80) {
      // WHATWG 12.3.1 の単独 0x80。ICU を使う Node.js では不正として扱う版もある。
      result += shiftJisDecoder.decode(bytes.subarray(start, at)) + "\u0080";
      start = at + 1;
    }
  }
  return result + shiftJisDecoder.decode(bytes.subarray(start));
}

/** 宣言のラベルを固定の候補と比較する。入力のラベルを文字列へ確保しない。 */
function declarationEncoding(input, start, end) {
  if (input.width !== 1 || !input.starts(start, "<?xml") || !isXmlSpace(input.unit(start + 5))) {
    return;
  }
  let at = start + 5;
  while (at < end) {
    at = skipSpace(input, at);
    const nameEndAt = nameEnd(input, at);
    if (nameEndAt === -1) { return; }
    const attribute = [at, nameEndAt];
    at = skipSpace(input, nameEndAt);
    if (!input.starts(at, "=")) { return; }
    at = skipSpace(input, at + 1);
    const quote = input.unit(at);
    if (quote !== 0x22 && quote !== 0x27) { return; }
    const value = at + 1;
    const valueEnd = input.find(value, quote);
    if (valueEnd === -1 || valueEnd > end) { return; }
    if (xmlEquals(input, attribute, "encoding")) {
      let from = value;
      let to = valueEnd;
      while (from < to && isXmlSpace(input.unit(from))) { from++; }
      while (to > from && isXmlSpace(input.unit(to - 1))) { to--; }
      input.shiftJis = shiftJisLabels.some((label) => equalsIgnoreCase(input.head, from, to, label));
      return;
    }
    at = valueEnd + 1;
  }
}

/** XML の原本の範囲と規則を比べる。UTF-16 のときも文字列のコピーは作らない */
function xmlEquals(input, range, expected) {
  const [start, end] = range;
  if (input.width === 1) {
    if (input.shiftJis) {
      const want = encoder.encode(expected);
      if (want.every((byte) => byte <= 0x7f)) { return sameBytes(input.head.subarray(start, end), want); }
      // Shift_JIS の一文字は最大2バイト。確保を期待名の長さで抑えてから復号する。
      if (end - start > expected.length * 2) { return false; }
      try {
        return decodeShiftJis(input.head.subarray(start, end)) === expected;
      } catch {
        return false;
      }
    }
    return sameBytes(input.head.subarray(start, end), encoder.encode(expected));
  }
  if (end - start !== expected.length * 2) {
    return false;
  }
  for (let i = 0; i < expected.length; i++) {
    const unit = input.unit(start + i * 2);
    if (unit !== expected.charCodeAt(i)) {
      return false;
    }
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = input.unit(start + (i + 1) * 2);
      if (i + 1 >= expected.length || next < 0xdc00 || next > 0xdfff) {
        return false;
      }
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      const previous = input.unit(start + (i - 1) * 2);
      if (i === 0 || previous < 0xd800 || previous > 0xdbff) {
        return false;
      }
    }
  }
  return true;
}

/** pattern の後の位置。なければ -1 */
function afterPattern(input, at, pattern) {
  for (; input.unit(at) !== undefined; at += input.width) {
    if (input.starts(at, pattern)) {
      return at + pattern.length * input.width;
    }
  }
  return -1;
}

/** 文書型宣言の後の位置。なければ -1 */
function afterDoctype(input, at) {
  let depth = 0;
  for (; input.unit(at) !== undefined; at += input.width) {
    const unit = input.unit(at);
    if (input.shiftJis && shiftJisLead(unit)) {
      const after = afterXmlCharacter(input, at);
      if (after === -1) { return -1; }
      at = after - input.width;
      continue;
    }
    if (unit === 0x22 || unit === 0x27) {
      const end = input.find(at + input.width, unit);
      if (end === -1) {
        return -1;
      }
      at = end;
    } else if (unit === 0x5b) {
      depth++;
    } else if (unit === 0x5d) {
      if (depth === 0) {
        return -1;
      }
      depth--;
    } else if (unit === 0x3e && depth === 0) {
      return at + input.width;
    }
  }
  return -1;
}

function isName(unit) {
  return !isXmlSpace(unit) && ![0x3c, 0x3e, 0x2f, 0x3d, 0x21, 0x3f, 0x22, 0x27, 0x26, 0x3b, 0x5b, 0x5d].includes(unit);
}

function skipSpace(input, at) {
  while (input.unit(at) !== undefined && isXmlSpace(input.unit(at))) {
    at += input.width;
  }
  return at;
}

function nameEnd(input, at) {
  while (input.unit(at) !== undefined && isName(input.unit(at))) {
    at = afterXmlCharacter(input, at);
    if (at === -1) { return -1; }
  }
  return input.unit(at) === undefined ? -1 : at;
}

/** 最初の開始タグの名前と名前空間の原本の範囲。宣言の正否と後の本文はコーデックで検査する */
function xmlRoot(head) {
  const input = xmlInput(head);
  if (input === undefined) {
    return undefined;
  }
  let at = input.start;
  for (;;) {
    at = skipSpace(input, at);
    if (input.starts(at, "<?")) {
      const start = at;
      at = afterPattern(input, at + 2 * input.width, "?>");
      if (start === 0 && at !== -1) { declarationEncoding(input, start, at - 2 * input.width); }
    } else if (input.starts(at, "<!--")) {
      at = afterPattern(input, at + 4 * input.width, "-->");
    } else if (input.starts(at, "<!DOCTYPE")) {
      at = afterDoctype(input, at + 9 * input.width);
    } else if (input.starts(at, "<")) {
      return startTag(input, at + input.width);
    } else {
      return undefined;
    }
    if (at === -1) {
      return undefined;
    }
  }
}

function startTag(input, at) {
  const end = nameEnd(input, at);
  if (end === -1 || end === at) {
    return undefined;
  }
  let colon = -1;
  for (let cursor = at; cursor < end; cursor += input.width) {
    if (input.unit(cursor) === 0x3a) {
      colon = cursor;
      break;
    }
  }
  const prefix = colon === -1 ? undefined : [at, colon];
  const local = [colon === -1 ? at : colon + input.width, end];
  if (local[0] === local[1]) {
    return undefined;
  }
  let namespace;
  at = end;
  for (;;) {
    at = skipSpace(input, at);
    if (input.starts(at, ">") || input.starts(at, "/>")) {
      return { input, local, namespace };
    }
    const attributeEnd = nameEnd(input, at);
    if (attributeEnd === -1 || attributeEnd === at) {
      return undefined;
    }
    const attribute = [at, attributeEnd];
    at = skipSpace(input, attributeEnd);
    if (!input.starts(at, "=")) {
      return undefined;
    }
    at = skipSpace(input, at + input.width);
    const quote = input.unit(at);
    if (quote !== 0x22 && quote !== 0x27) {
      return undefined;
    }
    const valueStart = at + input.width;
    const valueEnd = input.find(valueStart, quote);
    if (valueEnd === -1) {
      return undefined;
    }
    const declares = prefix === undefined
      ? xmlEquals(input, attribute, "xmlns")
      : input.starts(attribute[0], "xmlns:") &&
        sameBytes(input.head.subarray(attribute[0] + 6 * input.width, attribute[1]), input.head.subarray(...prefix));
    if (declares && valueStart < valueEnd) {
      namespace = [valueStart, valueEnd];
    }
    at = valueEnd + input.width;
  }
}

/** ZIP のアーカイブの中央ディレクトリの、項目の名前のどれかが pred に合うか。zumen-sdk の zip と同じ読み方 */
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
        xmlEquals(root.input, root.local, rule.name) &&
        (rule.namespace === null || (root.namespace !== undefined && xmlEquals(root.input, root.namespace, rule.namespace)))
      );
    }
    case "zip_entry":
      return anyZipName(head, (name) => name[name.length - 1] !== 0x2f && rule.suffixes.some((suffix) => endsWithIgnoreCase(name, suffix)));
    case "zip_name":
      return anyZipName(head, (name) => {
        const base = name.subarray(name.lastIndexOf(0x2f) + 1);
        return (
          base.length > 0 &&
          rule.prefixes.some((prefix) => base.length >= prefix.length && equalsIgnoreCase(base, 0, prefix.length, prefix)) &&
          rule.suffixes.some((suffix) => endsWithIgnoreCase(base, suffix))
        );
      });
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
 * 分けた wasm のモジュールを、要るときに読み込んで使い回す。trap で使えなくなったら、読み込み直す。
 *
 * `loaders` は、モジュールの名前から、読み込む関数（`load` に渡せるものを返す）を引く表。持たないモジュールは使えない。
 * `load` は、ラッパーの `load`。
 *
 * `plugins` は、プラグインを束ねたモジュール（決定 0017・0243）の一覧。1 つは、名前（`name`）、形式ごとの判定の規則
 * （`formats`、DETECTION の項目と同じ `format` と `rules`）、持つ op（`ops`）、読み込む関数（`load`）を持つ。コアの
 * どの形式でもない入力を、一覧の順に、その規則で試す。
 */
export class Modules {
  #loaders;
  #load;
  #plugins;
  #loaded = new Map();

  constructor(loaders, load, plugins = []) {
    this.#loaders = loaders;
    this.#load = load;
    this.#plugins = plugins;
  }

  #plugin(name) {
    return this.#plugins.find((plugin) => plugin.name === name);
  }

  /**
   * 名前のモジュールを読み込む。プラグインのモジュールは、その名前で引く。読み込んだインスタンスは使い回し、trap で
   * 使えなくなったら（Zumen.broken）、捨てて読み込み直す。読み込みに失敗したら覚えず、次に引いたときにやり直す
   * （決定 0259）
   */
  get(name) {
    const cached = this.#loaded.get(name);
    if (cached !== undefined && cached.zumen?.broken !== true) {
      return cached.loading;
    }
    const plugin = MODULES[name] === undefined ? this.#plugin(name) : undefined;
    const loader = plugin === undefined ? this.#loaders[name] : plugin.load;
    if ((MODULES[name] === undefined && plugin === undefined) || typeof loader !== "function") {
      return Promise.reject(new Error(`wasm のモジュール ${name} を読み込めない`));
    }
    const entry = { loading: undefined, zumen: undefined };
    entry.loading = this.#load(loader()).then(
      (zumen) => {
        entry.zumen = zumen;
        return zumen;
      },
      (error) => {
        if (this.#loaded.get(name) === entry) {
          this.#loaded.delete(name);
        }
        throw error;
      },
    );
    this.#loaded.set(name, entry);
    return entry.loading;
  }

  /** 入力の形式を判定する。コアの形式でなければ、プラグインの規則で試す。どの形式でもなければ undefined */
  detect(bytes) {
    const detection = detect(bytes);
    if (detection !== undefined) {
      return detection;
    }
    const head = view(bytes);
    for (const plugin of this.#plugins) {
      const format = plugin.formats.find((candidate) => candidate.rules.some((rule) => matches(head, rule)));
      if (format !== undefined) {
        return { format: format.format, module: plugin.name, rules: format.rules };
      }
    }
    return undefined;
  }

  /** 入力の形式を判定し、その形式を持つモジュールを読み込む。どの形式でもなければ undefined */
  forInput(bytes) {
    const detection = this.detect(bytes);
    return detection === undefined ? Promise.resolve(undefined) : this.get(detection.module);
  }

  /**
   * 入力を、その形式を持つモジュールの新しいセッションで開く（決定 0250）。`request` は open の要求の残りのフィールド（`doc`・
   * `format` など）で、`format` を書けば、その形式を持つモジュールで開く。`inputs` は、open の前にセッションに足す
   * ほかの入力（`saf` で名指す属性ファイルなど）。
   *
   * 図面の形式のプラグイン（op `lift` を持つもの）の入力は、プラグインのモジュールで開いて lift し、図面モデルの
   * バイナリを図面のモジュールのセッションで開く（決定 0247）。図面のモジュールを読み込めなければ、プラグインの
   * モジュールのセッションで開く。返すセッションは、呼び出し側が閉じる。どのモジュールの形式でもなければ undefined
   */
  async open(name, bytes, request = {}, inputs = {}) {
    const data = view(bytes);
    const target = this.#target(data, request.format);
    if (target === undefined) {
      return undefined;
    }
    const plugin = this.#plugin(target.module);
    if (plugin !== undefined && plugin.ops.includes("lift") && typeof this.#loaders.drawing === "function") {
      const source = (await this.get(plugin.name)).open();
      let lifted;
      try {
        source.push(name, data);
        source.call({ v: 1, op: "open", input: name });
        const { result } = source.call({ v: 1, op: "lift", doc: name });
        lifted = source.take(result.blob);
      } finally {
        source.close();
      }
      return this.#openIn("drawing", name, lifted, request, inputs);
    }
    return this.#openIn(target.module, name, data, request, inputs);
  }

  /** 名指した形式か、判定した形式の、形式とモジュール */
  #target(data, format) {
    if (format === undefined) {
      return this.detect(data);
    }
    const module = Object.keys(MODULES).find((name) => MODULES[name].formats.includes(format));
    return module === undefined ? undefined : { format, module };
  }

  /** モジュールの新しいセッションで開く。失敗したらセッションを閉じて、例外を投げ直す */
  async #openIn(module, name, data, request, inputs) {
    const zumen = await this.get(module);
    const session = zumen.open();
    try {
      for (const [input, value] of Object.entries(inputs)) {
        session.push(input, value);
      }
      session.push(name, data);
      const response = session.call({ ...request, v: 1, op: "open", input: name });
      return { zumen, session, response };
    } catch (error) {
      session.close();
      throw error;
    }
  }

  /** op を持つモジュールのうち、読み込めるものの最初を読み込む。コアのモジュールの後に、プラグインのモジュールを試す */
  forOp(op) {
    const name =
      modulesWith(op).find((candidate) => typeof this.#loaders[candidate] === "function") ??
      this.#plugins.find((plugin) => plugin.ops.includes(op))?.name;
    return name === undefined ? Promise.reject(new Error(`op ${op} を持つモジュールがない`)) : this.get(name);
  }
}
