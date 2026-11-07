import type { Diagnostic, DrawingResult } from "@zumen-jp/zumen";
import type { ElementRef, ElementSource, SceneInput, View2D, View2DEvent, View2DEventType, View2DOptions, View2DState } from "@zumen-jp/view2d";

export declare function useDrawingViewer(canvas: HTMLCanvasElement | OffscreenCanvas | null, options?: View2DOptions): View2D | null;
export declare function useViewerState(viewer: View2D | null): View2DState | null;
export declare function useViewerEvent(viewer: View2D | null, type: View2DEventType, listener: (event: View2DEvent) => void): void;

export interface DrawingOptions {
  /** 呼び出し側が選んだ wasm。コンパイル済みのモジュールは複数の読み込みで共有できる。 */
  loadModule(signal: AbortSignal): WebAssembly.Module | Promise<WebAssembly.Module>;
  /** scene の起点。省略時は最初の空間か用紙。 */
  from?: string | null;
  /** describe に渡す言語カタログの JSON。 */
  catalog?: string | null;
}
export interface DrawingDiagnostic {
  stage: "open" | "drawing" | "scene";
  diagnostic: Diagnostic;
  text: string;
}
export type DrawingState =
  | { status: "idle" | "loading" }
  | { status: "error"; error: Error; diagnostics: DrawingDiagnostic[] }
  | {
      status: "ready";
      input: SceneInput;
      drawing: DrawingResult;
      diagnostics: DrawingDiagnostic[];
      getSource(element: ElementRef): Promise<ElementSource | null>;
    };
export declare function useDrawing(file: Blob | null, options: DrawingOptions): DrawingState;
