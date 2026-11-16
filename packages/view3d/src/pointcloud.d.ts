import type { Scene3DReadLimits } from "@zumen-jp/zumen";
import type { DocumentIdentity, Hit3D, Position3D, Scene3DInput, View3D } from "./index.js";

/** 塊ごとの色の出どころ。auto は色・強度・分類の順に持つ列を使う。 */
export type PointColor = "auto" | "rgba" | "intensity" | "classification" | "none";
/** 重ねない理由。 */
export type PointCloudReason =
  | "empty" | "no-scene" | "unit-unknown" | "unit-not-metre" | "crs-unknown" | "map-unknown" | "crs-mismatch" | "map-unsupported" | "height-mismatch" | "removed";
/** 読み直しの要求。中身は利用側の load がそのまま受け取る。 */
export interface PointCloudQuery { readonly [key: string]: unknown }
export interface PointCloudOptions {
  readonly id: string;
  readonly input?: Scene3DInput;
  /** 要求から点群のシーンを読む。signal で中断を受け取る。 */
  readonly load?: (query: PointCloudQuery & { readonly signal: AbortSignal }) => Promise<Scene3DInput | null> | Scene3DInput | null;
  readonly color?: PointColor;
  /** 点の大きさ（CSS px）。既定 2。 */
  readonly pointSize?: number;
  /** 描く点の上限。既定 2,000,000。粗い段から近い順に選ぶ。 */
  readonly maxPoints?: number;
  /** 当たり判定で調べる点の上限。既定 10,000,000。 */
  readonly maxWork?: number;
  readonly scope?: "scene" | "viewer";
  readonly priority?: number;
  readonly readLimits?: Scene3DReadLimits;
}
export interface PointHit {
  readonly kind: "point";
  readonly document: DocumentIdentity;
  /** 塊の行と塊の中の行。点ごとの永続 ID ではない。 */
  readonly chunk: number;
  readonly row: number;
  readonly node: readonly number[] | null;
  readonly distance: number;
  readonly position: Position3D;
  readonly rgba: readonly number[] | null;
  readonly intensity: number | null;
  readonly classification: number | null;
}
export interface PointHitResult {
  readonly hits: readonly PointHit[];
  readonly truncated: boolean;
  /** 重ねていないときの理由。 */
  readonly reason?: PointCloudReason;
  /** 点より手前で当たった要素。これより奥の点は返さない。 */
  readonly occluder?: Hit3D | null;
}
export interface PointCloudReport {
  readonly status: "empty" | "unavailable" | "ready" | "removed";
  readonly reason: PointCloudReason | null;
  readonly document: DocumentIdentity | null;
  /** 高さの基準が両方にあり同じなら same、どちらかがなければ unverified。 */
  readonly height: "same" | "unverified" | null;
  readonly points: number;
  readonly chunks: number;
  readonly drawn: { readonly points: number; readonly chunks: number };
  readonly skipped: { readonly outside: number; readonly budget: number };
  readonly truncated: boolean;
  readonly tolerance: number | null;
  readonly color: PointColor | null;
  /** 表示用に作った色のバイト数。位置と元の色は blob から借用する。 */
  readonly displayBytes: number;
  readonly pending: boolean;
  readonly requests: { readonly applied: number; readonly stale: number; readonly cancelled: number; readonly failed: number; readonly firstFailure: unknown };
}
export type PointCloudOutcome =
  | { readonly outcome: "applied"; readonly report: PointCloudReport }
  | { readonly outcome: "stale" }
  | { readonly outcome: "cancelled" }
  | { readonly outcome: "failed"; readonly error: unknown };
export interface PointCloud {
  readonly id: string;
  readonly signal: AbortSignal;
  remove(): void;
  setInput(input: Scene3DInput | null): PointCloudReport;
  request(query?: PointCloudQuery): Promise<PointCloudOutcome>;
  cancel(reason?: string): void;
  hitTest(viewId: string, point: { readonly x: number; readonly y: number }, options?: { readonly tolerance?: number; readonly limit?: number }): PointHitResult;
  getReport(): PointCloudReport;
}
export function attachPointCloud(viewer: View3D, options: PointCloudOptions): PointCloud;
