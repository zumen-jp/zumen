// zumen の wasm のモジュールを呼ぶ薄いラッパーの型（決定 0011・0194）。要求と応答の型は messages.d.ts に生成する。

import type { Detection, ModuleName, StatusName } from "./constants.js";
import type { OpenRequest, OpenResult, Operations, Request, Response, SessionOptions } from "./messages.js";

export * from "./constants.js";
export type * from "./messages.js";
export { Scene, readScene } from "./scene.js";
export * from "./scene3d.js";
export * from "./scene3d-layout.js";

/** バイト列 */
export type Bytes = ArrayBuffer | ArrayBufferView;

/** 要求の op から、結果の型を引く */
export type ResultOf<R extends Request> = R extends { op: infer O extends keyof Operations }
  ? Operations[O]["result"]
  : unknown;

/**
 * API の関数が ok でない状態を返したか、wasm の中から例外が出た。wasm の中で trap などが起きたときは、状態が
 * panicked になり、cause に元の例外（WebAssembly.RuntimeError など）を持つ
 */
export declare class ZumenError extends Error {
  /** 呼んだ API の関数（zm_call など） */
  readonly operation: string;
  /** 状態コード */
  readonly code: number;
  /** 状態コードの名前 */
  readonly status: StatusName;
  /** 応答の JSON。書かれなかったときは null */
  readonly bytes: Uint8Array | null;
  /** 応答。書かれなかったときは undefined */
  readonly response: Response | undefined;
}

/** セッション。入力と、直前の呼び出しのバイナリの結果を持つ。使い終わったら close で閉じる */
export declare class Session {
  private constructor();
  /** 閉じたか */
  readonly closed: boolean;
  /** 名前のついた入力に、バイト列を足す。同じ名前で何回か呼ぶと、後ろに足していく */
  push(name: string | Bytes, data: string | Bytes): void;
  /** 要求の JSON（文字列かバイト列）を渡し、応答の JSON のバイト列を受け取る */
  callBytes(request: string | Bytes): Uint8Array;
  /** 要求を渡し、応答を受け取る。失敗した応答は、ZumenError の response にある */
  call<R extends Request>(request: R): Response<ResultOf<R>>;
  /** 直前の call が作ったバイナリの結果を、応答の blobs にある番号で取り出す */
  take(blob: number): Uint8Array;
  /** 動いている呼び出しを止める。動いている呼び出しがなければ、何もしない */
  cancel(): void;
  /**
   * セッションを閉じる。閉じたあとの呼び出しは、invalid の ZumenError になる。使えなくなったインスタンスでは何もせず、
   * finally の中で呼んでも、先に出た例外を隠さない
   */
  close(): void;
}

/** 読み込んだ wasm のモジュール */
export declare class Zumen {
  private constructor();
  /**
   * wasm の中で trap などが起き、もう使えないか。使えなければ、それからの呼び出しは panicked の ZumenError になる。
   * モジュールを読み込み直す
   */
  readonly broken: boolean;
  /** ABI のメジャーバージョン */
  abiVersion(): number;
  /** セッションを作る。設定は、値か JSON の文字列かバイト列で渡す。渡さなければ既定の設定 */
  open(options?: SessionOptions | string | Bytes): Session;
}

/**
 * モジュールを、コンパイルしたものかバイト列か fetch の応答から、非同期に読み込む。別の realm（Next.js の Edge のランタイム
 * など）で作ったものも受け取る
 */
export declare function load(
  source: WebAssembly.Module | Bytes | globalThis.Response | PromiseLike<WebAssembly.Module | Bytes | globalThis.Response>,
): Promise<Zumen>;

/** モジュールを、コンパイルしたものかバイト列から、同期で読み込む */
export declare function loadSync(source: WebAssembly.Module | Bytes): Zumen;

/** 入力の形式を、コアと同じ順と規則で判定する（決定 0221）。どの形式でもなければ undefined */
export declare function detect(bytes: Bytes): Detection | undefined;

/** op を持つ wasm のモジュールの名前 */
export declare function modulesWith(op: string): ModuleName[];

/** プラグインを束ねた wasm のモジュール（決定 0243） */
export interface PluginModule {
  /** モジュールの名前。コアのモジュールの名前と違うもの */
  name: string;
  /** 形式ごとの判定の規則。DETECTION の項目と同じ形 */
  formats: { format: string; rules: Detection["rules"] }[];
  /** 持つ op */
  ops: string[];
  /** `load` に渡せるもの（fetch の応答など）を返す */
  load: () => Parameters<typeof load>[0];
}

/** 分けた wasm のモジュールを、要るときに読み込んで使い回す。trap で使えなくなったら、読み込み直す */
export declare class Modules {
  private constructor();
  /**
   * 名前のモジュールを読み込む。プラグインのモジュールは、その名前で引く。読み込んだインスタンスは使い回し、trap で
   * 使えなくなったら（`Zumen.broken`）、捨てて読み込み直す。読み込みに失敗したら覚えず、次に引いたときにやり直す
   */
  get(name: ModuleName | string): Promise<Zumen>;
  /** 入力の形式を判定する。コアの形式でなければ、プラグインの規則で試す。どの形式でもなければ undefined */
  detect(bytes: Bytes): { format: string; module: string; rules: Detection["rules"] } | undefined;
  /** 入力の形式を判定し、その形式を持つモジュールを読み込む。どの形式でもなければ undefined */
  forInput(bytes: Bytes): Promise<Zumen | undefined>;
  /**
   * 入力を、その形式を持つモジュールの新しいセッションで開く（決定 0250）。図面の形式のプラグインの入力は、プラグインの
   * モジュールで lift し、図面モデルのバイナリを図面のモジュールで開く。返すセッションは呼び出し側が閉じる。どの
   * モジュールの形式でもなければ undefined
   */
  open(
    name: string,
    bytes: Bytes,
    request?: Omit<OpenRequest, "v" | "op" | "input">,
    inputs?: Record<string, string | Bytes>,
  ): Promise<{ zumen: Zumen; session: Session; response: Response<ResultOf<OpenRequest>> } | undefined>;
  /** op を持つモジュールのうち、読み込めるものの最初を読み込む */
  forOp(op: string): Promise<Zumen>;
}

/**
 * 分けた wasm のモジュール（決定 0013・0221）を、要るときに読み込む。`loaders` は、モジュールの名前から、
 * `load` に渡せるもの（fetch の応答など）を返す関数を引く表。`plugins` は、プラグインを束ねたモジュールの一覧
 */
export declare function loadModules(
  loaders: Partial<Record<ModuleName, () => Parameters<typeof load>[0]>>,
  plugins?: PluginModule[],
): Modules;

/** 範囲を読む関数。[start, end) のバイト列を返す */
export type RangeReader = (start: number, end: number) => Uint8Array | ArrayBuffer | ArrayBufferView | Promise<Uint8Array | ArrayBuffer | ArrayBufferView>;

export interface OpenRangedOptions {
  /** 1 回目に渡す初めの部分の大きさ（バイト）。既定は 64 KiB */
  head?: number;
  /** 入力の全体の大きさ。関数の読み口のときに渡す。Blob と URL は読み口から決める */
  size?: number;
  /** 範囲の読み出しを止める */
  signal?: AbortSignal;
}

/**
 * 入力の範囲を読みながら open する（決定 0232）。request.input の名前で初めの部分を渡し、size を書いて open を呼ぶ。
 * needs の範囲をその名前で push して同じ要求を呼び直し、読み終えた done の入力を close で捨てる。1 回の範囲の合計が
 * 要求の budget.input_bytes を超えるときと、同じ範囲を求め続けるときは止める。失敗したら渡した入力を捨てる。
 * source は Blob（File）・URL（Range で読む）・範囲を読む関数
 */
export declare function openRanged(
  session: Session,
  request: Omit<OpenRequest, "op" | "size"> & { op?: "open" },
  source: Blob | URL | string | RangeReader,
  options?: OpenRangedOptions,
): Promise<Response<OpenResult>>;
