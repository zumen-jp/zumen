// 生成したファイル。手で直さない。
// 元は schema/messages.json で、`cargo xtask codegen` で作り直す。
//
// 境界のメッセージの型（決定 0012・0194）。スキーマの定義ごとに 1 つの型がある。
// 列挙は、知らない値も受け取れるように、ほかの文字列も許す（決定 0015）。

/** 要求。op の値で形が決まる */
export type Request =
  | CapabilitiesRequest
  | OpenRequest
  | CloseRequest
  | DescribeRequest
  | DrawingRequest
  | EntitiesRequest
  | ApplyRequest
  | ConvertRequest
  | TablesRequest
  | SceneRequest
  | CheckRequest
  | ValidateRequest
  | ReviewCarryRequest
  | ReviewAdoptRequest
  | BuildingRequest
  | BuildingElementsRequest
  | IdsCheckRequest
  | ReviewAdoptIdsRequest
  | BcfReadRequest
  | BcfWriteRequest
  | CivilRequest
  | PointcloudRequest
  | DeviationRequest
  | CivilDrawingRequest
  | GeoRequest
  | DeliveryCheckRequest
  | PhotoRequest;

/** どの op の要求にも置ける欄 */
export interface RequestCommon {
  /** メッセージのスキーマの大版。今は 1 */
  v: number;
  /** 予算の単位ごとの上限。書かない単位は既定値にする（決定 0010） */
  budget?: Budget;
  /** 予算で止まったとき、止まった所までの結果を求める。書かなければ false（決定 0010） */
  partial?: boolean;
  /** 診断ごとに返す場所の数。書かなければ 16（決定 0009） */
  max_places?: number;
  /** 時間の上限（ミリ秒）。ネイティブの境界が時計を見て止める。止まる所は環境で変わる。wasm では受け付けない（決定 0010） */
  time_limit_ms?: number;
}

/** 予算の単位ごとの上限（決定 0010） */
export interface Budget {
  /** 受け取ったバイト数 */
  input_bytes?: Limit;
  /** 展開・復号で作ったバイト数。入れ子の分も足す */
  decoded_bytes?: Limit;
  /** L0・L1 で読んだレコードの数 */
  records?: Limit;
  /** L2・L3 で作った要素の数 */
  elements?: Limit;
  /** 入れ子と再帰の深さ */
  depth?: Limit;
  /** 幾何の分割で作った点・線・面の数 */
  segments?: Limit;
  /** ほかの単位に入らない繰り返しの回数 */
  work?: Limit;
}

/** 1 つの単位の上限。上限をなくすときは unlimited と書く */
export type Limit =
  /** この量までは使える */
  | number
  /** 上限なし */
  | "unlimited";

/** コアの版と、使える op・形式を尋ねる */
export interface CapabilitiesRequest extends RequestCommon {
  /** op の名前 */
  op: "capabilities";
}

/** コアの版と、使える op・形式（決定 0015） */
export interface CapabilitiesResult {
  /** パッケージの版（semver） */
  package: string;
  /** ABI の大版。zm_abi_version と同じ */
  abi: number;
  /** メッセージのスキーマの版 */
  schema: SchemaVersion;
  /** このコア（wasm ではこのモジュール）で使える op の名前 */
  ops: string[];
  /** 読み書きできる形式 */
  formats: Format[];
}

/** メッセージのスキーマの版。大版は要求と応答の v、小版は op・欄・列挙の値を足すたびに上げる */
export interface SchemaVersion {
  /** 大版 */
  major: number;
  /** 小版 */
  minor: number;
}

/** 読み書きできる形式 */
export interface Format {
  /** 形式の名前（dxf・sfc など） */
  id: string;
  /** 形式の系統（autocad・sxf など、決定 0008） */
  family: string;
  /** 読めるか */
  read: boolean;
  /** 書けるか */
  write: boolean;
  /** この形式の ID の規則の版（決定 0015） */
  id_rules: number;
}

/** 入力を読んで、文書を作る。形式は入力から判定する */
export interface OpenRequest extends RequestCommon {
  /** op の名前 */
  op: "open";
  /** 読む入力の名前。zm_push で渡したもの */
  input: string;
  /** 作る文書の名前。書かなければ入力の名前 */
  doc?: string;
  /** 入力の全体の大きさ（バイト）。渡した入力より大きければ、入力を全体の初めの部分とみなし、足りない範囲を needs で求める。範囲で読める形式（LAS・LAZ）だけが使う（決定 0232）。小版 48 から */
  size?: number;
  /** 見分ける印のない形式（xyz）を名指して読む。書かなければ、入力から判定する（決定 0232）。小版 49 から */
  format?: string;
}

/** 作った文書 */
export interface OpenResult {
  /** 文書の名前 */
  doc: string;
  /** 判定した形式の名前 */
  format: string;
  /** 読むのに足りない範囲。あれば文書はまだ作らない。範囲をその名前で zm_push してから、同じ要求をもう一度呼ぶ（決定 0232）。小版 48 から */
  needs?: InputRange[];
  /** 読み終えたので、close の inputs で捨ててよい範囲の入力。小版 48 から */
  done?: InputRange[];
  /** SXF を ZIP に入れた書庫（P2Z・SFZ）なら、SXF の項目のほかの項目（属性ファイル・画像など）の書庫の中の名前。書庫の順。apply は、SXF の項目だけを置き換え、ほかの項目を写した書庫を書く（#385）。小版 54 から */
  entries?: string[];
}

/** 入力の範囲と、その範囲を渡す入力の名前 */
export interface InputRange {
  /** 範囲を渡す入力の名前。<入力の名前>#<start>-<end> */
  input: string;
  /** 範囲の初め（バイト） */
  start: number;
  /** 範囲の終わり（バイト、含まない） */
  end: number;
}

/** 文書と入力を捨てる。名指ししたものが 1 つでもなければ、何も捨てない */
export interface CloseRequest extends RequestCommon {
  /** op の名前 */
  op: "close";
  /** 捨てる文書の名前 */
  docs?: string[];
  /** 捨てる入力の名前 */
  inputs?: string[];
}

/** 捨てたこと。中身はない */
export interface CloseResult {
}

/** 診断の文を、カタログから組み立てる（決定 0009） */
export interface DescribeRequest extends RequestCommon {
  /** op の名前 */
  op: "describe";
  /** カタログの入力の名前。zm_push で渡したもの。書かなければ、code と値をそのまま文にする */
  catalog?: string;
  /** 文にする診断。応答で受け取ったもの */
  diagnostics: Diagnostic[];
}

/** 診断の文 */
export interface DescribeResult {
  /** 診断ごとの文。要求の diagnostics と同じ順。カタログにない code は、code と値をそのまま書く */
  texts: string[];
}

/** 開いた文書を図面モデル（L2、決定 0022）に変換し、その要約を返す。ネイティブと wasm の出力の一致を、適合試験で比べるためにも使う（決定 0007） */
export interface DrawingRequest extends RequestCommon {
  /** op の名前 */
  op: "drawing";
  /** 文書の名前。open で作ったもの */
  doc: string;
}

/** 図面モデルの要約。移さなかった図形と値は、診断で返す */
export interface DrawingResult {
  /** 文書のハッシュ。open に渡したバイト列から作る。変更の一覧の前提条件に使う（v2: と 32 桁の 16 進、決定 0156） */
  hash?: string;
  /** 実寸の空間。モデルの順 */
  spaces: SpaceSummary[];
  /** 用紙。モデルの順 */
  sheets: SheetSummary[];
  /** 文書の点の表し方（marker を持たない点の見え方、決定 0160）。point_style の変更で変える。文書が持たなければ（JWW・SXF）持たない。小版 39 から */
  point_style?: PointStyleSummary;
}

/** 文書の点の表し方。欄の意味は point_style_edit と同じ */
export interface PointStyleSummary {
  /** 基本の形 */
  mark: PointMark;
  /** 円で囲む */
  circle: boolean;
  /** 四角で囲む */
  square: boolean;
  /** 大きさ。意味は size_kind で決まる */
  size: number;
  /** 大きさの意味 */
  size_kind: PointSizeKind;
}

/** 実寸の空間の要約 */
export interface SpaceSummary {
  /** 空間の ID（space/0）。entities の要素の ID の始まりと同じ（決定 0156） */
  id?: string;
  /** 文書の単位 */
  unit: Unit;
  /** 要素の数 */
  elements: ElementCounts;
  /** 線分の端点と文字の基点を囲む範囲。文書の単位。要素がなければ持たない */
  extent?: Extent;
}

/** 用紙の要約 */
export interface SheetSummary {
  /** 用紙の ID。LAYOUT がハンドルを持てば sheet/h22、なければ読んだ順の番号（sheet/0）。entities の要素の ID の始まりと同じ（決定 0156） */
  id?: string;
  /** 元の形式の用紙の名前（DXF・DWG のレイアウトの名前）。翻訳しない。なければ持たない */
  name?: string;
  /** 用紙の幅と高さ（mm）。分からなければ持たない */
  size?: number[];
  /** 要素の数 */
  elements: ElementCounts;
  /** ビューの数 */
  views: number;
  /** 線分の端点と文字の基点を囲む範囲。用紙の mm。要素がなければ持たない */
  extent?: Extent;
}

/** 実寸の単位 */
export type Unit =
  | "millimeter"
  | "centimeter"
  | "meter"
  | "inch"
  | "foot"
  | "unknown"
  | (string & {});

/** 種類ごとの要素の数 */
export interface ElementCounts {
  /** 線分 */
  lines: number;
  /** 文字 */
  texts: number;
}

/** 点を囲む、軸に沿った長方形 */
export interface Extent {
  /** X の最小 */
  min_x: number;
  /** Y の最小 */
  min_y: number;
  /** X の最大 */
  max_x: number;
  /** Y の最大 */
  max_y: number;
}

/** 開いた文書を図面モデル（L2、決定 0022）に変換し、要素の一覧を返す。実寸の空間、用紙の順に、それぞれの要素を入力の順に並べる（決定 0117） */
export interface EntitiesRequest extends RequestCommon {
  /** op の名前 */
  op: "entities";
  /** 文書の名前。open で作ったもの */
  doc: string;
  /** 返す最初の要素の位置。前の応答の next_offset を渡す。書かなければ 0 */
  offset?: number;
  /** 返す要素の数の上限。書かなければ 1000 */
  limit?: number;
  /** 返す欄の名前。1 つ以上で、重ねない。id は書かなくても返す。書かなければ id・kind・layer・source を返す（決定 0118） */
  fields?: EntityField[];
  /** [最小の X, 最小の Y, 最大の X, 最大の Y]。範囲がこの長方形と重なるか接する要素だけを返す。空間か用紙それぞれの座標で比べる（決定 0118） */
  region?: number[];
  /** 空間か用紙の ID（space/0・sheet/h22 など）。書けば、その空間か用紙の要素だけを返す。記号の定義の空間（space/h1F など。BLOCK のハンドルから作る、決定 0156）も指せ、書かなければ定義の中の要素は返さない。ない ID なら要素を返さない。小版 31 から（決定 0177） */
  place?: string;
}

/** 要素の一覧の一部。移さなかった図形と値は、診断で返す */
export interface EntitiesResult {
  /** offset から limit 個までの要素 */
  items: Entity[];
  /** 全体の要素の数 */
  total: number;
  /** 続きの最初の要素の位置。続きがなければ持たない */
  next_offset?: number;
}

/** 図面モデルの要素 */
export interface Entity {
  /** 空間か用紙と、出所で作る ID（space/0/h2F、space/0/r12、sheet/1/b4096 など）。出所の部分は、形式が持つ鍵（ハンドル）があれば鍵で作り、書き戻しても前の要素が増えても変わらない。同じバイト列には、どの版でも同じ ID を返す（決定 0117・0156） */
  id: string;
  /** 形の種類 */
  kind?: EntityKind;
  /** レイヤの元の名前。翻訳しない。図面モデルにレイヤがなければ持たない */
  layer?: string;
  /** 元の形式のレコード */
  source?: Source;
  /** 形の定義から求めた範囲。文字の字形・線の太さ・記号の中身は含めない。囲む点がなければ持たない（決定 0118） */
  bbox?: Extent;
  /** 要素のレイヤ・スタイル・形のハッシュ。出所の位置は含めない。変更の前提条件に使う（v2: と 32 桁の 16 進、決定 0156） */
  hash?: string;
  /** 形の値。線分・円・円弧・楕円・幅を持たない折れ線・1 点か 2 点の間に置く文字・書式つきの文字・ブロックの記号・制御点か当てはめる点で決まるスプライン・寸法・引出線（LEADER）だけが持つ。apply の replace と add に渡せる形（決定 0139）。小版 10 から */
  geometry?: Geometry;
}

/** entities で返す要素の欄（決定 0118） */
export type EntityField =
  | "id"
  | "kind"
  | "layer"
  | "source"
  | "bbox"
  | "hash"
  | "geometry"
  | (string & {});

/** 形の値。点は要素の空間か用紙の座標、長さは同じ単位（実寸の空間は文書の単位、用紙は mm）、角度は度で反時計回り。shape の値で形が決まる */
export type Geometry =
  | LineGeometry
  | CircleGeometry
  | ArcGeometry
  | EllipseGeometry
  | PolylineGeometry
  | TextGeometry
  | FormattedTextGeometry
  | SymbolGeometry
  | SplineGeometry
  | DimensionGeometry
  | LeaderGeometry
  | PointGeometry
  | CalloutGeometry;

/** 線分 */
export interface LineGeometry {
  /** 形の種類 */
  shape: "line";
  /** 始点 [X, Y] */
  start: number[];
  /** 終点 [X, Y] */
  end: number[];
}

/** 円 */
export interface CircleGeometry {
  /** 形の種類 */
  shape: "circle";
  /** 中心 [X, Y] */
  center: number[];
  /** 半径。0 より大きい */
  radius: number;
}

/** 円弧。始まりの角度から終わりの角度へ、反時計回りにたどる */
export interface ArcGeometry {
  /** 形の種類 */
  shape: "arc";
  /** 中心 [X, Y] */
  center: number[];
  /** 半径。0 より大きい */
  radius: number;
  /** 始まりの角度（度） */
  start_angle: number;
  /** 終わりの角度（度） */
  end_angle: number;
}

/** 楕円か楕円の弧。始まりの角度から終わりの角度へ、反時計回りにたどる。閉じた楕円は 0 度から 360 度。小版 11 から */
export interface EllipseGeometry {
  /** 形の種類 */
  shape: "ellipse";
  /** 中心 [X, Y] */
  center: number[];
  /** 長軸の端の点 [X, Y]。中心と違う点 */
  major: number[];
  /** 短軸の長軸に対する比。0 より大きく 1 以下 */
  ratio: number;
  /** 始まりの角度（度）。楕円を円に引き延ばしたときの角度 */
  start_angle: number;
  /** 終わりの角度（度）。楕円を円に引き延ばしたときの角度 */
  end_angle: number;
}

/** 頂点を結んだ折れ線。幅は持たない */
export interface PolylineGeometry {
  /** 形の種類 */
  shape: "polyline";
  /** 頂点の [X, Y] の並び。2 つ以上 */
  points: number[][];
  /** 頂点ごとの、次の頂点への弧の膨らみ（中心角の 4 分の 1 の正接。正は反時計回り）。points と同じ数。書かなければどれも 0 */
  bulges?: number[];
  /** 最後の頂点から最初の頂点へ結ぶ */
  closed: boolean;
}

/** 1 点か 2 点の間に置く文字。置き換えるときは、位置合わせ・幅の倍率・斜体・反転・文字スタイルを元の値のままにする。足すときは、2 点目がなければ左の基準線に合わせ、幅の倍率 1・斜体なし・反転なしにする */
export interface TextGeometry {
  /** 形の種類 */
  shape: "text";
  /** 内容。1 行 */
  content: string;
  /** 基点 [X, Y]。2 点の間に合わせる文字では 1 点目 */
  position: number[];
  /** 2 点の間に合わせる文字の 2 点目 [X, Y]。合わせ方（fit）と一緒に書く。置き換えでは、1 点に置く文字と 2 点の間に合わせる文字を入れ替えられない。小版 17 から */
  end?: number[];
  /** 2 点の間への合わせ方。2 点目（end）と一緒に書く。小版 17 から */
  fit?: TextFit;
  /** 高さ。0 より大きい */
  height: number;
  /** 回転（度）。2 点の間に合わせる文字では、1 点目から 2 点目への向きと同じでなければならない */
  rotation: number;
  /** 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。DXF に文字スタイルの定義がないときは Standard だけを受ける。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す。小版 12 から */
  style?: string;
  /** 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から */
  style_id?: string;
}

/** 2 点の間への合わせ方。aligned は縦横の比を保って高さも変え、fit は高さを保って幅だけを変える */
export type TextFit =
  | "aligned"
  | "fit"
  | (string & {});

/** 書式つきの文字（DXF の MTEXT）。置き換えるときは、枠の高さ・書き進める向き・行送り・背景・高さの意味・文字スタイルを元の値のままにする。足すときは、枠の高さなし・左から右・行送り 1 倍・背景なしにする。小版 13 から */
export interface FormattedTextGeometry {
  /** 形の種類 */
  shape: "formatted_text";
  /** 内容。書式の符号（\P の改行など）を含めた、元の形式の内容のまま */
  content: string;
  /** 基点 [X, Y] */
  position: number[];
  /** 高さ。0 より大きい */
  height: number;
  /** 回転（度） */
  rotation: number;
  /** 基点に合わせる枠の位置 */
  attachment?: Attachment;
  /** 折り返す枠の幅。0 より大きい。書かなければ折り返さない */
  width?: number;
  /** 文字スタイルの元の名前。entities は返し、足すときは図面にある名前を指す（省けば Standard）。置き換えるときは、元と同じ名前だけを受ける。style_id と一緒に書くなら、同じ文字スタイルを指す */
  style?: string;
  /** 文字スタイルの ID。tables が返したもの。entities は、名前で引いた文字スタイルの ID を返す。足す・置き換えるときは、名前の代わりに書ける（名前より先に使う）。同じ名前（大文字と小文字を区別しない）の項目が複数あれば書き戻せない。小版 23 から */
  style_id?: string;
}

/** スプライン（NURBS）。制御点で決まるもの（小版 15 から）と、当てはめる点で決まるもの（小版 18 から）がある。当てはめる点で決まるものは、節点と制御点を空にし、当てはめる点（fit）と、あれば両端の接線を持つ。元の図形に制御点や節点もあるときは形の値を返さず、置き換えない。置き換えでは、制御点で決まるものと当てはめる点で決まるものを入れ替えない */
export interface SplineGeometry {
  /** 形の種類 */
  shape: "spline";
  /** 次数。1 以上 */
  degree: number;
  /** 節点の並び。周期的でなければ、制御点の数と次数と 1 の和の数。当てはめる点で決まるものは空 */
  knots: number[];
  /** 制御点の [X, Y] の並び。次数より多い。当てはめる点で決まるものは空 */
  control: number[][];
  /** 制御点ごとの重み。0 より大きい。有理のスプラインだけが持ち、書かなければ有理でない */
  weights?: number[];
  /** 当てはめる点の [X, Y] の並び。2 つ以上。書けば、節点と制御点は空で、重みは書かない。小版 18 から */
  fit?: number[][];
  /** 当てはめる点で決まるスプラインの、始まりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から */
  start_tangent?: number[];
  /** 当てはめる点で決まるスプラインの、終わりの接線の向き [X, Y]。0 でない。書かなければ読む側が決める。小版 18 から */
  end_tangent?: number[];
  /** 閉じている */
  closed: boolean;
  /** 周期的 */
  periodic: boolean;
}

/** 寸法。種類（kind）ごとに、その種類の定義点の欄を持つ。測定値（measurement）は元の形式が保存した値で、定義点から測り直さない。寸法を描いた部品（ブロック）は形の値に含めない。entities が返すだけで、置き換えと追加は change.edit.unsupported になる（寸法を描いた部品を作り直す必要があるため）。小版 19 から */
export interface DimensionGeometry {
  /** 形の種類 */
  shape: "dimension";
  /** 寸法の種類 */
  kind: DimensionKind;
  /** 1 本目の補助線の点か、測る角度の 1 本目の線の点か、円弧の始めの点 [X, Y]。linear・aligned・angular・arc_length が持つ */
  first?: number[];
  /** 2 本目の補助線の点か、測る角度の 2 本目の線の点か、円弧の終わりの点 [X, Y]。linear・aligned・angular・arc_length が持つ */
  second?: number[];
  /** 寸法線が通る点 [X, Y]。linear・aligned が持つ */
  line?: number[];
  /** 測る向き（度）。linear が持つ */
  rotation?: number;
  /** 補助線の傾き（度）。傾けなければ 0。linear・aligned が持つ */
  oblique?: number;
  /** 角度の頂点 [X, Y]。angular が持つ */
  vertex?: number[];
  /** 1 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ */
  first_line?: number[][];
  /** 2 本目の線の始点と終点 [[X, Y], [X, Y]]。angular_lines が持つ */
  second_line?: number[][];
  /** 寸法の弧が通る点 [X, Y]。angular・angular_lines・arc_length が持つ */
  arc?: number[];
  /** 中心 [X, Y]。arc_length・radius が持つ */
  center?: number[];
  /** 円の上の点 [X, Y]。radius・diameter が持つ */
  chord?: number[];
  /** chord と向かい合う円の上の点 [X, Y]。diameter が持つ */
  opposite?: number[];
  /** 引出線の長さ。radius・diameter が持つ */
  leader?: number;
  /** 原点 [X, Y]。ordinate が持つ */
  origin?: number[];
  /** 測る点 [X, Y]。ordinate が持つ */
  feature?: number[];
  /** 引出線の終点 [X, Y]。ordinate が持つ */
  leader_end?: number[];
  /** 測る座標の軸。ordinate が持つ */
  axis?: DimensionAxis;
  /** 寸法値の文字の書き方 */
  label: DimensionLabel;
  /** 利用者が書いた寸法値の文字の元の内容。label が override のときだけ持つ。測定値に置き換える符号（DXF の <>）と書式の符号もそのまま持つ */
  content?: string;
  /** 元の形式が保存した測定値。単位と倍率は measurement_key の欄に従う */
  measurement?: number;
  /** measurement の形式と欄を指すキー（DXF の DIMENSION の 42 なら dxf.DIMENSION.42）。measurement と一緒に持つ */
  measurement_key?: string;
  /** 寸法値の文字の中央の点 [X, Y]。元の形式が持たなければ書かない */
  text_position?: number[];
  /** 寸法値の文字の回転（度）。寸法線に沿わせるなら書かない */
  text_rotation?: number;
  /** 寸法スタイルの元の名前 */
  style: string;
  /** 寸法スタイルの ID。tables が返したもの。名前で引いた寸法スタイルがなければ書かない。小版 23 から */
  style_id?: string;
}

/** 引出線（DXF・DWG の LEADER、決定 0177）。注記（MTEXT など）は別の要素で、annotation がその ID を指す。置き換えるときは寸法スタイルと注記を変えず、経路・曲線か・矢印を書き直す。足すときの注記は、同じ空間か用紙の MTEXT だけを指せる（小版 36 から。それまでは注記を持てなかった）。DWG は置き換えと追加を書き戻すが、矢印の印を必ず持つので、arrow を書かない引出線は書けない（小版 36 から。それまでは R2004 までの置き換えだけ）。小版 31 から */
export interface LeaderGeometry {
  /** 形の種類 */
  shape: "leader";
  /** 経路の頂点 [[X, Y], ...]。矢印の側から並べる。2 つ以上 */
  path: number[][];
  /** 頂点を通る曲線で結ぶか。偽なら折れ線 */
  spline: boolean;
  /** 始点に矢印を付けるか。書かなければ寸法スタイルに従う */
  arrow?: boolean;
  /** 注記の要素の ID。同じ空間か用紙の要素で、entities が返したもの。注記がなければ書かない。置き換えるときは元と同じ ID だけを受ける。足すときは、同じ空間か用紙の書式つきの文字（MTEXT）を指す */
  annotation?: string;
  /** 寸法スタイルの元の名前。足すときは図面にある寸法スタイルの名前を指す。置き換えるときは元と同じ名前だけを受ける */
  style: string;
  /** 寸法スタイルの ID。tables が返したもの。entities は、名前で引いた寸法スタイルの ID を返す。足す・置き換えるときは、名前と同じ寸法スタイルを指す ID だけを受ける */
  style_id?: string;
}

/** 引出線つきの注記（DXF・DWG の MULTILEADER、決定 0185）。注記は形の中にあり、content と text・symbol で種類と中身を示す。置き換えるときは枝の線の頂点・集まる点・折れ曲がりだけを変えられ、ほかの欄は元の値のままにする。DXF だけが置き換えを書き戻し、枝と線の数と、集まる点と折れ曲がりの有無は元と同じでなければならない。足すことと DWG の書き戻しは、まだしない。小版 36 から */
export interface CalloutGeometry {
  /** 形の種類 */
  shape: "callout";
  /** 枝。入力の順 */
  branches: CalloutBranch[];
  /** 線の引き方 */
  path: CalloutPath;
  /** 線を注記に、水平の線（ランディング）でつなぐか */
  landing: boolean;
  /** 端部記号の大きさ */
  arrow_size: number;
  /** 大きさに掛ける倍率 */
  scale: number;
  /** 注記の種類 */
  content: CalloutContent;
  /** 注記の書式つきの文字の元の内容。content が text のときだけ持つ */
  text?: string;
  /** 注記の記号のブロックの名前。content が symbol のときだけ持つ */
  symbol?: string;
}

/** 引出線つきの注記の枝。何本かの線が 1 点に集まり、折れ曲がりで注記につながる */
export interface CalloutBranch {
  /** 線ごとの頂点 [[[X, Y], ...], ...]。矢印の側から並べ、集まる点は含めない。線ごとに 1 つ以上 */
  lines: number[][][];
  /** 線が集まる点 [X, Y]。元の形式が持たなければ書かない */
  end?: number[];
  /** 集まる点から注記への折れ曲がり。なければ書かない */
  dogleg?: CalloutDogleg;
}

/** 枝の折れ曲がり */
export interface CalloutDogleg {
  /** 向き [X, Y]。長さは意味を持たない */
  direction: number[];
  /** 長さ */
  length: number;
}

/** 引出線つきの注記の線の引き方。straight は折れ線、spline は頂点を通る曲線、hidden は線を描かない */
export type CalloutPath =
  | "straight"
  | "spline"
  | "hidden"
  | (string & {});

/** 引出線つきの注記の中身の種類。none は注記を持たず、text は書式つきの文字、symbol は記号（バルーンなどの囲み） */
export type CalloutContent =
  | "none"
  | "text"
  | "symbol"
  | (string & {});

/** 点（決定 0160・0183）。marker がなければ文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に従う点で、DXF・DWG の POINT と JWW の記号でない点が当たる。marker があれば既定義の印の点で、SFC の点マーカーと JWW の記号の点が当たる。置き換えるときは、描き方（marker があるか）を元のまま保つ。DXF・DWG は marker と construction を持つ点を書かず、JWW の記号でない点は角度 0 だけを、SXF は SFC の点マーカーだけを書く。小版 35 から */
export interface PointGeometry {
  /** 形の種類 */
  shape: "point";
  /** 位置 [X, Y] */
  position: number[];
  /** 回転（度） */
  rotation: number;
  /** 既定義の印。書かなければ文書の点の表し方に従う */
  marker?: PointMarker;
  /** 作図の補助の点（JWW の仮点）か。書かなければ偽 */
  construction?: boolean;
}

/** 既定義の印の表のキーと尺度。印の形は、表示する側がキーから引く */
export interface PointMarker {
  /** 印の表の名前空間。sxf.marker（SFC の点マーカー）か jww.point（JWW の記号の点） */
  namespace: string;
  /** 名前空間の中の元のコード */
  code: string;
  /** 尺度 */
  scale: number;
}

/** 寸法の種類。linear は 2 点を測る向きに、aligned は 2 点を結ぶ向きに測った長さ。angular は頂点から 2 点へ向かう線の、angular_lines は 2 本の線のなす角度。arc_length は円弧の長さ。radius は半径、diameter は直径。ordinate は原点からの X か Y の座標 */
export type DimensionKind =
  | "linear"
  | "aligned"
  | "angular"
  | "angular_lines"
  | "arc_length"
  | "radius"
  | "diameter"
  | "ordinate"
  | (string & {});

/** 座標の軸 */
export type DimensionAxis =
  | "x"
  | "y"
  | (string & {});

/** 寸法値の文字の書き方。measurement は測定値を書き、override は利用者が書いた文字（content）を書き、hidden は書かない */
export type DimensionLabel =
  | "measurement"
  | "override"
  | "hidden"
  | (string & {});

/** 記号（DXF・DWG のブロックの INSERT）。定義の点を、尺度・回転・移動の順に写す。切り抜きは形の値に含めない。置き換えるときは切り抜きを元のままにし、属性は値だけを変えられる。属性を持つ記号の置く点・尺度・回転は変えない（属性ごと動かすなら transform を使う）。足すときは切り抜きを持たない。小版 14 から */
export interface SymbolGeometry {
  /** 形の種類 */
  shape: "symbol";
  /** 記号の定義（ブロック）の元の名前。足すときは図面にある定義の名前を指す */
  name: string;
  /** 定義の基点を置く点 [X, Y] */
  position: number[];
  /** X と Y の尺度 [X, Y]。0 でない。負なら反転する */
  scale: number[];
  /** 回転（度） */
  rotation: number;
  /** 格子に並べて置く列と行。書かなければ 1 つだけ置く */
  grid?: SymbolGrid;
  /** 属性のタグと値。entities は記号の属性を入力の順に返す。足すときは、定義の属性の定義（値が決まったものを除く）から属性を作り、ここに書いたタグの値で既定の値を替える。タグは大文字と小文字を区別せず、重複を受けない。必要な属性を作れなければ失敗する。置き換えるときは、元と同じ数・順・タグを受け、値を変えた属性の ATTRIB の値を書き直す（表題欄の項目の書き換えなど）。書式つきの値（複数行の属性）を持つ属性の値は変えられない。小版 16 から。置き換えで値を変えるのは小版 21 から */
  attributes?: SymbolAttribute[];
}

/** 記号の属性のタグと値 */
export interface SymbolAttribute {
  /** 元のタグ */
  tag: string;
  /** 値。1 行 */
  value: string;
}

/** 記号を格子に並べる列と行。間隔は、記号を回転する前の向きに測る。置き換えるときは、列と行の数を変えない */
export interface SymbolGrid {
  /** 列の数。1 以上 */
  columns: number;
  /** 行の数。1 以上。列と行がどちらも 1 の格子は持たない */
  rows: number;
  /** 列の間隔 */
  column_spacing: number;
  /** 行の間隔 */
  row_spacing: number;
}

/** 基点に合わせる枠の位置。entities は返し、足すときに書かなければ左上、置き換えるときに書かなければ元のまま */
export type Attachment =
  | "top_left"
  | "top_center"
  | "top_right"
  | "middle_left"
  | "middle_center"
  | "middle_right"
  | "bottom_left"
  | "bottom_center"
  | "bottom_right"
  | (string & {});

/** 要素の形の種類（決定 0022）。point（点、決定 0160）は小版 32 から。それまで点は symbol だった */
export type EntityKind =
  | "line"
  | "circle"
  | "curve"
  | "polyline"
  | "text"
  | "formatted_text"
  | "dimension"
  | "leader"
  | "callout"
  | "table"
  | "underlay"
  | "symbol"
  | "hatch"
  | "point"
  | (string & {});

/** 開いた文書に、変更の一覧を当てて書き戻す（決定 0139・0156）。要素は、entities が返す ID とハッシュで指す。どれか 1 つでも当てられなければ、何も変えない。当てたら、文書を書き戻したものに替え、書き戻したファイルを blobs で返す。小版 7 から */
export interface ApplyRequest extends RequestCommon {
  /** op の名前 */
  op: "apply";
  /** 文書の名前。open で作ったもの */
  doc: string;
  /** 読んだときの文書のハッシュ（drawing の hash）。書けば、今の文書と同じときだけ当てる */
  hash?: string;
  /** 変更。並べた順に当てる */
  edits: Edit[];
}

/** 当てた変更の結果。書き戻したファイルは blobs の file で返す */
export interface ApplyResult {
  /** 変更ごとの結果。要求の edits と同じ順 */
  applied: Applied[];
  /** 書き戻した文書のハッシュ。次の apply の前提条件に使う */
  hash: string;
  /** true なら、変えていない要素の ID がずれた。鍵を持たない形式（JWW）で、前の図形を除くか足したとき。ID を使う前に entities で読み直す。ずれないときは持たない。小版 30 から */
  ids_shifted?: boolean;
}

/** 開いた文書の図面モデルを、別の形式の新しいファイルに書き、新しい文書として開く（決定 0139・0206）。書けない要素は落とし、種類ごとの件数を診断で返す。書いたファイルは blobs で返す。今は DXF にだけ書ける。小版 41 から */
export interface ConvertRequest extends RequestCommon {
  /** op の名前 */
  op: "convert";
  /** 書く元の文書の名前。open で作ったもの */
  doc: string;
  /** 書く形式の名前（capabilities の formats の id）。今は dxf だけ */
  format: string;
  /** 作る文書の名前 */
  to: string;
}

/** 書いた文書。書いたファイルは blobs の file で返す */
export interface ConvertResult {
  /** 作った文書の名前 */
  doc: string;
  /** 書いた形式の名前 */
  format: string;
  /** 書いたファイルのハッシュ。apply の前提条件に使う */
  hash: string;
  /** 書いた要素の数。記号から離した属性の文字を含む */
  elements: number;
}

/** 当てた変更 1 つの結果 */
export interface Applied {
  /** 変えた要素（レイヤの変更では、そのレイヤ）の、書き戻した後の ID。除いたときと、ID を決められないときは持たない */
  id?: string;
  /** 変えた要素（レイヤ）の、書き戻した後のハッシュ。id があるときに持つ */
  hash?: string;
}

/** 要素 1 つへの変更。kind の値で形が決まる */
export type Edit =
  | RestyleEdit
  | TransformEdit
  | RemoveEdit
  | ReplaceEdit
  | AddEdit
  | LayerEdit
  | AddLayerEdit
  | RemoveLayerEdit
  | TextStyleEdit
  | AddTextStyleEdit
  | RemoveTextStyleEdit
  | LineTypeEdit
  | AddLineTypeEdit
  | RemoveLineTypeEdit
  | DimensionStyleEdit
  | AddDimensionStyleEdit
  | RemoveDimensionStyleEdit
  | PointStyleEdit;

/** 要素のレイヤ・色・線種・線種の倍率・線幅のうち、書いたものだけを変える。形は変えない */
export interface RestyleEdit {
  /** 変更の種類 */
  kind: "restyle";
  /** 変える要素の ID。entities が返したもの */
  id: string;
  /** 読んだときの要素のハッシュ。entities が返したもの */
  hash: string;
  /** レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない */
  layer?: string;
  /** レイヤの ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から */
  layer_id?: string;
  /** 色 */
  color?: ColorChange;
  /** 線種 */
  line_type?: LineTypeChange;
  /** 要素の線種の倍率 */
  line_type_scale?: number;
  /** 線幅 */
  line_weight?: LineWeightChange;
}

/** 要素を、アフィン変換で動かす。AutoCAD の系統では、図形とともに動く XDATA と記号の属性も同じ変換で直す。変換した形を元の種類で表せなければ当てない */
export interface TransformEdit {
  /** 変更の種類 */
  kind: "transform";
  /** 動かす要素の ID。entities が返したもの */
  id: string;
  /** 読んだときの要素のハッシュ。entities が返したもの */
  hash: string;
  /** [a, b, c, d, e, f]。要素の空間か用紙の点 (x, y) を (a x + b y + c, d x + e y + f) に写す */
  matrix: number[];
}

/** 要素を除く */
export interface RemoveEdit {
  /** 変更の種類 */
  kind: "remove";
  /** 除く要素の ID。entities が返したもの */
  id: string;
  /** 読んだときの要素のハッシュ。entities が返したもの */
  hash: string;
}

/** 要素の形を、渡す値で置き換える。形の種類は変えない。レイヤとスタイルは変えない。小版 10 から */
export interface ReplaceEdit {
  /** 変更の種類 */
  kind: "replace";
  /** 置き換える要素の ID。entities が返したもの */
  id: string;
  /** 読んだときの要素のハッシュ。entities が返したもの */
  hash: string;
  /** 新しい形。entities の geometry と同じ種類 */
  geometry: Geometry;
}

/** 空間か用紙に要素を足す。書かない色・線種・線幅はレイヤに従う。足せる形は、形の値のどの種類も。小版 10 から */
export interface AddEdit {
  /** 変更の種類 */
  kind: "add";
  /** 足す先の空間か用紙の ID（space/0・sheet/h22 など）。entities の ID の先頭の部分。小版 31 から、記号の定義の空間（space/h1F など）も指せる（決定 0177） */
  place: string;
  /** レイヤの元の名前。図面にあるレイヤで、名前が 1 つに決まるもの。layer_id と一緒には書かない。どちらも書かなければレイヤ 0 */
  layer?: string;
  /** レイヤの ID。tables が返したもの */
  layer_id?: string;
  /** 色。SXF はレイヤに従う色を書けないので、書く。小版 29 から */
  color?: ColorChange;
  /** 線種。小版 29 から */
  line_type?: LineTypeChange;
  /** 線幅。小版 29 から */
  line_weight?: LineWeightChange;
  /** 足す形 */
  geometry: Geometry;
}

/** レイヤの名前・状態・色・線種・線幅のうち、書いたものだけを変える（決定 0162）。DXF は、名前を変えると、図形と今のレイヤ（$CLAYER）も新しい名前で指す。DWG は、今あるレイヤの線種と真の色を変えられない。小版 22 から */
export interface LayerEdit {
  /** 変更の種類 */
  kind: "layer";
  /** 変えるレイヤの ID。tables が返したもの */
  id: string;
  /** 読んだときのレイヤのハッシュ。tables が返したもの */
  hash: string;
  /** 新しい名前。ほかのレイヤと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。0 と Defpoints の名前は変えられない */
  name?: string;
  /** 表示する。凍結するなら偽 */
  visible?: boolean;
  /** 印刷する */
  printable?: boolean;
  /** 編集できる（ロックしない） */
  editable?: boolean;
  /** 凍結する */
  frozen?: boolean;
  /** 色。aci が要る。inherit は書けない */
  color?: ColorChange;
  /** 線種。name か id で指す。inherit は書けない */
  line_type?: LineTypeChange;
  /** 線幅。mm か default。inherit は書けない */
  line_weight?: LineWeightChange;
}

/** レイヤを除く（決定 0162）。要素・ほかのオブジェクト・今のレイヤ（$CLAYER）が指すレイヤと、0・Defpoints・外部参照に依るレイヤは除けない。同じ apply の要素の変更が指すレイヤも除けない。後ろにハンドルのないレイヤがあれば、その ID が変わるので除けない。除いたレイヤより後ろのレイヤと、その上の要素のハッシュは変わるので、読み直す。小版 24 から */
export interface RemoveLayerEdit {
  /** 変更の種類 */
  kind: "remove_layer";
  /** 除くレイヤの ID。tables が返したもの */
  id: string;
  /** 読んだときのレイヤのハッシュ。tables が返したもの */
  hash: string;
}

/** 文字スタイルの名前・フォント・高さ・幅の倍率・傾き・裏返し・上下反転・縦書きのうち、書いたものだけを変える（決定 0163）。DXF は、名前を変えると、文字と今の文字スタイル（$TEXTSTYLE）も新しい名前で指す。表の書式などの図形でないオブジェクトが名前で指す文字スタイルの名前は変えられない。小版 25 から */
export interface TextStyleEdit {
  /** 変更の種類 */
  kind: "text_style";
  /** 変える文字スタイルの ID。tables が返したもの */
  id: string;
  /** 読んだときの文字スタイルのハッシュ。tables が返したもの */
  hash: string;
  /** 新しい名前。ほかの文字スタイルと重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。Standard の名前は変えられない */
  name?: string;
  /** 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない */
  font?: Font;
  /** 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く */
  big_font?: Font;
  /** 決まった高さ。文書の単位。0 なら決めない。負の数は書けない */
  height?: number;
  /** 幅の倍率。正の数 */
  width?: number;
  /** 傾き（度）。-85 から 85 まで */
  oblique?: number;
  /** 左右を裏返す */
  backward?: boolean;
  /** 上下を反転する */
  upside_down?: boolean;
  /** 縦書き */
  vertical?: boolean;
}

/** 文字スタイルを足す（決定 0163）。書かない欄は Standard と同じ。同じ apply の要素は、足した文字スタイルを指せない。小版 25 から */
export interface AddTextStyleEdit {
  /** 変更の種類 */
  kind: "add_text_style";
  /** 名前。text_style の name と同じ決まり */
  name: string;
  /** 主のフォント。形のファイル（shape）か、太字・斜体・文字集合・ピッチを持たない TrueType のファイル（named で、名前が .ttf・.ttc・.otf で終わるもの）。名前を空にすると除く。XDATA の書体の名前で指すフォントは変えられない */
  font?: Font;
  /** 大きいフォント。形のファイル（shape）だけ。名前を空にすると除く */
  big_font?: Font;
  /** 決まった高さ。文書の単位。0 なら決めない。負の数は書けない */
  height?: number;
  /** 幅の倍率。正の数 */
  width?: number;
  /** 傾き（度）。-85 から 85 まで */
  oblique?: number;
  /** 左右を裏返す */
  backward?: boolean;
  /** 上下を反転する */
  upside_down?: boolean;
  /** 縦書き */
  vertical?: boolean;
}

/** 文字スタイルを除く（決定 0163）。要素・表の書式や寸法スタイルなどのほかのオブジェクト・今の文字スタイル（$TEXTSTYLE）が指すものと、Standard・外部参照に依るものは除けない。同じ apply で足す要素が指すものも除けない。後ろにハンドルのない文字スタイルがあれば、その ID が変わるので除けない。小版 25 から */
export interface RemoveTextStyleEdit {
  /** 変更の種類 */
  kind: "remove_text_style";
  /** 除く文字スタイルの ID。tables が返したもの */
  id: string;
  /** 読んだときの文字スタイルのハッシュ。tables が返したもの */
  hash: string;
}

/** 線種の名前・説明・線と間の長さの列のうち、書いたものだけを変える（決定 0164）。DXF は、名前を変えると、要素・レイヤと今の線種（$CELTYPE）も新しい名前で指す。多重線のスタイルなどの図形でないオブジェクトが名前で指す線種の名前は変えられない。形か文字を添える線種と DWG の線種は、列の要素の数を変えられない。小版 26 から */
export interface LineTypeEdit {
  /** 変更の種類 */
  kind: "line_type";
  /** 変える線種の ID。tables が返したもの */
  id: string;
  /** 読んだときの線種のハッシュ。tables が返したもの */
  hash: string;
  /** 新しい名前。ほかの線種と重ならず（大文字と小文字を区別しない）、< > / \ " : ; ? * | = ` を含まないもの。ByLayer・ByBlock・Continuous と外部参照に依る線種の名前は変えられない */
  name?: string;
  /** 説明 */
  description?: string;
  /** 線と間の長さの列。文書の単位。正は線、負は間、0 は点 */
  pattern?: number[];
}

/** 線種を足す（決定 0164）。書かない欄は Continuous と同じ。同じ apply の要素とレイヤは、足した線種を指せない。小版 26 から */
export interface AddLineTypeEdit {
  /** 変更の種類 */
  kind: "add_line_type";
  /** 名前。line_type の name と同じ決まり */
  name: string;
  /** 説明 */
  description?: string;
  /** 線と間の長さの列。文書の単位。正は線、負は間、0 は点 */
  pattern?: number[];
}

/** 線種を除く（決定 0164）。要素・レイヤ・多重線のスタイルなどのほかのオブジェクト・今の線種（$CELTYPE）が指すものと、ByLayer・ByBlock・Continuous・外部参照に依るものは除けない。同じ apply で足す・変える要素とレイヤが指すものも除けない。後ろにハンドルのない線種があれば、その ID が変わるので除けない。小版 26 から */
export interface RemoveLineTypeEdit {
  /** 変更の種類 */
  kind: "remove_line_type";
  /** 除く線種の ID。tables が返したもの */
  id: string;
  /** 読んだときの線種のハッシュ。tables が返したもの */
  hash: string;
}

/** 寸法スタイルの名前・端部記号・その大きさ・斜線・倍率のうち、書いたものだけを変える（決定 0165）。DXF は、名前を変えると、寸法・引出線・公差と今の寸法スタイル（$DIMSTYLE）も新しい名前で指す。図形でないオブジェクトが名前で指す寸法スタイルの名前は変えられない。小版 27 から */
export interface DimensionStyleEdit {
  /** 変更の種類 */
  kind: "dimension_style";
  /** 変える寸法スタイルの ID。tables が返したもの */
  id: string;
  /** 読んだときの寸法スタイルのハッシュ。tables が返したもの */
  hash: string;
  /** 新しい名前。ほかの寸法スタイルと重ならず（大文字と小文字を区別しない）、$ と < > / \ " : ; ? * | = ` を含まないもの。Standard・子の寸法スタイル（名前に $ を持つもの）・子を持つ寸法スタイル・外部参照に依る寸法スタイルの名前は変えられない */
  name?: string;
  /** 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない */
  ends?: string[];
  /** 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり */
  leader?: string;
  /** 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない */
  size?: number;
  /** 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない */
  tick?: number;
  /** 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない */
  scale?: number;
}

/** 寸法スタイルを足す（決定 0165）。書かない欄は Standard と同じ。同じ apply の要素は、足した寸法スタイルを指せない。小版 27 から */
export interface AddDimensionStyleEdit {
  /** 変更の種類 */
  kind: "add_dimension_style";
  /** 名前。dimension_style の name と同じ決まり */
  name: string;
  /** 寸法線の 1 本目と 2 本目の補助線の側の端部記号。2 つのブロックの名前で、空の名前は既定の矢印（塗りつぶした閉じた矢印）。DXF は R2000 から変えられる。DWG の端部記号は変えられない */
  ends?: string[];
  /** 引出線の端部記号。ブロックの名前で、空の名前は既定の矢印。ends と同じ決まり */
  leader?: string;
  /** 端部記号の大きさ（DIMASZ）。文書の単位で、scale を掛ける前の値。負の数は書けない */
  size?: number;
  /** 端部記号の代わりに描く斜線の大きさ（DIMTSZ）。0 なら斜線を描かない。負の数は書けない */
  tick?: number;
  /** 大きさに掛ける倍率（DIMSCALE）。0 はビューの縮尺から決める。負の数は書けない */
  scale?: number;
}

/** 寸法スタイルを除く（決定 0165）。寸法・引出線・公差などのほかのオブジェクト・今の寸法スタイル（$DIMSTYLE）が指すものと、Standard・子の寸法スタイル・子を持つ寸法スタイル・外部参照に依るものは除けない。後ろにハンドルのない寸法スタイルがあれば、その ID が変わるので除けない。小版 27 から */
export interface RemoveDimensionStyleEdit {
  /** 変更の種類 */
  kind: "remove_dimension_style";
  /** 除く寸法スタイルの ID。tables が返したもの */
  id: string;
  /** 読んだときの寸法スタイルのハッシュ。tables が返したもの */
  hash: string;
}

/** 文書の点の表し方（marker を持たない点の見え方）を変える（決定 0190）。文書全体の設定なので要素を指さない。読んだときの文書は、apply の hash で確かめる。DXF・DWG は見出しの $PDMODE・$PDSIZE に書く。DWG は、変数を書く幅が変わる値（$PDMODE の 0 とほかの値の間の変更、$PDSIZE の 0・1 とほかの値の間の変更）を、小版 39 から書ける（決定 0193）。JWW・SXF は文書の点の表し方を持たないので断る。applied は id と hash を持たない。小版 38 から */
export interface PointStyleEdit {
  /** 変更の種類 */
  kind: "point_style";
  /** 基本の形 */
  mark: PointMark;
  /** 円で囲む。書かなければ偽 */
  circle?: boolean;
  /** 四角で囲む。書かなければ偽 */
  square?: boolean;
  /** 大きさ。0 より大きい有限の数。意味は size_kind で決まる */
  size: number;
  /** 大きさの意味 */
  size_kind: PointSizeKind;
}

/** 文書の点の基本の形。dot は小さな点、none はなし、plus は十字、cross はバツ、tick は上向きの短い線 */
export type PointMark =
  | "dot"
  | "none"
  | "plus"
  | "cross"
  | "tick"
  | (string & {});

/** 文書の点の大きさの意味。length は点のある空間の長さ、screen は画面の高さに対する百分率 */
export type PointSizeKind =
  | "length"
  | "screen"
  | (string & {});

/** レイヤを足す（決定 0162）。書かない欄は、状態なら表示・印刷・編集でき凍結せず、色・線種・線幅ならレイヤ 0 と同じ。同じ apply の要素は、足したレイヤを指せない。小版 22 から */
export interface AddLayerEdit {
  /** 変更の種類 */
  kind: "add_layer";
  /** 名前。layer の name と同じ決まり */
  name: string;
  /** 表示する。凍結するなら偽 */
  visible?: boolean;
  /** 印刷する */
  printable?: boolean;
  /** 編集できる（ロックしない） */
  editable?: boolean;
  /** 凍結する */
  frozen?: boolean;
  /** 色。aci が要る。inherit は書けない */
  color?: ColorChange;
  /** 線種。name か id で指す。inherit は書けない */
  line_type?: LineTypeChange;
  /** 線幅。mm か default。inherit は書けない */
  line_weight?: LineWeightChange;
}

/** 値を継承する先。layer はレイヤから、block は要素を置いた記号の配置から */
export type Inherit =
  | "layer"
  | "block"
  | (string & {});

/** 新しい色。inherit か、aci・sxf・jww・rgb の少なくとも 1 つのどちらかを書く。aci・sxf・jww は 1 つだけ書く */
export interface ColorChange {
  /** 継承する先 */
  inherit?: Inherit;
  /** AutoCAD の色の番号（1〜255） */
  aci?: number;
  /** SXF の色のコード。1〜16 は既定義色、17 からは利用者定義色を定義の順に数えたもの。小版 29 から */
  sxf?: number;
  /** Jw_cad のペンの番号。0〜9 と、版 420 からの SXF の拡張色 100〜356。小版 30 から */
  jww?: number;
  /** [赤, 緑, 青]。それぞれ 0〜255 */
  rgb?: number[];
}

/** 新しい線種。inherit・name・id のどれか 1 つを書く */
export interface LineTypeChange {
  /** 継承する先 */
  inherit?: Inherit;
  /** 線種の元の名前。図面にある線種で、名前が 1 つに決まるもの */
  name?: string;
  /** 線種の ID。tables が返したもの。同じ名前の項目が複数あるときは書き戻せない。小版 8 から */
  id?: string;
}

/** 新しい線幅。inherit・mm・default のどれか 1 つを書く */
export interface LineWeightChange {
  /** 継承する先 */
  inherit?: Inherit;
  /** 用紙の上の幅（mm）。0 以上 */
  mm?: number;
  /** true なら、文書の既定の線幅 */
  default?: boolean;
}

/** 開いた文書を図面モデル（L2）に変換し、レイヤと線種の表の項目を返す（決定 0156）。小版 8 から */
export interface TablesRequest extends RequestCommon {
  /** op の名前 */
  op: "tables";
  /** 文書の名前。open で作ったもの */
  doc: string;
}

/** 表の項目。図面モデルの順 */
export interface TablesResult {
  /** レイヤ */
  layers: LayerItem[];
  /** 線種。小版 26 から、定義を持つ */
  line_types: LineTypeItem[];
  /** 寸法スタイル。小版 27 から、端部記号と大きさを持つ */
  dimension_styles: DimensionStyleItem[];
  /** 文字スタイル（決定 0158）。要素が名前で指すだけで、ファイルに定義がないものは、定義の欄を持たない項目として後ろに並ぶ。小版 20 から */
  text_styles: TextStyleItem[];
}

/** 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、2D のシーンを blob で返す（決定 0172）。シーンの配置は schema/scene.toml にある。小版 33 から */
export interface SceneRequest extends RequestCommon {
  /** op の名前 */
  op: "scene";
  /** 文書の名前。open で作ったもの */
  doc: string;
  /** 起点。実寸の空間・記号の定義の空間・用紙の ID（drawing と entities が返すもの） */
  from: string;
  /** 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した空間の要素だけを描く。from が用紙のときだけ書ける */
  view?: number;
  /** [x の最小, y の最小, x の最大, y の最大]。起点の座標の長方形。範囲の重なる塊だけを返す */
  region?: number[];
  /** 弦の誤差（起点の長さ）。0 より大きい。書かなければ、用紙からは 0.02 mm、空間からは要素の範囲の対角線の 1/50000 */
  tolerance?: number;
  /** true なら印刷する要素を、書かないか false なら表示する要素を描く */
  print?: boolean;
  /** 色の表の入力の名前（zm_push で渡したもの）。プロファイルの色の語彙と同じ形の JSON で、values に rgb（#rrggbb）と aci・sxf・jww の符号を並べる（決定 0138・0178）。書かなければ、ファイルが RGB を持たない色は未解決の印を立てる */
  palette?: string;
  /** 文字を置くフォント（決定 0188）。書けば、文字を字形で置き、シーンの字形の区画に形と変換を返す。文字スタイルと書式の区間のフォントの名前（シーンの font_names）で引き、同じ名前、なければ ASCII の大文字と小文字を区別しない名前で合わせる。合うものがなければ名前のないフォントで置く。小版 37 から */
  fonts?: SceneFont[];
  /** 形式の宣言値の高さ（キーは dxf.TEXT.40・dxf.MTEXT.40・jww.moji.size_y など）が、フォントのどの寸法に当たるか（決定 0176・0188）。書かないキーは大文字の高さとみなす。fonts を書いたときだけ使う。小版 37 から */
  heights?: Record<string, HeightReference>;
}

/** シーンの文字を置くフォント（決定 0188） */
export interface SceneFont {
  /** 文字が指すフォントの名前（ファイルに書かれた元の名前）。書かなければ、名前の合わない文字に使う既定のフォント。同じ名前と、名前のないものは 1 つまで */
  name?: string;
  /** TrueType・OpenType（CFF・CFF2 を含む）のファイルの入力の名前（zm_push で渡したもの） */
  input: string;
  /** フォントのコレクション（TTC）の中の番号。書かなければ 0 */
  index?: number;
}

/** 宣言された高さが当たるフォントの寸法。cap_height は大文字の高さ、em は em、frame はアセンダーとディセンダーを合わせた高さ */
export type HeightReference =
  | "cap_height"
  | "em"
  | "frame"
  | (string & {});

/** シーンの要約と、シーンの blob の番号 */
export interface SceneResult {
  /** 起点の ID */
  from: string;
  /** 起点のビューの番号。ビューでなければ持たない */
  view?: number;
  /** 起点の座標の単位。用紙とビューからは millimeter */
  unit: Unit;
  /** 使った弦の誤差（起点の長さ） */
  tolerance: number;
  /** 返した描くものの数 */
  counts: SceneCounts;
  /** 返した塊の範囲。起点の座標。塊がなければ持たない */
  extent?: Extent;
  /** シーンの blob の番号（種類は scene）。zm_take で取り出す */
  blob: number;
}

/** シーンの描くものの種類ごとの数 */
export interface SceneCounts {
  /** 塊 */
  chunks: number;
  /** 線（折れ線） */
  lines: number;
  /** 線と面の頂点 */
  vertices: number;
  /** 面 */
  faces: number;
  /** 文字 */
  texts: number;
  /** 点 */
  points: number;
  /** 画像 */
  images: number;
  /** 置いた字形。fonts を書かなければ 0。小版 37 から */
  glyphs: number;
}

/** 開いた文書を図面モデル（L2）に変換し、起点から配置をたどって、プロファイルの規則と語彙で照合する（決定 0138・0173・0179）。小版 34 から */
export interface CheckRequest extends RequestCommon {
  /** op の名前 */
  op: "check";
  /** 文書の名前。open で作ったもの */
  doc: string;
  /** プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す */
  profiles: string[];
  /** 照合に使うプロファイル。profiles の中から選ぶ */
  profile: ProfileRef;
  /** たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる */
  from?: string;
  /** 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける */
  view?: number;
  /** 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から */
  file_name?: string;
  /** 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0 */
  offset?: number;
  /** 返す指摘の数の上限。書かなければ 1000 */
  limit?: number;
  /** 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く */
  palette?: string;
}

/** プロファイルの識別（決定 0138） */
export interface ProfileRef {
  /** 基準の系統（mlit-cad など） */
  id: string;
  /** 基準の版（2025-12 など） */
  edition: string;
}

/** 照合の結果。指摘は offset から limit 個まで */
export interface CheckResult {
  /** 照合に使ったプロファイルの鎖。extends の基から順 */
  chain: ProfileIdentity[];
  /** 照合した規則ごとの数。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す */
  rules: CheckRule[];
  /** 指摘。規則の順に、規則ごとにたどった順 */
  findings: CheckFinding[];
  /** 全体の指摘の数 */
  total: number;
  /** 続きの最初の指摘の位置。続きがなければ持たない */
  next_offset?: number;
}

/** 照合に使ったプロファイルの識別と、データの直し */
export interface ProfileIdentity {
  /** 基準の系統 */
  id: string;
  /** 基準の版 */
  edition: string;
  /** このデータの直し。1 から */
  revision: number;
}

/** 規則ごとの数 */
export interface CheckRule {
  /** 規則の id */
  id: string;
  /** 照合の種類（layer-name など） */
  check: string;
  /** 指摘の数。照合できなかったものを除く */
  findings: number;
  /** 照合できなかった数 */
  unsupported: number;
}

/** 指摘。diagnostic は診断と同じ形で、describe で文にできる。severity は規則の level で、照合できない値（profile.<種類>.unsupported）は info。count は 1 */
export interface CheckFinding {
  /** 規則の id */
  rule: string;
  /** code・重要度・場所（要素・実効のレイヤ・用紙の ID）・見つけた値 */
  diagnostic: Diagnostic;
}

/** zm_push で渡した IFC（IFC-SPF か ifcZIP）を、zm_push で渡した EXPRESS のスキーマに照らしてチェックする（決定 0207・0216）。インスタンスの実体の名前・抽象・属性の数・値の型・省略を見る。WHERE・UNIQUE・RULE の規則は評価せず、数を返す。zumen は EXPRESS を同梱しない。小版 42 から */
export interface ValidateRequest extends RequestCommon {
  /** op の名前 */
  op: "validate";
  /** IFC の入力の名前（zm_push で渡した .ifc か .ifczip） */
  input: string;
  /** EXPRESS の入力の名前（zm_push で渡した .exp） */
  express: string;
  /** 返す最初の指摘の位置。前の応答の next_offset を渡す。書かなければ 0 */
  offset?: number;
  /** 返す指摘の数の上限。書かなければ 1000 */
  limit?: number;
}

/** スキーマに照らしたチェックの結果。指摘は offset から limit 個まで。スキーマの名前の違いと評価しない規則は、診断で返す */
export interface ValidateResult {
  /** EXPRESS のスキーマの名前（大文字） */
  schema: string;
  /** IFC の FILE_SCHEMA の名前 */
  file_schema: string;
  /** EXPRESS のバイト列のハッシュ（v2: と 32 桁の 16 進）。どの EXPRESS でチェックしたかを残す */
  express_hash: string;
  /** 評価しなかった規則（WHERE・UNIQUE・RULE）の数。確認できなかった項目 */
  rules_not_evaluated: number;
  /** 指摘。インスタンスの番号の順に、属性の順。診断と同じ形で、describe で文にできる。count は 1 */
  findings: Diagnostic[];
  /** 全体の指摘の数 */
  total: number;
  /** 続きの最初の指摘の位置。続きがなければ持たない */
  next_offset?: number;
}

/** レビューの文書（決定 0214・0218）の 1 つの対象を、開いた図面の文書の版へ移す。要素の参照を図面の要素の ID で引き、参照ごとに unchanged・changed・removed を返す。場所と図形に元の版のハッシュと状態を書き、対象のハッシュを文書のハッシュにする。2D の点は動かさない。移した文書は blobs の file で返す。小版 43 から */
export interface ReviewCarryRequest extends RequestCommon {
  /** op の名前 */
  op: "review_carry";
  /** 新しい版の文書の名前。open で作ったもの。小版 44 から、IFC の建物モデルの文書も渡せ、要素の参照を建物モデルの要素の ID とハッシュで引く */
  doc: string;
  /** レビューの文書の入力の名前（zm_push で渡した JSON） */
  review: string;
  /** 移す対象の ID。レビューの文書の targets の id */
  target: string;
  /** 返す最初の参照の位置。前の応答の next_offset を渡す。書かなければ 0 */
  offset?: number;
  /** 返す参照の数の上限。書かなければ 1000 */
  limit?: number;
}

/** 移した結果。参照は offset から limit 個まで。移した文書は blobs の file で返す */
export interface ReviewCarryResult {
  /** 移す前の対象のハッシュ */
  from: string;
  /** 移した後の対象のハッシュ。文書のハッシュ */
  hash: string;
  /** 要素の参照ごとの状態。指摘の順に、場所の順、参照の順 */
  references: CarriedReference[];
  /** 全体の参照の数 */
  total: number;
  /** 続きの最初の参照の位置。続きがなければ持たない */
  next_offset?: number;
}

/** 移した要素の参照 */
export interface CarriedReference {
  /** 指摘の ID */
  topic: string;
  /** 場所の ID */
  location: string;
  /** 要素の ID */
  element: string;
  /** 状態 */
  state: CarriedState;
}

/** 移したときの状態（決定 0214）。unchanged は要素のハッシュが同じ、changed は違うか元のハッシュがない、removed は新しい版に要素がない */
export type CarriedState =
  | "unchanged"
  | "changed"
  | "removed"
  | (string & {});

/** 開いた文書を check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0214・0218）に足す。指摘の ID は対象のハッシュ・規則・診断から決め、取り込み済みの指摘は足さない。足した文書は blobs の file で返す。小版 43 から */
export interface ReviewAdoptRequest extends RequestCommon {
  /** op の名前 */
  op: "review_adopt";
  /** 照合する文書の名前。open で作ったもの */
  doc: string;
  /** プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す */
  profiles: string[];
  /** 照合に使うプロファイル。profiles の中から選ぶ */
  profile: ProfileRef;
  /** たどる起点。実寸の空間・記号の定義の空間・用紙の ID。書かなければ、すべての用紙から、用紙がなければ実寸の空間からたどる */
  from?: string;
  /** 起点の用紙のビューの番号。用紙のビューの並びの順で、0 から数える。書けば、そのビューを通した要素と、そのビューの縮尺だけを照合する。from が用紙のときだけ書ける */
  view?: number;
  /** 照合するファイルの名前（拡張子を含み、フォルダーを含まない）。file-name の規則で比べる。書かなければ、file-name の規則は照合できない（決定 0204）。小版 40 から */
  file_name?: string;
  /** 色の表の入力の名前（zm_push で渡したもの）。scene の palette と同じ形。ファイルが RGB を持たない色の RGB を引く */
  palette?: string;
  /** 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す */
  review?: string;
  /** 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す */
  target: string;
  /** 取り込む指摘の番号の列。check の指摘の並びの順で、0 から数える */
  findings: number[];
}

/** 取り込んだ結果。足した文書は blobs の file で返す */
export interface ReviewAdoptResult {
  /** 足した指摘の ID。取り込み済みで足さなかった指摘は持たない */
  added: string[];
  /** 足した後の文書の指摘の数 */
  topics: number;
}

/** 開いた IFC の建物モデル（決定 0207・0219）の要約を返す。小版 44 から */
export interface BuildingRequest extends RequestCommon {
  /** op の名前 */
  op: "building";
  /** 文書の名前。open で IFC を開いたもの */
  doc: string;
}

/** 建物モデルの要約 */
export interface BuildingResult {
  /** IFC のスキーマの名前（FILE_SCHEMA の最初） */
  schema: string;
  /** 入力のバイト列のハッシュ。review_carry の対象の版に使う */
  hash: string;
  /** プロジェクトの要素の ID。なければ持たない */
  project?: string;
  /** プロジェクトの名前。なければ持たない */
  project_name?: string;
  /** プロジェクトの単位 */
  units: BuildingUnit[];
  /** 要素の数 */
  elements: number;
  /** クラスごとの要素の数。クラスの名前の順 */
  classes: ClassCount[];
}

/** 単位 */
export interface BuildingUnit {
  /** 単位の種類（LENGTHUNIT・AREAUNIT など） */
  kind: string;
  /** SI の接頭辞（MILLI など） */
  prefix?: string;
  /** 単位の名前（METRE、換算する単位なら inch など） */
  name: string;
  /** 換算する単位の、SI の単位での大きさ。SI の単位は持たない */
  scale?: number;
}

/** クラスの要素の数 */
export interface ClassCount {
  /** IFC のクラスの名前（IfcWall など） */
  name: string;
  /** 要素の数 */
  count: number;
}

/** 開いた IFC の建物モデルの要素を、書いた順に返す。成分は components で選ぶ（決定 0207・0219）。小版 44 から */
export interface BuildingElementsRequest extends RequestCommon {
  /** op の名前 */
  op: "building_elements";
  /** 文書の名前。open で IFC を開いたもの */
  doc: string;
  /** 返す要素の ID。書けば、その要素だけを、書いた順に返す。ない ID は返さない */
  ids?: string[];
  /** IFC のクラスの名前。書けば、そのクラスの要素だけを返す。大文字と小文字を区別しない */
  class?: string;
  /** 返す成分の名前。1 つ以上で、重ねない。書かなければ class と identity を返す */
  components?: BuildingComponent[];
  /** 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0 */
  offset?: number;
  /** 返す数の上限。書かなければ 1000 */
  limit?: number;
}

/** building_elements で返す成分（決定 0207）。relations は索引から求める子・参照・型の実例・開口・埋める要素・構成員 */
export type BuildingComponent =
  | "identity"
  | "class"
  | "classification"
  | "spatial"
  | "parent"
  | "placement"
  | "type_of"
  | "properties"
  | "quantities"
  | "material"
  | "opening"
  | "group"
  | "connection"
  | "level"
  | "zone"
  | "style"
  | "relations"
  | (string & {});

/** 要素の一覧。offset から limit 個まで */
export interface BuildingElementsResult {
  /** 要素 */
  elements: BuildingElement[];
  /** 選んだ要素の数 */
  total: number;
  /** 続きの最初の位置。続きがなければ持たない */
  next_offset?: number;
}

/** 建物モデルの要素。成分は、選んだもののうち要素が持つものだけを持つ */
export interface BuildingElement {
  /** 要素の ID（GlobalId か、i と番号） */
  id: string;
  /** 要素の成分のハッシュ（v1: と 32 桁の 16 進、決定 0207） */
  hash: string;
  /** 名前・説明・タグ・長い名前 */
  identity?: IdentityComponent;
  /** クラス */
  class?: ClassComponent;
  /** 分類の参照 */
  classification?: ClassificationReference[];
  /** 空間の要素の種類と高さ */
  spatial?: SpatialComponent;
  /** 親と、参照される空間 */
  parent?: ParentComponent;
  /** 配置 */
  placement?: PlacementComponent;
  /** 型の要素の ID */
  type_of?: string;
  /** プロパティのセット */
  properties?: PropertySet[];
  /** 数量のセット */
  quantities?: QuantitySet[];
  /** 材料 */
  material?: MaterialComponent;
  /** 開口と、埋める要素 */
  opening?: OpeningComponent;
  /** 属するグループの要素の ID */
  group?: string[];
  /** 接続 */
  connection?: ConnectionLink[];
  /** レベル */
  level?: LevelComponent;
  /** 区画 */
  zone?: ZoneComponent;
  /** 表示のスタイル */
  style?: StyleComponent;
  /** 索引から求める関係 */
  relations?: RelationsComponent;
}

/** 名前・説明・タグ・長い名前 */
export interface IdentityComponent {
  /** 名前 */
  name?: string;
  /** 説明 */
  description?: string;
  /** タグ */
  tag?: string;
  /** 長い名前 */
  long_name?: string;
}

/** クラス */
export interface ClassComponent {
  /** IFC4 のクラスの名前。IFC2X3 の StandardCase などは、元の名前を original に残して直す */
  name: string;
  /** PredefinedType */
  predefined?: string;
  /** ObjectType など、USERDEFINED のときの名前 */
  user_defined?: string;
  /** ファイルに書いた実体の名前 */
  original: string;
}

/** 分類の参照 */
export interface ClassificationReference {
  /** 分類の体系の名前 */
  system?: string;
  /** 体系の版 */
  edition?: string;
  /** 分類の記号 */
  code?: string;
  /** 分類の名前 */
  name?: string;
}

/** 単位とともに持つ値 */
export interface Measured {
  /** 値 */
  value: number;
  /** 単位。解けなければ持たない */
  unit?: BuildingUnit;
}

/** 空間の要素 */
export interface SpatialComponent {
  /** site・building・storey・space・spatial_zone か、元の実体の名前 */
  kind: string;
  /** 高さ */
  elevation?: Measured;
  /** 基準の高さ */
  ref_elevation?: Measured;
}

/** 親 */
export interface ParentComponent {
  /** 親の要素 */
  of?: Membership;
  /** 参照される空間の要素の ID */
  referenced_in: string[];
}

/** 親との関係 */
export interface Membership {
  /** 親の要素の ID */
  target: string;
  /** 関係の種類 */
  kind: ParentKind;
  /** 入れ子の順。0 から */
  order?: number;
}

/** 配置 */
export interface PlacementComponent {
  /** 配置の親の要素の ID */
  relative_to?: string;
  /** 親をたどった世界の変換。3 行 4 列（回転と移動）。解けなければ持たない */
  world?: number[][];
  /** 解けなかった配置の実体の名前 */
  unsupported?: string;
}

/** 値 */
export interface BuildingValue {
  /** 値の型の名前（IfcLengthMeasure・IfcLabel など） */
  type_name?: string;
  /** 値の種類 */
  kind: BuildingValueKind;
  /** real と、2^53 − 1 までの integer の値 */
  number?: number;
  /** text と enumeration の値と、2^53 − 1 を超える integer の値 */
  text?: string;
  /** boolean と logical の値。logical の不明は持たない */
  boolean?: boolean;
  /** element の要素の ID */
  element?: string;
  /** list の値 */
  items?: BuildingValue[];
}

/** プロパティのセット */
export interface PropertySet {
  /** 名前 */
  name: string;
  /** プロパティ */
  properties: Property[];
}

/** プロパティ */
export interface Property {
  /** 名前 */
  name: string;
  /** 値。値がなければ持たない */
  value?: BuildingValue;
  /** 単位 */
  unit?: BuildingUnit;
}

/** 数量のセット */
export interface QuantitySet {
  /** 名前 */
  name: string;
  /** 計測の方法 */
  method?: string;
  /** 数量 */
  quantities: Quantity[];
}

/** 数量 */
export interface Quantity {
  /** 名前 */
  name: string;
  /** 種類 */
  kind: QuantityKind;
  /** 値 */
  value: Measured;
  /** 式 */
  formula?: string;
}

/** 材料 */
export interface MaterialComponent {
  /** 材料の組み方 */
  kind: MaterialKind;
  /** 部分 */
  parts: MaterialPart[];
  /** 層の向き */
  direction?: string;
  /** 層の基準からのずれ */
  offset?: Measured;
}

/** 材料の部分 */
export interface MaterialPart {
  /** 材料の要素の ID */
  material?: string;
  /** 名前 */
  name?: string;
  /** 厚さ */
  thickness?: Measured;
}

/** 開口 */
export interface OpeningComponent {
  /** この開口が穴を開ける要素の ID */
  voids?: string;
  /** この要素が埋める開口の要素の ID */
  fills?: string;
}

/** 接続 */
export interface ConnectionLink {
  /** 関係の実体の名前 */
  kind: string;
  /** 接続する要素の ID */
  target: string;
}

/** レベル */
export interface LevelComponent {
  /** 名前 */
  name?: string;
  /** 種類 */
  kind?: string;
  /** 高さ */
  height: Measured;
  /** 基準 */
  datum?: string;
}

/** 区画 */
export interface ZoneComponent {
  /** 区画の種類 */
  kind?: string;
  /** 構成する要素の ID */
  members: string[];
}

/** 表示のスタイル */
export interface StyleComponent {
  /** 面の色の RGB（0 から 1） */
  surface_color?: number[];
  /** 透明度（0 から 1） */
  transparency?: number;
  /** 線の色の RGB（0 から 1） */
  curve_color?: number[];
  /** 提示のレイヤの名前 */
  layers: string[];
}

/** 索引から求める関係。要素の ID の列 */
export interface RelationsComponent {
  /** 子（集約・入れ子・包含） */
  children: string[];
  /** 参照する要素 */
  referenced: string[];
  /** 型の実例 */
  instances: string[];
  /** 開口 */
  openings: string[];
  /** 開口を埋める要素 */
  fillings: string[];
  /** グループ・システム・区画の構成員 */
  members: string[];
}

/** 親との関係の種類。集約・入れ子・包含 */
export type ParentKind =
  | "aggregates"
  | "nests"
  | "contained"
  | (string & {});

/** 値の種類 */
export type BuildingValueKind =
  | "real"
  | "integer"
  | "text"
  | "boolean"
  | "logical"
  | "enumeration"
  | "element"
  | "list"
  | (string & {});

/** 数量の種類 */
export type QuantityKind =
  | "length"
  | "area"
  | "volume"
  | "count"
  | "weight"
  | "time"
  | (string & {});

/** 材料の組み方。単一・層・断面・構成・一覧 */
export type MaterialKind =
  | "single"
  | "layers"
  | "profiles"
  | "constituents"
  | "list"
  | (string & {});

/** 仕様の結果。unknown は確かめられない facet があった */
export type IdsStatus =
  | "passed"
  | "failed"
  | "unknown"
  | (string & {});

/** 開いた IFC の建物モデルを、zm_push で渡した IDS で照合する（決定 0217・0219）。確かめられない facet は、指摘と分けて info の指摘で返す。小版 44 から */
export interface IdsCheckRequest extends RequestCommon {
  /** op の名前 */
  op: "ids_check";
  /** 文書の名前。open で IFC を開いたもの */
  doc: string;
  /** IDS の入力の名前（zm_push で渡した .ids） */
  ids: string;
  /** EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす */
  express?: string;
  /** 返す最初の位置。前の応答の next_offset を渡す。書かなければ 0 */
  offset?: number;
  /** 返す数の上限。書かなければ 1000 */
  limit?: number;
}

/** 照合の結果。指摘は offset から limit 個まで */
export interface IdsCheckResult {
  /** IDS の題 */
  title?: string;
  /** 仕様ごとの結果。IDS の順 */
  specifications: IdsSpecification[];
  /** 指摘。仕様の順 */
  findings: IdsFinding[];
  /** 全体の指摘の数 */
  total: number;
  /** 続きの最初の位置。続きがなければ持たない */
  next_offset?: number;
}

/** 仕様の結果 */
export interface IdsSpecification {
  /** 名前 */
  name: string;
  /** 識別子 */
  identifier?: string;
  /** 結果 */
  status: IdsStatus;
  /** 適用の条件に合う要素の数 */
  applicable: number;
  /** 要件を満たさない要素の数 */
  failed: number;
  /** 確かめられなかった要素の数 */
  unsupported: number;
}

/** 指摘。diagnostic は診断と同じ形で、describe で文にできる */
export interface IdsFinding {
  /** 仕様の名前 */
  specification: string;
  /** code・重要度・場所（要素の ID）・値 */
  diagnostic: Diagnostic;
}

/** 開いた IFC を ids_check と同じに照合し、選んだ指摘だけを開いた指摘としてレビューの文書（決定 0218・0219）に足す。足した文書は blobs の file で返す。小版 44 から */
export interface ReviewAdoptIdsRequest extends RequestCommon {
  /** op の名前 */
  op: "review_adopt_ids";
  /** 文書の名前。open で IFC を開いたもの */
  doc: string;
  /** IDS の入力の名前（zm_push で渡した .ids） */
  ids: string;
  /** EXPRESS の入力の名前（zm_push で渡した .exp）。渡せば、IFC の属性の全体を EXPRESS の名前で照らす。渡さなければ、建物モデルの成分にある属性だけを照らす */
  express?: string;
  /** 足す先のレビューの文書の入力の名前（zm_push で渡した JSON）。書かなければ空の文書に足す */
  review?: string;
  /** 指摘が指す対象の ID。文書になければ、文書の形式とハッシュで足す */
  target: string;
  /** 取り込む指摘の番号の列。ids_check の指摘の並びの順で、0 から数える */
  findings: number[];
}

/** zm_push で渡した BCF 3.0 の bcfzip を読み、開いた IFC を対象にしたレビューの文書（決定 0214・0220）にする。topic は指摘に、comment はコメントに、viewpoint は 3D の場所に写し、要素の参照のハッシュは建物モデルの要素のハッシュで埋める。文書は blobs の file で返す。小版 45 から */
export interface BcfReadRequest extends RequestCommon {
  /** op の名前 */
  op: "bcf_read";
  /** BCF が指す建物モデルの文書の名前。open で IFC を開いたもの */
  doc: string;
  /** bcfzip の入力の名前（zm_push で渡した .bcf） */
  input: string;
  /** レビューの文書の対象の ID */
  target: string;
  /** 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す */
  statuses?: BcfStatus[];
}

/** 状態の対応 */
export interface BcfStatus {
  /** BCF の TopicStatus の文字列 */
  text: string;
  /** 指摘の状態 */
  status: ReviewStatus;
}

/** 指摘の状態（決定 0214） */
export type ReviewStatus =
  | "open"
  | "resolved"
  | "closed"
  | (string & {});

/** 読んだ bcfzip */
export interface BcfReadResult {
  /** BCF の版 */
  version: string;
  /** 指摘の数 */
  topics: number;
}

/** zm_push で渡したレビューの文書を、BCF 3.0 の bcfzip に書く（決定 0214・0220）。target を対象にした 3D の場所を viewpoint にし、2D の場所と図形は書かずに診断する。original を渡せば、変えていない topic を元のバイト列のまま書き、変えた topic でも写さない要素と知らない要素を保つ。bcfzip は blobs の file で返す。小版 45 から */
export interface BcfWriteRequest extends RequestCommon {
  /** op の名前 */
  op: "bcf_write";
  /** レビューの文書の入力の名前（zm_push で渡した JSON） */
  review: string;
  /** viewpoint にする 3D の場所の対象の ID */
  target: string;
  /** 元の bcfzip の入力の名前（zm_push で渡した .bcf）。書かなければ新しく書く */
  original?: string;
  /** 指摘の状態と BCF の TopicStatus の文字列の対応の表。書かなければ Open・Resolved・Closed。読むとき、表にない文字列は open にし、元の文字列を status_label に残す */
  statuses?: BcfStatus[];
  /** 種類のない指摘に書く TopicType */
  kind?: string;
  /** 作者のない指摘とコメントに書く作者 */
  author?: string;
  /** 日時のない指摘とコメントに書く日時（RFC 3339） */
  date?: string;
}

/** 書いた bcfzip */
export interface BcfWriteResult {
  /** 書いた指摘の数 */
  topics: number;
}

/** 表の項目 */
export interface TableItem {
  /** ID。表のレコードがハンドルを持てば layer/h10・line-type/h14、なければ読んだ順の番号（layer/2）。寸法スタイルは dimension-style/h1A か、寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
  id: string;
  /** 元の名前。翻訳しない */
  name: string;
  /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
  hash: string;
}

/** レイヤの表の項目（決定 0162）。小版 22 から、状態とスタイルを持つ */
export interface LayerItem {
  /** ID。表のレコードがハンドルを持てば layer/h10、なければ読んだ順の番号（layer/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
  id: string;
  /** 元の名前。翻訳しない */
  name: string;
  /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
  hash: string;
  /** ファイルに定義がある。偽なら、要素が名前で指すだけのレイヤで、ほかの欄を持たない。小版 22 から */
  defined: boolean;
  /** 表示する。小版 22 から */
  visible?: boolean;
  /** 印刷する。小版 22 から */
  printable?: boolean;
  /** 編集できる。小版 22 から */
  editable?: boolean;
  /** 凍結する。小版 22 から */
  frozen?: boolean;
  /** 色。inherit を持たない。定義が色を持たなければ書かない。小版 22 から */
  color?: ColorChange;
  /** 線種の ID。定義が線種を持たなければ書かない。小版 22 から */
  line_type_id?: string;
  /** 線幅。inherit を持たない。定義が線幅を持たなければ書かない。小版 22 から */
  line_weight?: LineWeightChange;
}

/** 線種の表の項目（決定 0164）。小版 26 から、定義を持つ */
export interface LineTypeItem {
  /** ID。表のレコードがハンドルを持てば line-type/h14、なければ読んだ順の番号（line-type/2）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
  id: string;
  /** 元の名前。翻訳しない */
  name: string;
  /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
  hash: string;
  /** ファイルに定義がある。偽なら、要素が名前で指すだけの線種で、ほかの欄を持たない。小版 26 から */
  defined: boolean;
  /** 元の説明。小版 26 から */
  description?: string;
  /** 線と間の長さの列。正は線、負は間、0 は点。空なら実線。長さの単位は paper が決める。小版 26 から */
  pattern?: number[];
  /** 列の長さが用紙の mm なら真、文書の単位なら偽。用紙の mm の列は line_type で変えられない。小版 26 から */
  paper?: boolean;
  /** 列の要素に形か文字を添える。添えれば、line_type で列の要素の数を変えられない。小版 26 から */
  marked?: boolean;
  /** 列の要素に添える形と文字（決定 0150・0166）。添えなければ空。小版 28 から */
  marks?: LineTypeMark[];
}

/** 寸法スタイルの表の項目（決定 0165）。小版 27 から、端部記号と大きさを持つ */
export interface DimensionStyleItem {
  /** ID。寸法スタイルのレコードがハンドルを持てば dimension-style/h1A、なければ寸法スタイルの並びの中の順（dimension-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
  id: string;
  /** 元の名前。翻訳しない */
  name: string;
  /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
  hash: string;
  /** 寸法線の 1 本目と 2 本目の補助線の側の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から */
  ends?: string[];
  /** 引出線の端部記号の、ブロックの名前。空の名前は既定の矢印。小版 27 から */
  leader?: string;
  /** 端部記号の大きさ。文書の単位で、scale を掛ける前の値。小版 27 から */
  size?: number;
  /** 端部記号の代わりに描く斜線の大きさ。0 なら斜線を描かない。小版 27 から */
  tick?: number;
  /** 大きさに掛ける倍率。0 はビューの縮尺から決める。小版 27 から */
  scale?: number;
}

/** 線種の列の要素に添える形か文字（決定 0150・0166）。要素の始まりに、線の向きとずれで置く。小版 28 から */
export interface LineTypeMark {
  /** 添える列の要素の番号（pattern の添字） */
  index: number;
  /** 形（shape）か文字（text） */
  kind: LineTypeMarkKind;
  /** 形の形のファイル。元の形式が指す文字スタイルがなければ書かない */
  font?: Font;
  /** 形の番号 */
  number?: number;
  /** 文字の文字列 */
  text?: string;
  /** 文字の文字スタイルの元の名前。元の形式が指す文字スタイルがなければ書かない */
  style?: string;
  /** 文字の文字スタイルの ID。tables の text_styles の ID で、名前で引いたもの（大文字と小文字を区別せず、同じ名前があれば先の項目） */
  style_id?: string;
  /** 文字の高さ。文字スタイルが決まった高さを持てば、それに scale を掛けたもの。持たなければ scale。単位は pattern と同じ */
  height?: number;
  /** 倍率。文字なら、文字スタイルの高さへの倍率 */
  scale: number;
  /** 回転（度、反時計回り） */
  rotation: number;
  /** 回転の基準 */
  rotation_basis: MarkRotation;
  /** 置く位置のずれ（x・y）。線の向きを x とする。単位は pattern と同じ */
  offset: number[];
}

/** 線種に添えるものの種類。shape は形のファイルの形、text は文字 */
export type LineTypeMarkKind =
  | "shape"
  | "text"
  | (string & {});

/** 回転の基準。relative は線の向きから、absolute は文書の x 軸から測る。upright は線の向きから測り、文字が逆さに見えるなら半回転する */
export type MarkRotation =
  | "relative"
  | "absolute"
  | "upright"
  | (string & {});

/** 文字スタイルの表の項目（決定 0158）。フォントと縦書きは、この文字スタイルを指す文字に効く。高さ・幅の倍率・傾き・裏返し・上下反転は、新しく作る文字の既定の値で、今の文字はそれぞれの値を持つ。小版 20 から */
export interface TextStyleItem {
  /** ID。表のレコードがハンドルを持てば text-style/h11、なければ文字スタイルの並びの中の順（text-style/0）。ID が重なれば、2 つ目から /1・/2 を足す。書き戻しても変わらない（決定 0156） */
  id: string;
  /** 元の名前。翻訳しない */
  name: string;
  /** 項目のハッシュ（v2: と 32 桁の 16 進、決定 0156） */
  hash: string;
  /** ファイルに定義がある。偽なら、ほかの欄を持たない */
  defined: boolean;
  /** 主のフォント。ファイルが決めていなければ書かない */
  font?: Font;
  /** 大きいフォント（漢字などの形のファイル）。なければ書かない */
  big_font?: Font;
  /** 決まった高さ。文書の単位。決めなければ書かない */
  height?: number;
  /** 幅の倍率 */
  width?: number;
  /** 傾き（度） */
  oblique?: number;
  /** 左右を裏返す */
  backward?: boolean;
  /** 上下を反転する */
  upside_down?: boolean;
  /** 縦書き */
  vertical?: boolean;
}

/** フォントの指定。取得と字形の評価は使い手が行う */
export interface Font {
  /** 元の名前。形のファイルならファイルの名前、名前で指すフォントなら書体の名前。翻訳しない */
  name: string;
  /** 種類 */
  kind: FontKind;
  /** 太字。名前で指すフォントだけ */
  bold?: boolean;
  /** 斜体。名前で指すフォントだけ */
  italic?: boolean;
  /** 文字集合の番号（128 は Shift_JIS）。名前で指すフォントで、元の形式が決めていれば */
  charset?: number;
  /** ピッチと系統の番号。名前で指すフォントで、元の形式が決めていれば */
  pitch?: number;
}

/** フォントの種類。shape は形のファイル（SHX など）、named は名前で指すフォント（TrueType など） */
export type FontKind =
  | "shape"
  | "named"
  | (string & {});

/** 要素の出所。L1 のレコードの番号かバイト範囲（決定 0008）と、形式が持つ鍵（決定 0156）。ファイルから読んでいない要素は、レコードの番号もバイト範囲も持たない */
export interface Source {
  /** L1 のレコードの番号 */
  record?: number;
  /** L1 のバイト範囲 */
  bytes?: ByteRange;
  /** AutoCAD の系統のハンドル。大文字の 16 進。書き戻しても変わらない */
  handle?: string;
  /** SXF のインスタンスの番号（#n の n）。10 進の文字列。小版 6 から */
  instance?: string;
}

/** 応答。失敗したときも同じ形で、result を持たない */
export interface Response<R = unknown> {
  /** メッセージのスキーマの大版 */
  v: number;
  /** op ごとの結果（<op>_result の形）。失敗したときは持たない */
  result?: R;
  /** 診断。入力の順 */
  diagnostics: Diagnostic[];
  /** バイナリの結果。take で取り出す。次の call で捨てる */
  blobs: Blob[];
}

/** 診断。人向けの文は持たず、文は describe で組み立てる（決定 0009） */
export interface Diagnostic {
  /** 変わらない名前（budget.records.exceeded など）。catalog/codes.toml にある */
  code: string;
  /** 重さ。code ごとに決まる */
  severity: Severity;
  /** 変換の結果の段。code ごとに決まる。変換の結果を表さない code では持たない */
  fidelity?: Fidelity;
  /** まとめた件数 */
  count: number;
  /** 場所。入力の順に先頭の数件 */
  at: Place[];
  /** 仕様の節 */
  cite?: Cite;
  /** 文に差し込む値。名前と型は code ごとに台帳が決める。2^53 − 1 を超える整数は文字列にする */
  params: Record<string, ParamValue>;
}

/** 診断の重さ（決定 0009）。info は何も失っていない、warning は結果を使える、error は失敗した */
export type Severity =
  | "info"
  | "warning"
  | "error"
  | (string & {});

/** 変換の結果の段のうち、`native` 以外のもの（決定 0009） */
export type Fidelity =
  | "equivalent"
  | "approximated"
  | "drawn"
  | "dropped"
  | (string & {});

/** 診断の場所。ID と、L1 のレコード・バイト範囲（決定 0008 の出所） */
export interface Place {
  /** ID */
  id?: string;
  /** L1 のレコードの番号 */
  record?: number;
  /** L1 のバイト範囲 */
  bytes?: ByteRange;
}

/** バイト範囲（start から end の手前まで） */
export interface ByteRange {
  /** 先頭の位置 */
  start: number;
  /** 終わりの位置（含まない） */
  end: number;
}

/** 仕様の節。仕様の本文は写さない */
export interface Cite {
  /** 仕様の一覧での名前 */
  spec: string;
  /** 節 */
  section: string;
}

/** 診断の値。数か文字列 */
export type ParamValue =
  /** 数 */
  | number
  /** 文字列。図面に記録された名前、列挙の値のキー、2^53 − 1 を超える整数 */
  | string;

/** バイナリの結果 */
export interface Blob {
  /** 番号。zm_take に渡す */
  id: number;
  /** 種類 */
  kind: BlobKind;
  /** バイト数 */
  size: number;
}

/** バイナリの結果の種類。file は書き出したファイル、image は描いた画像、scene は 2D のシーン（schema/scene.toml の配置、決定 0137・0172）、arrow は表（Arrow の IPC ストリーム。ネイティブでは zm_take_arrow でも取り出せる） */
export type BlobKind =
  | "file"
  | "image"
  | "scene"
  | "arrow"
  | (string & {});

/** zm_session_new に渡す設定。渡さなければ既定の設定 */
export interface SessionOptions {
  /** メッセージのスキーマの大版。今は 1 */
  v: number;
  /** 実行の形。書かなければ、ネイティブのライブラリでは `native`、wasm のモジュールでは `wasm`（決定 0013） */
  engine?: Engine;
  /** engine が `process` のときの、子プロセスにする CLI（zumen）の実行ファイルのパス。書かなければ、ライブラリと同じフォルダーの zumen を使う（決定 0013・0200） */
  process_path?: string;
}

/** 実行の形（決定 0013）。`native` はプロセスの中でコアを動かす、`process` は子プロセスの CLI に送る、`wasm` は wasm のモジュールで動かす */
export type Engine =
  | "native"
  | "process"
  | "wasm"
  | (string & {});

/** 線形（決定 0230）の上の所を、追加距離か測点の表記から求め、点・方向角・高さ・勾配を返す。線形は、要求に書く（alignment）か、open で開いた J-LandXML の文書（doc）の線形を id で指す。distances・stations・interval の所を、この順に並べて返す。求められない所は、理由を診断して除く。doc を渡せば、文書の線形と地表面の要約も返す。図面のモジュールに置く。小版 46 から（doc と id は小版 47 から） */
export interface CivilRequest extends RequestCommon {
  /** op の名前 */
  op: "civil";
  /** 線形。doc と一緒には書かない */
  alignment?: CivilAlignment;
  /** open で開いた J-LandXML の文書の名前。小版 47 から */
  doc?: string;
  /** doc の線形の ID。文書に線形が 1 つなら書かなくてよい。小版 47 から */
  id?: string;
  /** 追加距離（メートル）の列 */
  distances?: number[];
  /** 測点の表記（No.12+5.000・No.12+5・No.12）の列。測点の不連続のために同じ値が何か所にもあれば、最初の所にして診断する */
  stations?: string[];
  /** 線形の初めから終わりまで、この間隔（メートル）ごとの所を足す。終わりの所も足す */
  interval?: number;
  /** 測点の間隔（メートル）。書かなければ 20 */
  pitch?: number;
  /** 測点の表記の端数の小数の桁数。書かなければ 3 */
  digits?: number;
}

/** 線形。点は地図の空間（北と東、メートル）に置き、方向角は北から時計回りのラジアンにする。半径は、右へ曲がるものを正にする（決定 0230） */
export interface CivilAlignment {
  /** 平面線形の区間。線形の初めから順に並べる */
  horizontal: CivilHorizontal[];
  /** 縦断線形の区間。書かなければ高さを返さない */
  vertical?: CivilVertical[];
  /** 線形の初めの測点の値（メートル）。書かなければ 0 */
  start_station?: number;
  /** 測点の不連続。追加距離の順に並べる */
  breaks?: CivilStationBreak[];
}

/** 平面線形の区間 */
export interface CivilHorizontal {
  /** 区間の種類 */
  kind: CivilHorizontalKind;
  /** 初めの点の北（メートル） */
  northing: number;
  /** 初めの点の東（メートル） */
  easting: number;
  /** 初めの方向角（北から時計回りのラジアン） */
  direction: number;
  /** 長さ（メートル） */
  length: number;
  /** 円弧の半径（メートル）。arc に書く */
  radius?: number;
  /** 緩和曲線の初めの半径（メートル）。直線の側は書かない */
  start_radius?: number;
  /** 緩和曲線の終わりの半径（メートル）。直線の側は書かない */
  end_radius?: number;
  /** 緩和曲線の種類。transition に書き、書かなければ clothoid */
  curve?: CivilTransitionCurve;
}

/** 縦断線形の区間 */
export interface CivilVertical {
  /** 区間の種類 */
  kind: CivilVerticalKind;
  /** 初めの追加距離（メートル） */
  start_distance: number;
  /** 初めの高さ（メートル） */
  start_height: number;
  /** 初めの勾配（高さ ÷ 平面の距離） */
  start_gradient: number;
  /** 平面の距離での長さ（メートル） */
  length: number;
  /** 放物線の終わりの勾配。parabola に書く */
  end_gradient?: number;
  /** 円弧の半径（メートル）。上に凸のものを正にする。circular_arc に書く */
  radius?: number;
}

/** 平面線形の区間の種類。直線・円弧・緩和曲線 */
export type CivilHorizontalKind =
  | "line"
  | "arc"
  | "transition"
  | (string & {});

/** 緩和曲線の種類。クロソイド・3 次放物線・サイン半波長逓減曲線・コサイン曲線・ブロス曲線・ヘルマート曲線・ウィーン曲線 */
export type CivilTransitionCurve =
  | "clothoid"
  | "cubic_parabola"
  | "sine_half_wave"
  | "cosine"
  | "bloss"
  | "helmert"
  | "viennese"
  | (string & {});

/** 縦断線形の区間の種類。一定の勾配・2 次放物線・円弧 */
export type CivilVerticalKind =
  | "constant_gradient"
  | "parabola"
  | "circular_arc"
  | (string & {});

/** 測点の不連続 */
export interface CivilStationBreak {
  /** 追加距離（メートル） */
  distance: number;
  /** 戻りの測点の値（不連続の手前から数えた値） */
  back: number;
  /** 進みの測点の値（不連続の先で数え直す値） */
  ahead: number;
}

/** 線形の上の所 */
export interface CivilResult {
  /** 平面線形の長さ（メートル） */
  length: number;
  /** 求めた所 */
  positions: CivilPosition[];
  /** 所を求めた doc の線形の ID。小版 47 から */
  alignment?: string;
  /** doc の線形の要約。doc を渡したときに返す。小版 47 から */
  alignments?: CivilAlignmentSummary[];
  /** doc の地表面（TIN）の要約。doc を渡したときに返す。小版 47 から */
  surfaces?: CivilSurfaceSummary[];
  /** doc の水平の座標系の EPSG のコード。分からなければ書かない。小版 47 から */
  crs?: number;
}

/** 線形の要約 */
export interface CivilAlignmentSummary {
  /** 線形の ID */
  id: string;
  /** 名前 */
  name?: string;
  /** 平面線形の長さ（メートル） */
  length: number;
  /** 線形の初めの測点の値（メートル） */
  start_station: number;
  /** 平面線形の区間の数 */
  horizontal: number;
  /** 縦断線形の区間の数 */
  vertical: number;
  /** 測点の不連続の数 */
  breaks: number;
  /** 横断の数 */
  cross_sections: number;
}

/** 地表面（TIN）の要約 */
export interface CivilSurfaceSummary {
  /** 地表面の ID */
  id: string;
  /** 名前 */
  name?: string;
  /** 役割。existing（現況）・design（計画）か、元の形式の値 */
  role: string;
  /** 点の数 */
  points: number;
  /** 三角形の数 */
  triangles: number;
}

/** 線形の上の 1 つの所 */
export interface CivilPosition {
  /** 追加距離（メートル） */
  distance: number;
  /** 測点の値（メートル） */
  station: number;
  /** 測点の表記。表記にできなければ書かない */
  label?: string;
  /** 点の北（メートル） */
  northing: number;
  /** 点の東（メートル） */
  easting: number;
  /** 接線の方向角（北から時計回りのラジアン） */
  direction: number;
  /** 縦断線形の高さ（メートル）。求められなければ書かない */
  height?: number;
  /** 縦断線形の勾配。高さと同じく、求められなければ書かない */
  gradient?: number;
}

/** open で開いた点群（LAS・LAZ）の要約と、条件に合う点を返す。点は八分木の浅い段の節から順に選ぶ（決定 0232）。図面のモジュールに置く。小版 48 から */
export interface PointcloudRequest extends RequestCommon {
  /** op の名前 */
  op: "pointcloud";
  /** open で開いた点群の文書の名前 */
  doc: string;
  /** 点を選ぶ範囲。書かなければ全体 */
  region?: PointcloudBox;
  /** 許容差（点の間隔、メートル）。これより細かい段の点を選ばない。書かなければすべての段 */
  tolerance?: number;
  /** 返す点の数の上限。書かなければ点を返さず、要約だけを返す */
  limit?: number;
}

/** 地図の空間の箱。北・東・標高（メートル）の最小と最大 */
export interface PointcloudBox {
  /** 最小の北・東・標高 */
  min: number[];
  /** 最大の北・東・標高 */
  max: number[];
}

/** 点群の要約と、選んだ点 */
export interface PointcloudResult {
  /** 点の数 */
  points: number;
  /** 点の範囲。点がなければ書かない */
  bounds?: PointcloudBox;
  /** 水平の座標系の EPSG のコード。分からなければ書かない */
  crs?: number;
  /** 高さの座標系の EPSG のコード。分からなければ書かない */
  vertical?: number;
  /** 点が持つ属性（position・intensity・classification・color・returns・time） */
  attributes: string[];
  /** 八分木のいちばん深い段 */
  depth: number;
  /** 八分木の節の数 */
  nodes: number;
  /** 選んだ点。limit を書いたときに返す */
  selected?: PointcloudPoints;
}

/** 選んだ点。どの列も、選んだ点の順で同じ長さにする */
export interface PointcloudPoints {
  /** 北・東・標高（メートル） */
  positions: number[][];
  /** 強度。点群が持つときに返す */
  intensity?: number[];
  /** 分類。点群が持つときに返す */
  classification?: number[];
  /** 色（赤・緑・青。16 ビット）。点群が持つときに返す */
  color?: number[][];
  /** limit のために選ばなかった点がある */
  truncated: boolean;
  /** 選んだいちばん深い段 */
  level: number;
}

/** 電子納品の成果品（決定 0241）を読み、管理ファイルの要約を返す。成果品は、zm_push で渡した入力のうち root の下にある名前の集まりか、ZIP の入力（archive）で渡す。profiles と profile を渡せば、プロファイルの delivery- で始まる照合の種類の規則と照らし、指摘を返す（#384）。ほかの種類の規則は飛ばす。図面のモジュールに置く。小版 53 から */
export interface DeliveryCheckRequest extends RequestCommon {
  /** op の名前 */
  op: "delivery_check";
  /** 成果品の起点。/ で終わる入力の名前の前の部分（納品/ など）。archive と一緒には書かない */
  root?: string;
  /** 成果品の ZIP の入力の名前。root と一緒には書かない */
  archive?: string;
  /** プロファイルの入力の名前の列。extends で指す先も、すべて渡す */
  profiles?: string[];
  /** 照合に使うプロファイル。profiles の中から選ぶ */
  profile?: ProfileRef;
  /** 返す指摘の初めの位置 */
  offset?: number;
  /** 返す指摘の数の上限。書かなければ 10000 */
  limit?: number;
}

/** 成果品の要約と、照合の結果。照合の結果は check と同じ形 */
export interface DeliveryCheckResult {
  /** 成果品のファイルの数 */
  files: number;
  /** 管理ファイルの要約。名前の順 */
  managed: DeliveryManaged[];
  /** 照合に使ったプロファイルの鎖。基から順に。プロファイルを渡さなければ空 */
  chain: ProfileIdentity[];
  /** 規則ごとの指摘の数 */
  rules: CheckRule[];
  /** 指摘 */
  findings: CheckFinding[];
  /** 指摘の数の全体 */
  total: number;
  /** 続きの指摘の位置。続きがなければ書かない */
  next_offset?: number;
}

/** 工事写真（JPEG）の Exif と XMP を読み、画素数・向き・撮影の日時・位置などを返す（#387）。画像は復号しない。図面のモジュールに置く。小版 55 から */
export interface PhotoRequest extends RequestCommon {
  /** op の名前 */
  op: "photo";
  /** JPEG の入力の名前 */
  input: string;
}

/** 写真の情報。ファイルが持たない値は書かない */
export interface PhotoResult {
  /** 画素の幅（フレームの見出し） */
  width?: number;
  /** 画素の高さ（フレームの見出し） */
  height?: number;
  /** 向き（Exif の Orientation、1〜8） */
  orientation?: number;
  /** 撮影の日時（YYYY-MM-DDTHH:MM:SS、時差があれば +09:00 などを足す） */
  taken?: string;
  /** 機器の作り手 */
  make?: string;
  /** 機器の型 */
  model?: string;
  /** 撮った位置（GPS の緯度・経度、度）。測地系は換算しない */
  position?: PhotoPosition;
  /** 撮った向き（度） */
  direction?: number;
  /** 向きの基準（T は真北、M は磁北） */
  direction_reference?: string;
  /** GPS の測地系の名前（GPSMapDatum） */
  datum?: string;
  /** XMP の性質。文書の順 */
  xmp: PhotoProperty[];
}

/** 撮った位置 */
export interface PhotoPosition {
  /** 緯度（度、北が正） */
  latitude: number;
  /** 経度（度、東が正） */
  longitude: number;
  /** 高さ（m、海面より下は負） */
  altitude?: number;
}

/** XMP の性質 */
export interface PhotoProperty {
  /** 名前空間の URI */
  namespace: string;
  /** 接頭辞を除いた名前 */
  name: string;
  /** 値。配列は最初の項目の値 */
  value: string;
}

/** 管理ファイルの要約 */
export interface DeliveryManaged {
  /** 成果品の中の名前 */
  name: string;
  /** 文書型宣言の DTD の名前 */
  doctype?: string;
  /** 根の要素の名前 */
  root: string;
  /** 要素の記録の数 */
  records: number;
  /** ファイルの名前の形の値の数 */
  links: number;
  /** 成果品の中に見つからないファイルの名前。書いた順 */
  missing: string[];
}

/** open で開いた CityGML の文書（決定 0233）の地理の領域モデルの要約と、id で指した地物の中身を返す。図面のモジュールに置く。小版 52 から */
export interface GeoRequest extends RequestCommon {
  /** op の名前 */
  op: "geo";
  /** open で開いた CityGML の文書の名前 */
  doc: string;
  /** 中身を返す地物の ID */
  id?: string;
}

/** 地理の領域モデルの要約 */
export interface GeoResult {
  /** 平面の座標系の EPSG のコード。分からなければ書かない */
  crs?: number;
  /** 高さの座標系の EPSG のコード。分からなければ書かない */
  height?: number;
  /** 地物の数 */
  features: number;
  /** 地物の型ごとの数。型の名前の順 */
  kinds: GeoKind[];
  /** id で指した地物の中身 */
  feature?: GeoFeature;
}

/** 地物の型と数 */
export interface GeoKind {
  /** 型。名前空間の接頭辞をつけた元の名前 */
  kind: string;
  /** 地物の数 */
  count: number;
}

/** 地物の中身 */
export interface GeoFeature {
  /** ID */
  id: string;
  /** 型 */
  kind: string;
  /** 分類コード。元の値の文字のまま */
  code?: string;
  /** 名前 */
  name?: string;
  /** 親の地物の ID */
  parent?: string;
  /** 子の地物の数 */
  children: number;
  /** 属性。入力の順 */
  attributes: GeoAttribute[];
  /** 形。入力の順 */
  geometries: GeoGeometry[];
}

/** 属性 */
export interface GeoAttribute {
  /** 名前。接頭辞をつけた元の名前 */
  name: string;
  /** 値の型 */
  kind: GeoValueKind;
  /** 値の文字 */
  value: string;
  /** 単位つきの量の単位 */
  unit?: string;
  /** コードの一覧（codeSpace） */
  space?: string;
}

/** 属性の値の型 */
export type GeoValueKind =
  | "text"
  | "integer"
  | "real"
  | "bool"
  | "date"
  | "code"
  | "measure"
  | (string & {});

/** 形の種類 */
export type GeoForm =
  | "point"
  | "line"
  | "area"
  | "solid"
  | (string & {});

/** 形の要約 */
export interface GeoGeometry {
  /** LOD。2D の地物は書かない */
  lod?: number;
  /** 形の種類 */
  form: GeoForm;
  /** 点の数。立体と面の集まりは、面の輪の点の数の和 */
  points: number;
  /** 立体と面の集まりの面の数 */
  faces?: number;
}

/** open で開いた J-LandXML の文書の線形から、平面図・縦断図・横断図の図面モデルを作り（決定 0231）、用紙を 1 枚選んで新しい DXF の文書として開く（決定 0239）。尺度・文字の高さ・測点の間隔は、プロファイルを渡せば語彙 civil-drawing と text-height から読み、渡さなければ作図の慣習の値にする。縦断図は用紙の要素に焼き込む（決定 0238）。書いたファイルは blobs で返す。作った文書は scene と check に渡せる。図面のモジュールに置く。小版 51 から */
export interface CivilDrawingRequest extends RequestCommon {
  /** op の名前 */
  op: "civil_drawing";
  /** open で開いた J-LandXML の文書の名前 */
  doc: string;
  /** doc の線形の ID。文書に線形が 1 つなら書かなくてよい */
  id?: string;
  /** 書く用紙の名前（結果の sheets の 1 つ）。書かなければ平面図 */
  sheet?: string;
  /** 作る文書の名前 */
  to: string;
  /** プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す */
  profiles?: string[];
  /** 作図の値に使うプロファイル。profiles の中から選ぶ。profiles と一緒に書く */
  profile?: ProfileRef;
}

/** 作った文書。書いたファイルは blobs の file で返す */
export interface CivilDrawingResult {
  /** 作った文書の名前 */
  doc: string;
  /** 書いた形式の名前。今は dxf */
  format: string;
  /** 書いたファイルのハッシュ。apply の前提条件に使う */
  hash: string;
  /** 書いた要素の数 */
  elements: number;
  /** 図を作った線形の ID */
  alignment: string;
  /** 作った用紙の名前。図を描ける順（平面図・縦断図・横断図） */
  sheets: string[];
  /** 書いた用紙の名前 */
  sheet: string;
  /** 使った作図の値 */
  options: CivilDrawingOptions;
  /** 作図の値を読んだプロファイルの、extends を解いた鎖。基から順。プロファイルを渡さなければ書かない */
  profile?: ProfileIdentity[];
}

/** 土木の図の作図の値 */
export interface CivilDrawingOptions {
  /** 平面図の尺度の分母 */
  plan: number;
  /** 縦断図の横の尺度の分母 */
  horizontal: number;
  /** 縦断図の縦の尺度の分母 */
  vertical: number;
  /** 横断図の尺度の分母 */
  cross_section: number;
  /** 文字の高さ（mm） */
  text_height: number;
  /** 測点の間隔（メートル） */
  station_pitch: number;
}

/** 開いた点群と、土木の文書の TIN の標高の差を求め、プロファイルの surface-elevation の規則の区分ごとに集計して、規格値と比べる（決定 0234）。図面のモジュールに置く。小版 50 から */
export interface DeviationRequest extends RequestCommon {
  /** op の名前 */
  op: "deviation";
  /** open で開いた点群の文書の名前 */
  doc: string;
  /** 設計面の TIN */
  surface: DeviationSurface;
  /** プロファイルの入力の名前の列（zm_push で渡した profile.json）。extends で指す先も、すべて渡す */
  profiles: string[];
  /** 規格値に使うプロファイル。profiles の中から選ぶ */
  profile: ProfileRef;
  /** 工種の名前。規則の work と比べ、同じものだけを使う */
  work: string;
  /** 返す最初の評価した点の位置。前の応答の next_offset を渡す。書かなければ 0 */
  offset?: number;
  /** 返す評価した点の数の上限。書かなければ点を返さない */
  limit?: number;
}

/** 設計面の TIN を持つ文書と、TIN の ID */
export interface DeviationSurface {
  /** open で開いた土木の文書の名前 */
  doc: string;
  /** TIN の ID。civil の surfaces の id */
  id: string;
}

/** 標高の差の、規則の区分ごとの集計と指摘。合格・適合とは言わない */
export interface DeviationResult {
  /** 使ったプロファイルの鎖。extends の基から順 */
  chain: ProfileIdentity[];
  /** 点群の点の数 */
  points: number;
  /** TIN の外なので評価しなかった点の数 */
  outside: number;
  /** どの規則の勾配にも入らない三角形の上なので評価しなかった点の数 */
  unmatched: number;
  /** 区分の境の近く（規則の exclude の内）なので除いた点の数 */
  excluded: number;
  /** 規則ごとの集計。プロファイルの順。使えない規則は持たず、profile.rule.unsupported の診断で返す */
  rules: DeviationRule[];
  /** 指摘。規則の順。規格値を外れれば profile.surface-elevation.invalid、確認できなければ profile.surface-elevation.unsupported */
  findings: CheckFinding[];
  /** 評価した点の数 */
  evaluated: number;
  /** 評価した点。点群の順に、offset から limit 個まで */
  differences: DeviationPoint[];
  /** 続きの最初の評価した点の位置。続きがなければ持たない */
  next_offset?: number;
}

/** 規則の区分ごとの集計。差はメートル */
export interface DeviationRule {
  /** 規則の id */
  rule: string;
  /** 区分の名前 */
  part: string;
  /** 評価した点の数 */
  points: number;
  /** 区分の三角形の、平面に投影した面積（m²） */
  area: number;
  /** 計測の密度（点／m²）。面積がなければ書かない */
  density?: number;
  /** 差の平均。点がなければ書かない */
  mean?: number;
  /** 差の最小。点がなければ書かない */
  min?: number;
  /** 差の最大。点がなければ書かない */
  max?: number;
  /** 指摘の数（確認できなかったものを除く） */
  findings: number;
  /** 確認できなかった数 */
  unsupported: number;
}

/** 評価した点 */
export interface DeviationPoint {
  /** 点群の中の番号 */
  index: number;
  /** 北・東・標高（メートル） */
  position: number[];
  /** 点の標高から設計面の標高を引いた差（メートル） */
  difference: number;
  /** 評価した規則の id */
  rule: string;
}

/** describe に使うカタログのファイル。catalog/<言語>.toml から生成し、zm_push で渡す（決定 0009） */
export interface Catalog {
  /** カタログの形の大版。今は 1 */
  v: number;
  /** 言語（ja・en） */
  lang: string;
  /** パッケージの版。コアと同じ版で出す */
  package: string;
  /** 数の書き方 */
  number: NumberFormat;
  /** code ごとの文のひな形。{名前} に値が、{count} にまとめた件数が入る */
  codes: Record<string, string>;
}

/** 文に差し込む数の書き方。OS のロケールは使わない（決定 0009） */
export interface NumberFormat {
  /** 整数部を 3 桁ごとに区切る文字。空なら区切らない */
  group_separator: string;
  /** 小数点の文字 */
  decimal_separator: string;
}

/** op ごとの要求と結果の型 */
export interface Operations {
  capabilities: { request: CapabilitiesRequest; result: CapabilitiesResult };
  open: { request: OpenRequest; result: OpenResult };
  close: { request: CloseRequest; result: CloseResult };
  describe: { request: DescribeRequest; result: DescribeResult };
  drawing: { request: DrawingRequest; result: DrawingResult };
  entities: { request: EntitiesRequest; result: EntitiesResult };
  apply: { request: ApplyRequest; result: ApplyResult };
  convert: { request: ConvertRequest; result: ConvertResult };
  tables: { request: TablesRequest; result: TablesResult };
  scene: { request: SceneRequest; result: SceneResult };
  check: { request: CheckRequest; result: CheckResult };
  validate: { request: ValidateRequest; result: ValidateResult };
  review_carry: { request: ReviewCarryRequest; result: ReviewCarryResult };
  review_adopt: { request: ReviewAdoptRequest; result: ReviewAdoptResult };
  building: { request: BuildingRequest; result: BuildingResult };
  building_elements: { request: BuildingElementsRequest; result: BuildingElementsResult };
  ids_check: { request: IdsCheckRequest; result: IdsCheckResult };
  review_adopt_ids: { request: ReviewAdoptIdsRequest; result: unknown };
  bcf_read: { request: BcfReadRequest; result: BcfReadResult };
  bcf_write: { request: BcfWriteRequest; result: BcfWriteResult };
  civil: { request: CivilRequest; result: CivilResult };
  pointcloud: { request: PointcloudRequest; result: PointcloudResult };
  deviation: { request: DeviationRequest; result: DeviationResult };
  civil_drawing: { request: CivilDrawingRequest; result: CivilDrawingResult };
  geo: { request: GeoRequest; result: GeoResult };
  delivery_check: { request: DeliveryCheckRequest; result: DeliveryCheckResult };
  photo: { request: PhotoRequest; result: PhotoResult };
}
