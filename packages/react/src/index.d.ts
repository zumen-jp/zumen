import type { ConvertEstimateResult, ConvertResult, Diagnostic, DrawingResult, Source } from "@zumen-jp/zumen";
import type { ElementRef, ElementSource, ReviewLayer, ReviewStore, ReviewStoreEvent, ReviewStoreState, SceneInput, View2D, View2DErrorReason, View2DEvent, View2DEventType, View2DOptions, View2DState } from "@zumen-jp/view2d";
import type { ReviewSceneResult, ReviewSceneStyle } from "@zumen-jp/zumen";

export declare function useDrawingViewer(canvas: HTMLCanvasElement | OffscreenCanvas | null, options?: View2DOptions, onError?: (error: Error & { readonly reason?: View2DErrorReason }) => void): View2D | null;
export declare function useViewerState(viewer: View2D | null): View2DState | null;
export declare function useViewerEvent(viewer: View2D | null, type: View2DEventType, listener: (event: View2DEvent) => void): void;
export declare function useReviewState(store: ReviewStore | null): ReviewStoreState | null;
export declare function useReviewEvent<T extends ReviewStoreEvent["type"]>(store: ReviewStore | null, type: T, listener: (event: Extract<ReviewStoreEvent, { type: T }>) => void): void;
/** 図面と重ねる表示部品が揃う間だけ有効。store は同じ参照を渡す。 */
export declare function useReviewLayer(base: View2D | null, overlay: View2D | null, store?: ReviewStore | null): ReviewLayer | null;

export interface DrawingOptions {
  /** 呼び出し側が選んだ wasm。コンパイル済みのモジュールは複数の読み込みで共有できる。 */
  loadModule(signal: AbortSignal): WebAssembly.Module | Promise<WebAssembly.Module>;
  /** scene の起点。省略時は最初の空間か用紙。 */
  from?: string | null;
  /** describe に渡す言語カタログの JSON。 */
  catalog?: string | null;
}
export interface DrawingDiagnostic {
  stage: "open" | "drawing" | "scene" | "tables" | "convert" | "convert_estimate" | "input" | "review_scene";
  diagnostic: Diagnostic;
  text: string;
  /** 読み取り段階の明細が持つ L1 の出所。diagnostic.at と同じ順序。 */
  sources?: Source[];
}
export type DrawingState =
  | { status: "idle" | "loading" }
  | { status: "error"; error: Error; diagnostics: DrawingDiagnostic[] }
  | {
      status: "ready";
      input: SceneInput;
      drawing: DrawingResult;
      /** レイヤの公開 ID と元の名前。表を取得できなかったときは空のマップ。 */
      layerNames: ReadonlyMap<string, string>;
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
      /** この wasm が review_scene を提供する。 */
      canReview: boolean;
      /**
       * レビューの文書の対象を、開いた起点のシーンにする（review_scene）。文書は呼ぶたびに渡し、Worker は保持しない。
       * 失敗したときは、Error の diagnostics にレビューの診断を入れて拒む。
       */
      reviewScene(review: string, options: { target: string; style: ReviewSceneStyle; tolerance?: number }): Promise<ReviewSceneView>;
    };
export interface ReviewSceneView {
  input: SceneInput & { readonly result: ReviewSceneResult };
  diagnostics: DrawingDiagnostic[];
}
export interface DrawingConversion {
  bytes: Uint8Array;
  result: ConvertResult;
  diagnostics: DrawingDiagnostic[];
}
export interface DrawingConversionEstimate {
  result: ConvertEstimateResult;
  diagnostics: DrawingDiagnostic[];
  /** 読み取り段階の診断の文。対応する core でだけ返す。 */
  inputDiagnostics?: { counts: DrawingDiagnostic[]; details: DrawingDiagnostic[] };
}
export declare function useDrawing(file: Blob | null, options: DrawingOptions): DrawingState;
