// zumen の wasm のモジュールを呼ぶ薄いラッパー（決定 0011・0194）。
//
// ブラウザーと Node.js の両方で、標準の WebAssembly の API だけを使う。wasm-bindgen は使わない。
// API の関数（schema/boundary.toml）を、セッションのメソッドにして出す。要求と応答の型は messages.d.ts にある。
// wasm の中のパニックとメモリ不足は trap になり、そのインスタンスはもう使えない。ラッパーは、それからの呼び出しを
// panicked にする。ホストは、モジュールを読み込み直す（決定 0010・0259）。

import { ABI_VERSION, STATUS, WASM_EXPORTS } from "./constants.js";

export * from "./constants.js";
export { Scene, readScene } from "./scene.js";
export { detect, modulesWith } from "./modules.js";

import { Modules } from "./modules.js";

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** wasm の中から例外が出たときの状態コード */
const PANICKED = Number(Object.keys(STATUS).find((code) => STATUS[code] === "panicked"));

/** 値の型の名前（`[object …]` の中）。別の realm で作った値でも同じになる */
function tagOf(value) {
  return Object.prototype.toString.call(value).slice(8, -1);
}

/** API の関数が ok でない状態を返したか、wasm の中から例外が出た */
export class ZumenError extends Error {
  /**
   * @param {string} operation 呼んだ API の関数
   * @param {number} code 状態コード
   * @param {Uint8Array | null} bytes 応答の JSON。書かれなかったときは null
   * @param {unknown} [cause] wasm の中から出た例外（trap など）
   */
  constructor(operation, code, bytes, cause) {
    const status = STATUS[code] ?? `unknown(${code})`;
    super(`${operation} が ${status} を返した`, cause === undefined ? undefined : { cause });
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
  if (tagOf(value) === "ArrayBuffer") {
    return new Uint8Array(value);
  }
  throw new TypeError("バイト列か文字列を渡す");
}

/** API の関数と、モジュールのメモリの読み書き */
class Core {
  constructor(instance) {
    const exports = instance.exports;
    // 入力の所有権を渡す関数は最適化用。古い ABI 1 のモジュールでは zm_push を使う（決定 0271）
    const missing = WASM_EXPORTS.filter((name) => name !== "zm_push_owned" && typeof exports[name] !== "function");
    if (missing.length > 0 || tagOf(exports.memory) !== "WebAssembly.Memory") {
      throw new Error(`zumen の wasm のモジュールでない（ない関数: ${missing.join(", ") || "memory"}）`);
    }
    this.e = exports;
    // wasm の中から出た例外。出たら、インスタンスはもう使えない
    this.failure = null;
    const version = this.invoke("zm_abi_version");
    if (version !== ABI_VERSION) {
      throw new Error(`ABI のメジャーバージョンが ${version}（このラッパーは ${ABI_VERSION}）`);
    }
    // プラグインを束ねたモジュール（決定 0243）は、使う前にプラグインを登録する
    if (typeof exports.zm_plugin_init === "function") {
      this.invoke("zm_plugin_init");
    }
  }

  /** wasm の関数を呼ぶ。中から例外が出たら（trap など）、インスタンスを使えないものにして、panicked の ZumenError を投げる */
  invoke(name, ...args) {
    if (this.failure !== null) {
      throw new ZumenError(name, PANICKED, null, this.failure);
    }
    try {
      return this.e[name](...args);
    } catch (error) {
      this.failure = error;
      throw new ZumenError(name, PANICKED, null, error);
    }
  }

  /** 後片付けの関数を呼ぶ。使えないインスタンスでは呼ばず、先に出た例外を隠さない */
  tidy(name, ...args) {
    if (this.failure === null) {
      this.invoke(name, ...args);
    }
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
    const ptr = data.length === 0 ? 0 : this.invoke("zm_alloc", data.length) >>> 0;
    if (data.length > 0) {
      this.bytes().set(data, ptr);
    }
    try {
      return fn(ptr, data.length);
    } finally {
      if (data.length > 0) {
        this.tidy("zm_dealloc", ptr, data.length);
      }
    }
  }

  /** 入力を写して所有権を渡す。呼ぶ前に失敗した場合だけ、自分で領域を返す */
  pushOwned(handle, namePtr, nameLen, data) {
    const length = data.length;
    const ptr = length === 0 ? 0 : this.invoke("zm_alloc", length) >>> 0;
    let handed = false;
    try {
      if (length > 0) {
        this.bytes().set(data, ptr);
      }
      // zm_push_owned は、invalid・busy・panicked を含め、返した状態によらず領域を引き取る
      handed = true;
      return this.invoke("zm_push_owned", handle, namePtr, nameLen, ptr, length);
    } finally {
      if (!handed && length > 0) {
        this.tidy("zm_dealloc", ptr, length);
      }
    }
  }

  /** zm_buf（wasm32 では ptr と len の 2 つの u32）を確保して fn に渡し、書かれたバイト列を写して返す */
  withBuf(fn) {
    const buf = this.invoke("zm_alloc", 8) >>> 0;
    this.view().setUint32(buf, 0, true);
    this.view().setUint32(buf + 4, 0, true);
    try {
      const code = fn(buf);
      const ptr = this.view().getUint32(buf, true);
      const len = this.view().getUint32(buf + 4, true);
      const data = ptr === 0 ? (code === 0 ? new Uint8Array() : null) : this.bytes().slice(ptr, ptr + len);
      this.invoke("zm_free", buf);
      return [code, data];
    } finally {
      this.tidy("zm_dealloc", buf, 8);
    }
  }

  /** u32 を 1 つ書かせる所を確保して fn に渡し、書かれた値を返す */
  withOut(fn) {
    const out = this.invoke("zm_alloc", 4) >>> 0;
    this.view().setUint32(out, 0, true);
    try {
      const code = fn(out);
      return [code, this.view().getUint32(out, true)];
    } finally {
      this.tidy("zm_dealloc", out, 4);
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
    // 入力がこの wasm のメモリのビューでも、名前や入力の確保による memory.grow で無効にならないようにする
    const stable = (value) => {
      const bytes = toBytes(value);
      // Buffer.slice はメモリを共有するので、Uint8Array のコンストラクターで必ず写す。
      return bytes.buffer === core.e.memory.buffer ? new Uint8Array(bytes) : bytes;
    };
    const nameBytes = stable(name);
    const bytes = stable(data);
    const code = core.withBytes(nameBytes, (namePtr, nameLen) => {
      return typeof core.e.zm_push_owned === "function"
        ? core.pushOwned(this.#handle, namePtr, nameLen, bytes)
        : core.withBytes(bytes, (dataPtr, dataLen) =>
            core.invoke("zm_push", this.#handle, namePtr, nameLen, dataPtr, dataLen),
          );
    });
    if (code !== 0) {
      throw new ZumenError("zm_push", code, null);
    }
  }

  /** 要求の JSON（文字列かバイト列）を渡し、応答の JSON のバイト列を受け取る */
  callBytes(request) {
    const core = this.#core;
    const [code, bytes] = core.withBytes(toBytes(request), (ptr, len) =>
      core.withBuf((buf) => core.invoke("zm_call", this.#handle, ptr, len, buf)),
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
    const [code, bytes] = core.withBuf((buf) => core.invoke("zm_take", this.#handle, blob >>> 0, buf));
    if (code !== 0) {
      throw new ZumenError("zm_take", code, null);
    }
    return bytes;
  }

  /** 動いている呼び出しを止める。動いている呼び出しがなければ、何もしない */
  cancel() {
    this.#core.tidy("zm_cancel", this.#handle);
  }

  /**
   * セッションを閉じる。閉じたあとの呼び出しは、invalid の ZumenError になる。使えなくなったインスタンスでは何もせず、
   * finally の中で呼んでも、先に出た例外を隠さない
   */
  close() {
    this.#core.tidy("zm_session_close", this.#handle);
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

  /** wasm の中で trap などが起き、もう使えないか。使えなければ、モジュールを読み込み直す */
  get broken() {
    return this.#core.failure !== null;
  }

  /** ABI のメジャーバージョン */
  abiVersion() {
    return this.#core.invoke("zm_abi_version");
  }

  /** セッションを作る。設定（session_options）は、値か JSON の文字列かバイト列で渡す。渡さなければ既定の設定 */
  open(options) {
    const core = this.#core;
    const raw =
      options === undefined || typeof options === "string" || ArrayBuffer.isView(options) || tagOf(options) === "ArrayBuffer";
    const data = raw ? toBytes(options) : encoder.encode(JSON.stringify(options));
    const [code, handle] = core.withBytes(data, (ptr, len) =>
      core.withOut((out) => core.invoke("zm_session_new", ptr, len, out)),
    );
    if (code !== 0) {
      throw new ZumenError("zm_session_new", code, null);
    }
    return new Session(core, handle);
  }
}

/**
 * モジュールを、コンパイルしたものかバイト列か fetch の応答から、非同期に読み込む。別の realm（Next.js の Edge のランタイム
 * など）で作ったものも受け取る
 */
export async function load(source) {
  let input = await source;
  if (tagOf(input) === "Response") {
    if (!input.ok) {
      throw new Error(`wasm のモジュールを取得できない（${input.status}）`);
    }
    input = await input.arrayBuffer();
  }
  const module = tagOf(input) === "WebAssembly.Module" ? input : await WebAssembly.compile(toBytes(input));
  return new Zumen(await WebAssembly.instantiate(module, {}));
}

/**
 * モジュールを、コンパイルしたものかバイト列から、同期で読み込む。ブラウザーの主スレッドでは、大きなモジュールを同期で
 * コンパイル・インスタンス化できない（Worker の中で使うか、主スレッドでは load を使う）
 */
export function loadSync(source) {
  const module = tagOf(source) === "WebAssembly.Module" ? source : new WebAssembly.Module(toBytes(source));
  return new Zumen(new WebAssembly.Instance(module, {}));
}

/**
 * 分けた wasm のモジュール（決定 0013・0221）を、要るときに読み込む。`loaders` は、モジュールの名前（MODULES）から、
 * `load` に渡せるもの（fetch の応答など）を返す関数を引く表。`plugins` は、プラグインを束ねたモジュール（決定 0243）の
 * 一覧で、コアのどの形式でもない入力を、その判定の規則で試す
 */
export function loadModules(loaders, plugins = []) {
  return new Modules(loaders, load, plugins);
}
