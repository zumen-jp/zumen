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
/** 応答の版に属する生存行。元の応答バイト列は利用中に変更しない */
export interface LiveSelection {
  readonly length: number;
  /** 全件の検証後、昇順で借りる。次の要求後も有効 */
  rows(): IterableIterator<number>;
}
/** 表示のスタイル [画層, 色] の画層の値に対応する画層。DXF・DWG は画層の表の位置、JWW・SXF は図面モデルの画層の ID */
export interface LiveLayer {
  readonly value: number;
  readonly name: string;
  /** 形式が状態を持たなければ null */
  readonly state: { readonly visible: boolean; readonly printable: boolean; readonly editable: boolean; readonly frozen: boolean } | null;
  readonly rgb: readonly [number, number, number] | null;
  readonly code: { readonly system: "aci" | "sxf" | "jww"; readonly value: number } | null;
  readonly followsPlacement: boolean;
}
/** restyle の線種に使える線種。value は図面モデルの線種の ID */
export interface LiveLineType {
  readonly value: number;
  readonly name: string;
  /** 線と間の列を持たなければ null。列は正が線、負が間、0 が点。basis は列の長さの空間（用紙の mm か要素の空間） */
  readonly definition: {
    readonly description: string;
    readonly pattern: readonly number[];
    readonly basis: "paper" | "element";
    /** 形か文字を添える（中身は運ばない） */
    readonly marks: boolean;
  } | null;
}
/** 行の文字。kind は文字でない行で null、内容は制御文字や書式の符号を含む元のまま */
export interface LiveText {
  readonly kind: "text" | "formatted" | null;
  readonly content: string;
}
/** 記号の定義の表示。挿入の定義の値は 0 から total 未満 */
export interface LiveSymbols {
  /** 文書の記号の定義の数 */
  readonly total: number;
  /** 要求した定義の順に、display での先頭の行と行の数。行は 1 要素 1 行で隙間なく並ぶ */
  readonly definitions: readonly { readonly first: number; readonly rows: number }[];
  /** 定義の座標のままの全量の表示。入れ子の挿入は挿入のまま */
  readonly display: LiveDisplay;
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
  /** 表示・保存・対象・診断・選択（写しの行）・画層の表・描く順・線種の表・文字の元のバイト列。次の要求後も有効 */
  data: Uint8Array;
  display: LiveDisplay | null;
  target: { id: string; hash: string } | null;
  selection: LiveSelection | null;
  /** 図面モデルの画層と同じ順。全件の検証後に公開する */
  layers: readonly LiveLayer[] | null;
  /** 先に描く行から並べた、表示に出る生きた行。重複はない */
  order: LiveSelection | null;
  /** 値は重複しない。全件の検証後に公開する */
  lineTypes: readonly LiveLineType[] | null;
  /** 要求した行の順。全件の検証後に公開する */
  texts: readonly LiveText[] | null;
  /** 全件の検証後に公開する */
  symbols: LiveSymbols | null;
}
export interface LiveTransform {
  /** 選択した行が属する版。現在の版と異なれば stale */
  revision: bigint | number;
  rows: readonly number[] | Uint32Array;
  /** x' = a*x + b*y + c、y' = d*x + e*y + f の [a,b,c,d,e,f]。既存の apply と同じ */
  matrix: readonly number[] | Float64Array;
}
export interface LiveRestyle {
  /** 選択した行が属する版。現在の版と異なれば stale */
  revision: bigint | number;
  rows: readonly number[] | Uint32Array;
  /** layers の value。表にない値は invalid */
  layer?: number;
  /** 継承か、形式の色番号と RGB のどちらかまたは両方。DXF の RGB は R2004 から */
  color?: "layer" | "block" | { code?: { system: "aci" | "sxf" | "jww"; value: number }; rgb?: readonly [number, number, number] };
  /** 継承・文書の既定か、用紙の上の幅（mm） */
  lineWeight?: "layer" | "block" | "default" | number;
  /** 正の数 */
  lineTypeScale?: number;
  /** 継承か、lineTypes の value。表にない値は invalid */
  lineType?: "layer" | "block" | number;
}
export interface LiveSetText {
  /** 選択した行が属する版。現在の版と異なれば stale */
  revision: bigint | number;
  rows: readonly number[] | Uint32Array;
  /** 新しい内容。UTF-8 で 1 MiB まで。形式の符号（MTEXT の書式など）は解釈せずにそのまま書く */
  content: string;
}
export interface LiveRemove {
  /** 選択した行が属する版。現在の版と異なれば stale */
  revision: bigint | number;
  rows: readonly number[] | Uint32Array;
}
/** DXF・DWG・JWW・SXF の常駐文書。通常の同時呼び出しは ZumenError の busy */
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
  layers(revision: bigint | number, options?: LiveOptions): Promise<LiveReply>;
  /** restyle の線種に使える線種の表。DXF・DWG は図面モデルの線種、JWW・SXF は形式が書ける番号の線種 */
  lineTypes(revision: bigint | number, options?: LiveOptions): Promise<LiveReply>;
  /** 行の文字（TEXT・MTEXT・JWW の文字・SXF の文字要素）。属性・寸法・表の中の文字は返さない。生きていない行があれば missing */
  texts(rows: readonly number[] | Uint32Array, revision: bigint | number, options?: LiveOptions): Promise<LiveReply>;
  /** 記号の定義の表示（DXF・DWG・SXF）。JWW はまだ rejected。範囲外の定義は invalid */
  symbols(definitions: readonly number[] | Uint32Array, revision: bigint | number, options?: LiveOptions): Promise<LiveReply>;
  /** 図面モデルの描く順（DXF・DWG は SORTENTSTABLE、JWW は画層グループ、SXF は用紙の並び）に合わせた行 */
  order(revision: bigint | number, options?: LiveOptions): Promise<LiveReply>;
  /** モデル空間の [minX,minY,maxX,maxY] と囲む箱が交差する行。NaN・逆転は空、無限大は全域指定 */
  select(rect: readonly number[] | Float64Array, revision: bigint | number, options?: LiveOptions): Promise<LiveReply>;
  transform(change: LiveTransform, options?: LiveDisplayOptions): Promise<LiveReply>;
  /** 行の画層・色・線幅・線種・線種の倍率を変える。result は変えた数 */
  restyle(change: LiveRestyle, options?: LiveDisplayOptions): Promise<LiveReply>;
  /** 文字の行の内容を替える。替えられるのは texts の kind が "text" の行と、区間が文字列 1 つだけの "formatted" の行。
   * ほかの行を含めば rejected で何も替えない。result は替えた数 */
  setText(change: LiveSetText, options?: LiveDisplayOptions): Promise<LiveReply>;
  /** 行を除く。result は除いた数 */
  remove(change: LiveRemove, options?: LiveDisplayOptions): Promise<LiveReply>;
  /** 行を写し、写しを matrix で動かす。写しの行を selection に対象の順で返す。表示は display で取り直す。JWW・SXF は rejected */
  copy(change: LiveTransform, options?: LiveOptions): Promise<LiveReply>;
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
