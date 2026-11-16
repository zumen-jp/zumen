// 検査用の最小の ZMS3 1.0 を列から組む。値は既知の期待値と照らすために自作する。
import { SCENE3D_SECTIONS } from "@zumen-jp/zumen";

const sizes = { u8: 1, u16: 2, u32: 4, f32: 4, f64: 8 };
const setters = { u8: "setUint8", u16: "setUint16", u32: "setUint32", f32: "setFloat32", f64: "setFloat64" };

/** 列の値の配列から blob と各列の開始位置を作る。 */
export function encode(columns) {
  const definitions = SCENE3D_SECTIONS.slice(0, 20);
  let end = 16 + definitions.length * 8;
  const starts = {};
  for (const s of definitions) {
    end = Math.ceil(end / 8) * 8;
    starts[s.name] = end;
    end += columns[s.name].length * sizes[s.type];
  }
  const bytes = new Uint8Array(end), view = new DataView(bytes.buffer);
  bytes.set([0x5a, 0x4d, 0x53, 0x33]);
  view.setUint16(4, 1, true); view.setUint16(6, 0, true);
  view.setUint32(8, columns.chunk_origins.length / 3, true); view.setUint32(12, definitions.length, true);
  for (const [i, s] of definitions.entries()) {
    const values = columns[s.name];
    view.setUint32(16 + i * 8, starts[s.name], true);
    view.setUint32(20 + i * 8, values.length * sizes[s.type], true);
    values.forEach((value, at) => view[setters[s.type]](starts[s.name] + at * sizes[s.type], value, true));
  }
  return { bytes, view, starts };
}

/** 平行移動と対角の尺度の 3 行 4 列（行優先）。 */
export const place = (sx, sy, sz, tx, ty, tz = 0) => [sx, 0, 0, tx, 0, sy, 0, ty, 0, 0, sz, tz];

export const ORIGIN = [1e12, 2e12, 0];

/**
 * 大きな原点の塊に、1 の正方形を置いた合成のシーン。上から見た平行投影で 1 単位 10 px。
 * - red: 2 つの配置（相似）、mirror: 反転、stretch: 非一様な尺度、half: 半透明の青
 * - invisible: 透明度 0、unresolved: 未解決の色、storey: 形のない要素
 */
export function synthetic() {
  const text = [];
  const string = (value) => {
    const bytes = new TextEncoder().encode(value), start = text.length;
    text.push(...bytes);
    return [start, bytes.length];
  };
  const elements = ["red", "mirror", "stretch", "half", "invisible", "unresolved", "storey"];
  const instances = [
    // [要素, 色, 変換]
    [0, 0, place(2, 2, 2, -8, 2)],
    [0, 0, place(2, 2, 2, -8, -4)],
    [1, 0, place(-2, 2, 2, 6, 2)],
    [2, 0, place(3, 1, 1, 3, -4)],
    [3, 1, place(2, 2, 2, -1, -1)],
    [4, 2, place(2, 2, 2, -1, 5)],
    [5, 3, place(2, 2, 2, -1, -8)],
  ];
  const columns = {
    chunk_origins: [...ORIGIN], chunk_bounds: [-8, -8, 0, 6, 7, 0], chunk_instances: [0],
    mesh_vertices: [0], mesh_triangles: [0], mesh_edges: [0],
    vertex_xyz: [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0], triangle_vertices: [0, 1, 2, 0, 2, 3], edge_vertices: [0, 1, 1, 2, 2, 3, 3, 0],
    instance_meshes: instances.map(() => 0), instance_elements: instances.map((i) => i[0]),
    instance_colors: instances.map((i) => i[1]), instance_transforms: instances.flatMap((i) => i[2]),
    element_ids: elements.flatMap(string), element_classes: elements.flatMap((id) => string(id === "storey" ? "IfcBuildingStorey" : "IfcWall")),
    element_names: elements.flatMap(() => string("")), element_storeys: elements.map((id) => (id === "storey" ? 0xffffffff : 6)),
    color_rgba: [1, 0, 0, 1, 0, 0, 1, 0.5, 0, 1, 0, 0, 0, 0, 0, 1], color_flags: [0, 0, 0, 1],
    strings: text,
  };
  return { columns, elements, instances };
}

/** 上から見る平行投影。幅 20 単位を 200 px に写す。 */
export const topView = (id = "top", rect = { x: 0, y: 0, width: 200, height: 200 }) => ({
  id, rect,
  camera: { projection: "orthographic", position: [ORIGIN[0], ORIGIN[1], 10], direction: [0, 0, -1], up: [0, 1, 0], width: 20 * rect.width / 200 },
});

export const document = Object.freeze({ doc: "synthetic.ifc", format: "ifc", hash: "0".repeat(64) });

/** 列から応答の counts を作る。 */
export function result(columns, doc = document.doc) {
  const rows = (name, width) => columns[name].length / width;
  return {
    doc, unit: { kind: "LENGTHUNIT", name: "METRE" }, tolerance: 0.001,
    counts: {
      chunks: rows("chunk_origins", 3), meshes: rows("mesh_vertices", 1), instances: rows("instance_meshes", 1),
      elements: rows("element_storeys", 1), colors: rows("color_flags", 1), vertices: rows("vertex_xyz", 3),
      triangles: rows("triangle_vertices", 3), edges: rows("edge_vertices", 2),
    },
    blob: 0,
  };
}
