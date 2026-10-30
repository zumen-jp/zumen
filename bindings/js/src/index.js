// zumen の wasm のモジュールを呼ぶ薄い包み（決定 0011・0194）。
//
// ブラウザーと Node.js の両方で、標準の WebAssembly の API だけを使う。wasm-bindgen は使わない。
// 境界の関数（schema/boundary.toml）を、セッションのメソッドにして出す。要求と応答の型は messages.d.ts にある。

import { ABI_VERSION, STATUS, WASM_EXPORTS } from "./constants.js";

export * from "./constants.js";
export { Scene, readScene } from "./scene.js";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** 境界の関数が ok でない状態を返した */
export class ZumenError extends Error {
  /**
   * @param {string} operation 呼んだ境界の関数
   * @param {number} code 状態コード
   * @param {Uint8Array | null} bytes 応答の JSON。書かれなかったときは null
   */
  constructor(operation, code, bytes) {
    const status = STATUS[code] ?? `unknown(${code})`;
    super(`${operation} が ${status} を返した`);
    this.name = "ZumenError";
    this.operation = operation;
    this.code = code;
    this.status = status;
    this.bytes = bytes;
  }

  /** 応答。書かれなかったときは undefined */
  get response() {
    return this.bytes === null ? undefined : JSON.parse(decoder.decode(this.bytes));
  }
}

function toBytes(value) {
  if (value === undefined || value === null) {
    return new Uint8Array();
  }
  if (typeof value === "string") {
    return encoder.encode(value);
  }
  if (value instanceof Uint8Array) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  throw new TypeError("バイト列か文字列を渡す");
}

/** 境界の関数と、モジュールのメモリの読み書き */
class Core {
  constructor(instance) {
    const exports = instance.exports;
    const missing = WASM_EXPORTS.filter((name) => typeof exports[name] !== "function");
    if (missing.length > 0 || !(exports.memory instanceof WebAssembly.Memory)) {
      throw new Error(`zumen の wasm のモジュールでない（ない関数: ${missing.join(", ") || "memory"}）`);
    }
    const version = exports.zm_abi_version();
    if (version !== ABI_VERSION) {
      throw new Error(`ABI の大版が ${version}（この包みは ${ABI_VERSION}）`);
    }
    this.e = exports;
  }

  // メモリは確保で育つと作り直されるので、毎回取り直す
  bytes() {
    return new Uint8Array(this.e.memory.buffer);
  }

  view() {
    return new DataView(this.e.memory.buffer);
  }

  /** バイト列をモジュールのメモリに写して fn に渡し、終わったら返す */
  withBytes(data, fn) {
    const ptr = data.length === 0 ? 0 : this.e.zm_alloc(data.length) >>> 0;
    if (data.length > 0) {
      this.bytes().set(data, ptr);
    }
    try {
      return fn(ptr, data.length);
    } finally {
      if (data.length > 0) {
        this.e.zm_dealloc(ptr, data.length);
      }
    }
  }

  /** zm_buf（wasm32 では ptr と len の 2 つの u32）を確保して fn に渡し、書かれたバイト列を写して返す */
  withBuf(fn) {
    const buf = this.e.zm_alloc(8) >>> 0;
    this.view().setUint32(buf, 0, true);
    this.view().setUint32(buf + 4, 0, true);
    try {
      const code = fn(buf);
      const ptr = this.view().getUint32(buf, true);
      const len = this.view().getUint32(buf + 4, true);
      const data = ptr === 0 ? (code === 0 ? new Uint8Array() : null) : this.bytes().slice(ptr, ptr + len);
      this.e.zm_free(buf);
      return [code, data];
    } finally {
      this.e.zm_dealloc(buf, 8);
    }
  }

  /** u32 を 1 つ書かせる所を確保して fn に渡し、書かれた値を返す */
  withOut(fn) {
    const out = this.e.zm_alloc(4) >>> 0;
    this.view().setUint32(out, 0, true);
    try {
      const code = fn(out);
      return [code, this.view().getUint32(out, true)];
    } finally {
      this.e.zm_dealloc(out, 4);
    }
  }
}

/** セッション。入力と、直前の呼び出しのバイナリの結果を持つ。使い終わったら close で閉じる */
export class Session {
  #core;
  #handle;

  constructor(core, handle) {
    this.#core = core;
    this.#handle = handle;
  }

  /** 閉じたか */
  get closed() {
    return this.#handle === 0;
  }

  /** 名前のついた入力に、バイト列を足す。同じ名前で何回か呼ぶと、後ろに足していく */
  push(name, data) {
    const core = this.#core;
    const code = core.withBytes(toBytes(name), (namePtr, nameLen) =>
      core.withBytes(toBytes(data), (dataPtr, dataLen) =>
        core.e.zm_push(this.#handle, namePtr, nameLen, dataPtr, dataLen),
      ),
    );
    if (code !== 0) {
      throw new ZumenError("zm_push", code, null);
    }
  }

  /** 要求の JSON（文字列かバイト列）を渡し、応答の JSON のバイト列を受け取る */
  callBytes(request) {
    const core = this.#core;
    const [code, bytes] = core.withBytes(toBytes(request), (ptr, len) =>
      core.withBuf((buf) => core.e.zm_call(this.#handle, ptr, len, buf)),
    );
    if (code !== 0) {
      throw new ZumenError("zm_call", code, bytes);
    }
    return bytes;
  }

  /** 要求を渡し、応答を受け取る。失敗した応答は、ZumenError の response にある */
  call(request) {
    return JSON.parse(decoder.decode(this.callBytes(JSON.stringify(request))));
  }

  /** 直前の call が作ったバイナリの結果を、応答の blobs にある番号で取り出す */
  take(blob) {
    const core = this.#core;
    const [code, bytes] = core.withBuf((buf) => core.e.zm_take(this.#handle, blob >>> 0, buf));
    if (code !== 0) {
      throw new ZumenError("zm_take", code, null);
    }
    return bytes;
  }

  /** 動いている呼び出しを止める。動いている呼び出しがなければ、何もしない */
  cancel() {
    this.#core.e.zm_cancel(this.#handle);
  }

  /** セッションを閉じる。閉じたあとの呼び出しは、invalid の ZumenError になる */
  close() {
    this.#core.e.zm_session_close(this.#handle);
    this.#handle = 0;
  }
}

// using で閉じられるようにする。Symbol.dispose のない環境（Safari 15 など）では付けない
if (typeof Symbol.dispose === "symbol") {
  Session.prototype[Symbol.dispose] = Session.prototype.close;
}

/** 読み込んだ wasm のモジュール */
export class Zumen {
  #core;

  constructor(instance) {
    this.#core = new Core(instance);
  }

  /** ABI の大版 */
  abiVersion() {
    return this.#core.e.zm_abi_version();
  }

  /** セッションを作る。設定（session_options）は、値か JSON の文字列かバイト列で渡す。渡さなければ既定の設定 */
  open(options) {
    const core = this.#core;
    const raw = options === undefined || typeof options === "string" || ArrayBuffer.isView(options) || options instanceof ArrayBuffer;
    const data = raw ? toBytes(options) : encoder.encode(JSON.stringify(options));
    const [code, handle] = core.withBytes(data, (ptr, len) =>
      core.withOut((out) => core.e.zm_session_new(ptr, len, out)),
    );
    if (code !== 0) {
      throw new ZumenError("zm_session_new", code, null);
    }
    return new Session(core, handle);
  }
}

/** モジュールを、コンパイルしたものかバイト列か fetch の応答から、非同期に読み込む */
export async function load(source) {
  let input = await source;
  if (typeof Response !== "undefined" && input instanceof Response) {
    if (!input.ok) {
      throw new Error(`wasm のモジュールを取得できない（${input.status}）`);
    }
    input = await input.arrayBuffer();
  }
  const module = input instanceof WebAssembly.Module ? input : await WebAssembly.compile(toBytes(input));
  return new Zumen(await WebAssembly.instantiate(module, {}));
}

/** モジュールを、コンパイルしたものかバイト列から、同期で読み込む。ブラウザーの主スレッドでは、大きなモジュールを同期でコンパイルできない */
export function loadSync(source) {
  const module = source instanceof WebAssembly.Module ? source : new WebAssembly.Module(toBytes(source));
  return new Zumen(new WebAssembly.Instance(module, {}));
}
