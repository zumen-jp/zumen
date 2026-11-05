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
  /** 選択・強調・非表示レイヤの合計、購読・当たり判定・独自ツール・重ね描きの各件数。既定 100,000。 */
  stateItems: number;
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
  /** 選択と hover の色。既定 #ff8800・#00aacc。 */
  selectionColor?: string;
  hoverColor?: string;
  /** 計測線と選択範囲の色。既定 #cc0066。ラベルは利用側で描く。 */
  overlayColor?: string;
  /** 用紙 mm の線幅を CSS px にする倍率。既定 96 / 25.4。ズームには連動しない。 */
  pixelsPerMillimeter?: number;
  fallbackFont?: "sans-serif" | "serif" | "monospace";
  limits?: Partial<Limits>;
}
export interface ElementRef {
  readonly id: string;
  readonly path: readonly { readonly id: string; readonly column: number; readonly row: number }[];
}
export interface Highlight {
  readonly name: string;
  readonly color: string;
  readonly elements: readonly ElementRef[];
}
/** 起点の座標を原点と相対座標に分け、大きな原点を足す丸めを避ける。 */
export interface ScenePosition { readonly origin: Point; readonly offset: Point; readonly element?: ElementRef }
export interface Snap {
  readonly position: ScenePosition;
  readonly screen: Point;
  readonly kind: "vertex" | "point";
  /** CSS px。 */
  readonly distance: number;
  readonly tolerance: number;
}
export type RealDistance = { readonly status: "known"; readonly distance: number; readonly unit: "millimeter" | "centimeter" | "meter" | "inch" | "foot" }
  | { readonly status: "unknown"; readonly reason: "paper" | "definition" | "unit-unknown" | "placement-scale" | "unbound" };
/** シーン上の距離。tolerance はコアの展開時の許容差で、実寸の精度保証ではない。 */
export interface Measurement {
  readonly start: ScenePosition;
  readonly end: ScenePosition;
  readonly delta: Point;
  readonly distance: number;
  readonly from: string;
  readonly view?: SceneResult["view"];
  readonly unit: SceneResult["unit"];
  readonly tolerance: number;
  readonly real: RealDistance;
}
export interface InputOptions {
  pointer: boolean;
  touch: boolean;
  drag: boolean;
  pinch: boolean;
  wheel: boolean;
  keyboard: boolean;
  doubleClick: boolean;
  /** 既定 ctrlOrMeta。none は修飾キーなしでもホイールを受ける。 */
  wheelModifier: "ctrlOrMeta" | "none";
}
export interface InputModifiers { readonly shiftKey?: boolean; readonly ctrlKey?: boolean; readonly altKey?: boolean; readonly metaKey?: boolean }
/** DOM に依らない共通の入口。座標とホイール量は CSS px。 */
export type ToolInput = InputModifiers & (
  { readonly type: "pointerdown" | "pointermove" | "pointerup"; readonly point: Point; readonly pointerId: number; readonly pointerType?: "mouse" | "pen" | "touch"; readonly button?: number }
  | { readonly type: "wheel"; readonly point: Point; readonly deltaY: number }
  | { readonly type: "keydown"; readonly key: string }
  | { readonly type: "dblclick"; readonly point: Point }
  | { readonly type: "cancel"; readonly reason?: string }
);
export interface Gesture { readonly type: "select" | "pan" | "measure" | "pinch" | "custom"; readonly start: Point; readonly end: Point }
export type Overlay = (context: CanvasRenderingContext2D | OffscreenCanvasRenderingContext2D, state: View2DState) => void;
export interface ToolContext {
  readonly viewer: View2D;
  /** 中断・変更・差し替え・破棄で abort。次の入力では新しい有効期間になる。 */
  readonly signal: AbortSignal;
  /** このツールの有効期間に結ぶ購読と重ね描き。 */
  subscribe(listener: (state: View2DState) => void): () => void;
  on(type: View2DEventType, listener: (event: View2DEvent) => void): () => void;
  addOverlay(draw: Overlay): () => void;
}
export interface ToolDefinition {
  /** 最初の入力で呼ぶ。返した関数は有効期間の終わりに呼ぶ。 */
  onActivate?(context: ToolContext): void | (() => void);
  /** true なら入力を消費し、DOM の既定の動作を止める。 */
  onInput(input: ToolInput, context: ToolContext): boolean | void;
  onCancel?(reason: string, context: ToolContext): void;
}
export interface View2DState {
  readonly scene: SceneSnapshot | null;
  readonly view: Camera;
  readonly size: Size;
  readonly layers: readonly string[];
  readonly hiddenLayers: readonly string[];
  readonly selection: readonly ElementRef[];
  readonly hovered: ElementRef | null;
  readonly highlights: readonly Highlight[];
  readonly tool: string;
  readonly input: Readonly<InputOptions>;
  readonly gesture: Gesture | null;
  readonly measurement: Measurement | null;
}
export type View2DEventType = "scene" | "view" | "size" | "layers" | "selection" | "hover" | "highlight" | "tool" | "input" | "measurement";
export interface View2DEvent { readonly type: View2DEventType; readonly state: View2DState }
export type SelectionMode = "replace" | "add" | "remove" | "toggle";
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
  /** 状態が変わるまでは同じ凍結した参照。Scene の列は共有する。 */
  getState(): View2DState;
  getLayers(): readonly string[];
  /** 登録時には呼ばず、更新後に同期で通知する。返す関数で解除。 */
  subscribe(listener: (state: View2DState) => void): () => void;
  on(type: View2DEventType, listener: (event: View2DEvent) => void): () => void;
  setHiddenLayers(ids: readonly string[]): RenderReport;
  setLayersVisible(ids: readonly string[], visible: boolean): RenderReport;
  /** 現在のシーンの ID と配置の道筋。未知の参照は RangeError。 */
  select(elements: readonly ElementRef[], mode?: SelectionMode): RenderReport;
  setHovered(element: ElementRef | null): RenderReport;
  /** 同じ名前を置き換え、空の一覧で除く。色の既定は #cc00cc。 */
  setHighlight(name: string, elements: readonly ElementRef[], color?: string): RenderReport;
  /** CSS px。既定の許容半径は 3。可視の形と円が触れる配置を、描画の逆順で返す。 */
  hitTest(point: Point, tolerance?: number): readonly ElementRef[];
  /** CSS px の四角と可視の形が触れる配置。代替文字は外形の四角で近似する。 */
  hitTestRect(rect: Extent): readonly ElementRef[];
  /** シーン上の位置を原点と相対座標に分けて返す。要素への吸着はしない。 */
  positionAt(point: Point): ScenePosition;
  /** 可視の線・面の頂点と点の中心。既定の許容半径は 8 CSS px。 */
  snapAt(point: Point, tolerance?: number): Snap | null;
  setMeasurement(value: { readonly start: ScenePosition; readonly end: ScenePosition } | null): RenderReport;
  /** select・pan・measure か、addTool で登録した名前。 */
  setTool(name: string): RenderReport;
  addTool(name: string, definition: ToolDefinition): () => void;
  /** 省略した設定を保つ。変更時には操作を中断する。 */
  setInputOptions(options: Partial<InputOptions>): RenderReport;
  /** 明示的に DOM へ接続する。touch-action・tabindex・CSS は利用側で指定する。 */
  attachInput(element?: HTMLElement, options?: Partial<InputOptions>): () => void;
  /** 処理した入力なら true。OffscreenCanvas でも使える。 */
  handleInput(input: ToolInput): boolean;
  cancelInput(reason?: string): RenderReport;
  /** CSS px のコンテキストを渡す。既定の scope は scene。描画中は状態を変更しない。 */
  addOverlay(draw: Overlay, options?: { scope?: "scene" | "viewer" }): () => void;
  invalidate(): RenderReport;
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
  toScreen(point: Point | ScenePosition): Point;
  toScene(point: Point): Point;
  resize(width: number, height: number, pixelRatio?: number): RenderReport;
  /** 冪等。ほかのメソッドは破棄後に Error を返す。 */
  destroy(): void;
}
