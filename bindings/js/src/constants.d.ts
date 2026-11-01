// 生成したファイル。手で直さない。
// 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。

/** パッケージの版（決定 0015） */
export declare const VERSION: string;
/** ABI の大版 */
export declare const ABI_VERSION: 1;
/** メッセージのスキーマの大版（要求と応答の v） */
export declare const SCHEMA_MAJOR: 1;
/** メッセージのスキーマの小版 */
export declare const SCHEMA_MINOR: number;

/** 状態コードの名前 */
export type StatusName =
  /** 成功した */
  | "ok"
  /** 操作が失敗した。応答に error の診断がある */
  | "failed"
  /** 引数が使えない（NULL、長さが合わない、使えない名前など）。応答は書かない */
  | "invalid"
  /** 別のスレッドが、同じセッションを使っている */
  | "busy"
  /** コアの不具合で止まった。セッションは閉じるしかない */
  | "panicked";

/** 状態コードの名前。値から引く */
export declare const STATUS: Readonly<Record<number, StatusName>>;

/** wasm のモジュールが出す関数 */
export declare const WASM_EXPORTS: readonly string[];

/** wasm のモジュールの名前（決定 0013・0221） */
export type ModuleName =
  /** 図面（DXF・DWG・SXF・JWW）と、図面のレビュー。土木の線形（J-LandXML）と、地理の地物（CityGML）。電子納品の成果品 */
  | "drawing"
  /** IFC と建物モデル、EXPRESS・IDS の照合と BCF */
  | "bim";

/** wasm のモジュールが持つ形式と op */
export interface ModuleInfo {
  readonly description: string;
  readonly formats: readonly string[];
  readonly ops: readonly string[];
}

/** wasm のモジュール。名前から引く */
export declare const MODULES: Readonly<Record<ModuleName, ModuleInfo>>;

/** 形式の判定の規則（zumen-sdk の Signature） */
export type DetectionRule =
  | { readonly kind: "bytes"; readonly offset: number; readonly bytes: readonly number[] }
  | { readonly kind: "lines"; readonly lines: readonly string[] }
  | { readonly kind: "step_schema"; readonly prefix: string }
  | { readonly kind: "xml_root"; readonly name: string; readonly namespace: string | null };

/** 形式の判定。rules のどれかに合えば format とする */
export interface Detection {
  readonly format: string;
  readonly module: ModuleName;
  readonly rules: readonly DetectionRule[];
}

/** open が形式を判定する順と規則 */
export declare const DETECTION: readonly Detection[];

/** シーンの見出しの始まりの 4 バイト */
export declare const SCENE_MAGIC: "ZMSC";
/** シーンの大版。区画の意味を変えるときに上げる */
export declare const SCENE_MAJOR: 1;
/** シーンの小版。区画か値を足すたびに上げる */
export declare const SCENE_MINOR: number;
/** 番号の「なし」 */
export declare const SCENE_NONE: 4294967295;

/** シーンの区画の名前 */
export type SceneSectionName =
  | "chunk_origins"
  | "chunk_bounds"
  | "chunk_lines"
  | "chunk_faces"
  | "chunk_texts"
  | "chunk_points"
  | "chunk_images"
  | "line_vertices"
  | "line_units"
  | "line_xy"
  | "face_rings"
  | "face_units"
  | "ring_vertices"
  | "face_xy"
  | "text_strings"
  | "text_xy"
  | "text_second"
  | "text_heights"
  | "text_widths"
  | "text_rotations"
  | "text_obliques"
  | "text_boxes"
  | "text_aligns"
  | "text_flags"
  | "text_lines"
  | "text_fonts"
  | "text_units"
  | "point_xy"
  | "point_rotations"
  | "point_markers"
  | "point_scales"
  | "point_units"
  | "point_style"
  | "image_names"
  | "image_pages"
  | "image_xy"
  | "image_edges"
  | "image_kinds"
  | "image_frames"
  | "image_units"
  | "unit_entities"
  | "unit_colors"
  | "unit_weights"
  | "unit_clips"
  | "unit_layers"
  | "entity_ids"
  | "entity_kinds"
  | "entity_paths"
  | "step_ids"
  | "step_cells"
  | "color_systems"
  | "color_codes"
  | "color_files"
  | "color_rgba"
  | "color_flags"
  | "clip_polygons"
  | "polygon_vertices"
  | "polygon_flags"
  | "clip_xy"
  | "layer_ids"
  | "font_names"
  | "strings"
  | "text_glyphs"
  | "glyph_shapes"
  | "glyph_axes"
  | "glyph_xy"
  | "shape_rings"
  | "outline_vertices"
  | "outline_xy";

/** シーンの区画の値の型 */
export type SceneValueType = | "u8" | "u32" | "f32" | "f64";

/** シーンの区画の定義 */
export interface SceneSection {
  readonly name: SceneSectionName;
  readonly type: SceneValueType;
  /** 行ごとの値の数 */
  readonly width: number;
}

/** シーンの区画の定義。番号の順 */
export declare const SCENE_SECTIONS: readonly SceneSection[];

/** シーンの区画ごとの列。行ごとに width 個の値を並べる */
export interface SceneColumns {
  /** 塊の原点（起点の座標の X・Y）。塊の座標は、ここからの相対の値（f64 × 2） */
  readonly chunk_origins: Float64Array;
  /** 塊の描くものの相対座標の範囲（X の最小・Y の最小・X の最大・Y の最大）。文字は基点だけを含む（f32 × 4） */
  readonly chunk_bounds: Float32Array;
  /** 塊の線の始まり（line_vertices の行）（u32 × 1） */
  readonly chunk_lines: Uint32Array;
  /** 塊の面の始まり（face_rings の行）（u32 × 1） */
  readonly chunk_faces: Uint32Array;
  /** 塊の文字の始まり（text_strings の行）（u32 × 1） */
  readonly chunk_texts: Uint32Array;
  /** 塊の点の始まり（point_xy の行）（u32 × 1） */
  readonly chunk_points: Uint32Array;
  /** 塊の画像の始まり（image_names の行）（u32 × 1） */
  readonly chunk_images: Uint32Array;
  /** 線（折れ線）の頂点の始まり（line_xy の行）（u32 × 1） */
  readonly line_vertices: Uint32Array;
  /** 線の描く単位（unit_entities の行）（u32 × 1） */
  readonly line_units: Uint32Array;
  /** 線の頂点。塊の原点からの X・Y（f32 × 2） */
  readonly line_xy: Float32Array;
  /** 面の輪の始まり（ring_vertices の行）。輪は偶奇の規則で塗る（u32 × 1） */
  readonly face_rings: Uint32Array;
  /** 面の描く単位（unit_entities の行）（u32 × 1） */
  readonly face_units: Uint32Array;
  /** 輪の頂点の始まり（face_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1） */
  readonly ring_vertices: Uint32Array;
  /** 面の輪の頂点。塊の原点からの X・Y（f32 × 2） */
  readonly face_xy: Float32Array;
  /** 文字の内容（strings の始まりと長さ）（u32 × 2） */
  readonly text_strings: Uint32Array;
  /** 文字の基点。塊の原点からの X・Y（f32 × 2） */
  readonly text_xy: Float32Array;
  /** 2 点の間に置く文字の 2 点目。塊の原点からの X・Y。2 点の間に置かなければ NaN（f32 × 2） */
  readonly text_second: Float32Array;
  /** 文字の高さ（起点の長さ）（f32 × 1） */
  readonly text_heights: Float32Array;
  /** 字形の幅の倍率（f32 × 1） */
  readonly text_widths: Float32Array;
  /** 回転（度、反時計回り）（f32 × 1） */
  readonly text_rotations: Float32Array;
  /** 斜体の角度（度）（f32 × 1） */
  readonly text_obliques: Float32Array;
  /** 折り返す枠の幅（起点の長さ）。折り返さなければ 0（f32 × 1） */
  readonly text_boxes: Float32Array;
  /** 位置合わせ。水平（horizontal の値）と垂直（vertical の値）（u8 × 2） */
  readonly text_aligns: Uint8Array;
  /** 印（text_flag の値の和）（u8 × 1） */
  readonly text_flags: Uint8Array;
  /** 書式つきの文字の行の番号。0 から数える。1 行の文字は 0（u32 × 1） */
  readonly text_lines: Uint32Array;
  /** フォント（font_names の行）。分からなければ「なし」（u32 × 1） */
  readonly text_fonts: Uint32Array;
  /** 文字の描く単位（unit_entities の行）（u32 × 1） */
  readonly text_units: Uint32Array;
  /** 点の位置。塊の原点からの X・Y（f32 × 2） */
  readonly point_xy: Float32Array;
  /** 点の回転（度、反時計回り）（f32 × 1） */
  readonly point_rotations: Float32Array;
  /** 既定義の印のキー。名前空間（strings の始まりと長さ）と、元のコード（strings の始まりと長さ）。文書の点の表し方に従う点は、長さが 0（u32 × 4） */
  readonly point_markers: Uint32Array;
  /** 既定義の印の尺度。文書の点の表し方に従う点は 1（f32 × 1） */
  readonly point_scales: Float32Array;
  /** 点の描く単位（unit_entities の行）（u32 × 1） */
  readonly point_units: Uint32Array;
  /** 文書の点の表し方（決定 0160）。形（point_mark の値）・囲み（point_frame の値の和）・大きさの意味（point_size の値）・大きさ。文書が持たなければ行がない（f32 × 4） */
  readonly point_style: Float32Array;
  /** 下敷きの参照名（strings の始まりと長さ）。コアは開かない（u32 × 2） */
  readonly image_names: Uint32Array;
  /** 資源の中のページ（strings の始まりと長さ）。なければ長さが 0（u32 × 2） */
  readonly image_pages: Uint32Array;
  /** 画像の基点（左下の角）。塊の原点からの X・Y（f32 × 2） */
  readonly image_xy: Float32Array;
  /** 画像の X の辺と Y の辺のベクトル（起点の長さ）。image_frames が corners なら内容の全体の辺、unit なら内容の 1 単位の辺（f32 × 4） */
  readonly image_edges: Float32Array;
  /** 資源の種類（image_kind の値）（u8 × 1） */
  readonly image_kinds: Uint8Array;
  /** 辺のベクトルの意味（image_frame の値）（u8 × 1） */
  readonly image_frames: Uint8Array;
  /** 画像の描く単位（unit_entities の行）（u32 × 1） */
  readonly image_units: Uint32Array;
  /** 描く単位の要素（entity_ids の行）（u32 × 1） */
  readonly unit_entities: Uint32Array;
  /** 描く単位の色（color_systems の行）（u32 × 1） */
  readonly unit_colors: Uint32Array;
  /** 線幅（用紙の mm）。0 は、いちばん細い線（f32 × 1） */
  readonly unit_weights: Float32Array;
  /** 切り抜き（clip_polygons の行）。切り抜かなければ「なし」（u32 × 1） */
  readonly unit_clips: Uint32Array;
  /** 実効のレイヤ（layer_ids の行）（u32 × 1） */
  readonly unit_layers: Uint32Array;
  /** 要素の ID（strings の始まりと長さ、決定 0156）。記号の属性なら、属性を持つ記号の ID（u32 × 2） */
  readonly entity_ids: Uint32Array;
  /** 要素の種類（entity_kind の値）（u8 × 1） */
  readonly entity_kinds: Uint8Array;
  /** 配置の道筋の始まり（step_ids の行）。外側から内側へ（u32 × 1） */
  readonly entity_paths: Uint32Array;
  /** 配置の要素の ID（strings の始まりと長さ）（u32 × 2） */
  readonly step_ids: Uint32Array;
  /** 格子に並べた記号の列と行。格子でなければ 0 と 0（u32 × 2） */
  readonly step_cells: Uint32Array;
  /** 色の符号の系統（color_system の値）（u8 × 1） */
  readonly color_systems: Uint8Array;
  /** 系統の中の符号。系統がなければ 0（u32 × 1） */
  readonly color_codes: Uint32Array;
  /** ファイルが持つ RGB と、持つかどうか（持てば 255、持たなければ 0）（u8 × 4） */
  readonly color_files: Uint8Array;
  /** 表示する RGBA。ファイルの RGB か、色の表（palette）から引いた値。未解決なら 0（u8 × 4） */
  readonly color_rgba: Uint8Array;
  /** 印（color_flag の値の和）（u8 × 1） */
  readonly color_flags: Uint8Array;
  /** 切り抜きの組の多角形の始まり（polygon_vertices の行）。描くものは、組のすべての多角形の内側だけを描く（u32 × 1） */
  readonly clip_polygons: Uint32Array;
  /** 多角形の頂点の始まり（clip_xy の行）（u32 × 1） */
  readonly polygon_vertices: Uint32Array;
  /** 多角形の印。1 なら、内側を隠す（外側だけを描く）（u8 × 1） */
  readonly polygon_flags: Uint8Array;
  /** 切り抜きの多角形の頂点。起点の座標の X・Y（f64 × 2） */
  readonly clip_xy: Float64Array;
  /** レイヤの ID（strings の始まりと長さ）。tables の op が返すものと同じ形（u32 × 2） */
  readonly layer_ids: Uint32Array;
  /** フォントの元の名前（strings の始まりと長さ）（u32 × 2） */
  readonly font_names: Uint32Array;
  /** 文字列を並べた UTF-8（u8 × 1） */
  readonly strings: Uint8Array;
  /** 文字の字形の始まり（glyph_shapes の行）。字形を持つ文字は、文字列の代わりに字形を描く。持たなければ（フォントがない・置けない）文字列を描く（u32 × 1） */
  readonly text_glyphs: Uint32Array;
  /** 置いた字形の形（shape_rings の行）（u32 × 1） */
  readonly glyph_shapes: Uint32Array;
  /** 形の座標（字形の単位）を写す変換の線形の部分 [a, b, c, d]。形の点 (u, v) は、x = a·u + c·v + e、y = b·u + d·v + f に置く（e・f は glyph_xy）（f32 × 4） */
  readonly glyph_axes: Float32Array;
  /** 字形の原点（形の座標の (0, 0)）を置く点 [e, f]。塊の原点からの X・Y（f32 × 2） */
  readonly glyph_xy: Float32Array;
  /** 字形の形の輪の始まり（outline_vertices の行）。輪は非零の規則で塗る。空白などは輪を持たない（u32 × 1） */
  readonly shape_rings: Uint32Array;
  /** 字形の輪の頂点の始まり（outline_xy の行）。輪は閉じているものとし、始点を繰り返さない（u32 × 1） */
  readonly outline_vertices: Uint32Array;
  /** 字形の輪の頂点（字形の単位）。曲線は、許容差を字形の単位に直して 2 の冪に切り下げた刻みで折れ線にする（f32 × 2） */
  readonly outline_xy: Float32Array;
}

/** シーンの列に入れる値の組 */
export declare const SCENE_VALUES: {
  /** 文字の水平の位置合わせ */
  readonly horizontal: {
    /** 左 */
    readonly left: 0;
    /** 中央 */
    readonly center: 1;
    /** 右 */
    readonly right: 2;
    /** 文字の外形の中央 */
    readonly middle: 3;
    /** 2 点の間に、縦横の比を保って合わせる */
    readonly aligned: 4;
    /** 2 点の間に、高さを保って幅だけを合わせる */
    readonly fit: 5;
  };
  /** 文字の垂直の位置合わせ */
  readonly vertical: {
    /** 基準線 */
    readonly baseline: 0;
    /** 下 */
    readonly bottom: 1;
    /** 中央 */
    readonly middle: 2;
    /** 上 */
    readonly top: 3;
  };
  /** 文字の印。和で持つ */
  readonly text_flag: {
    /** 左右に反転する */
    readonly backward: 1;
    /** 上下に反転する */
    readonly upside_down: 2;
    /** 縦書き */
    readonly vertical: 4;
    /** 書式つきの文字の区間。基点は枠の基点で、行の中の位置は表示する側が送る */
    readonly formatted: 8;
    /** 上下に重ねた文字の上 */
    readonly stack_upper: 16;
    /** 上下に重ねた文字の下 */
    readonly stack_lower: 32;
    /** 高さは文字枠の高さ（決定 0184）。height_frame と height_em のどちらもなければ、大文字の高さか、字形との関係が決まらない形式の宣言値 */
    readonly height_frame: 64;
    /** 高さは em（決定 0184） */
    readonly height_em: 128;
  };
  /** 文書の点の基本の形 */
  readonly point_mark: {
    /** 小さな点 */
    readonly dot: 0;
    /** なし */
    readonly none: 1;
    /** 十字 */
    readonly plus: 2;
    /** バツ */
    readonly cross: 3;
    /** 上向きの短い線 */
    readonly tick: 4;
  };
  /** 文書の点の囲み。和で持つ */
  readonly point_frame: {
    /** 円で囲む */
    readonly circle: 1;
    /** 四角で囲む */
    readonly square: 2;
  };
  /** 文書の点の大きさの意味 */
  readonly point_size: {
    /** 起点の長さ */
    readonly length: 0;
    /** 画面の高さに対する百分率 */
    readonly screen: 1;
  };
  /** 下敷きの資源の種類 */
  readonly image_kind: {
    /** 画素の画像 */
    readonly image: 0;
    /** PDF */
    readonly pdf: 1;
    /** DWF（決定 0184） */
    readonly dwf: 2;
    /** DGN（決定 0184） */
    readonly dgn: 3;
  };
  /** 画像の辺のベクトルの意味 */
  readonly image_frame: {
    /** 内容の全体を、2 つの辺の平行四辺形に収める */
    readonly corners: 0;
    /** 辺は内容の 1 単位。内容の大きさを掛ける */
    readonly unit: 1;
  };
  /** 要素の種類（entities の op の kind と同じ） */
  readonly entity_kind: {
    /** 線分 */
    readonly line: 0;
    /** 円 */
    readonly circle: 1;
    /** 曲線 */
    readonly curve: 2;
    /** 折れ線 */
    readonly polyline: 3;
    /** 文字 */
    readonly text: 4;
    /** 書式つきの文字 */
    readonly formatted: 5;
    /** 寸法 */
    readonly dimension: 6;
    /** 引出線 */
    readonly leader: 7;
    /** 引出線つきの注記 */
    readonly callout: 8;
    /** 表 */
    readonly table: 9;
    /** 下敷き */
    readonly underlay: 10;
    /** 記号 */
    readonly symbol: 11;
    /** ハッチング */
    readonly hatch: 12;
    /** 点 */
    readonly point: 13;
    /** 記号の属性 */
    readonly attribute: 14;
  };
  /** 色の符号の系統 */
  readonly color_system: {
    /** 符号を持たない */
    readonly none: 0;
    /** AutoCAD の色の番号 */
    readonly aci: 1;
    /** SXF の色のコード */
    readonly sxf: 2;
    /** Jw_cad のペンの番号 */
    readonly jww: 3;
  };
  /** 色の印。和で持つ */
  readonly color_flag: {
    /** 表示する RGBA が決まらない。表示する側が決める */
    readonly unresolved: 1;
    /** 背景で変わる色（ACI の 7）。表示する側が、背景に対する前景の色にする */
    readonly foreground: 2;
  };
};
