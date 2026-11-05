import { SCENE_MAJOR, SCENE_MINOR, SCENE_NONE, SCENE_SECTIONS, SCENE_VALUES } from "@zumen-jp/zumen";

export const DEFAULT_LIMITS = Object.freeze({
  columnValues: 16_000_000,
  work: 32_000_000,
  issues: 10_000,
  textLength: 16_384,
  canvasPixels: 16_777_216,
});

export function valid(condition, field) {
  if (!condition) throw new RangeError(`不正な値: ${field}`);
}

export function finite(value, field) {
  valid(typeof value === "number" && Number.isFinite(value) && Math.abs(value) <= 1e150, field);
  return value;
}

export function bounds(value, field = "bounds") {
  valid(value && typeof value === "object", field);
  const out = {};
  for (const key of ["min_x", "min_y", "max_x", "max_y"]) out[key] = finite(value[key], `${field}.${key}`);
  valid(out.min_x <= out.max_x && out.min_y <= out.max_y, field);
  return Object.freeze(out);
}

export function limitsOf(options = {}) {
  const limits = { ...DEFAULT_LIMITS, ...options };
  for (const [key, value] of Object.entries(limits)) {
    valid(Object.prototype.hasOwnProperty.call(DEFAULT_LIMITS, key) && Number.isSafeInteger(value) && value > 0, `limits.${key}`);
  }
  return Object.freeze(limits);
}

export const KINDS = ["face", "line", "text", "point", "image"];
export const STARTS = { face: "face_rings", line: "line_vertices", text: "text_strings", point: "point_xy", image: "image_names" };
const TYPES = { u8: "Uint8Array", u32: "Uint32Array", f32: "Float32Array", f64: "Float64Array" };
const WIDTHS = Object.fromEntries(SCENE_SECTIONS.map(({ name, width }) => [name, width]));
export const rows = (columns, name) => columns[name].length / WIDTHS[name];
export const end = (starts, index, count) => starts[index + 1] ?? count;

/** 列は写さない。列を使う間、呼び出し側は元のバッファーを変更・転送しない。 */
export function prepare(input, limits) {
  valid(input && typeof input === "object", "scene input");
  const { scene, result, document } = input;
  valid(scene?.major === SCENE_MAJOR && scene.minor <= SCENE_MINOR && Number.isInteger(scene.minor) && scene.minor >= 0, "scene.version");
  const c = scene.columns;
  valid(c && Number.isSafeInteger(scene.chunks) && scene.chunks >= 0, "scene.chunks");
  let values = 0;
  let work = 0;
  const charge = (count) => {
    work += count;
    valid(Number.isSafeInteger(work) && work <= limits.work, "limits.work");
  };
  for (const { name, type, width } of SCENE_SECTIONS) {
    const column = c[name];
    valid(Object.prototype.toString.call(column) === `[object ${TYPES[type]}]` && column.length % width === 0, name);
    values += column.length;
    valid(values <= limits.columnValues, "limits.columnValues");
  }
  charge(values);
  for (const { name, type } of SCENE_SECTIONS) {
    if (type === "f32" || type === "f64") {
      if (name === "text_second") continue;
      for (const value of c[name]) finite(value, name);
    }
  }
  const same = (base, names) => {
    const n = rows(c, base);
    for (const name of names) valid(rows(c, name) === n, name);
  };
  same("chunk_origins", ["chunk_bounds", ...KINDS.map((kind) => `chunk_${kind}s`)]);
  valid(rows(c, "chunk_origins") === scene.chunks, "chunk_origins");
  same("line_vertices", ["line_units"]);
  same("face_rings", ["face_units"]);
  same("text_strings", ["text_xy", "text_second", "text_heights", "text_widths", "text_rotations", "text_obliques", "text_boxes", "text_aligns", "text_flags", "text_lines", "text_fonts", "text_units"]);
  same("point_xy", ["point_rotations", "point_markers", "point_scales", "point_units"]);
  same("image_names", ["image_pages", "image_xy", "image_edges", "image_kinds", "image_frames", "image_units"]);
  same("unit_entities", ["unit_colors", "unit_weights", "unit_clips", "unit_layers"]);
  same("entity_ids", ["entity_kinds", "entity_paths"]);
  same("step_ids", ["step_cells"]);
  same("color_systems", ["color_codes", "color_files", "color_rgba", "color_flags"]);
  same("polygon_vertices", ["polygon_flags"]);
  same("glyph_shapes", ["glyph_axes", "glyph_xy"]);
  valid(c.point_style.length === 0 || c.point_style.length === 4, "point_style");
  if (c.text_glyphs.length) same("text_strings", ["text_glyphs"]);
  else valid(c.glyph_shapes.length === 0, "text_glyphs");

  function offsets(name, target) {
    const column = c[name];
    const count = rows(c, target);
    valid(column.length !== 0 || count === 0, name);
    let previous = 0;
    for (let i = 0; i < column.length; i++) {
      valid(column[i] >= previous && column[i] <= count && (i !== 0 || column[i] === 0), name);
      previous = column[i];
    }
  }
  for (const kind of KINDS) offsets(`chunk_${kind}s`, STARTS[kind]);
  for (const [name, target] of [
    ["line_vertices", "line_xy"], ["face_rings", "ring_vertices"], ["ring_vertices", "face_xy"],
    ["entity_paths", "step_ids"], ["clip_polygons", "polygon_vertices"], ["polygon_vertices", "clip_xy"],
    ["text_glyphs", "glyph_shapes"], ["shape_rings", "outline_vertices"], ["outline_vertices", "outline_xy"],
  ]) offsets(name, target);

  function refs(name, target, optional = false) {
    const count = rows(c, target);
    for (const value of c[name]) valid(value < count || (optional && value === SCENE_NONE), name);
  }
  for (const kind of KINDS) refs(`${kind}_units`, "unit_entities");
  refs("unit_entities", "entity_ids");
  refs("unit_colors", "color_systems", true);
  refs("unit_clips", "clip_polygons", true);
  refs("unit_layers", "layer_ids", true);
  refs("text_fonts", "font_names", true);
  refs("glyph_shapes", "shape_rings");
  for (const name of ["text_strings", "point_markers", "image_names", "image_pages", "entity_ids", "step_ids", "layer_ids", "font_names"]) {
    const decode = name === "text_strings" || name === "entity_ids" || name === "step_ids";
    for (let i = 0; i < c[name].length; i += 2) {
      valid(c[name][i] + c[name][i + 1] <= c.strings.length, name);
      // 同じ文字列を多数の ID・配置から引く場合も、復号する長さを数える。
      if (decode) charge(c[name][i + 1]);
    }
  }
  for (let i = 0; i < c.chunk_bounds.length; i += 4) {
    valid(c.chunk_bounds[i] <= c.chunk_bounds[i + 2] && c.chunk_bounds[i + 1] <= c.chunk_bounds[i + 3], "chunk_bounds");
  }
  for (const name of ["unit_weights", "text_heights", "text_widths", "text_boxes", "point_scales"]) {
    for (const value of c[name]) valid(value >= 0, name);
  }
  for (let i = 0; i < c.text_heights.length; i++) {
    const x = c.text_second[2 * i], y = c.text_second[2 * i + 1];
    const align = c.text_aligns[2 * i];
    if (align === SCENE_VALUES.horizontal.aligned || align === SCENE_VALUES.horizontal.fit) {
      finite(x, "text_second"); finite(y, "text_second");
    } else valid((Number.isNaN(x) && Number.isNaN(y)) || (Number.isFinite(x) && Number.isFinite(y)), "text_second");
    valid(c.text_strings[2 * i + 1] <= limits.textLength, "limits.textLength");
    if ((c.text_glyphs[i] ?? 0) === end(c.text_glyphs, i, c.glyph_shapes.length)) {
      const measurements = align === SCENE_VALUES.horizontal.middle ? 1 : c.text_aligns[2 * i + 1] === SCENE_VALUES.vertical.baseline ? 0 : 2;
      charge(measurements * c.text_strings[2 * i + 1]); // 代替文字の位置合わせを measureText で調べる長さも数える。
    }
  }

  valid(document && ["doc", "format", "hash"].every((key) => typeof document[key] === "string" && document[key].length > 0 && document[key].length <= 4096), "document");
  valid(result && typeof result.from === "string" && result.from.length > 0 && typeof result.unit === "string" && result.unit.length > 0, "result");
  finite(result.tolerance, "result.tolerance");
  valid(result.tolerance >= 0 && Number.isSafeInteger(result.blob) && result.blob >= 0, "result");
  valid(result.view === undefined || (Number.isSafeInteger(result.view) && result.view >= 0), "result.view");
  const counts = {
    chunks: scene.chunks, lines: c.line_units.length, faces: c.face_units.length, texts: c.text_units.length,
    points: c.point_units.length, images: c.image_units.length, glyphs: c.glyph_shapes.length,
    vertices: rows(c, "line_xy") + rows(c, "face_xy"),
  };
  for (const [name, count] of Object.entries(counts)) valid(result.counts?.[name] === count, `result.counts.${name}`);
  const metadata = Object.freeze({
    scene,
    result: Object.freeze({ ...result, counts: Object.freeze(counts), ...(result.extent === undefined ? {} : { extent: bounds(result.extent, "result.extent") }) }),
    document: Object.freeze({ doc: document.doc, format: document.format, hash: document.hash }),
  });

  // 共有した切り抜きと字形も、実際に描く回数で数える。Canvas への描画を始める前に全体を確かめる。
  const clipWork = new Float64Array(c.clip_polygons.length);
  for (let i = 0; i < clipWork.length; i++) {
    for (let p = c.clip_polygons[i]; p < end(c.clip_polygons, i, c.polygon_vertices.length); p++) {
      clipWork[i] += 1 + end(c.polygon_vertices, p, rows(c, "clip_xy")) - c.polygon_vertices[p];
    }
  }
  const shapeWork = new Float64Array(c.shape_rings.length);
  for (let i = 0; i < shapeWork.length; i++) {
    for (let r = c.shape_rings[i]; r < end(c.shape_rings, i, c.outline_vertices.length); r++) {
      shapeWork[i] += 1 + end(c.outline_vertices, r, rows(c, "outline_xy")) - c.outline_vertices[r];
    }
  }
  for (const kind of KINDS) for (const unit of c[`${kind}_units`]) charge(1 + (clipWork[c.unit_clips[unit]] ?? 0));
  for (const shape of c.glyph_shapes) charge(shapeWork[shape]);

  // 描く単位の順の索引だけを持つ。形・座標・書式を要素ごとの別モデルに写さない。
  const order = [];
  for (let chunk = 0; chunk < scene.chunks; chunk++) {
    const ranges = KINDS.map((kind) => [c[`chunk_${kind}s`][chunk], end(c[`chunk_${kind}s`], chunk, rows(c, STARTS[kind]))]);
    const bases = [0];
    for (const [start, finish] of ranges) bases.push(bases[bases.length - 1] + finish - start);
    const count = bases[bases.length - 1];
    charge(count * Math.max(1, Math.ceil(Math.log2(count + 1))));
    const units = new Uint32Array(count);
    for (let k = 0; k < KINDS.length; k++) units.set(c[`${KINDS[k]}_units`].subarray(...ranges[k]), bases[k]);
    const locate = (index) => {
      let k = 0;
      while (index >= bases[k + 1]) k++;
      return [KINDS[k], ranges[k][0] + index - bases[k]];
    };
    const indices = Uint32Array.from({ length: count }, (_, i) => i);
    indices.sort((a, b) => units[a] - units[b] || a - b);
    order.push({ indices, locate });
  }
  return { metadata, order, columns: c };
}
