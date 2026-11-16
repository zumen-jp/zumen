// 自作の最小列で、GPU へ渡す前の範囲・参照・値・予算の検査を確かめる。
import assert from "node:assert/strict";
import { readScene3D, SCENE3D_SECTIONS } from "../src/index.js";

function fixture(future = false, empty = false, name = "壁") {
  const text = [];
  function string(value) {
    const bytes = new TextEncoder().encode(value), start = text.length;
    text.push(...bytes);
    return [start, bytes.length];
  }
  const c = {
    chunk_origins: [1e12, 0, 0], chunk_bounds: [0, 0, 0, 1001, 1, 0], chunk_instances: [0],
    mesh_vertices: [0], mesh_triangles: [0], mesh_edges: [0],
    vertex_xyz: [0, 0, 0, 1, 0, 0, 0, 1, 0], triangle_vertices: [0, 1, 2], edge_vertices: [0, 1, 1, 2, 2, 0],
    instance_meshes: [0, 0], instance_elements: [0, 0], instance_colors: [0, 0],
    instance_transforms: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1000, 0, 1, 0, 0, 0, 0, 1, 0],
    element_ids: [...string("wall"), ...string("floor")],
    element_classes: [...string("IfcWall"), ...string("IfcBuildingStorey")],
    element_names: [...string(name), ...string("1階")], element_storeys: [1, 0xffffffff],
    color_rgba: [0.25, 0.5, 1, 0.75], color_flags: [0], strings: text,
  };
  const definitions = [...SCENE3D_SECTIONS];
  if (future) { definitions.push({ name: "future", type: "u8", width: 1 }); c.future = [1, 2, 3]; }
  const sizes = { u8: 1, u32: 4, f32: 4, f64: 8 };
  const setters = { u8: "setUint8", u32: "setUint32", f32: "setFloat32", f64: "setFloat64" };
  let end = 16 + definitions.length * 8;
  const starts = {};
  for (const s of definitions) { end = Math.ceil(end / 8) * 8; starts[s.name] = end; end += empty ? 0 : c[s.name].length * sizes[s.type]; }
  const bytes = new Uint8Array(end), view = new DataView(bytes.buffer);
  bytes.set([0x5a, 0x4d, 0x53, 0x33]);
  view.setUint16(4, 1, true); view.setUint16(6, future ? 1 : 0, true);
  view.setUint32(8, empty ? 0 : 1, true); view.setUint32(12, definitions.length, true);
  for (const [i, s] of definitions.entries()) {
    const values = empty ? [] : c[s.name];
    view.setUint32(16 + i * 8, starts[s.name], true);
    view.setUint32(20 + i * 8, values.length * sizes[s.type], true);
    values.forEach((value, at) => view[setters[s.type]](starts[s.name] + at * sizes[s.type], value, true));
  }
  return { bytes, view, starts };
}

const { bytes } = fixture();
const scene = readScene3D(bytes);
assert.equal(scene.chunk(0).origin[0], 1e12);
assert.equal(scene.element(0).name, "壁");
assert.equal(readScene3D(fixture(false, false, "\uFEFF壁").bytes).element(0).name, "\uFEFF壁");
assert.equal(scene.element(scene.element(0).storey).id, "floor");
assert.equal(scene.elementById("wall"), 0);
assert.equal(scene.elementById("missing"), undefined);
assert.equal(scene.mesh(scene.instance(1).mesh).vertices.length, 9);
assert.deepEqual([...scene.color(0).rgba], [0.25, 0.5, 1, 0.75]);
assert.equal(readScene3D(fixture(true).bytes).sections, 21);
assert.equal(readScene3D(fixture(false, true).bytes).rows("element_ids"), 0);
const unaligned = new Uint8Array(bytes.length + 1); unaligned.set(bytes, 1);
assert.equal(readScene3D(unaligned.subarray(1)).element(0).name, "壁");
for (let end = 0; end < bytes.length; end += 1) assert.throws(() => readScene3D(bytes.subarray(0, end)));
for (const mutate of [
  ({ bytes }) => { bytes[0] = 0; }, ({ bytes }) => { bytes[4] = 2; },
  ({ view }) => view.setUint32(12, 0xffffffff, true),
  ({ view }) => view.setUint32(16, 0, true),
  ({ view }) => view.setUint32(20, 1, true),
  ...["instance_meshes", "instance_elements", "instance_colors", "element_storeys", "triangle_vertices"].map(name =>
    ({ view, starts }) => view.setUint32(starts[name], 42, true)),
  ({ view, starts }) => view.setFloat32(starts.vertex_xyz, NaN, true),
  ({ view, starts }) => view.setFloat32(starts.color_rgba, 2, true),
  ({ bytes, starts }) => { bytes[starts.color_flags] = 1; },
  ({ bytes, starts }) => bytes.fill(0, starts.instance_transforms, starts.instance_transforms + 48),
  ({ bytes, starts }) => { bytes[starts.strings] = 255; },
  ({ bytes, starts }) => bytes.copyWithin(starts.element_ids + 8, starts.element_ids, starts.element_ids + 8),
  ({ view, starts }) => view.setFloat32(starts.chunk_bounds, 2000, true),
]) {
  const changed = fixture(); mutate(changed); assert.throws(() => readScene3D(changed.bytes));
}
for (const options of [{ maxBytes: 0 }, { maxWork: 0 }, { maxAllocatedBytes: 0 }, { maxWork: -1 }, { unknown: 1 }]) {
  assert.throws(() => readScene3D(bytes, options));
}
for (const row of [-1, 2, 0xffffffff, NaN]) assert.throws(() => scene.instance(row));
assert.throws(() => scene.text(0xffffffff, 1));
assert.throws(() => scene.string("vertex_xyz", 0));
console.log("3D の読み手: 列・参照・UTF-8・不正な値・予算・未整列・将来の列を確認");
