import type { Budget, Extent, ReviewSceneResult, ReviewSceneSource, Response, Scene, SceneCounts, SceneResult, Session, Source } from "@zumen-jp/zumen";

export interface Point { readonly x: number; readonly y: number }
/** 起点の座標の中心、CSS px / 起点の長さ、反時計回りのラジアン。 */
export interface Camera { readonly center: Point; readonly scale: number; readonly rotation: number }
/** カメラと入力が受け付ける拡大率の両端。 */
export declare const VIEW_SCALE_LIMITS: Readonly<{ min: number; max: number }>;
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
  /** 利用側へ渡す、1 資源の入力と復号のバイト数の上限。既定 64 MiB。利用側が確保前に数える。 */
  underlayBytes: number;
  /** 保持した画像と未完了の描画要求の画素数の合計。既定 16,777,216。 */
  underlayPixels: number;
  /** 要求する画像の一辺の上限。既定 8192。 */
  underlaySide: number;
  /** 未完了の解決・描画の数。中断して未完了のものも数える。既定 4。 */
  underlayJobs: number;
  /** 解決中・保持中・中断して未解放の資源の数。既定 64。 */
  underlayResources: number;
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
  underlayRenderer?: UnderlayRenderer;
  /** 共有する WebGPU デバイス。省略時は既定の共有デバイスを使う。 */
  device?: GPUDevice;
}
export interface ElementRef {
  readonly id: string;
  readonly path: readonly { readonly id: string; readonly column: number; readonly row: number }[];
}
export interface ElementSource extends ElementRef { readonly source: Source | null }
/** 表示した版を確かめ、配置の道筋を保って entities の出所へたどる。 */
export declare function findSource(session: Session, document: DocumentIdentity, element: ElementRef, maxPages?: number): ElementSource | null;
export interface MeasureDistanceOptions { readonly budget?: Budget; readonly max_places?: number }
/** コアの距離計測の応答。新しい生成型がない公開版の型とも組み合わせられる構造を使う。 */
export interface CalibratedDistanceResult {
  readonly hash: string;
  readonly from: string;
  readonly calibration: string;
  readonly distance: number;
  readonly unit: SceneResult["unit"];
  readonly unit_label: string;
}
/** 分割座標と文書の hash をコアへ渡す。診断と予算停止を含む応答をそのまま返す。 */
export declare function measureDistance(session: Session, document: DocumentIdentity, calibration: string,
  measurement: Pick<Measurement, "from" | "start" | "end">, options?: MeasureDistanceOptions): Response<CalibratedDistanceResult>;
export interface UnderlayDescriptor {
  readonly index: number;
  readonly element: ElementRef;
  readonly document: DocumentIdentity;
  readonly kind: "image" | "pdf" | "dwf" | "dgn" | "unknown";
  /** 元の参照名とページ名。URL として取得せず、ページ番号へ変換しない。 */
  readonly name: string;
  readonly page: string;
}
export interface UnderlayRequest extends UnderlayDescriptor {
  readonly signal: AbortSignal;
  readonly limits: Readonly<{ maxBytes: number; maxPixels: number; maxSide: number }>;
}
export interface UnderlayRenderRequest extends UnderlayRequest {
  /** 左下原点・Y が上の内容の単位。corners は内容全体を枠へ、unit は内容の 1 単位を辺へ写す。 */
  readonly region: Readonly<Extent>;
  /** 返す画像の画素数。pixelRatio を反映済みで、この寸法と一致させる。 */
  readonly width: number;
  readonly height: number;
}
export interface UnderlayRaster {
  /** 左上原点の画素。渡した後は、dispose まで変更・転送・close しない。 */
  readonly source: ImageBitmap | HTMLCanvasElement | OffscreenCanvas;
  /** 所有権を移す。古い結果にも 1 回呼ぶ。共有資源の参照数は利用側で扱う。 */
  dispose(): void | Promise<void>;
}
export interface UnderlayResource {
  /** ページ・画像の内容の単位での寸法。PDF の単位やページ名の対応は利用側で決める。 */
  readonly width: number;
  readonly height: number;
  render(request: UnderlayRenderRequest): UnderlayRaster | Promise<UnderlayRaster>;
  /** 中断した描画の完了を待ち、その画像を解放してから 1 回呼ぶ。 */
  dispose(): void | Promise<void>;
}
/** 必ず同期通知の外で呼ぶ。利用側は上限と signal を守り、URL やページ名を明示して解く。 */
export type UnderlayRenderer = (request: UnderlayRequest) => UnderlayResource | Promise<UnderlayResource>;
export type UnderlayStatus = "ready" | "resource-unresolved" | "resource-pending" | "resource-cancelled" | "resource-outside-view"
  | "resource-limit" | "resource-invalid" | "resource-resolve-failed" | "resource-render-failed" | "resource-release-failed"
  | "resource-kind-unsupported" | "resource-frame-invalid" | "clip-unsupported";
export interface UnderlayState extends UnderlayDescriptor {
  readonly status: UnderlayStatus;
  readonly width?: number;
  readonly height?: number;
}
export interface Highlight {
  readonly name: string;
  readonly color: string;
  readonly elements: readonly ElementRef[];
}
/** 起点の座標を原点と相対座標に分け、大きな原点を足す丸めを避ける。 */
export interface ScenePosition { readonly origin: Point; readonly offset: Point; readonly element?: ElementRef }
export type SnapKind = "vertex" | "point" | "midpoint" | "center" | "quadrant"
  | "intersection" | "perpendicular" | "nearest" | "insertion";
/** シーンと同じ座標・尺度へ配置した元の形。element は現在のシーンの可視要素の参照。 */
export type SnapGeometry = { readonly shape: "circle"; readonly element: ElementRef; readonly center: ScenePosition; readonly radius: number }
  | { readonly shape: "arc"; readonly element: ElementRef; readonly center: ScenePosition; readonly radius: number;
    readonly startAngle: number; readonly endAngle: number }
  | { readonly shape: "insertion"; readonly element: ElementRef; readonly position: ScenePosition };
export interface SnapOptions {
  /** 省略時は vertex と point。 */
  readonly kinds?: readonly SnapKind[];
  /** 垂線の足を求める起点。perpendicular を選ぶ場合に必須。 */
  readonly reference?: ScenePosition;
  /** 円・円弧の中心と四半円点、記号の基点。角度は度で反時計回り。 */
  readonly geometry?: readonly SnapGeometry[];
}
export interface Snap {
  readonly position: ScenePosition;
  readonly screen: Point;
  readonly kind: SnapKind;
  /** CSS px。 */
  readonly distance: number;
  readonly tolerance: number;
}
export type RealDistance = { readonly status: "known"; readonly distance: number; readonly unit: "millimeter" | "centimeter" | "meter" | "inch" | "foot" }
  | { readonly status: "unknown"; readonly reason: "paper" | "definition" | "unit-unknown" | "placement-scale" | "unbound" };
export type CalibratedDistance = { readonly status: "measured"; readonly calibration: string; readonly from: string;
  readonly distance: number; readonly unit: CalibratedDistanceResult["unit"]; readonly unit_label: string }
  | { readonly status: "unavailable"; readonly calibration: string;
    readonly reason: "outside-bounds" | "sheet-mismatch" | "overflow" | "calibration-missing" | "document-mismatch" | "stopped" | "failed" };
export interface MeasurementCalibration {
  readonly id: string;
  /** 同期だけ。成功した元ラベルは 4096 文字まで。解決中は View2D の状態を変更しない。 */
  readonly resolve: (measurement: Measurement, document: DocumentIdentity) => Response<CalibratedDistanceResult>;
}
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
  /** 明示的に校正を選んだときだけ持つ。紙上の距離と real は変更しない。 */
  readonly calibrated?: CalibratedDistance;
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
/** WebGPU 描画パスへ直接図形を発行する重ね描き用の 2D コンテキスト。 */
export interface OverlayContext {
  strokeStyle: string;
  fillStyle: string;
  lineWidth: number;
  lineCap: "butt" | "round" | "square";
  lineJoin: "round" | "bevel" | "miter";
  globalAlpha: number;
  font: string;
  textAlign: "left" | "right" | "center" | "start" | "end";
  textBaseline: "top" | "hanging" | "middle" | "alphabetic" | "ideographic" | "bottom";
  save(): void;
  restore(): void;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void;
  setTransform(transform?: DOMMatrix2DInit): void;
  resetTransform(): void;
  transform(a: number, b: number, c: number, d: number, e: number, f: number): void;
  translate(x: number, y: number): void;
  rotate(angle: number): void;
  scale(x: number, y: number): void;
  getTransform(): DOMMatrix | { a: number; b: number; c: number; d: number; e: number; f: number };
  setLineDash(segments: readonly number[]): void;
  getLineDash(): number[];
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  closePath(): void;
  rect(x: number, y: number, w: number, h: number): void;
  arc(x: number, y: number, radius: number, startAngle: number, endAngle: number, counterclockwise?: boolean): void;
  stroke(): void;
  fill(fillRule?: "nonzero" | "evenodd"): void;
  fillRect(x: number, y: number, w: number, h: number): void;
  strokeRect(x: number, y: number, w: number, h: number): void;
  fillText(text: string, x: number, y: number): void;
  drawImage(source: ImageBitmap | HTMLCanvasElement | OffscreenCanvas, dx: number, dy: number, dw?: number, dh?: number): void;
}
export type Overlay = (context: OverlayContext, state: View2DState) => void;
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
  readonly underlays: readonly UnderlayState[];
  readonly contextLost: boolean;
}
export type View2DEventType = "scene" | "view" | "size" | "layers" | "selection" | "hover" | "highlight" | "tool" | "input" | "measurement" | "underlay" | "contextlost";
export interface View2DEvent { readonly type: View2DEventType; readonly state: View2DState }
export type SelectionMode = "replace" | "add" | "remove" | "toggle";
export type RenderReason = "browser-text" | "text-layout-unsupported" | "marker-unresolved" | "point-style-unsupported" | "color-unresolved" | Exclude<UnderlayStatus, "ready">;
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
export type View2DErrorReason = "webgpu-unavailable" | "context-lost";
export interface Snapshot2D {
  /** デバイスの画素。 */
  readonly width: number;
  readonly height: number;
  readonly pixelRatio: number;
  /** RGBA、上の行から。 */
  readonly pixels: Uint8Array;
}
export declare class View2D {
  /** WebGPU を初期化し、Canvas に描く View2D を作る。WebGPU を使えないときは reason が webgpu-unavailable の例外。 */
  static create(canvas: HTMLCanvasElement | OffscreenCanvas, options?: View2DOptions): Promise<View2D>;
  private constructor();
  /** 検査に失敗すれば RangeError。以前のシーンと画面を保つ。 */
  setScene(input: SceneInput | null): RenderReport;
  getScene(): SceneSnapshot | null;
  getView(): Camera;
  getSize(): Size;
  getReport(): RenderReport;
  /** 状態が変わるまでは同じ凍結した参照。Scene の列は共有する。 */
  getState(): View2DState;
  /** 資源と進行中の要求を解放し、現在のシーンを新しい関数で解決する。null は停止。 */
  setUnderlayRenderer(renderer: UnderlayRenderer | null): RenderReport;
  /** 失敗と中断を解除し、資源を解決し直す。 */
  refreshUnderlays(): RenderReport;
  /** 再読込かシーン・関数の差し替えまで停止する。 */
  cancelUnderlays(): RenderReport;
  getLayers(): readonly string[];
  /** シーンの要素行の順。選択と同じ ID と配置の道筋を持つ読み取り専用の一覧。 */
  getElements(): readonly ElementRef[];
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
  /** 指定した種類の吸着。既定は頂点と点の中心、許容半径は 8 CSS px。 */
  snapAt(point: Point, tolerance?: number, options?: SnapOptions): Snap | null;
  setMeasurement(value: { readonly start: ScenePosition; readonly end: ScenePosition } | null): RenderReport;
  /** 明示的な ID と同期の解決関数。null で解除。シーンの差し替え・消去・破棄でも解除する。 */
  setMeasurementCalibration(value: MeasurementCalibration | null): RenderReport;
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
  /** 描画済みの画面から device px の RGBA 画素を非同期に読み出す。 */
  snapshot(): Promise<Snapshot2D>;
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

/** レビューの文書の項目の参照。場所は指摘の ID と組にする */
export type ReviewRef =
  | { readonly kind: "markup"; readonly id: string }
  | { readonly kind: "location"; readonly topic: string; readonly id: string };
export type ReviewJson = null | boolean | number | string | readonly ReviewJson[] | { readonly [key: string]: ReviewJson };
export type ReviewMarkupKind = "redline" | "cloud" | "text" | "photo";
export type ReviewStatus = "open" | "resolved" | "closed";
export interface ReviewMarkup {
  readonly id: string; readonly kind: ReviewMarkupKind; readonly target: string; readonly view?: string;
  readonly points: readonly (readonly [number, number])[]; readonly text?: string; readonly topic?: string;
  readonly [key: string]: unknown;
}
export interface ReviewLocation {
  readonly id: string; readonly target: string; readonly kind: "2d" | "3d"; readonly view?: string;
  readonly pin?: readonly [number, number]; readonly extent?: readonly [number, number, number, number];
  readonly [key: string]: unknown;
}
export interface ReviewTopic {
  readonly id: string; readonly title: string; readonly status: ReviewStatus; readonly locations?: readonly ReviewLocation[];
  readonly [key: string]: unknown;
}
export interface ReviewDocument {
  readonly format: "zumen-review"; readonly version: number;
  readonly targets?: readonly { readonly id: string; readonly [key: string]: unknown }[];
  readonly topics?: readonly ReviewTopic[]; readonly markups?: readonly ReviewMarkup[];
  readonly [key: string]: unknown;
}
type ReviewNullable<T> = { [K in keyof T]?: T[K] | null };
export interface ReviewMarkupInput {
  id?: string; kind: ReviewMarkupKind; target: string; view?: string; points: readonly (readonly [number, number])[];
  text?: string; topic?: string;
}
export type ReviewMarkupPatch = ReviewNullable<Omit<ReviewMarkupInput, "id">>;
export interface ReviewTopicInput {
  id?: string; title: string; status: ReviewStatus; description?: string; status_label?: string; kind?: string;
  priority?: string; labels?: readonly string[]; assignee?: string; due?: string;
}
export type ReviewTopicPatch = ReviewNullable<Omit<ReviewTopicInput, "id">>;
export interface ReviewLocationInput {
  id?: string; kind?: "2d"; target: string; view?: string; pin?: readonly [number, number];
  extent?: readonly [number, number, number, number];
}
export type ReviewLocationPatch = ReviewNullable<Omit<ReviewLocationInput, "id" | "kind">>;
export interface ReviewFilter {
  targets?: readonly string[]; views?: readonly string[]; kinds?: readonly (ReviewMarkupKind | "location")[];
  topics?: readonly string[]; statuses?: readonly ReviewStatus[];
}
export interface ReviewItem {
  readonly ref: ReviewRef; readonly item: ReviewMarkup | ReviewLocation; readonly topic?: ReviewTopic;
  /** review_scene の sources の id と同じ */
  readonly sceneId: string;
}
export interface ReviewStoreState {
  readonly document: ReviewDocument; readonly selection: readonly ReviewRef[]; readonly filter: Readonly<ReviewFilter>;
  readonly revision: number; readonly canUndo: boolean; readonly canRedo: boolean;
}
export type ReviewChangeReason = "add" | "update" | "remove" | "batch" | "undo" | "redo" | "load";
export type ReviewStoreEvent =
  | { readonly type: "document"; readonly state: ReviewStoreState; readonly change: { readonly reason: ReviewChangeReason; readonly items: readonly ReviewRef[] } }
  | { readonly type: "selection" | "filter" | "history"; readonly state: ReviewStoreState };
export interface ReviewStoreOptions {
  /** 新しい項目の ID。省略すると、ID を渡さない追加は RangeError */
  createId?: () => string;
  /** 指摘の作者と時刻。省略すると付けない */
  stamp?: () => { author?: string; time?: string };
  limits?: { history?: number; items?: number };
}
/** レビューの文書の編集の状態。描画はコアの review_scene に任せ、文書をどこへも送らない */
export declare class ReviewStore {
  constructor(document: string | ReviewDocument, options?: ReviewStoreOptions);
  getState(): ReviewStoreState;
  getDocument(): ReviewDocument;
  subscribe(listener: (state: ReviewStoreState) => void): () => void;
  on<T extends ReviewStoreEvent["type"]>(type: T, listener: (event: Extract<ReviewStoreEvent, { type: T }>) => void): () => void;
  toJSON(): string;
  /** 絞り込みに合う図形と 2D の場所だけを残した文書。review_scene に渡す */
  filteredJSON(): string;
  load(document: string | ReviewDocument): void;
  batch<T>(fn: (store: this) => T): T;
  addMarkup(markup: ReviewMarkupInput): string;
  updateMarkup(id: string, patch: ReviewMarkupPatch): void;
  removeMarkup(id: string): void;
  addTopic(topic: ReviewTopicInput): string;
  updateTopic(id: string, patch: ReviewTopicPatch): void;
  removeTopic(id: string): void;
  addLocation(topic: string, location: ReviewLocationInput): string;
  updateLocation(topic: string, id: string, patch: ReviewLocationPatch): void;
  removeLocation(topic: string, id: string): void;
  undo(): boolean;
  redo(): boolean;
  select(refs: readonly ReviewRef[], mode?: "replace" | "add" | "remove" | "toggle"): void;
  setFilter(filter: ReviewFilter): void;
  items(): readonly ReviewItem[];
}
export declare function reviewReference(source: ReviewSceneSource): ReviewRef;
export declare function reviewSceneId(ref: ReviewRef): string;
/** review_scene のシーンを、背景を透明にした別の View2D で図面に重ねる。表示範囲と大きさは図面に合わせる */
export declare class ReviewLayer {
  /** store を渡すと、その選択を重ねたシーンの選択に写す */
  constructor(base: View2D, overlay: View2D, store?: ReviewStore | null);
  /** review_scene の結果を重ねた View2D に描く。null で消す */
  setResult(input: (SceneInput & { readonly result: ReviewSceneResult }) | null): RenderReport;
  /** Canvas 左上からの CSS px の位置にある文書の項目。上に描いたものから返す */
  pick(point: Point, tolerance?: number): readonly ReviewRef[];
  sourceOf(sceneId: string): ReviewSceneSource | null;
  /** 購読を外す。2 つの View2D は利用側が破棄する。冪等 */
  destroy(): void;
}
