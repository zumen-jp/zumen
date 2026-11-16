// AcadLive を保持する CLI の、Node.js / Electron main 向け接続。
import { spawn } from "node:child_process";

import { STATUS, ZumenError } from "./index.js";
import { ProcessError } from "./process.js";
import { HEADER, PREFIX, MAX_PACKET, NONE, control, frame, invalid, response, responseLength, u64 } from "./live-wire.js";

const BUSY = Number(Object.keys(STATUS).find((code) => STATUS[code] === "busy"));
const MAX_INPUT = 256 * 1024 * 1024;

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
    if (bytes.length > MAX_INPUT) throw new ProcessError("limit", "図面の入力は 256 MiB までにする");
    const threads = options.threads ?? 1;
    if (!Number.isInteger(threads) || threads < 1 || threads > 64) throw new RangeError("threads は 1 から 64 までにする");
    return this.#request(1, flagsOf(options, true), null, options, 8 + bytes.length, (out) => {
      out.writeUInt32LE(threads, 0);
      out.set(bytes, 8);
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

  /** モデル空間の囲む箱と交差する行を、指定した版で取得する */
  async select(rect, revision, options = {}) {
    this.#check();
    if ((!Array.isArray(rect) && !(rect instanceof Float64Array)) || rect.length !== 4
      || Array.from(rect).some((value) => typeof value !== "number")) throw new TypeError("rect は 4 つの数値にする");
    return this.#request(10, 0, u64(revision, "revision"), options, 32, (out) => {
      for (let i = 0; i < 4; i++) out.writeDoubleLE(rect[i], i * 8);
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
