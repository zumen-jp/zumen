// 生成したファイル。手で直さない。
// 元は schema/boundary.toml と schema/scene.toml で、`cargo xtask codegen` で作り直す。

/** パッケージの版（決定 0015） */
export const VERSION = "0.2.0";
/** ABI の大版 */
export const ABI_VERSION = 1;
/** メッセージのスキーマの大版（要求と応答の v） */
export const SCHEMA_MAJOR = 1;
/** メッセージのスキーマの小版 */
export const SCHEMA_MINOR = 46;

/** 状態コードの名前。値から引く */
export const STATUS = Object.freeze({
  0: "ok",
  1: "failed",
  2: "invalid",
  3: "busy",
  4: "panicked",
});

/** wasm のモジュールが出す関数 */
export const WASM_EXPORTS = Object.freeze([
  "zm_abi_version",
  "zm_alloc",
  "zm_call",
  "zm_cancel",
  "zm_dealloc",
  "zm_free",
  "zm_push",
  "zm_session_close",
  "zm_session_new",
  "zm_take",
]);

/** wasm のモジュール。名前から、持つ形式と op を引く（決定 0013・0221） */
export const MODULES = Object.freeze({
  drawing: Object.freeze({
    description: "図面（DXF・DWG・SXF・JWW）と、図面のレビュー",
    formats: Object.freeze(["dxf", "dwg", "sxf", "jww"]),
    ops: Object.freeze(["capabilities", "open", "close", "describe", "drawing", "entities", "apply", "convert", "tables", "scene", "check", "review_carry", "review_adopt", "civil"]),
  }),
  bim: Object.freeze({
    description: "IFC と建物モデル、EXPRESS・IDS の照合と BCF",
    formats: Object.freeze(["ifc"]),
    ops: Object.freeze(["capabilities", "open", "close", "describe", "validate", "review_carry", "building", "building_elements", "ids_check", "review_adopt_ids", "bcf_read", "bcf_write"]),
  }),
});

/** open が形式を判定する順と規則。rules のどれかに合えば format とする */
export const DETECTION = Object.freeze([
  Object.freeze({
    format: "ifc",
    module: "bim",
    rules: Object.freeze([
      Object.freeze({ kind: "step_schema", prefix: "IFC" }),
    ]),
  }),
  Object.freeze({
    format: "dxf",
    module: "drawing",
    rules: Object.freeze([
      Object.freeze({ kind: "lines", lines: Object.freeze(["0", "SECTION"]) }),
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x75, 0x74, 0x6f, 0x43, 0x41, 0x44, 0x20, 0x42, 0x69, 0x6e, 0x61, 0x72, 0x79, 0x20, 0x44, 0x58, 0x46, 0x0d, 0x0a, 0x1a, 0x00]) }),
    ]),
  }),
  Object.freeze({
    format: "dwg",
    module: "drawing",
    rules: Object.freeze([
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x43, 0x31, 0x30, 0x31, 0x32]) }),
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x43, 0x31, 0x30, 0x31, 0x34]) }),
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x43, 0x31, 0x30, 0x31, 0x35]) }),
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x43, 0x31, 0x30, 0x31, 0x38]) }),
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x43, 0x31, 0x30, 0x32, 0x31]) }),
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x43, 0x31, 0x30, 0x32, 0x34]) }),
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x43, 0x31, 0x30, 0x32, 0x37]) }),
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x41, 0x43, 0x31, 0x30, 0x33, 0x32]) }),
    ]),
  }),
  Object.freeze({
    format: "sxf",
    module: "drawing",
    rules: Object.freeze([
      Object.freeze({ kind: "lines", lines: Object.freeze(["ISO-10303-21;"]) }),
    ]),
  }),
  Object.freeze({
    format: "jww",
    module: "drawing",
    rules: Object.freeze([
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x4a, 0x77, 0x77, 0x44, 0x61, 0x74, 0x61, 0x2e]) }),
    ]),
  }),
  Object.freeze({
    format: "ifc",
    module: "bim",
    rules: Object.freeze([
      Object.freeze({ kind: "bytes", offset: 0, bytes: Object.freeze([0x50, 0x4b, 0x03, 0x04]) }),
    ]),
  }),
]);

/** シーンの見出しの始まりの 4 バイト */
export const SCENE_MAGIC = "ZMSC";
/** シーンの大版。区画の意味を変えるときに上げる */
export const SCENE_MAJOR = 1;
/** シーンの小版。区画か値を足すたびに上げる */
export const SCENE_MINOR = 2;
/** 番号の「なし」 */
export const SCENE_NONE = 4294967295;

/** シーンの区画の定義。番号の順 */
export const SCENE_SECTIONS = Object.freeze([
  Object.freeze({ name: "chunk_origins", type: "f64", width: 2 }),
  Object.freeze({ name: "chunk_bounds", type: "f32", width: 4 }),
  Object.freeze({ name: "chunk_lines", type: "u32", width: 1 }),
  Object.freeze({ name: "chunk_faces", type: "u32", width: 1 }),
  Object.freeze({ name: "chunk_texts", type: "u32", width: 1 }),
  Object.freeze({ name: "chunk_points", type: "u32", width: 1 }),
  Object.freeze({ name: "chunk_images", type: "u32", width: 1 }),
  Object.freeze({ name: "line_vertices", type: "u32", width: 1 }),
  Object.freeze({ name: "line_units", type: "u32", width: 1 }),
  Object.freeze({ name: "line_xy", type: "f32", width: 2 }),
  Object.freeze({ name: "face_rings", type: "u32", width: 1 }),
  Object.freeze({ name: "face_units", type: "u32", width: 1 }),
  Object.freeze({ name: "ring_vertices", type: "u32", width: 1 }),
  Object.freeze({ name: "face_xy", type: "f32", width: 2 }),
  Object.freeze({ name: "text_strings", type: "u32", width: 2 }),
  Object.freeze({ name: "text_xy", type: "f32", width: 2 }),
  Object.freeze({ name: "text_second", type: "f32", width: 2 }),
  Object.freeze({ name: "text_heights", type: "f32", width: 1 }),
  Object.freeze({ name: "text_widths", type: "f32", width: 1 }),
  Object.freeze({ name: "text_rotations", type: "f32", width: 1 }),
  Object.freeze({ name: "text_obliques", type: "f32", width: 1 }),
  Object.freeze({ name: "text_boxes", type: "f32", width: 1 }),
  Object.freeze({ name: "text_aligns", type: "u8", width: 2 }),
  Object.freeze({ name: "text_flags", type: "u8", width: 1 }),
  Object.freeze({ name: "text_lines", type: "u32", width: 1 }),
  Object.freeze({ name: "text_fonts", type: "u32", width: 1 }),
  Object.freeze({ name: "text_units", type: "u32", width: 1 }),
  Object.freeze({ name: "point_xy", type: "f32", width: 2 }),
  Object.freeze({ name: "point_rotations", type: "f32", width: 1 }),
  Object.freeze({ name: "point_markers", type: "u32", width: 4 }),
  Object.freeze({ name: "point_scales", type: "f32", width: 1 }),
  Object.freeze({ name: "point_units", type: "u32", width: 1 }),
  Object.freeze({ name: "point_style", type: "f32", width: 4 }),
  Object.freeze({ name: "image_names", type: "u32", width: 2 }),
  Object.freeze({ name: "image_pages", type: "u32", width: 2 }),
  Object.freeze({ name: "image_xy", type: "f32", width: 2 }),
  Object.freeze({ name: "image_edges", type: "f32", width: 4 }),
  Object.freeze({ name: "image_kinds", type: "u8", width: 1 }),
  Object.freeze({ name: "image_frames", type: "u8", width: 1 }),
  Object.freeze({ name: "image_units", type: "u32", width: 1 }),
  Object.freeze({ name: "unit_entities", type: "u32", width: 1 }),
  Object.freeze({ name: "unit_colors", type: "u32", width: 1 }),
  Object.freeze({ name: "unit_weights", type: "f32", width: 1 }),
  Object.freeze({ name: "unit_clips", type: "u32", width: 1 }),
  Object.freeze({ name: "unit_layers", type: "u32", width: 1 }),
  Object.freeze({ name: "entity_ids", type: "u32", width: 2 }),
  Object.freeze({ name: "entity_kinds", type: "u8", width: 1 }),
  Object.freeze({ name: "entity_paths", type: "u32", width: 1 }),
  Object.freeze({ name: "step_ids", type: "u32", width: 2 }),
  Object.freeze({ name: "step_cells", type: "u32", width: 2 }),
  Object.freeze({ name: "color_systems", type: "u8", width: 1 }),
  Object.freeze({ name: "color_codes", type: "u32", width: 1 }),
  Object.freeze({ name: "color_files", type: "u8", width: 4 }),
  Object.freeze({ name: "color_rgba", type: "u8", width: 4 }),
  Object.freeze({ name: "color_flags", type: "u8", width: 1 }),
  Object.freeze({ name: "clip_polygons", type: "u32", width: 1 }),
  Object.freeze({ name: "polygon_vertices", type: "u32", width: 1 }),
  Object.freeze({ name: "polygon_flags", type: "u8", width: 1 }),
  Object.freeze({ name: "clip_xy", type: "f64", width: 2 }),
  Object.freeze({ name: "layer_ids", type: "u32", width: 2 }),
  Object.freeze({ name: "font_names", type: "u32", width: 2 }),
  Object.freeze({ name: "strings", type: "u8", width: 1 }),
  Object.freeze({ name: "text_glyphs", type: "u32", width: 1 }),
  Object.freeze({ name: "glyph_shapes", type: "u32", width: 1 }),
  Object.freeze({ name: "glyph_axes", type: "f32", width: 4 }),
  Object.freeze({ name: "glyph_xy", type: "f32", width: 2 }),
  Object.freeze({ name: "shape_rings", type: "u32", width: 1 }),
  Object.freeze({ name: "outline_vertices", type: "u32", width: 1 }),
  Object.freeze({ name: "outline_xy", type: "f32", width: 2 }),
]);

/** シーンの列に入れる値の組 */
export const SCENE_VALUES = Object.freeze({
  horizontal: Object.freeze({
    left: 0,
    center: 1,
    right: 2,
    middle: 3,
    aligned: 4,
    fit: 5,
  }),
  vertical: Object.freeze({
    baseline: 0,
    bottom: 1,
    middle: 2,
    top: 3,
  }),
  text_flag: Object.freeze({
    backward: 1,
    upside_down: 2,
    vertical: 4,
    formatted: 8,
    stack_upper: 16,
    stack_lower: 32,
    height_frame: 64,
    height_em: 128,
  }),
  point_mark: Object.freeze({
    dot: 0,
    none: 1,
    plus: 2,
    cross: 3,
    tick: 4,
  }),
  point_frame: Object.freeze({
    circle: 1,
    square: 2,
  }),
  point_size: Object.freeze({
    length: 0,
    screen: 1,
  }),
  image_kind: Object.freeze({
    image: 0,
    pdf: 1,
    dwf: 2,
    dgn: 3,
  }),
  image_frame: Object.freeze({
    corners: 0,
    unit: 1,
  }),
  entity_kind: Object.freeze({
    line: 0,
    circle: 1,
    curve: 2,
    polyline: 3,
    text: 4,
    formatted: 5,
    dimension: 6,
    leader: 7,
    callout: 8,
    table: 9,
    underlay: 10,
    symbol: 11,
    hatch: 12,
    point: 13,
    attribute: 14,
  }),
  color_system: Object.freeze({
    none: 0,
    aci: 1,
    sxf: 2,
    jww: 3,
  }),
  color_flag: Object.freeze({
    unresolved: 1,
    foreground: 2,
  }),
});
