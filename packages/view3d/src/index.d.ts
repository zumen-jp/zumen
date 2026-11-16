import type { Bytes, Scene3dCounts, Scene3dMap, Scene3dResult } from "@zumen-jp/zumen";

/** 検査を通した three.js の r 版。 */
export declare const THREE_REVISION: "187";

export type View3DErrorReason = "invalid-input" | "invalid-scene" | "limit" | "three-version" | "webgl2-unavailable"
  | "destroyed" | "no-scene" | "unknown-element" | "other-document" | "unknown-view" | "outside-view" | "aspect-ratio" | "context-lost" | "reentrant";
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
/** 要素の属性。storey は所属階の要素 ID。 */
export interface Element3D { readonly id: string; readonly class: string; readonly name: string; readonly storey: string | null; readonly row: number }
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
  readonly scene: {
    readonly unit: Scene3dResult["unit"]; readonly tolerance: number; readonly counts: Readonly<Required<Scene3dCounts>>;
    /** 文書の座標から地図の座標への変換。持たない文書は null。 */
    readonly map: Readonly<Scene3dMap> | null;
    /** 高さの基準の EPSG。分からなければ null。 */
    readonly heightCrs: number | null;
  } | null;
  readonly views: readonly View3DViewState[];
  /** 登録した追加描画。three.js のオブジェクトは含めない。 */
  readonly objects: readonly ExtraState[];
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
/** 追加描画の当たり。幾何の記述は part、利用側の Object3D は当たった節を object に持つ。 */
export interface ExtraHit {
  readonly kind: "geometry" | "object";
  readonly id: string;
  readonly priority: number;
  readonly part?: number;
  readonly object?: object;
  readonly distance: number;
  readonly position: Position3D;
}
export type Hit3D = ElementHit | ExtraHit;
/** 距離、優先度（大きいほう、要素は 0）、種類（要素・幾何の記述・Object3D）、要素の行か ID、部分の順。 */
export interface HitTestResult { readonly hits: readonly Hit3D[]; readonly truncated: boolean }
/** 長さの単位。シーンの unit と name・prefix・scale が同じもの。 */
export interface LengthUnit { readonly name: string; readonly prefix?: string; readonly scale?: number }
export interface ExtraOptions {
  /** 利用側の ID。コアの要素 ID とは kind で区別する。 */
  readonly id: string;
  /** f64 の原点（文書単位）。 */
  readonly origin: Vector3;
  /** 局所の変換。列優先の 16 値で、アフィンで逆を持つもの。 */
  readonly matrix?: ArrayLike<number>;
  readonly unit: LengthUnit;
  /** scene はシーンの差し替えで外す。viewer は残し、単位の違うシーンでは外す。既定 scene。 */
  readonly scope?: "scene" | "viewer";
  /** 同じ距離の当たりの順。既定 0。 */
  readonly priority?: number;
  readonly pickable?: boolean;
  /** 線・点の当たりの許容差（CSS px）。既定 3。 */
  readonly pickTolerance?: number;
}
export interface ObjectOptions extends ExtraOptions {
  /** 材質が切断面に従うか。false の物は切断面のあるビューで描かず選ばない。既定 true。 */
  readonly clipping?: boolean;
  /** 登録を外すときに一度だけ呼ぶ。部品は借用した資源を解放しない。 */
  readonly dispose?: () => void;
}
export type ImageSource = ImageData | ImageBitmap | HTMLCanvasElement | OffscreenCanvas | HTMLImageElement;
export interface LabelSpec {
  /** 描画済みの字形か画像。部品は写しを作り、URL やフォントを取りに行かない。 */
  readonly image: ImageSource;
  /** CSS px。 */
  readonly width: number;
  readonly height: number;
  /** 位置に合わせる画像の点。0〜1、左下が [0, 0]。既定 [0.5, 0.5]。 */
  readonly anchor?: readonly [number, number];
}
/** 位置は登録の局所の座標（文書単位）。 */
export type GeometryItem =
  | { readonly kind: "line"; readonly points: readonly Vector3[]; readonly closed?: boolean; readonly color?: string | number }
  | { readonly kind: "region"; readonly points: readonly Vector3[]; readonly color?: string | number; readonly opacity?: number; readonly tolerance?: number }
  | { readonly kind: "circle"; readonly center: Vector3; readonly radius: number; readonly normal?: Vector3; readonly tolerance?: number; readonly filled?: boolean; readonly color?: string | number; readonly opacity?: number }
  | { readonly kind: "dimension"; readonly from: Vector3; readonly to: Vector3; readonly offset?: Vector3; readonly color?: string | number; readonly label?: LabelSpec }
  | ({ readonly kind: "label"; readonly position: Vector3 } & LabelSpec);
export interface GeometryPart { readonly part: number; readonly kind: GeometryItem["kind"]; readonly length?: number; readonly segments?: number }
export interface Registration {
  readonly id: string;
  /** 何回呼んでも後始末を重ねない。 */
  remove(): void;
  /** 解除で abort する。reason は removed・scene・unit・destroy。 */
  readonly signal: AbortSignal;
}
export interface GeometryRegistration extends Registration { readonly parts: readonly GeometryPart[] }
export interface ExtraState {
  readonly id: string; readonly kind: "geometry" | "object"; readonly scope: "scene" | "viewer";
  readonly priority: number; readonly pickable: boolean; readonly clipping: boolean;
}
export interface InputOptions {
  /** クリック・タップで選ぶ。 */
  readonly select?: boolean;
  /** 中ボタンで移動。 */
  readonly pan?: boolean;
  /** Shift と中ボタンで注視点の周りに回す。 */
  readonly orbit?: boolean;
  /** Ctrl・Command とホイールで拡大。 */
  readonly zoom?: boolean;
  /** 2 本指で移動・拡大。 */
  readonly touch?: boolean;
  /** クリックとみなす移動の上限（CSS px）。既定 4。 */
  readonly dragThreshold?: number;
  /** ホイールの 1 CSS px あたりの拡大の指数。既定 0.002。 */
  readonly zoomSpeed?: number;
  /** 1 CSS px あたりの回転（ラジアン）。既定 0.01。 */
  readonly rotateSpeed?: number;
}
export interface PointLike { readonly x: number; readonly y: number }
interface Modifiers { readonly shiftKey?: boolean; readonly ctrlKey?: boolean; readonly altKey?: boolean; readonly metaKey?: boolean }
/** 正規化した入力。点は Canvas 左上からの CSS px。 */
export type View3DInput =
  | ({ readonly type: "pointerdown" | "pointermove" | "pointerup"; readonly pointerId: number; readonly pointerType?: "mouse" | "pen" | "touch"; readonly button?: number; readonly point: PointLike } & Modifiers)
  | ({ readonly type: "pointercancel"; readonly pointerId: number; readonly pointerType?: "mouse" | "pen" | "touch" } & Modifiers)
  | ({ readonly type: "wheel"; readonly point: PointLike; readonly deltaY: number } & Modifiers)
  | ({ readonly type: "keydown"; readonly key: string } & Modifiers);
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
  | { readonly type: "render"; readonly views: number; readonly excluded: readonly { readonly view: string; readonly id: string; readonly reason: "clipping-unsupported" }[] }
  | { readonly type: "pick"; readonly view: string; readonly point: PointLike; readonly hits: readonly Hit3D[] }
  | { readonly type: "input"; readonly action: "start" | "end" | "cancel"; readonly gesture: "pan" | "orbit" | "touch"; readonly view: string; readonly reason?: string }
  | { readonly type: "contextlost" }
  | { readonly type: "contextrestored" };

export declare class View3D {
  /** WebGL2 を使えないときは reason が webgl2-unavailable の例外。 */
  constructor(canvas: HTMLCanvasElement | OffscreenCanvas, options?: View3DOptions);
  getState(): View3DState;
  /** 表示中の文書の要素。クラスは地物では型。未知の ID は null。シーンがなければ no-scene、別文書は other-document。 */
  getElement(document: DocumentIdentity, id: string): Element3D | null;
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
  /** 親のない root を借用して描く。表示部品と同じ three.js の実体の Object3D に限る。 */
  addObject(root: object, options: ObjectOptions): Registration;
  /** 幾何の記述から部品が資源を作って描く。 */
  addGeometry(items: readonly GeometryItem[], options: ExtraOptions): GeometryRegistration;
  /** 省けば Canvas。戻り値で接続を外す。 */
  attachInput(element?: HTMLElement | HTMLCanvasElement, options?: InputOptions): () => void;
  setInputOptions(options: InputOptions): void;
  /** 扱った入力なら true。通知の中から呼ぶと reason が reentrant の例外。 */
  handleInput(input: View3DInput): boolean;
  /** 移動・回転・2 本指の操作は元のカメラへ戻す。 */
  cancelInput(reason?: string): void;
  getStats(): View3DStats;
  invalidate(): void;
  /** context を失っている間は描かずに false を返す。 */
  render(): boolean;
  destroy(): void;
}
