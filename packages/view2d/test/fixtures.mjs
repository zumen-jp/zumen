import { readScene, SCENE_MAGIC, SCENE_MAJOR, SCENE_MINOR, SCENE_NONE, SCENE_SECTIONS, SCENE_VALUES as V } from "@zumen-jp/zumen";

const TYPES = { u8: Uint8Array, u32: Uint32Array, f32: Float32Array, f64: Float64Array };

/** 外部の図面を保存せず、列の契約を確かめる最小の blob を作る。 */
export function bundle(source, changes = {}) {
  const columns = SCENE_SECTIONS.map(({ name, type }) => new TYPES[type](source[name] ?? []));
  let length = 16 + 8 * columns.length;
  const offsets = columns.map((column) => { length = Math.ceil(length / 8) * 8; const start = length; length += column.byteLength; return start; });
  const bytes = new Uint8Array(length);
  bytes.set(new TextEncoder().encode(SCENE_MAGIC));
  const data = new DataView(bytes.buffer);
  data.setUint16(4, SCENE_MAJOR, true);
  data.setUint16(6, SCENE_MINOR, true);
  data.setUint32(8, (source.chunk_origins?.length ?? 0) / 2, true);
  data.setUint32(12, columns.length, true);
  for (let i = 0; i < columns.length; i++) {
    data.setUint32(16 + i * 8, offsets[i], true);
    data.setUint32(20 + i * 8, columns[i].byteLength, true);
    const { type } = SCENE_SECTIONS[i];
    const setter = { u8: "setUint8", u32: "setUint32", f32: "setFloat32", f64: "setFloat64" }[type];
    for (let j = 0; j < columns[i].length; j++) data[setter](offsets[i] + j * columns[i].BYTES_PER_ELEMENT, columns[i][j], true);
  }
  const scene = readScene(bytes);
  const c = scene.columns;
  return { scene, document: { doc: "synthetic", format: "dxf", hash: "v2:00000000000000000000000000000001" }, result: {
    from: "space/0", unit: "unknown", tolerance: 0.01, blob: 0,
    counts: { chunks: scene.chunks, lines: c.line_units.length, faces: c.face_units.length, texts: c.text_units.length,
      points: c.point_units.length, images: c.image_units.length, glyphs: c.glyph_shapes.length, vertices: (c.line_xy.length + c.face_xy.length) / 2 },
    ...changes,
  } };
}

export function fixture(origin = 0) {
  const c = {};
  const bytes = [];
  const str = (s) => { const b = new TextEncoder().encode(s); const at = bytes.length; bytes.push(...b); return [at, b.length]; };
  c.chunk_origins = [origin, origin];
  c.chunk_bounds = [-45, -45, 45, 45];
  for (const kind of ["lines", "faces", "texts", "points", "images"]) c[`chunk_${kind}`] = [0];
  c.entity_ids = [0, 1, 2, 3, 4, 5, 6, 7].flatMap((i) => str(`space/0/h${i}`));
  c.entity_kinds = [V.entity_kind.hatch, V.entity_kind.line, V.entity_kind.text, V.entity_kind.text, V.entity_kind.point, V.entity_kind.underlay, V.entity_kind.hatch, V.entity_kind.point];
  c.entity_paths = [0, 0, 1, 1, 1, 1, 1, 1];
  c.step_ids = str("space/0/hinsert");
  c.step_cells = [2, 3];
  c.unit_entities = [0, 1, 2, 3, 4, 5, 6, 7];
  c.unit_colors = [0, 1, 1, 1, 1, 1, 2, 3];
  c.unit_weights = [0, 2, 0, 0, 1, 0, 0, 1];
  c.unit_clips = [0, SCENE_NONE, SCENE_NONE, SCENE_NONE, SCENE_NONE, SCENE_NONE, 1, SCENE_NONE];
  c.unit_layers = Array(8).fill(SCENE_NONE);
  c.color_systems = [0, 0, 0, V.color_system.aci];
  c.color_codes = [0, 0, 0, 7];
  c.color_files = Array(16).fill(0);
  c.color_rgba = [255, 0, 0, 255, 0, 0, 255, 255, 0, 180, 0, 255, 0, 0, 0, 0];
  c.color_flags = [0, 0, 0, V.color_flag.foreground | V.color_flag.unresolved];
  c.clip_polygons = [0, 1];
  c.polygon_vertices = [0, 4, 8];
  c.polygon_flags = [1, 0, 0];
  c.clip_xy = [-10, -10, 10, -10, 10, 10, -10, 10, -40, -40, -20, -40, -20, -20, -40, -20,
    -35, -45, -25, -45, -25, -15, -35, -15].map((v) => v + origin);
  c.face_rings = [0, 2];
  c.face_units = [0, 6];
  c.ring_vertices = [0, 4, 8];
  c.face_xy = [-40, -40, 40, -40, 40, 40, -40, 40, 20, 20, 30, 20, 30, 30, 20, 30, -45, -45, 45, -45, 45, 45, -45, 45];
  c.line_vertices = [0];
  c.line_units = [1];
  c.line_xy = [-30, 15, 30, 15];
  c.text_strings = [...str("MUST NOT DRAW"), ...str("Fallback")];
  c.text_xy = [0, 35, -35, -32];
  c.text_second = [NaN, NaN, NaN, NaN];
  c.text_heights = [8, 5];
  c.text_widths = [1, 1];
  c.text_rotations = [0, 0];
  c.text_obliques = [0, 0];
  c.text_boxes = [0, 0];
  c.text_aligns = [0, 0, 0, 0];
  c.text_flags = [0, 0];
  c.text_lines = [0, 0];
  c.text_fonts = [SCENE_NONE, SCENE_NONE];
  c.text_units = [2, 3];
  c.text_glyphs = [0, 1];
  c.glyph_shapes = [0];
  c.glyph_axes = [1, 0, 0, 1];
  c.glyph_xy = [-30, 25];
  c.shape_rings = [0];
  c.outline_vertices = [0, 4];
  // 内側の輪は逆向き。非零規則で穴になる。
  c.outline_xy = [0, 0, 8, 0, 8, 8, 0, 8, 2, 2, 2, 6, 6, 6, 6, 2];
  c.point_xy = [35, -35, 35, 35];
  c.point_rotations = [0, 0];
  c.point_markers = [0, 0, 0, 0, 0, 0, 0, 0];
  c.point_scales = [1, 1];
  c.point_units = [4, 7];
  c.point_style = [V.point_mark.plus, V.point_frame.circle, V.point_size.screen, 8];
  c.image_names = str("https://example.invalid/do-not-fetch.png");
  c.image_pages = [0, 0];
  c.image_xy = [0, 0];
  c.image_edges = [10, 0, 0, 10];
  c.image_kinds = [V.image_kind.image];
  c.image_frames = [V.image_frame.corners];
  c.image_units = [5];
  c.strings = bytes;
  return c;
}
