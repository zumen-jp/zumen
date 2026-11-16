// Node.js から、常駐するネイティブの CLI を非同期で呼ぶ（#358・決定 0201）。
// 標準入出力の JSON-RPC を使い、コアの応答のバイト列をそのまま返す。

import { spawn } from "node:child_process";

import { STATUS, ZumenError } from "./index.js";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });
const codes = new Map(Object.entries(STATUS).map(([code, name]) => [name, Number(code)]));
const DEFAULT_LIMIT = 64 * 1024 * 1024;
const CLI_LIMIT = 1024 * 1024 * 1024;

function toBytes(value) {
  if (value === undefined || value === null) return new Uint8Array();
  if (typeof value === "string") return encoder.encode(value);
  if (ArrayBuffer.isView(value)) return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  if (Object.prototype.toString.call(value) === "[object ArrayBuffer]") return new Uint8Array(value);
  throw new TypeError("バイト列か文字列を渡す");
}

function optionBytes(options) {
  return options === undefined || typeof options === "string" || ArrayBuffer.isView(options)
    || Object.prototype.toString.call(options) === "[object ArrayBuffer]"
    ? toBytes(options) : encoder.encode(JSON.stringify(options));
}

function limitsOf({ timeoutMs = 30_000, maxMessageBytes = DEFAULT_LIMIT } = {}) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 2_147_483_647) {
    throw new RangeError("timeoutMs は 1 から 2147483647 までの整数にする");
  }
  if (!Number.isSafeInteger(maxMessageBytes) || maxMessageBytes < 1024 || maxMessageBytes > CLI_LIMIT) {
    throw new RangeError("maxMessageBytes は 1024 から 1073741824 までの整数にする");
  }
  return { timeoutMs, maxMessageBytes };
}

/** 起動・通信・上限による失敗。コアが返した状態は ZumenError で返す */
export class ProcessError extends Error {
  constructor(code, message, cause) {
    super(message, cause === undefined ? undefined : { cause });
    this.name = "ProcessError";
    this.code = code;
  }
}

/** 1 つの子プロセスとセッション。通常の呼び出しは一度に 1 つ、中断と終了は処理中も受ける */
export class ProcessSession {
  #child;
  #limits;
  #pending = new Map();
  #next = 1;
  #active = null;
  #buffer = Buffer.alloc(0);
  #size = 0;
  #failure = null;
  #panicked = false;
  #closed = false;
  #ended = false;
  #exit;
  #killTimer;
  #cancelTimer;
  #cancelling = null;

  constructor(child, limits) {
    this.#child = child;
    this.#limits = limits;
    this.#exit = new Promise((resolve) => {
      child.once("close", () => {
        this.#ended = true;
        clearTimeout(this.#killTimer);
        if (!this.#closed && !this.#panicked && !this.#failure) {
          this.#fail(new ProcessError("io", "子プロセスが終了した"));
        }
        resolve();
      });
    });
    child.on("error", (cause) => this.#fail(new ProcessError("spawn", "子プロセスを起動できない", cause)));
    child.stdin.on("error", (cause) => this.#fail(new ProcessError("io", "子プロセスに書き込めない", cause)));
    child.stdout.on("error", (cause) => this.#fail(new ProcessError("io", "子プロセスの応答を読めない", cause)));
    child.stdout.on("data", (chunk) => this.#read(chunk));
    child.stdout.on("end", () => {
      if (!this.#closed && !this.#panicked) {
        this.#fail(new ProcessError("io", "子プロセスの応答が途切れた"));
      }
    });
  }

  // テストも同じ起動後の処理を通す。公開の入口は openProcess。
  static async create(child, options, limits) {
    const session = new ProcessSession(child, limits);
    try {
      const bytes = optionBytes(options);
      await session.#invoke("session_new", "zm_session_new", { options_base64: session.#base64(bytes) });
      return session;
    } catch (error) {
      await session.close();
      throw error;
    }
  }

  get closed() { return this.#closed; }
  get broken() { return this.#failure !== null || this.#panicked; }

  #check(operation) {
    if (this.#closed) throw new ZumenError(operation, codes.get("invalid"), null);
    if (this.#panicked) throw new ZumenError(operation, codes.get("panicked"), null);
    if (this.#failure) throw this.#failure;
    if (this.#active !== null || this.#cancelling !== null) throw new ZumenError(operation, codes.get("busy"), null);
  }

  #base64(bytes) {
    if (Math.ceil(bytes.length / 3) * 4 > this.#limits.maxMessageBytes) {
      throw new ProcessError("limit", "要求が maxMessageBytes を超える");
    }
    return Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength).toString("base64");
  }

  #rejectAll(error) {
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(error);
    }
    this.#pending.clear();
    this.#buffer = Buffer.alloc(0);
    this.#size = 0;
    clearTimeout(this.#cancelTimer);
  }

  #terminate(force) {
    if (this.#ended) return;
    this.#child.stdin.destroy();
    this.#child.kill(force ? "SIGKILL" : "SIGTERM");
    if (!force && !this.#killTimer) {
      this.#killTimer = setTimeout(() => this.#child.kill("SIGKILL"), 1000);
    }
  }

  #fail(error) {
    if (this.#closed || this.#failure || this.#panicked) return;
    this.#failure = error;
    this.#rejectAll(error);
    this.#terminate(true);
  }

  #read(chunk) {
    if (this.#closed || this.#failure || this.#panicked) return;
    let start = 0;
    while (start < chunk.length) {
      const newline = chunk.indexOf(10, start);
      const end = newline < 0 ? chunk.length : newline;
      const part = chunk.subarray(start, end);
      if (this.#size + part.length > this.#limits.maxMessageBytes) {
        this.#fail(new ProcessError("limit", "応答が maxMessageBytes を超える"));
        return;
      }
      const needed = this.#size + part.length;
      if (this.#buffer.length < needed) {
        const capacity = Math.min(this.#limits.maxMessageBytes, Math.max(4096, needed, this.#buffer.length * 2));
        const grown = Buffer.allocUnsafe(capacity);
        this.#buffer.copy(grown, 0, 0, this.#size);
        this.#buffer = grown;
      }
      part.copy(this.#buffer, this.#size);
      this.#size += part.length;
      if (newline < 0) return;
      const line = this.#buffer.subarray(0, this.#size);
      this.#size = 0;
      try {
        const message = JSON.parse(decoder.decode(line));
        const pending = this.#pending.get(message?.id);
        if (message?.jsonrpc !== "2.0" || !pending || message.error !== undefined || !message.result
          || typeof message.result !== "object" || Array.isArray(message.result)) {
          throw new Error("応答が送った要求に対応しない");
        }
        this.#pending.delete(message.id);
        clearTimeout(pending.timer);
        pending.resolve(message.result);
      } catch (cause) {
        this.#fail(new ProcessError("protocol", "子プロセスの応答が JSON-RPC の契約と違う", cause));
        return;
      }
      if (this.#buffer.length > 65536) this.#buffer = Buffer.alloc(0);
      start = newline + 1;
    }
  }

  #request(method, params) {
    const id = this.#next++;
    const line = JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n";
    if (Buffer.byteLength(line) > this.#limits.maxMessageBytes) {
      return Promise.reject(new ProcessError("limit", "要求が maxMessageBytes を超える"));
    }
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => this.#fail(new ProcessError("timeout", "子プロセスの応答を待つ時間を超えた")), this.#limits.timeoutMs);
      this.#pending.set(id, { resolve, reject, timer });
      this.#child.stdin.write(line, (cause) => {
        if (cause) this.#fail(new ProcessError("io", "子プロセスに書き込めない", cause));
      });
    });
  }

  async #invoke(method, operation, params) {
    this.#check(operation);
    const active = { method };
    this.#active = active;
    try {
      const result = await this.#request(method, params);
      const code = codes.get(result.status);
      if (code === undefined) {
        const error = new ProcessError("protocol", "子プロセスが未知の状態を返した");
        this.#fail(error);
        throw error;
      }
      let bytes = null;
      try {
        if (Object.hasOwn(result, "response")) {
          if (typeof result.response !== "string" || Object.hasOwn(result, "response_base64")) throw new Error("response の形が違う");
          bytes = encoder.encode(result.response);
        } else if (Object.hasOwn(result, "response_base64")) {
          bytes = decodeBase64(result.response_base64);
        }
        if (code === 0 && method === "call" && bytes === null) throw new Error("response がない");
        if (code === 0 && method === "take") bytes = decodeBase64(result.base64);
      } catch (cause) {
        const error = new ProcessError("protocol", "子プロセスが返したバイト列の形が違う", cause);
        this.#fail(error);
        throw error;
      }
      if (result.status === "panicked") {
        this.#panicked = true;
        this.#rejectAll(new ZumenError(operation, code, null));
        this.#terminate(true);
      }
      if (code !== 0) throw new ZumenError(operation, code, bytes);
      return bytes;
    } finally {
      if (this.#active === active) this.#active = null;
      clearTimeout(this.#cancelTimer);
    }
  }

  async push(name, data) {
    this.#check("zm_push");
    const nameBytes = toBytes(name);
    const dataBytes = toBytes(data);
    if (Math.ceil(nameBytes.length / 3) * 4 + Math.ceil(dataBytes.length / 3) * 4 > this.#limits.maxMessageBytes) {
      throw new ProcessError("limit", "要求が maxMessageBytes を超える");
    }
    await this.#invoke("push", "zm_push", { name_base64: this.#base64(nameBytes), base64: this.#base64(dataBytes) });
  }

  async callBytes(request) {
    this.#check("zm_call");
    return this.#invoke("call", "zm_call", { request_base64: this.#base64(toBytes(request)) });
  }

  async call(request) {
    return JSON.parse(decoder.decode(await this.callBytes(JSON.stringify(request))));
  }

  async take(blob) {
    return this.#invoke("take", "zm_take", { blob: blob >>> 0 });
  }

  async cancel() {
    if (this.#closed || this.broken || this.#active?.method !== "call") return;
    if (this.#cancelling) return this.#cancelling;
    const active = this.#active;
    this.#cancelTimer = setTimeout(() => {
      if (this.#active === active) this.#fail(new ProcessError("timeout", "中断の合図を送っても子プロセスが応答しない"));
    }, 1000);
    this.#cancelling = this.#request("cancel", {}).then(() => undefined).finally(() => { this.#cancelling = null; });
    return this.#cancelling;
  }

  async close() {
    if (!this.#closed) {
      this.#closed = true;
      this.#rejectAll(new ProcessError("closed", "処理中にセッションを閉じた"));
      this.#terminate(false);
    }
    await this.#exit;
  }
}

function decodeBase64(text) {
  if (typeof text !== "string" || text.length % 4 !== 0) {
    throw new Error("base64 の形が違う");
  }
  const bytes = Buffer.from(text, "base64");
  if (bytes.toString("base64") !== text) throw new Error("base64 の末尾のビットが違う");
  return new Uint8Array(bytes);
}

if (typeof Symbol.asyncDispose === "symbol") {
  ProcessSession.prototype[Symbol.asyncDispose] = ProcessSession.prototype.close;
}

/** CLI を serve --stdio で起動する。シェルを介さず、セッションごとにプロセスを分ける */
export async function openProcess(executable, options, transport) {
  const limits = limitsOf(transport);
  if (typeof executable !== "string" || executable.length === 0) throw new TypeError("CLI の実行ファイルの道筋を渡す");
  const child = spawn(executable, ["serve", "--stdio"], { stdio: ["pipe", "pipe", "ignore"], windowsHide: true, shell: false });
  return ProcessSession.create(child, options, limits);
}
