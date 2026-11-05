import type { Extent, Scene, SceneCounts, SceneResult } from "@zumen-jp/zumen";

export interface Point { readonly x: number; readonly y: number }
/** 起点の座標の中心、CSS px / 起点の長さ、反時計回りのラジアン。 */
export interface Camera { readonly center: Point; readonly scale: number; readonly rotation: number }
export interface Size { readonly width: number; readonly height: number; readonly pixelRatio: number }
/** セッション内の文書名だけで同一性を決めず、drawing の結果の hash も渡す。 */
export interface DocumentIdentity { readonly doc: string; readonly format: string; readonly hash: string }
/** 同じ要求の応答と blob、同じ版の文書を一組で渡す。columns は共有し、変更・転送しない。 */
export interface SceneInput { readonly scene: Scene; readonly result: Readonly<SceneResult>; readonly document: DocumentIdentity }
export interface SceneSnapshot extends Omit<SceneInput, "result"> {
  readonly result: Readonly<Omit<SceneResult, "counts" | "extent">> & {
    readonly counts: Readonly<SceneCounts>;
    readonly extent?: Readonly<Extent>;
  };
}
export interface Limits {
  /** すべての列の値の合計。既定 16,000,000。 */
  columnValues: number;
  /** 列の検査・並べ替え・切り抜き・字形の反復を含む。既定 32,000,000。 */
  work: number;
  /** 代替と未描画の報告の上限。既定 10,000。 */
  issues: number;
  /** 1 つの文字の UTF-8 バイト数の上限。既定 16,384。 */
  textLength: number;
  /** Canvas の画素数の上限。既定 16,777,216。 */
  canvasPixels: number;
}
export interface View2DOptions {
  /** CSS px。省略時は Canvas の clientWidth・clientHeight（OffscreenCanvas は width・height）。 */
  width?: number;
  height?: number;
  /** 既定 1。上限 8。CSS の大きさは呼び出し側が設定する。 */
  pixelRatio?: number;
  view?: Camera;
  /** Canvas が解釈できる CSS 色。既定は白・#111111・#777777。 */
  background?: string;
  foreground?: string;
  unresolvedColor?: string;
  /** 用紙 mm の線幅を CSS px にする倍率。既定 96 / 25.4。ズームには連動しない。 */
  pixelsPerMillimeter?: number;
  fallbackFont?: "sans-serif" | "serif" | "monospace";
  limits?: Partial<Limits>;
}
export interface ElementRef {
  readonly id: string;
  readonly path: readonly { readonly id: string; readonly column: number; readonly row: number }[];
}
export type RenderReason = "browser-text" | "text-layout-unsupported" | "marker-unresolved" | "point-style-unsupported" | "resource-unresolved" | "color-unresolved" | "clip-unsupported";
export interface RenderIssue {
  readonly element: ElementRef;
  readonly unit: number;
  readonly primitive: { readonly kind: "line" | "face" | "text" | "point" | "image"; readonly index: number };
  readonly reason: RenderReason;
  readonly disposition: "fallback" | "omitted";
}
/** drawn は描画を発行した数。画面外・切り抜かれたものも含む。 */
export interface RenderReport {
  readonly drawn: Readonly<{ lines: number; faces: number; texts: number; points: number; images: number; glyphs: number }>;
  readonly issues: readonly RenderIssue[];
}
export declare class View2D {
  constructor(canvas: HTMLCanvasElement | OffscreenCanvas, options?: View2DOptions);
  /** 検査に失敗すれば RangeError。以前のシーンと画面を保つ。 */
  setScene(input: SceneInput | null): RenderReport;
  getScene(): SceneSnapshot | null;
  getView(): Camera;
  getSize(): Size;
  getReport(): RenderReport;
  render(): RenderReport;
  jumpTo(view: Camera): RenderReport;
  fitBounds(bounds: Extent, padding?: number): RenderReport;
  /** SceneResult.extent に収める。文字列は基点だけの範囲であり、代替文字の外形は含まない。 */
  fit(padding?: number): RenderReport;
  /** anchor は Canvas 左上からの CSS px。省略時は中心。 */
  zoomBy(factor: number, anchor?: Point): RenderReport;
  rotateBy(radians: number, anchor?: Point): RenderReport;
  panBy(dx: number, dy: number): RenderReport;
  /** 起点の座標と Canvas 左上からの CSS px を相互に変換する。実寸への変換ではない。 */
  toScreen(point: Point): Point;
  toScene(point: Point): Point;
  resize(width: number, height: number, pixelRatio?: number): RenderReport;
  /** 冪等。ほかのメソッドは破棄後に Error を返す。 */
  destroy(): void;
}
