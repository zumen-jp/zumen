import type { Bytes, Scene3dCounts, Scene3dResult } from "@zumen-jp/zumen";

/** 検査を通した three.js の r 版。 */
export declare const THREE_REVISION: "187";

export type View3DErrorReason = "invalid-input" | "invalid-scene" | "limit" | "three-version" | "webgl2-unavailable"
  | "destroyed" | "no-scene" | "unknown-element" | "other-document" | "unknown-view" | "outside-view";
/** 表示部品の失敗。reason で種類を区別する。 */
export declare class View3DError extends Error {
  readonly reason: View3DErrorReason;
}

export type Vector3 = readonly [number, number, number];
/** セッション内の文書名だけで同一性を決めず、building の結果の hash も渡す。 */
export interface DocumentIdentity { readonly doc: string; readonly format: string; readonly hash: string }
/** 同じ scene3d の要求の blob と応答、同じ版の文書を一組で渡す。blob は表示中に変更・転送しない。 */
export interface Scene3DInput { readonly blob: Bytes; readonly result: Readonly<Scene3dResult>; readonly document: DocumentIdentity }

/** プロジェクト空間・文書の長さの単位・Z が上。position は f64 のまま表示原点に使う。 */
export type Camera3D =
  | { readonly projection: "perspective"; readonly position: Vector3; readonly direction: Vector3; readonly up: Vector3; /** 縦の視野角（度） */ readonly fov: number }
  | { readonly projection: "orthographic"; readonly position: Vector3; readonly direction: Vector3; readonly up: Vector3; /** 横の幅（文書単位） */ readonly width: number };
/** Canvas 左上からの CSS px の枠。枠どうしは重ねない。 */
export interface Rect { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
export interface View3DView { readonly id: string; readonly rect: Rect; readonly camera: Camera3D }

export interface Limits {
  /** 配置の数。既定 1,000,000。 */
  instances: number;
  /** 展開して GPU へ渡すバイト数の見積もり。既定 512 MiB。 */
  gpuBytes: number;
  /** シーンの組み立てと当たり判定の反復。既定 100,000,000。 */
  work: number;
  /** Canvas の画素数。既定 16,777,216。 */
  canvasPixels: number;
  /** 選択・非表示の件数、購読・通知の数。既定 100,000。 */
  stateItems: number;
  /** ビューの数。既定 16。 */
  views: number;
  /** 当たりの数。既定 1000。 */
  hits: number;
  /** 報告の数。既定 10,000。 */
  issues: number;
  /** readScene3D に渡す上限。既定は読み手と同じ。 */
  readBytes: number;
  readAllocatedBytes: number;
  readWork: number;
}
export interface View3DOptions {
  /** CSS px。省略時は Canvas の clientWidth・clientHeight。 */
  width?: number;
  height?: number;
  /** 既定 1。上限 8。CSS の大きさは呼び出し側が設定する。 */
  pixelRatio?: number;
  /** #rrggbb か 0xRRGGBB。既定は白・#b4b4b4・#ff8800・#202020。 */
  background?: string | number;
  unresolvedColor?: string | number;
  selectionColor?: string | number;
  edgeColor?: string | number;
  /** lambert は照明つき、unlit は元の色のまま。既定 lambert。 */
  shading?: "lambert" | "unlit";
  /** 稜線を描く。既定 true。 */
  edges?: boolean;
  antialias?: boolean;
  limits?: Partial<Limits>;
  views?: readonly View3DView[];
}
export interface RenderIssue {
  readonly reason: "color-unresolved" | "points-unsupported";
  readonly disposition: "substituted" | "omitted";
  readonly element?: string;
  readonly count?: number;
}
export interface Scene3DReport {
  readonly drawn: { readonly instances: number; readonly batches: number; readonly individual: number; readonly edges: number };
  /** 透明度 0 の配置。描画と当たり判定から除く。 */
  readonly skipped: { readonly transparent: number };
  readonly gpuBytes: number;
  readonly issues: readonly RenderIssue[];
  readonly truncatedIssues: number;
}
export interface View3DState {
  readonly document: DocumentIdentity | null;
  readonly scene: { readonly unit: Scene3dResult["unit"]; readonly tolerance: number; readonly counts: Readonly<Required<Scene3dCounts>> } | null;
  readonly views: readonly View3DView[];
  readonly selection: readonly string[];
  readonly hidden: readonly string[];
  readonly size: { readonly width: number; readonly height: number; readonly pixelRatio: number };
  readonly contextLost: boolean;
  readonly destroyed: boolean;
}
/** 大きな原点を足す丸めを避け、f64 の原点と相対位置に分ける。 */
export interface Position3D { readonly origin: Vector3; readonly offset: Vector3 }
export interface Ray3D extends Position3D { readonly direction: Vector3 }
export interface ElementHit {
  readonly kind: "element";
  readonly document: DocumentIdentity;
  readonly id: string;
  /** 元のシーンの要素の行と配置の行。永続 ID には使わない。 */
  readonly element: number;
  readonly instance: number;
  /** 文書単位。 */
  readonly distance: number;
  readonly position: Position3D;
}
export interface HitTestResult { readonly hits: readonly ElementHit[]; readonly truncated: boolean }
export interface View3DStats {
  readonly geometries: number; readonly textures: number; readonly programs: number;
  readonly calls: number; readonly triangles: number; readonly lines: number;
  readonly gpuBytes: number; readonly subscribers: number; readonly listeners: number; readonly frame: boolean;
}
export type View3DEvent =
  | { readonly type: "render"; readonly views: number }
  | { readonly type: "contextlost" }
  | { readonly type: "contextrestored" };

export declare class View3D {
  /** WebGL2 を使えないときは reason が webgl2-unavailable の例外。 */
  constructor(canvas: HTMLCanvasElement | OffscreenCanvas, options?: View3DOptions);
  getState(): View3DState;
  subscribe(listener: (state: View3DState) => void): () => void;
  on<T extends View3DEvent["type"]>(type: T, listener: (event: Extract<View3DEvent, { type: T }>) => void): () => void;
  setSize(width: number, height: number, pixelRatio?: number): void;
  setViews(views: readonly View3DView[]): void;
  /** 成功時だけ差し替える。選択と非表示は引き継がない。 */
  setScene(input: Scene3DInput): Scene3DReport;
  setScene(input: null): null;
  select(document: DocumentIdentity, ids: readonly string[]): void;
  setHidden(document: DocumentIdentity, ids: readonly string[]): void;
  fit(viewId: string, margin?: number): void;
  getRay(viewId: string, point: { readonly x: number; readonly y: number }): Ray3D;
  hitTest(viewId: string, point: { readonly x: number; readonly y: number }, options?: { readonly limit?: number }): HitTestResult;
  getStats(): View3DStats;
  invalidate(): void;
  /** context を失っている間は描かずに false を返す。 */
  render(): boolean;
  destroy(): void;
}
