import type { Bytes } from "./index.js";

export type LiveBudgetKind = "inputBytes" | "decodedBytes" | "records" | "elements" | "depth" | "segments" | "work";
/** 未指定の単位はコアの既定値、null は明示した上限なし */
export type LiveBudget = Partial<Record<LiveBudgetKind, number | bigint | null>>;
export interface LiveOptions { budget?: LiveBudget; }
export interface LiveDisplayOptions extends LiveOptions { display?: boolean; full?: boolean; }
export interface LiveOpenOptions extends LiveDisplayOptions { threads?: number; }
export interface LiveProcessLimits {
  /** 1 要求の期限。既定 30000 ms。1 から 2147483647 まで */
  timeoutMs?: number;
  /** ヘッダーを除く送受信の上限。既定 256 MiB + 136。1024 から 1 GiB + 136 まで */
  maxMessageBytes?: number;
}
export type LiveStatus = "ok" | "invalid" | "unsupported" | "stale" | "missing" | "rejected"
  | "exceeded" | "cancelled" | "timedOut" | "noDocument" | "alreadyOpen" | "noDiagnostics";
export interface LiveBlock {
  start: number;
  rows: number;
  /** 元の応答を借りる little endian のビット列。利用中は変更しない */
  bytes: Uint8Array;
}
export interface LiveColumn {
  readonly id: number;
  readonly rows: number;
  readonly stride: number;
  blocks(): IterableIterator<LiveBlock>;
}
export interface LiveDisplay {
  readonly full: boolean;
  /** 固定順の 13 列。全体の検証が済んでから公開する */
  readonly columns: readonly LiveColumn[];
}
export interface LiveReply {
  requestId: bigint;
  status: LiveStatus;
  /** 編集が ok でも表示だけが停止する場合がある */
  displayStatus: "ok" | "invalid" | "exceeded" | "cancelled" | "timedOut";
  revision: bigint | null;
  state: bigint | null;
  result: bigint;
  /** 差分が前提にする受領済みの版。全体表示では null */
  base: bigint | null;
  diagnosticCount: number;
  used: Record<LiveBudgetKind, bigint>;
  stop: { kind: LiveBudgetKind; limit: bigint; requested: bigint } | { kind: "cancelled" | "timedOut" } | null;
  /** 表示・保存・対象・診断の元のバイト列。次の要求後も有効 */
  data: Uint8Array;
  display: LiveDisplay | null;
  target: { id: string; hash: string } | null;
}
export interface LiveTransform {
  /** 選択した行が属する版。現在の版と異なれば stale */
  revision: bigint | number;
  rows: readonly number[] | Uint32Array;
  /** x' = a*x + b*y + c、y' = d*x + e*y + f の [a,b,c,d,e,f]。既存の apply と同じ */
  matrix: readonly number[] | Float64Array;
}
/** DXF / DWG の常駐文書。通常の同時呼び出しは ZumenError の busy */
export declare class LiveProcess {
  private constructor();
  readonly closed: boolean;
  readonly broken: boolean;
  readonly revision: bigint | null;
  open(input: Bytes, options?: LiveOpenOptions): Promise<LiveReply>;
  /** 最後に受信した表示を適用してから呼ぶ。受信だけでは自動確認しない */
  acknowledge(revision: bigint | number): void;
  display(options?: LiveDisplayOptions): Promise<LiveReply>;
  target(row: number, revision: bigint | number, options?: LiveOptions): Promise<LiveReply>;
  transform(change: LiveTransform, options?: LiveDisplayOptions): Promise<LiveReply>;
  undo(revision: bigint | number, options?: LiveDisplayOptions): Promise<LiveReply>;
  redo(revision: bigint | number, options?: LiveDisplayOptions): Promise<LiveReply>;
  save(options?: LiveOptions): Promise<LiveReply>;
  /** 最後の通常要求の診断。data は UTF-8 JSON。取得の停止後も再取得できる */
  diagnostics(requestId: bigint | number, options?: LiveOptions): Promise<LiveReply>;
  /** 実行中の要求へ中断を送り、完了を待つ。1 秒以内に戻らなければ子を終了する */
  cancel(): Promise<void>;
  /** 処理中の要求を失敗にし、子プロセスの終了まで待つ */
  close(): Promise<void>;
}
/** CLI を live --stdio で起動する。図面は返った接続の open で渡す */
export declare function openLiveProcess(executable: string, transport?: LiveProcessLimits): Promise<LiveProcess>;
