// zumen の wasm のモジュールを呼ぶ薄い包みの型（決定 0011・0194）。要求と応答の型は messages.d.ts に生成する。

import type { Detection, ModuleName, StatusName } from "./constants.js";
import type { Operations, Request, Response, SessionOptions } from "./messages.js";

export * from "./constants.js";
export type * from "./messages.js";
export { Scene, readScene } from "./scene.js";

/** バイト列 */
export type Bytes = ArrayBuffer | ArrayBufferView;

/** 要求の op から、結果の型を引く */
export type ResultOf<R extends Request> = R extends { op: infer O extends keyof Operations }
  ? Operations[O]["result"]
  : unknown;

/** 境界の関数が ok でない状態を返した */
export declare class ZumenError extends Error {
  /** 呼んだ境界の関数（zm_call など） */
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
  /** セッションを閉じる。閉じたあとの呼び出しは、invalid の ZumenError になる */
  close(): void;
}

/** 読み込んだ wasm のモジュール */
export declare class Zumen {
  private constructor();
  /** ABI の大版 */
  abiVersion(): number;
  /** セッションを作る。設定は、値か JSON の文字列かバイト列で渡す。渡さなければ既定の設定 */
  open(options?: SessionOptions | string | Bytes): Session;
}

/** モジュールを、コンパイルしたものかバイト列か fetch の応答から、非同期に読み込む */
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

/** 分けた wasm のモジュールを、要るときに 1 度だけ読み込む */
export declare class Modules {
  private constructor();
  /** 名前のモジュールを読み込む。プラグインのモジュールは、その名前で引く */
  get(name: ModuleName | string): Promise<Zumen>;
  /** 入力の形式を判定する。コアの形式でなければ、プラグインの規則で試す。どの形式でもなければ undefined */
  detect(bytes: Bytes): { format: string; module: string; rules: Detection["rules"] } | undefined;
  /** 入力の形式を判定し、その形式を持つモジュールを読み込む。どの形式でもなければ undefined */
  forInput(bytes: Bytes): Promise<Zumen | undefined>;
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
