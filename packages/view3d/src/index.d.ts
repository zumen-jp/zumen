import type { Bytes, Scene3dCounts, Scene3dResult } from "@zumen-jp/zumen";

/** 検査を通した three.js の r 版。 */
export declare const THREE_REVISION: "187";

export type View3DErrorReason = "invalid-input" | "invalid-scene" | "limit" | "three-version" | "webgl2-unavailable"
  | "destroyed" | "no-scene" | "unknown-element" | "other-document" | "unknown-view" | "outside-view" | "aspect-ratio" | "context-lost";
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
/** 面の点と法線。法線の向きの半空間を取り除く（n · (q - point) > 0）。描画と当たり判定で同じ条件。 */
export interface ClipPlane { readonly point: Vector3; readonly normal: Vector3 }
/** 画面で切る箱。外側を取り除く 6 面と同じで、要素を絞る箱ではない。 */
export interface ClipBox { readonly min: Vector3; readonly max: Vector3 }
/** 要素の色。レビュー文書と同じ #RRGGBB か #AARRGGBB で、透明度は保つが描かない。 */
export interface ColoredElements { readonly color: string; readonly ids: readonly string[] }
export interface View3DView {
  readonly id: string;
  readonly rect: Rect;
  readonly camera: Camera3D;
  readonly clips?: readonly ClipPlane[];
  readonly box?: ClipBox | null;
  /** 表示中のシーンの要素 ID。シーンを差し替えると消える。 */
  readonly selected?: readonly string[];
  /** 既定で要素を見せるか。既定 true。 */
  readonly visible?: boolean;
  /** 既定と逆に見せる・隠す要素。 */
  readonly exceptions?: readonly string[];
  readonly colored?: readonly ColoredElements[];
}
/** 状態の snapshot のビュー。要素はシーンの行の順で、色は色ごとにまとめる。 */
export interface View3DViewState extends View3DView {
  readonly clips: readonly ClipPlane[];
  readonly box: ClipBox | null;
  readonly selected: readonly string[];
  readonly visible: boolean;
  readonly exceptions: readonly string[];
  readonly colored: readonly ColoredElements[];
}

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
  /** ビューごとの切断面の数（箱は 6 面）。既定 16。 */
  clips: number;
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
  readonly views: readonly View3DViewState[];
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
/** レビュー文書（zumen-review）の要素の参照。 */
export interface ReviewReference { readonly id: string; readonly hash?: string }
/** レビュー文書のカメラ。縦横比を持たず、BCF の互換枠は 1:1。 */
export type ReviewCamera =
  | { readonly projection: "perspective"; readonly field_of_view: number; readonly position: Vector3; readonly direction: Vector3; readonly up: Vector3 }
  | { readonly projection: "orthographic"; readonly width: number; readonly position: Vector3; readonly direction: Vector3; readonly up: Vector3 };
/** レビュー文書の 3D の場所。location の id・target と合わせて文書に入れる。 */
export interface ReviewPlace3D {
  readonly kind: "3d";
  readonly camera?: ReviewCamera;
  readonly selected?: readonly ReviewReference[];
  readonly visible?: boolean;
  readonly exceptions?: readonly ReviewReference[];
  readonly colored?: readonly { readonly color: string; readonly elements: readonly ReviewReference[] }[];
  readonly clips?: readonly ClipPlane[];
}
export interface ReviewAdjustment { readonly field: string; readonly from: number; readonly to: number; readonly reason: "square-frame" }
export interface ReviewIssue {
  readonly reason: "unit-not-metre" | "element-missing" | "element-changed" | "hash-unchecked" | "color-overridden";
  readonly disposition: "kept" | "omitted" | "substituted";
  readonly count?: number;
  /** 報告の数の上限までの要素 ID。 */
  readonly elements?: readonly string[];
  readonly unit?: Scene3dResult["unit"];
}
export interface ReviewExport {
  readonly place: ReviewPlace3D & { readonly camera: ReviewCamera };
  /** m・1:1 で調整も報告もないとき true。BCF の厳密な往復はこの場合に限る。 */
  readonly strict: boolean;
  readonly adjustments: readonly ReviewAdjustment[];
  readonly issues: readonly ReviewIssue[];
}
export interface ReviewApplyResult {
  readonly strict: boolean;
  readonly adjustments: readonly ReviewAdjustment[];
  readonly issues: readonly ReviewIssue[];
  /** 1:1 の互換枠を収めた、枠の中の正方形（CSS px）。 */
  readonly frame: Rect;
}
export interface Snapshot3D {
  readonly view: string;
  readonly rect: Rect;
  readonly pixelRatio: number;
  /** デバイスの画素。 */
  readonly width: number;
  readonly height: number;
  /** RGBA、上の行から。 */
  readonly pixels: Uint8Array;
}
export interface ViewTarget { readonly view?: string }

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
  /** view を省けばすべてのビュー。 */
  select(document: DocumentIdentity, ids: readonly string[], options?: ViewTarget): void;
  /** visible を true にし、ids を隠す。 */
  setHidden(document: DocumentIdentity, ids: readonly string[], options?: ViewTarget): void;
  setColors(document: DocumentIdentity, colored: readonly ColoredElements[], options?: ViewTarget): void;
  fit(viewId: string, margin?: number): void;
  getRay(viewId: string, point: { readonly x: number; readonly y: number }): Ray3D;
  hitTest(viewId: string, point: { readonly x: number; readonly y: number }, options?: { readonly limit?: number }): HitTestResult;
  /** frame を省けば exact で、1:1 でない枠は reason が aspect-ratio の例外。 */
  toReviewPlace(viewId: string, options?: { readonly frame?: "exact" | "contain"; readonly hashes?: ReadonlyMap<string, string> }): ReviewExport;
  applyReviewPlace(viewId: string, document: DocumentIdentity, place: ReviewPlace3D, options?: { readonly hashes?: ReadonlyMap<string, string> }): ReviewApplyResult;
  /** 利用側の明示した要求で描き、画素を返す。保存・送信はしない。 */
  snapshot(viewId: string): Snapshot3D;
  getStats(): View3DStats;
  invalidate(): void;
  /** context を失っている間は描かずに false を返す。 */
  render(): boolean;
  destroy(): void;
}
