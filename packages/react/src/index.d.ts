import type { ConvertEstimateResult, ConvertResult, Diagnostic, DrawingResult } from "@zumen-jp/zumen";
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
  stage: "open" | "drawing" | "scene" | "convert" | "convert_estimate";
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
      /** この wasm が convert_estimate を提供する。未対応のバージョンでは false。 */
      canEstimate: boolean;
      diagnostics: DrawingDiagnostic[];
      getSource(element: ElementRef): Promise<ElementSource | null>;
      /** 保存前に忠実度と欠落を調べる。文書やファイルは登録しない。limit は明細数（省略時 100、最大 1000）。 */
      estimate(options?: { limit?: number }): Promise<DrawingConversionEstimate>;
      /**
       * 開いた図面を DXF に変換する（convert、決定 0206）。bytes は書いた DXF、diagnostics は変換の診断（stage は convert）。
       * 失敗したときは、Error の diagnostics に変換の診断を入れて拒む。
       */
      convert(preview?: DrawingConversionEstimate): Promise<DrawingConversion>;
    };
export interface DrawingConversion {
  bytes: Uint8Array;
  result: ConvertResult;
  diagnostics: DrawingDiagnostic[];
}
export interface DrawingConversionEstimate {
  result: ConvertEstimateResult;
  diagnostics: DrawingDiagnostic[];
}
export declare function useDrawing(file: Blob | null, options: DrawingOptions): DrawingState;
