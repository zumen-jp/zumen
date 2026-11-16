import type { DocumentIdentity, Hit3D, Position3D, Vector3, View3D } from "./index.js";

/** View2D の要素の参照と同じ形。 */
export interface Ground2DElementRef {
  readonly id: string;
  readonly path: readonly { readonly id: string; readonly column: number; readonly row: number }[];
}
interface Point2D { readonly x: number; readonly y: number }
interface ScenePosition2D { readonly origin: Point2D; readonly offset: Point2D }
/** アダプターが使う View2D の操作。View2D を読み込まず、利用側の実体を受け取る。 */
export interface View2DLike {
  getState(): { readonly scene: { readonly result: { readonly from: string; readonly view?: number; readonly unit: string } } | null; readonly selection: readonly Ground2DElementRef[] };
  getSize(): { readonly width: number; readonly height: number; readonly pixelRatio: number };
  getView(): { readonly scale: number };
  getReport(): { readonly issues: readonly unknown[] };
  getElements(): readonly Ground2DElementRef[];
  subscribe(listener: (state: unknown) => void): () => void;
  select(elements: readonly Ground2DElementRef[], mode?: "replace"): unknown;
  hitTest(point: Point2D, tolerance?: number): readonly Ground2DElementRef[];
  toScreen(point: ScenePosition2D): Point2D;
  toScene(point: Point2D): Point2D;
}
export interface Ground2DPlane {
  /** 2D の anchor を置く 3D の点（文書単位、f64）。 */
  readonly origin: Vector3;
  /** 2D の +x・+y の向き。直交する 0 でない向き。表は xAxis × yAxis の側。 */
  readonly xAxis: Vector3;
  readonly yAxis: Vector3;
  /** 2D の起点の座標。既定 { x: 0, y: 0 }。 */
  readonly anchor?: Point2D;
}
export interface Ground2DLink {
  /** 3D の文書識別。 */
  readonly document: DocumentIdentity;
  /** 選択を連動する 3D のビュー。省けば最初のビュー。 */
  readonly view?: string;
  /** 2D の要素から 3D の要素 ID へ。同じ建物から切り出した図面の対応を利用側が渡す。 */
  readonly to3d: (element: Ground2DElementRef) => readonly string[];
  readonly to2d: (id: string) => readonly Ground2DElementRef[];
}
export interface Ground2DOptions {
  readonly id: string;
  readonly view2d: View2DLike;
  /** View2D に渡した Canvas。写しを作らず texture にする。 */
  readonly canvas: HTMLCanvasElement | OffscreenCanvas;
  readonly plane: Ground2DPlane;
  /** 2D の 1 単位あたりの 3D の文書単位。省けば両方の単位から決める。 */
  readonly scale?: number;
  /** front は表からだけ描き選ぶ。既定 double。 */
  readonly side?: "front" | "double";
  readonly scope?: "scene" | "viewer";
  readonly priority?: number;
  /** texture の画素数と一辺の上限。既定 16,777,216 と 4096。 */
  readonly maxPixels?: number;
  readonly maxSide?: number;
  readonly link?: Ground2DLink;
}
export type Ground2DReason = "no-scene" | "no-scene-2d" | "paper" | "definition" | "unit-unknown" | "unit-mismatch" | "canvas-mismatch" | "limit";
export type Ground2DHit =
  | { readonly status: "hit"; readonly distance: number; readonly position: Position3D; readonly scene: ScenePosition2D; readonly screen: Point2D; readonly elements: readonly Ground2DElementRef[] }
  | { readonly status: "occluded"; readonly occluder: Hit3D; readonly distance: number; readonly position: Position3D; readonly scene: ScenePosition2D; readonly screen: Point2D }
  | { readonly status: "miss"; readonly reason: Ground2DReason | "removed" | "parallel" | "behind" | "back" | "clipped" | "outside";
    readonly position?: Position3D; readonly scene?: ScenePosition2D; readonly screen?: Point2D };
export interface Ground2DReport {
  readonly status: "ready" | "unavailable" | "removed";
  readonly reason: Ground2DReason | "removed" | null;
  /** 2D の 1 単位あたりの 3D の文書単位。 */
  readonly scale: number | null;
  readonly texture: { readonly width: number; readonly height: number } | null;
  /** texture の 1 画素の辺の長さ。scene は 2D の起点の単位、document は 3D の文書単位。 */
  readonly resolution: { readonly scene: number; readonly document: number } | null;
  /** 元の View2D の未描画・代替の報告。 */
  readonly issues: readonly unknown[];
  readonly link: { readonly unmatched: number; readonly failures: number; readonly firstFailure: unknown } | null;
}
export interface Ground2D {
  readonly id: string;
  readonly signal: AbortSignal;
  remove(): void;
  /** 状態を変えずに View2D の render を呼んだ後、texture の転送を求める。 */
  update(): void;
  /** CSS px の tolerance は View2D の当たり判定へ渡す。既定 3。 */
  hitTest(viewId: string, point: Point2D, options?: { readonly tolerance?: number }): Ground2DHit;
  getReport(): Ground2DReport;
}
export declare function attachGround2D(viewer: View3D, options: Ground2DOptions): Ground2D;
