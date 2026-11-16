import type { Bytes, ResultOf } from "./index.js";
import type { Request, Response, SessionOptions } from "./messages.js";

/** 子プロセスとの通信の上限。コアの作業量は各要求の budget で指定する */
export interface ProcessLimits {
  /** 1 つの応答を待つ時間。既定は 30000 ms。1 から 2147483647 まで */
  timeoutMs?: number;
  /** JSON-RPC の 1 行の送受信の上限。既定は 64 MiB。1024 から 1 GiB まで */
  maxMessageBytes?: number;
}

/** 起動・通信・上限による失敗。コアの失敗は既存の ZumenError で返す */
export declare class ProcessError extends Error {
  constructor(code: ProcessError["code"], message: string, cause?: unknown);
  readonly code: "spawn" | "io" | "protocol" | "closed" | "timeout" | "limit";
}

/** CLI を常駐させる非同期のセッション。通常の呼び出しの重複は busy になる */
export declare class ProcessSession {
  private constructor();
  readonly closed: boolean;
  /** 子プロセスか通信が壊れたか。新しいセッションで開き直す */
  readonly broken: boolean;
  push(name: string | Bytes, data: string | Bytes): Promise<void>;
  callBytes(request: string | Bytes): Promise<Uint8Array>;
  call<R extends Request>(request: R): Promise<Response<ResultOf<R>>>;
  take(blob: number): Promise<Uint8Array>;
  /** 実行中の call を中断する。1 秒以内に call が終わらなければ子プロセスを終了する */
  cancel(): Promise<void>;
  /** 処理中の呼び出しを失敗にし、子プロセスの終了まで待つ。繰り返し呼んでもよい */
  close(): Promise<void>;
}

/**
 * Node.js から CLI を serve --stdio で起動する。実行ファイルは別に導入し、道筋を渡す。
 * セッションの設定は CLI にそのまま渡す。通常は省略するか engine: "native" にする。
 */
export declare function openProcess(
  executable: string,
  options?: SessionOptions | string | Bytes,
  transport?: ProcessLimits,
): Promise<ProcessSession>;
