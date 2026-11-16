// ZMS3 の読み手。GPU へ渡す前に列と参照を検査する。blob と列は呼び出し側で変更しない。
import { SCENE3D_MAGIC, SCENE3D_MAJOR, SCENE3D_NONE, SCENE3D_SECTIONS } from "./scene3d-layout.js";

const arrays = { u8: Uint8Array, u32: Uint32Array, f32: Float32Array, f64: Float64Array };
const getters = { u8: "getUint8", u32: "getUint32", f32: "getFloat32", f64: "getFloat64" };
const littleEndian = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;
const definitions = new Map(SCENE3D_SECTIONS.map(s => [s.name, s]));
const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const defaults = Object.freeze({ maxBytes: 536870912, maxAllocatedBytes: 268435456, maxWork: 100000000 });

function bad(reason) { throw new RangeError("3D シーン: " + reason); }
function index(row, count) {
  if (!Number.isSafeInteger(row) || row < 0 || row >= count) bad("行の範囲");
  return row;
}
function bytesOf(input) {
  if (ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  if (Object.prototype.toString.call(input) === "[object ArrayBuffer]") return new Uint8Array(input);
  throw new TypeError("バイト列を渡す");
}
function limits(options, bytes) {
  const values = { ...defaults, ...options };
  for (const [key, value] of Object.entries(values)) {
    if (!(key in defaults) || !Number.isSafeInteger(value) || value < 0) bad("読み取りの上限");
  }
  if (bytes > 536870912 || bytes > values.maxBytes) bad("バイト数の上限");
  let work = 0, allocated = 0;
  return {
    work(n) { work += n; if (work > values.maxWork) bad("作業量の上限"); },
    allocate(n) { allocated += n; if (allocated > values.maxAllocatedBytes) bad("確保量の上限"); },
  };
}

/** 検査済みの 3D シーン。列は blob を共有し、メッシュの頂点番号はメッシュ内の番号。 */
export class Scene3D {
  #ids;
  constructor(major, minor, chunks, sections, columns, ids) {
    this.major = major; this.minor = minor; this.chunks = chunks; this.sections = sections;
    this.columns = Object.freeze(columns); this.#ids = ids;
    Object.freeze(this);
  }
  rows(name) {
    const definition = definitions.get(name);
    if (!definition) bad("セクションの名前");
    return this.columns[name].length / definition.width;
  }
  text(start, length) {
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(length) || start < 0 || length < 0 || start + length > this.columns.strings.length) bad("文字列の範囲");
    return decoder.decode(this.columns.strings.subarray(start, start + length));
  }
  string(name, row) {
    if (!["element_ids", "element_classes", "element_names"].includes(name)) bad("文字列の列");
    const at = index(row, this.rows(name)) * 2;
    return this.text(this.columns[name][at], this.columns[name][at + 1]);
  }
  element(row) {
    index(row, this.rows("element_ids"));
    const storey = this.columns.element_storeys[row];
    return { id: this.string("element_ids", row), class: this.string("element_classes", row), name: this.string("element_names", row), storey: storey === SCENE3D_NONE ? null : storey };
  }
  elementById(id) { return this.#ids.get(id); }
  instance(row) {
    index(row, this.rows("instance_meshes"));
    const c = this.columns;
    return { mesh: c.instance_meshes[row], element: c.instance_elements[row], color: c.instance_colors[row], transform: c.instance_transforms.subarray(row * 12, row * 12 + 12) };
  }
  mesh(row) {
    index(row, this.rows("mesh_vertices"));
    const c = this.columns;
    const slice = (starts, target, width) => target.subarray(starts[row] * width, (starts[row + 1] ?? target.length / width) * width);
    return { vertices: slice(c.mesh_vertices, c.vertex_xyz, 3), triangles: slice(c.mesh_triangles, c.triangle_vertices, 3), edges: slice(c.mesh_edges, c.edge_vertices, 2) };
  }
  color(row) {
    index(row, this.rows("color_rgba"));
    return { rgba: this.columns.color_rgba.subarray(row * 4, row * 4 + 4), unresolved: (this.columns.color_flags[row] & 1) !== 0 };
  }
  chunk(row) {
    index(row, this.chunks);
    const c = this.columns;
    return { origin: c.chunk_origins.subarray(row * 3, row * 3 + 3), bounds: c.chunk_bounds.subarray(row * 6, row * 6 + 6), begin: c.chunk_instances[row], end: c.chunk_instances[row + 1] ?? this.rows("instance_meshes") };
  }
}

function validate(scene, budget, ids) {
  const c = scene.columns;
  const rows = name => scene.rows(name);
  const equal = (names, n) => { for (const name of names) if (rows(name) !== n) bad("列の行数"); };
  const meshes = rows("mesh_vertices"), instances = rows("instance_meshes"), elements = rows("element_ids"), colors = rows("color_rgba");
  equal(["chunk_origins", "chunk_bounds", "chunk_instances"], scene.chunks);
  equal(["mesh_triangles", "mesh_edges"], meshes);
  equal(["instance_elements", "instance_colors", "instance_transforms"], instances);
  equal(["element_classes", "element_names", "element_storeys"], elements);
  equal(["color_flags"], colors);
  for (const s of SCENE3D_SECTIONS) {
    budget.work(c[s.name].length);
    if (s.type === "f32" || s.type === "f64") for (const v of c[s.name]) if (!Number.isFinite(v)) bad("有限でない値");
  }
  // 大きな文字列全体の写しを作らず、未参照の部分も UTF-8 として検査する。
  budget.allocate(16384);
  const utf8 = new TextDecoder("utf-8", { fatal: true });
  for (let at = 0; at < c.strings.length; at += 4096) utf8.decode(c.strings.subarray(at, at + 4096), { stream: true });
  utf8.decode();
  for (const [source, target] of [["mesh_vertices", "vertex_xyz"], ["mesh_triangles", "triangle_vertices"], ["mesh_edges", "edge_vertices"], ["chunk_instances", "instance_meshes"]]) {
    const starts = c[source], count = rows(target);
    budget.work(starts.length);
    if ((starts.length === 0 && count !== 0) || (starts.length > 0 && starts[0] !== 0)) bad("開始位置");
    let previous = 0;
    for (const value of starts) { if (value < previous || value > count) bad("開始位置"); previous = value; }
  }
  for (let row = 0; row < meshes; row++) {
    const mesh = scene.mesh(row), count = mesh.vertices.length / 3;
    budget.work(mesh.triangles.length + mesh.edges.length + 1);
    for (const column of [mesh.triangles, mesh.edges]) for (const at of column) if (at >= count) bad("頂点の参照");
  }
  for (let row = 0; row < elements; row++) {
    for (const name of ["element_ids", "element_classes", "element_names"]) {
      const length = c[name][row * 2 + 1];
      budget.work(length + 1); budget.allocate(length * 2 + 32);
    }
    budget.allocate(128);
    const element = scene.element(row);
    if (!element.id || ids.has(element.id)) bad("要素 ID の重複か空文字列");
    ids.set(element.id, row);
    if (element.storey !== null && element.storey >= elements) bad("階の参照");
  }
  for (let row = 0; row < colors; row++) {
    budget.work(4);
    const color = scene.color(row);
    for (const value of color.rgba) if (value < 0 || value > 1) bad("色の範囲");
    if (color.unresolved && color.rgba.subarray(0, 3).some(v => v !== 0)) bad("未解決の色");
  }
  budget.allocate(elements * 4);
  const owners = new Uint32Array(elements); owners.fill(SCENE3D_NONE);
  for (let row = 0; row < scene.chunks; row++) {
    budget.work(1);
    const chunk = scene.chunk(row);
    for (let axis = 0; axis < 3; axis++) if (chunk.bounds[axis] > chunk.bounds[axis + 3]) bad("塊の範囲");
    for (let i = chunk.begin; i < chunk.end; i++) {
      budget.work(1);
      const instance = scene.instance(i);
      if (instance.mesh >= meshes || instance.element >= elements || instance.color >= colors) bad("インスタンスの参照");
      if (owners[instance.element] !== SCENE3D_NONE && owners[instance.element] !== row) bad("要素が複数の塊にある");
      owners[instance.element] = row;
      const [a,b,c,,d,e,f,,g,h,j] = instance.transform;
      if (a * (e * j - f * h) - b * (d * j - f * g) + c * (d * h - e * g) === 0) bad("特異な変換");
    }
  }
}

/** 3D シーンを検査して読む。確保量と作業量の上限はコアの予算とは別。 */
export function readScene3D(input, options = {}) {
  const bytes = bytesOf(input), budget = limits(options, bytes.length);
  budget.work(1);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.length < 16 || decoder.decode(bytes.subarray(0, 4)) !== SCENE3D_MAGIC) bad("ZMS3 のヘッダー");
  const major = view.getUint16(4, true), minor = view.getUint16(6, true), chunks = view.getUint32(8, true), sections = view.getUint32(12, true);
  if (major !== SCENE3D_MAJOR || sections < SCENE3D_SECTIONS.length || 16 + 8 * sections > bytes.length) bad("版かセクションの表");
  budget.work(sections);
  const columns = {};
  let previous = 16 + sections * 8;
  for (let i = 0; i < sections; i++) {
    const start = view.getUint32(16 + i * 8, true), length = view.getUint32(20 + i * 8, true);
    if (start % 8 !== 0 || start < previous || start + length > bytes.length) bad("セクションの範囲");
    previous = start + length;
    const definition = SCENE3D_SECTIONS[i];
    if (!definition) continue;
    const Type = arrays[definition.type], size = Type.BYTES_PER_ELEMENT;
    if (length % (size * definition.width) !== 0) bad("行の幅");
    if (littleEndian && (bytes.byteOffset + start) % size === 0) columns[definition.name] = new Type(bytes.buffer, bytes.byteOffset + start, length / size);
    else {
      budget.allocate(length); budget.work(length / size);
      const column = new Type(length / size);
      for (let at = 0; at < column.length; at++) column[at] = view[getters[definition.type]](start + at * size, true);
      columns[definition.name] = column;
    }
  }
  const ids = new Map(), scene = new Scene3D(major, minor, chunks, sections, columns, ids);
  validate(scene, budget, ids);
  return scene;
}
