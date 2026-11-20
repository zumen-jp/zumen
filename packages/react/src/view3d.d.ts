import type { Diagnostic, ModuleName } from "@zumen-jp/zumen";
import type { InputOptions, ReviewPlace3D, Scene3DInput, View3D, View3DErrorReason, View3DEvent, View3DOptions, View3DState } from "@zumen-jp/view3d";

/** Canvas の接続後に View3D を返し、Canvas・設定の差し替えとアンマウントで破棄する。 */
export declare function useView3D(canvas: HTMLCanvasElement | OffscreenCanvas | null, options?: View3DOptions, onError?: (error: Error & { readonly reason?: View3DErrorReason }) => void): View3D | null;
/** attachInput を要素につなぎ、表示部品・要素・設定の差し替えとアンマウントで外す。設定は同じ参照を渡す。 */
export declare function useView3DInput(viewer: View3D | null, element: HTMLElement | null, options?: InputOptions): void;
export declare function useView3DState(viewer: View3D | null): View3DState | null;
export declare function useView3DEvent<T extends View3DEvent["type"]>(viewer: View3D | null, type: T, listener: (event: Extract<View3DEvent, { type: T }>) => void): void;

export interface Scene3DOptions {
  /** 呼び出し側が選んだ wasm。name は入力の形式の scene3d を持つモジュール（bim・civil・pointcloud）。 */
  loadModule(name: ModuleName, signal: AbortSignal): WebAssembly.Module | Promise<WebAssembly.Module>;
  /** describe に渡す言語カタログの JSON。 */
  catalog?: string | null;
  /** scene3d の曲面の許容差（文書単位）。省略時はコアの既定。 */
  tolerance?: number;
}
export interface Scene3DDiagnostic {
  stage: "open" | "building" | "scene3d" | "building_elements" | "bcf_write" | "bcf_read";
  diagnostic: Diagnostic;
  text: string;
}
/** 一覧の要素。シーンの行の順で最初の 1000 件。 */
export interface Scene3DListItem { readonly id: string; readonly class: string; readonly name: string }
export type Scene3DKind = "building" | "feature" | "pointcloud";
export interface BcfWriteOptions {
  /** 指摘の題。 */
  title: string;
  /** toReviewPlace の place。 */
  place: ReviewPlace3D;
  author: string;
  /** ISO 8601 の日時。 */
  date: string;
  /** 省略時は 3.0。 */
  version?: "2.1" | "3.0";
}
export interface BcfLocation { readonly topic: string; readonly location: ReviewPlace3D & { readonly id: string; readonly target: string } }
export type Scene3DState =
  | { status: "idle" | "loading" }
  | { status: "error"; error: Error & { format?: string | null }; diagnostics: Scene3DDiagnostic[] }
  | {
      status: "ready";
      input: Scene3DInput;
      kind: Scene3DKind;
      module: ModuleName;
      elements: readonly Scene3DListItem[];
      /** 要素の総数。 */
      total: number;
      /** 塊の原点と範囲から求めた文書の座標の範囲。塊がなければ null。 */
      bounds: { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] } | null;
      diagnostics: Scene3DDiagnostic[];
      /** IFC の要素の名前・クラス・空間・親・プロパティ（building_elements）。IFC 以外と、ない ID は null。 */
      getElement(id: string): Promise<Record<string, unknown> | null>;
      /** IFC だけ。開いたモデルを対象にした BCF のバイト列を返す。保存は利用側が明示操作で行う。 */
      writeBcf: ((options: BcfWriteOptions) => Promise<{ bytes: Uint8Array; diagnostics: Scene3DDiagnostic[] }>) | null;
      /** IFC だけ。BCF を開いたモデルに当てて読み、レビュー文書の 3D の場所を返す。 */
      readBcf: ((bytes: ArrayBuffer | Uint8Array) => Promise<{ locations: BcfLocation[]; diagnostics: Scene3DDiagnostic[] }>) | null;
    };
export declare function useScene3D(file: Blob | null, options: Scene3DOptions): Scene3DState;
