// 常駐文書（DXF・DWG・JWW・SXF）を保持する CLI の、Node.js / Electron main 向け接続。
import { spawn } from "node:child_process";

import { STATUS, ZumenError } from "./index.js";
import { ProcessError } from "./process.js";
import { HEADER, PREFIX, MAX_PACKET, NONE, control, frame, invalid, response, responseLength, u64 } from "./live-wire.js";

const BUSY = Number(Object.keys(STATUS).find((code) => STATUS[code] === "busy"));
const MAX_INPUT = 256 * 1024 * 1024;
const MAX_SAF = 64 * 1024 * 1024;
const OPEN_HAS_SAF = 0x80000000;

function limitsOf({ timeoutMs = 30_000, maxMessageBytes = MAX_INPUT + PREFIX } = {}) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 2_147_483_647) throw new RangeError("timeoutMs の範囲が違う");
  if (!Number.isSafeInteger(maxMessageBytes) || maxMessageBytes < 1024 || maxMessageBytes > MAX_PACKET + PREFIX) throw new RangeError("maxMessageBytes の範囲が違う");
  return { timeoutMs, maxMessageBytes };
}

function bytesOf(value) {
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  if (Object.prototype.toString.call(value) === "[object ArrayBuffer]") return new Uint8Array(value);
  throw new TypeError("図面のバイト列を渡す");
}

function rowOf(row) {
  if (!Number.isInteger(row) || row < 0 || row > 0xffffffff) throw new RangeError("row は u32 の整数にする");
  return row;
}

function flagsOf(options, defaultDisplay) {
  const display = options.display ?? defaultDisplay;
  const full = options.full ?? false;
  if (typeof display !== "boolean" || typeof full !== "boolean" || (full && !display)) throw new TypeError("full は display とともに指定する");
  return display ? full ? 3 : 1 : 0;
}

const COLOR_SYSTEMS = { aci: 1, sxf: 2, jww: 3 };

function paddedUtf8(value, label, maxBytes) {
  if (typeof value !== "string" || !value.isWellFormed()) throw new TypeError(`${label} は対になったサロゲートだけの文字列にする`);
  const buf = Buffer.from(value, "utf8");
  if (buf.length > maxBytes) throw new RangeError(`${label} は UTF-8 で ${maxBytes} バイトまでにする`);
  return { buf, padded: Math.ceil(buf.length / 4) * 4 };
}

function ownColorWords(color) {
  if (color === null) return { colorFlags: 0, code: 0, rgb: 0 };
  if (typeof color !== "object") throw new TypeError("color は null か { code, rgb } にする");
  let colorFlags = 0, code = 0, rgb = 0;
  if (color.code !== undefined) {
    const system = COLOR_SYSTEMS[color.code?.system];
    const value = color.code?.value;
    if (!system || !Number.isInteger(value) || value < 0 || value > 0xffffff) throw new TypeError("color.code は aci・sxf・jww と 24 bit の値にする");
    colorFlags |= 1; code = ((system << 24) | value) >>> 0;
  }
  if (color.rgb !== undefined) {
    const values = Array.from(color.rgb ?? []);
    if (values.length !== 3 || values.some((v) => !Number.isInteger(v) || v < 0 || v > 255)) throw new TypeError("color.rgb は 0〜255 の 3 つの整数にする");
    colorFlags |= 2; rgb = (values[0] << 16) | (values[1] << 8) | values[2];
  }
  if (colorFlags === 0) throw new TypeError("color は code か rgb を持つ");
  return { colorFlags, code, rgb };
}

/** restyle の本文の行の前（線種を変えなければ 48 バイト、変えれば 56 バイト）を作る。形が合わなければ例外にする */
function styleWords({ layer, color, lineWeight, lineTypeScale, lineType }) {
  let mask = 0, layerValue = 0, colorKind = 0, colorFlags = 0, code = 0, rgb = 0, weightKind = 0, width = 0, scale = 0;
  if (layer !== undefined) { mask |= 1; layerValue = rowOf(layer); }
  if (color !== undefined) {
    mask |= 2;
    if (color === "layer" || color === "block") colorKind = color === "layer" ? 0 : 1;
    else if (color !== null && typeof color === "object") {
      colorKind = 2;
      if (color.code !== undefined) {
        const system = COLOR_SYSTEMS[color.code?.system];
        const value = color.code?.value;
        if (!system || !Number.isInteger(value) || value < 0 || value > 0xffffff) throw new TypeError("color.code は aci・sxf・jww と 24 bit の値にする");
        colorFlags |= 1; code = ((system << 24) | value) >>> 0;
      }
      if (color.rgb !== undefined) {
        const values = Array.from(color.rgb ?? []);
        if (values.length !== 3 || values.some((v) => !Number.isInteger(v) || v < 0 || v > 255)) throw new TypeError("color.rgb は 0〜255 の 3 つの整数にする");
        colorFlags |= 2; rgb = (values[0] << 16) | (values[1] << 8) | values[2];
      }
      if (colorFlags === 0) throw new TypeError("color は code か rgb を持つ");
    } else throw new TypeError("color は \"layer\"・\"block\" か { code, rgb } にする");
  }
  if (lineWeight !== undefined) {
    mask |= 4;
    if (lineWeight === "layer") weightKind = 0;
    else if (lineWeight === "block") weightKind = 1;
    else if (lineWeight === "default") weightKind = 3;
    else if (typeof lineWeight === "number" && Number.isFinite(lineWeight) && lineWeight >= 0) { weightKind = 2; width = lineWeight; }
    else throw new TypeError("lineWeight は \"layer\"・\"block\"・\"default\" か 0 以上の mm にする");
  }
  if (lineTypeScale !== undefined) {
    if (typeof lineTypeScale !== "number" || !Number.isFinite(lineTypeScale) || lineTypeScale <= 0) throw new TypeError("lineTypeScale は正の数にする");
    mask |= 8; scale = lineTypeScale;
  }
  let lineTypeKind = 0, lineTypeValue = 0;
  if (lineType !== undefined) {
    mask |= 16;
    if (lineType === "layer") lineTypeKind = 0;
    else if (lineType === "block") lineTypeKind = 1;
    else { lineTypeKind = 2; lineTypeValue = rowOf(lineType); }
  }
  if (mask === 0) throw new TypeError("layer・color・lineWeight・lineTypeScale・lineType のどれかを変える");
  const size = lineType === undefined ? 48 : 56;
  return {
    size,
    write(out) {
      for (const [i, value] of [mask, layerValue, colorKind, colorFlags, code, rgb, weightKind].entries()) out.writeUInt32LE(value, 4 + i * 4);
      out.writeDoubleLE(width, 32);
      out.writeDoubleLE(scale, 40);
      if (size === 56) { out.writeUInt32LE(lineTypeKind, 48); out.writeUInt32LE(lineTypeValue, 52); }
    },
  };
}

/** 1 子プロセスの 1 文書。コアの失敗は状態として返し、通信の失敗だけを例外にする */
export class LiveProcess {
  #child;
  #limits;
  #pending = null;
  #next = 1n;
  #revision = null;
  #lastDisplay = null;
  #ack = null;
  #header = Buffer.alloc(HEADER);
  #body = null;
  #size = 0;
  #failure = null;
  #closed = false;
  #ended = false;
  #exit;
  #killTimer;
  #cancelTimer;

  constructor(child, limits) {
    this.#child = child;
    this.#limits = limits;
    this.#exit = new Promise((resolve) => {
      child.once("close", () => {
        this.#ended = true;
        clearTimeout(this.#killTimer);
        if (!this.#closed && !this.#failure) this.#fail(new ProcessError("io", "文書の子プロセスが終了した"));
        resolve();
      });
    });
    child.on("error", (cause) => this.#fail(new ProcessError("spawn", "文書の子プロセスを起動できない", cause)));
    child.stdin.on("error", (cause) => this.#fail(new ProcessError("io", "文書の子プロセスに書き込めない", cause)));
    child.stdout.on("error", (cause) => this.#fail(new ProcessError("io", "文書の応答を読めない", cause)));
    child.stdout.on("data", (chunk) => this.#read(chunk));
    child.stdout.on("end", () => { if (!this.#closed) this.#fail(new ProcessError("io", "文書の応答が途切れた")); });
  }

  // テストも本番と同じ通信処理を通す。公開の起動関数は openLiveProcess。
  static async create(child, transport) {
    const session = new LiveProcess(child, limitsOf(transport));
    try {
      await new Promise((resolve, reject) => {
        const timer = setTimeout(() => reject(new ProcessError("timeout", "文書の起動を待つ時間を超えた")), session.#limits.timeoutMs);
        child.once("spawn", () => { clearTimeout(timer); resolve(); });
        child.once("error", (cause) => { clearTimeout(timer); reject(new ProcessError("spawn", "文書の子プロセスを起動できない", cause)); });
      });
      return session;
    } catch (error) {
      await session.close();
      throw error;
    }
  }

  get closed() { return this.#closed; }
  get broken() { return this.#failure !== null; }
  get revision() { return this.#revision; }

  #check() {
    if (this.#closed) throw new ProcessError("closed", "文書の接続は閉じている");
    if (this.#failure) throw this.#failure;
    if (this.#pending) throw new ZumenError("live", BUSY, null);
  }

  #settle(error, reply) {
    const pending = this.#pending;
    this.#pending = null;
    this.#body = null;
    this.#size = 0;
    clearTimeout(this.#cancelTimer);
    this.#cancelTimer = undefined;
    if (pending) {
      clearTimeout(pending.timer);
      if (error) pending.reject(error); else pending.resolve(reply);
    }
  }

  #fail(error) {
    if (this.#failure || this.#closed) return;
    this.#failure = error;
    this.#settle(error);
    this.#child.stdin.destroy();
    if (!this.#ended) this.#child.kill("SIGKILL");
  }

  #read(chunk) {
    if (this.#closed || this.#failure) return;
    try {
      let start = 0;
      while (start < chunk.length) {
        const pending = this.#pending;
        if (!pending) throw invalid();
        const target = this.#body ?? this.#header;
        const count = Math.min(target.length - this.#size, chunk.length - start);
        chunk.copy(target, this.#size, start, start + count);
        this.#size += count;
        start += count;
        if (this.#size < target.length) continue;
        if (this.#body === null) {
          const length = responseLength(this.#header, pending, this.#limits.maxMessageBytes);
          this.#body = Buffer.allocUnsafeSlow(length);
          this.#size = 0;
        } else {
          const reply = response(this.#body, pending);
          this.#revision = reply.revision;
          if (reply.display) this.#lastDisplay = reply.revision;
          this.#settle(null, reply);
        }
      }
    } catch (cause) {
      this.#fail(cause instanceof ProcessError ? cause : new ProcessError("protocol", "文書 IPC の応答を読めない", cause));
    }
  }

  #request(command, flags, revision, options, extra, fill) {
    this.#check();
    const length = 72 + extra;
    if (length > this.#limits.maxMessageBytes) throw new ProcessError("limit", "要求が maxMessageBytes を超える");
    if (this.#next >= NONE) throw new ProcessError("limit", "要求番号の上限に達した");
    const id = this.#next;
    const bytes = frame(command, id, flags, length);
    control(bytes, options.budget === undefined ? {} : options.budget, revision, command === 1 || command === 8 ? null : this.#ack);
    if (fill) fill(bytes.subarray(HEADER + 72));
    this.#next++;
    let pending;
    const done = new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.#fail(new ProcessError("timeout", "文書の応答を待つ時間を超えた")), this.#limits.timeoutMs);
      pending = { id, command, flags, resolve, reject, timer };
      this.#pending = pending;
      this.#child.stdin.write(bytes, (cause) => { if (cause) this.#fail(new ProcessError("io", "文書の要求を送れない", cause)); });
    });
    pending.done = done;
    return done;
  }

  async open(input, options = {}) {
    this.#check();
    const bytes = bytesOf(input);
    const saf = options.saf === undefined ? null : bytesOf(options.saf);
    if (saf !== null && saf.length > MAX_SAF) throw new ProcessError("limit", "属性ファイル（SAF）は 64 MiB までにする");
    if (bytes.length + (saf?.length ?? 0) > MAX_INPUT) throw new ProcessError("limit", "図面の入力は 256 MiB までにする");
    const threads = options.threads ?? 1;
    if (!Number.isInteger(threads) || threads < 1 || threads > 64) throw new RangeError("threads は 1 から 64 までにする");
    const extra = 8 + bytes.length + (saf?.length ?? 0);
    return this.#request(1, flagsOf(options, true), null, options, extra, (out) => {
      out.writeUInt32LE(threads, 0);
      out.writeUInt32LE(saf === null ? 0 : ((OPEN_HAS_SAF | saf.length) >>> 0), 4);
      out.set(bytes, 8);
      if (saf !== null) out.set(saf, 8 + bytes.length);
    });
  }

  /** 表示の適用後に呼ぶ。次の要求で同じ版の受領確認を送る */
  acknowledge(revision) {
    const value = u64(revision, "revision");
    if (value !== this.#lastDisplay) throw new RangeError("最後に受け取った表示の版を指定する");
    this.#ack = value;
  }

  async display(options = {}) { return this.#request(2, flagsOf({ ...options, display: true }, true), this.#revision, options, 0); }
  async save(options = {}) { return this.#request(6, 0, this.#revision, options, 0); }
  async undo(revision, options = {}) { return this.#request(4, flagsOf(options, true), u64(revision, "revision"), options, 0); }
  async redo(revision, options = {}) { return this.#request(5, flagsOf(options, true), u64(revision, "revision"), options, 0); }

  async target(row, revision, options = {}) {
    rowOf(row);
    return this.#request(7, 0, u64(revision, "revision"), options, 4, (out) => out.writeUInt32LE(row));
  }

  /** 表示に出る生きた行を、先に描くものから並べて、指定した版で取得する */
  async order(revision, options = {}) { return this.#request(12, 0, u64(revision, "revision"), options, 0); }

  /** 表示のスタイルの画層の値と、その画層の名前・状態・色を、指定した版で取得する */
  async layers(revision, options = {}) { return this.#request(11, 0, u64(revision, "revision"), options, 0); }

  /** restyle の線種に使える線種の値と、その名前・定義を、指定した版で取得する */
  async lineTypes(revision, options = {}) { return this.#request(16, 0, u64(revision, "revision"), options, 0); }

  /** モデル空間の囲む箱と交差する行を、指定した版で取得する */
  async select(rect, revision, options = {}) {
    this.#check();
    if ((!Array.isArray(rect) && !(rect instanceof Float64Array)) || rect.length !== 4
      || Array.from(rect).some((value) => typeof value !== "number")) throw new TypeError("rect は 4 つの数値にする");
    return this.#request(10, 0, u64(revision, "revision"), options, 32, (out) => {
      for (let i = 0; i < 4; i++) out.writeDoubleLE(rect[i], i * 8);
    });
  }

  /** 表示の座標の点から許容距離 tolerance 以内にある生きた最上位の行を、指定した版で取得する */
  async selectPoint(point, tolerance, revision, options = {}) {
    this.#check();
    if ((!Array.isArray(point) && !(point instanceof Float64Array)) || point.length !== 2
      || Array.from(point).some((value) => typeof value !== "number" || !Number.isFinite(value))) {
      throw new TypeError("point は 2 つの有限の数値にする");
    }
    if (typeof tolerance !== "number" || !Number.isFinite(tolerance) || tolerance < 0) {
      throw new RangeError("tolerance は 0 以上の有限の数値にする");
    }
    return this.#request(21, 0, u64(revision, "revision"), options, 24, (out) => {
      out.writeDoubleLE(point[0], 0);
      out.writeDoubleLE(point[1], 8);
      out.writeDoubleLE(tolerance, 16);
    });
  }

  async transform({ revision, rows, matrix }, options = {}) {
    this.#check();
    if ((!Array.isArray(rows) && !(rows instanceof Uint32Array)) || rows.length > 65536) throw new RangeError("rows は 65536 個までの行番号にする");
    if ((!Array.isArray(matrix) && !(matrix instanceof Float64Array)) || matrix.length !== 6
      || Array.from(matrix).some((value) => typeof value !== "number")) throw new TypeError("matrix は 6 つの数値にする");
    return this.#request(3, flagsOf(options, true), u64(revision, "revision"), options, 56 + rows.length * 4, (out) => {
      out.writeUInt32LE(rows.length, 0);
      for (let i = 0; i < 6; i++) out.writeDoubleLE(matrix[i], 8 + i * 8);
      for (let i = 0; i < rows.length; i++) out.writeUInt32LE(rowOf(rows[i]), 56 + i * 4);
    });
  }

  /** 行の文字の種類と内容を、要求の順に取得する。生きていない行があれば missing */
  async texts(rows, revision, options = {}) {
    this.#check();
    if ((!Array.isArray(rows) && !(rows instanceof Uint32Array)) || rows.length > 65536) throw new RangeError("rows は 65536 個までの行番号にする");
    return this.#request(17, 0, u64(revision, "revision"), options, 8 + rows.length * 4, (out) => {
      out.writeUInt32LE(rows.length, 0);
      for (let i = 0; i < rows.length; i++) out.writeUInt32LE(rowOf(rows[i]), 8 + i * 4);
    });
  }

  /** 記号の定義の表示を、definitions の順に返す。空なら定義の数だけを返す */
  async symbols(definitions, revision, options = {}) {
    this.#check();
    if ((!Array.isArray(definitions) && !(definitions instanceof Uint32Array)) || definitions.length > 65536) throw new RangeError("definitions は 65536 個までの定義の番号にする");
    for (const index of definitions) {
      if (!Number.isInteger(index) || index < 0 || index > 0xfffffffe) throw new RangeError("定義の番号は 0 から 4294967294 の整数にする");
    }
    return this.#request(19, 0, u64(revision, "revision"), options, 8 + definitions.length * 4, (out) => {
      out.writeUInt32LE(definitions.length, 0);
      for (let i = 0; i < definitions.length; i++) out.writeUInt32LE(definitions[i], 8 + i * 4);
    });
  }

  /** 文字の行の内容を content に替える。既定で表示の差分を返す */
  async setText({ revision, rows, content }, options = {}) {
    this.#check();
    if ((!Array.isArray(rows) && !(rows instanceof Uint32Array)) || rows.length > 65536) throw new RangeError("rows は 65536 個までの行番号にする");
    if (typeof content !== "string" || !content.isWellFormed()) throw new TypeError("content は対になったサロゲートだけの文字列にする");
    const text = Buffer.from(content, "utf8");
    if (text.length > 1 << 20) throw new RangeError("content は UTF-8 で 1 MiB までにする");
    const padded = Math.ceil(text.length / 4) * 4;
    return this.#request(18, flagsOf(options, true), u64(revision, "revision"), options, 8 + padded + rows.length * 4, (out) => {
      out.writeUInt32LE(rows.length, 0);
      out.writeUInt32LE(text.length, 4);
      text.copy(out, 8);
      for (let i = 0; i < rows.length; i++) out.writeUInt32LE(rowOf(rows[i]), 8 + padded + i * 4);
    });
  }

  /** 行を除く。既定で表示の差分を返す */
  async remove({ revision, rows }, options = {}) {
    this.#check();
    if ((!Array.isArray(rows) && !(rows instanceof Uint32Array)) || rows.length > 65536) throw new RangeError("rows は 65536 個までの行番号にする");
    return this.#request(13, flagsOf(options, true), u64(revision, "revision"), options, 8 + rows.length * 4, (out) => {
      out.writeUInt32LE(rows.length, 0);
      for (let i = 0; i < rows.length; i++) out.writeUInt32LE(rowOf(rows[i]), 8 + i * 4);
    });
  }

  /** 行の画層・色・線幅・線種・線種の倍率を変える。画層は layers の、線種は lineTypes の value で指す。既定で表示の差分を返す */
  async restyle({ revision, rows, ...style }, options = {}) {
    this.#check();
    if ((!Array.isArray(rows) && !(rows instanceof Uint32Array)) || rows.length > 65536) throw new RangeError("rows は 65536 個までの行番号にする");
    const { size, write } = styleWords(style);
    return this.#request(15, flagsOf(options, true), u64(revision, "revision"), options, size + rows.length * 4, (out) => {
      out.writeUInt32LE(rows.length, 0);
      write(out);
      for (let i = 0; i < rows.length; i++) out.writeUInt32LE(rowOf(rows[i]), size + i * 4);
    });
  }

  /** 行を写し、写しを動かす。写しの行を selection に返し、表示は返さない */
  async copy({ revision, rows, matrix }, options = {}) {
    this.#check();
    if (options.display !== undefined || options.full !== undefined) throw new TypeError("copy は表示を返さない。display で取り直す");
    if ((!Array.isArray(rows) && !(rows instanceof Uint32Array)) || rows.length > 65536) throw new RangeError("rows は 65536 個までの行番号にする");
    if ((!Array.isArray(matrix) && !(matrix instanceof Float64Array)) || matrix.length !== 6
      || Array.from(matrix).some((value) => typeof value !== "number")) throw new TypeError("matrix は 6 つの数値にする");
    return this.#request(14, 0, u64(revision, "revision"), options, 56 + rows.length * 4, (out) => {
      out.writeUInt32LE(rows.length, 0);
      for (let i = 0; i < 6; i++) out.writeDoubleLE(matrix[i], 8 + i * 8);
      for (let i = 0; i < rows.length; i++) out.writeUInt32LE(rowOf(rows[i]), 56 + i * 4);
    });
  }

  /** 行 template の画層・スタイルに倣って、表示の座標の線分・円・円弧を 1 つ足す。足した行を selection に返す */
  async draw({ revision, template, shape }, options = {}) {
    this.#check();
    if (options.display !== undefined || options.full !== undefined) throw new TypeError("draw は表示を返さない。display で取り直す");
    rowOf(template);
    const kinds = shape !== null && typeof shape === "object" ? Object.keys(shape) : [];
    const sizes = { line: 4, circle: 3, arc: 5 };
    const kind = kinds.length === 1 ? kinds[0] : null;
    const values = kind !== null && Object.hasOwn(sizes, kind) ? shape[kind] : null;
    if ((!Array.isArray(values) && !(values instanceof Float64Array)) || values.length !== sizes[kind]
      || Array.from(values).some((value) => typeof value !== "number" || !Number.isFinite(value))) {
      throw new TypeError("shape は { line: [x0,y0,x1,y1] }・{ circle: [x,y,r] }・{ arc: [x,y,r,start,end] } のどれか 1 つの有限の数値にする");
    }
    if (kind !== "line" && !(values[2] > 0)) throw new RangeError("半径は正の数にする");
    return this.#request(20, 0, u64(revision, "revision"), options, 48, (out) => {
      out.writeUInt32LE(template, 0);
      out.writeUInt32LE(kind === "line" ? 1 : kind === "circle" ? 2 : 3, 4);
      for (let i = 0; i < values.length; i++) out.writeDoubleLE(values[i], 8 + i * 8);
    });
  }

  /** 変更された記号の定義の番号を selection に返す。clear が true なら取得後に変更フラグを消す */
  async dirtySymbols(revision, options = {}) {
    this.#check();
    const clear = options.clear ?? false;
    if (typeof clear !== "boolean") throw new TypeError("clear は真偽値にする");
    return this.#request(22, 0, u64(revision, "revision"), options, 4, (out) => out.writeUInt32LE(clear ? 1 : 0, 0));
  }

  /** 画層の変更・追加・削除を行う。既定で表示の差分を返す */
  async editLayer(change, options = {}) {
    this.#check();
    const opName = change?.op ?? "update";
    const revision = u64(change?.revision, "revision");
    if (opName === "remove") {
      const layer = rowOf(change.layer);
      return this.#request(23, flagsOf(options, true), revision, options, 8, (out) => {
        out.writeUInt32LE(3, 0);
        out.writeUInt32LE(layer, 4);
      });
    }
    if (opName !== "update" && opName !== "add") throw new TypeError("op は \"update\"・\"add\"・\"remove\" のどれかにする");
    const layer = opName === "update" ? rowOf(change.layer) : 0;
    let mask = 0, stateFlags = 0, colorFlags = 0, code = 0, rgb = 0, weightKind = 0, width = 0, ltKind = 0, ltValue = 0;
    if (change.state !== undefined) {
      if (change.state === null || typeof change.state !== "object") throw new TypeError("state は { visible, printable, editable, frozen } にする");
      const { visible, printable, editable, frozen } = change.state;
      if ([visible, printable, editable, frozen].some((v) => typeof v !== "boolean")) throw new TypeError("state の各項目は真偽値にする");
      mask |= 1;
      stateFlags = (visible ? 1 : 0) | (printable ? 2 : 0) | (editable ? 4 : 0) | (frozen ? 8 : 0);
    }
    if (change.color !== undefined) {
      mask |= 2;
      ({ colorFlags, code, rgb } = ownColorWords(change.color));
    }
    if (change.lineWeight !== undefined) {
      mask |= 4;
      if (change.lineWeight === null) weightKind = 0;
      else if (change.lineWeight === "default") weightKind = 3;
      else if (typeof change.lineWeight === "number" && Number.isFinite(change.lineWeight) && change.lineWeight >= 0) {
        weightKind = 2; width = change.lineWeight;
      } else throw new TypeError("lineWeight は null・\"default\" か 0 以上の mm にする");
    }
    if (change.lineType !== undefined) {
      mask |= 8;
      if (change.lineType === null) ltKind = 0;
      else { ltKind = 2; ltValue = rowOf(change.lineType); }
    }
    let namePart = { buf: Buffer.alloc(0), padded: 0 };
    if (change.name !== undefined) {
      namePart = paddedUtf8(change.name, "name", 4096);
      if (namePart.buf.length === 0) throw new RangeError("name は 1 文字以上にする");
      mask |= 16;
    }
    if (mask === 0 || (opName === "add" && (mask & 16) === 0)) throw new TypeError("変更する画層の項目（add では name 必須）を指定する");
    return this.#request(23, flagsOf(options, true), revision, options, 52 + namePart.padded, (out) => {
      out.writeUInt32LE(opName === "update" ? 1 : 2, 0);
      out.writeUInt32LE(layer, 4);
      out.writeUInt32LE(mask, 8);
      out.writeUInt32LE(stateFlags, 12);
      out.writeUInt32LE(colorFlags, 16);
      out.writeUInt32LE(code, 20);
      out.writeUInt32LE(rgb, 24);
      out.writeUInt32LE(weightKind, 28);
      out.writeDoubleLE(width, 32);
      out.writeUInt32LE(ltKind, 40);
      out.writeUInt32LE(ltValue, 44);
      out.writeUInt32LE(namePart.buf.length, 48);
      namePart.buf.copy(out, 52);
    });
  }

  /** 線種の変更・追加・削除を行う。既定で表示の差分を返す */
  async editLineType(change, options = {}) {
    this.#check();
    const opName = change?.op ?? "update";
    const revision = u64(change?.revision, "revision");
    if (opName === "remove") {
      const lineType = rowOf(change.lineType);
      return this.#request(24, flagsOf(options, true), revision, options, 8, (out) => {
        out.writeUInt32LE(3, 0);
        out.writeUInt32LE(lineType, 4);
      });
    }
    if (opName !== "update" && opName !== "add") throw new TypeError("op は \"update\"・\"add\"・\"remove\" のどれかにする");
    const lineType = opName === "update" ? rowOf(change.lineType) : 0;
    const basis = change.basis ?? "element";
    if (basis !== "element" && basis !== "paper") throw new TypeError("basis は \"element\" か \"paper\" にする");
    const namePart = paddedUtf8(change.name, "name", 4096);
    if (namePart.buf.length === 0) throw new RangeError("name は 1 文字以上にする");
    const descPart = paddedUtf8(change.description ?? "", "description", 4096);
    const pattern = Array.from(change.pattern ?? []);
    if (pattern.length > 256 || pattern.some((v) => typeof v !== "number" || !Number.isFinite(v))) {
      throw new TypeError("pattern は 256 個までの有限の数値にする");
    }
    const extra = 24 + namePart.padded + descPart.padded + pattern.length * 8;
    return this.#request(24, flagsOf(options, true), revision, options, extra, (out) => {
      out.writeUInt32LE(opName === "update" ? 1 : 2, 0);
      out.writeUInt32LE(lineType, 4);
      out.writeUInt32LE(basis === "paper" ? 1 : 0, 8);
      out.writeUInt32LE(namePart.buf.length, 12);
      out.writeUInt32LE(descPart.buf.length, 16);
      out.writeUInt32LE(pattern.length, 20);
      namePart.buf.copy(out, 24);
      descPart.buf.copy(out, 24 + namePart.padded);
      const patBase = 24 + namePart.padded + descPart.padded;
      for (let i = 0; i < pattern.length; i++) out.writeDoubleLE(pattern[i], patBase + i * 8);
    });
  }

  /** 行 template に倣って表示の座標の折れ線・文字・記号挿入を 1 つ足す。足した行を selection に返し、表示は返さない */
  async addElement({ revision, template, element }, options = {}) {
    this.#check();
    if (options.display !== undefined || options.full !== undefined) throw new TypeError("addElement は表示を返さない。display で取り直す");
    rowOf(template);
    const rev = u64(revision, "revision");
    const keys = element !== null && typeof element === "object" ? Object.keys(element) : [];
    const kind = keys.length === 1 ? keys[0] : null;
    if (kind === "polyline") {
      const poly = element.polyline;
      const closed = poly?.closed ?? false;
      const vertices = Array.from(poly?.vertices ?? []);
      if (typeof closed !== "boolean" || vertices.length < 2 || vertices.length > 65536) {
        throw new RangeError("polyline.vertices は 2〜65536 個の頂点にする");
      }
      const flat = vertices.map((v) => {
        const arr = Array.from(v ?? []);
        if ((arr.length !== 2 && arr.length !== 3) || arr.some((n) => typeof n !== "number" || !Number.isFinite(n))) {
          throw new TypeError("各頂点は [x, y] または [x, y, bulge] の有限の数値にする");
        }
        return [arr[0], arr[1], arr[2] ?? 0];
      });
      return this.#request(25, 0, rev, options, 16 + flat.length * 24, (out) => {
        out.writeUInt32LE(template, 0);
        out.writeUInt32LE(1, 4);
        out.writeUInt32LE(closed ? 1 : 0, 8);
        out.writeUInt32LE(flat.length, 12);
        for (let i = 0; i < flat.length; i++) {
          out.writeDoubleLE(flat[i][0], 16 + i * 24);
          out.writeDoubleLE(flat[i][1], 24 + i * 24);
          out.writeDoubleLE(flat[i][2], 32 + i * 24);
        }
      });
    }
    if (kind === "text") {
      const txt = element.text;
      const pos = Array.from(txt?.position ?? []);
      const height = txt?.height;
      const rotation = txt?.rotation ?? 0;
      const widthFactor = txt?.widthFactor ?? 1;
      if (pos.length !== 2 || [pos[0], pos[1], height, rotation, widthFactor].some((n) => typeof n !== "number" || !Number.isFinite(n))) {
        throw new TypeError("text の座標・高さ・角度・幅倍率は有限の数値にする");
      }
      if (!(height > 0) || !(widthFactor > 0)) throw new RangeError("text の height と widthFactor は正の数にする");
      const content = paddedUtf8(txt?.content, "content", 1 << 20);
      return this.#request(25, 0, rev, options, 56 + content.padded, (out) => {
        out.writeUInt32LE(template, 0);
        out.writeUInt32LE(2, 4);
        out.writeUInt32LE(content.buf.length, 8);
        out.writeUInt32LE(0, 12);
        out.writeDoubleLE(pos[0], 16);
        out.writeDoubleLE(pos[1], 24);
        out.writeDoubleLE(height, 32);
        out.writeDoubleLE(rotation, 40);
        out.writeDoubleLE(widthFactor, 48);
        content.buf.copy(out, 56);
      });
    }
    if (kind === "symbol") {
      const sym = element.symbol;
      const index = sym?.definition;
      if (!Number.isInteger(index) || index < 0 || index > 0xfffffffe) throw new RangeError("symbol.definition は 0 から 4294967294 の整数にする");
      const pos = Array.from(sym?.position ?? []);
      const scale = Array.from(sym?.scale ?? [1, 1]);
      const rotation = sym?.rotation ?? 0;
      if (pos.length !== 2 || scale.length !== 2 || [pos[0], pos[1], scale[0], scale[1], rotation].some((n) => typeof n !== "number" || !Number.isFinite(n))) {
        throw new TypeError("symbol の位置・尺度・角度は有限の数値にする");
      }
      if (scale[0] === 0 || scale[1] === 0) throw new RangeError("symbol.scale は 0 以外にする");
      return this.#request(25, 0, rev, options, 56, (out) => {
        out.writeUInt32LE(template, 0);
        out.writeUInt32LE(3, 4);
        out.writeUInt32LE(index, 8);
        out.writeUInt32LE(0, 12);
        out.writeDoubleLE(pos[0], 16);
        out.writeDoubleLE(pos[1], 24);
        out.writeDoubleLE(scale[0], 32);
        out.writeDoubleLE(scale[1], 40);
        out.writeDoubleLE(rotation, 48);
      });
    }
    throw new TypeError("element は { polyline }・{ text }・{ symbol } のどれか 1 つにする");
  }

  /** 記号の定義を 1 つ足し、新しい定義の番号を result に返す。既定で表示の差分を返す */
  async addSymbol({ revision, template, name, base = [0, 0], elements = [] }, options = {}) {
    this.#check();
    rowOf(template);
    const rev = u64(revision, "revision");
    const namePart = paddedUtf8(name, "name", 4096);
    if (namePart.buf.length === 0) throw new RangeError("name は 1 文字以上にする");
    const baseArr = Array.from(base ?? []);
    if (baseArr.length !== 2 || baseArr.some((n) => typeof n !== "number" || !Number.isFinite(n))) {
      throw new TypeError("base は [x, y] の有限の数値にする");
    }
    const items = Array.from(elements ?? []);
    if (items.length > 4096) throw new RangeError("elements は 4096 個までにする");
    const sizes = { line: 4, circle: 3, arc: 5 };
    const encoded = items.map((shape) => {
      const kinds = shape !== null && typeof shape === "object" ? Object.keys(shape) : [];
      const kind = kinds.length === 1 ? kinds[0] : null;
      const values = kind !== null && Object.hasOwn(sizes, kind) ? shape[kind] : null;
      if ((!Array.isArray(values) && !(values instanceof Float64Array)) || values.length !== sizes[kind]
        || Array.from(values).some((v) => typeof v !== "number" || !Number.isFinite(v))) {
        throw new TypeError("elements の各項目は line・circle・arc の有限の数値にする");
      }
      if (kind !== "line" && !(values[2] > 0)) throw new RangeError("半径は正の数にする");
      return { code: kind === "line" ? 1 : kind === "circle" ? 2 : 3, values };
    });
    const extra = 32 + namePart.padded + encoded.length * 48;
    return this.#request(26, flagsOf(options, true), rev, options, extra, (out) => {
      out.writeUInt32LE(template, 0);
      out.writeUInt32LE(namePart.buf.length, 4);
      out.writeUInt32LE(encoded.length, 8);
      out.writeUInt32LE(0, 12);
      out.writeDoubleLE(baseArr[0], 16);
      out.writeDoubleLE(baseArr[1], 24);
      namePart.buf.copy(out, 32);
      const elemBase = 32 + namePart.padded;
      for (let i = 0; i < encoded.length; i++) {
        const off = elemBase + i * 48;
        out.writeUInt32LE(encoded[i].code, off);
        for (let j = 0; j < encoded[i].values.length; j++) out.writeDoubleLE(encoded[i].values[j], off + 8 + j * 8);
      }
    });
  }

  /** 記号の定義 symbol の中の要素 rows（定義内添字）を定義座標の matrix で動かす。既定で表示の差分を返す */
  async transformSymbol({ revision, symbol, rows, matrix }, options = {}) {
    this.#check();
    if (!Number.isInteger(symbol) || symbol < 0 || symbol > 0xfffffffe) throw new RangeError("symbol は 0 から 4294967294 の整数にする");
    if ((!Array.isArray(rows) && !(rows instanceof Uint32Array)) || rows.length > 65536) throw new RangeError("rows は 65536 個までの要素番号にする");
    if ((!Array.isArray(matrix) && !(matrix instanceof Float64Array)) || matrix.length !== 6
      || Array.from(matrix).some((value) => typeof value !== "number")) throw new TypeError("matrix は 6 つの数値にする");
    return this.#request(27, flagsOf(options, true), u64(revision, "revision"), options, 56 + rows.length * 4, (out) => {
      out.writeUInt32LE(symbol, 0);
      out.writeUInt32LE(rows.length, 4);
      for (let i = 0; i < 6; i++) out.writeDoubleLE(matrix[i], 8 + i * 8);
      for (let i = 0; i < rows.length; i++) out.writeUInt32LE(rowOf(rows[i]), 56 + i * 4);
    });
  }

  /** 診断は別の予算で明示的に取得する。data に UTF-8 JSON を返す */
  async diagnostics(requestId, options = {}) {
    return this.#request(8, 0, null, options, 8, (out) => out.writeBigUInt64LE(u64(requestId, "requestId")));
  }

  async cancel() {
    const pending = this.#pending;
    if (this.#closed || this.#failure || !pending) return;
    if (!this.#cancelTimer) {
      this.#cancelTimer = setTimeout(() => {
        if (this.#pending === pending) this.#fail(new ProcessError("timeout", "中断の合図に文書が応答しない"));
      }, 1000);
      this.#child.stdin.write(frame(0, pending.id, 0, 0), (cause) => {
        if (cause) this.#fail(new ProcessError("io", "中断の合図を送れない", cause));
      });
    }
    await pending.done;
  }

  async close() {
    if (!this.#closed) {
      this.#closed = true;
      const busy = this.#pending !== null;
      this.#settle(new ProcessError("closed", "処理中に文書の接続を閉じた"));
      if (!this.#ended) {
        this.#killTimer = setTimeout(() => this.#child.kill("SIGKILL"), 1000);
        if (!busy && !this.#failure && this.#next < NONE) this.#child.stdin.end(frame(9, this.#next++, 0, 0));
        else { this.#child.stdin.destroy(); this.#child.kill("SIGTERM"); }
      }
    }
    await this.#exit;
  }
}

if (typeof Symbol.asyncDispose === "symbol") LiveProcess.prototype[Symbol.asyncDispose] = LiveProcess.prototype.close;

/** 64 bit ネイティブ CLI を起動する。図面は返った接続の open で渡す */
export async function openLiveProcess(executable, transport) {
  const limits = limitsOf(transport);
  if (typeof executable !== "string" || executable.length === 0) throw new TypeError("CLI の実行ファイルの道筋を渡す");
  return LiveProcess.create(spawn(executable, ["live", "--stdio"], { stdio: ["pipe", "pipe", "ignore"], windowsHide: true, shell: false }), limits);
}
