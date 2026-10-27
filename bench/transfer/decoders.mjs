// ページの側の復号。受け取ったものを、描画に使える型付き配列にする。Node とブラウザーで共有する。
import { tableFromIPC } from "apache-arrow";

export const now = () => performance.timeOrigin + performance.now();

// 受け取ったものから、原点（f64）・頂点の始まり（u32）・相対座標（f32）を取り出す。
export const decoders = {
  json(message) {
    const scene = JSON.parse(message.text);
    const parsed = now();
    let vertices = 0;
    for (const chunk of scene.chunks) {
      for (const polyline of chunk.polylines) vertices += polyline.points.length / 2;
    }
    const coords = new Float32Array(vertices * 2);
    const start = new Uint32Array(scene.chunks.reduce((n, c) => n + c.polylines.length, 0) + 1);
    const origins = new Float64Array(scene.chunks.length * 2);
    let p = 0;
    let v = 0;
    scene.chunks.forEach((chunk, i) => {
      origins[i * 2] = chunk.origin[0];
      origins[i * 2 + 1] = chunk.origin[1];
      for (const polyline of chunk.polylines) {
        start[p++] = v / 2;
        coords.set(polyline.points, v);
        v += polyline.points.length;
      }
    });
    start[p] = v / 2;
    return { origins, start, coords, parsed, keep: scene };
  },
  columnar(message) {
    const buffer = message.buffer;
    const head = new DataView(buffer);
    if (head.getUint32(0, true) !== 0x43534d5a) throw new Error("not ZMSC");
    const sections = head.getUint32(20, true);
    const view = (index, Type) => {
      if (index >= sections) return undefined;
      const at = 24 + index * 8;
      const offset = head.getUint32(at, true);
      return new Type(buffer, offset, head.getUint32(at + 4, true) / Type.BYTES_PER_ELEMENT);
    };
    return {
      origins: view(0, Float64Array),
      start: view(2, Uint32Array),
      coords: view(6, Float32Array),
      layer: view(3, Uint32Array),
    };
  },
  flatbuffers(message) {
    const buffer = message.buffer;
    const data = new DataView(buffer);
    const table = data.getUint32(0, true);
    const vtable = table - data.getInt32(table, true);
    const vector = (index, Type) => {
      const field = data.getUint16(vtable + 4 + index * 2, true);
      if (field === 0) return undefined;
      const at = table + field;
      const position = at + data.getUint32(at, true);
      return new Type(buffer, position + 4, data.getUint32(position, true));
    };
    return {
      origins: vector(0, Float64Array),
      start: vector(2, Uint32Array),
      coords: vector(6, Float32Array),
      layer: vector(3, Uint32Array),
    };
  },
  arrow(message) {
    const bytes = new Uint8Array(message.buffer);
    const length = new DataView(message.buffer).getUint32(0, true);
    const polylines = tableFromIPC(bytes.subarray(8, 8 + length));
    const origins = tableFromIPC(bytes.subarray(8 + length));
    const points = polylines.getChild("points").data[0];
    const x = origins.getChild("x").toArray();
    const y = origins.getChild("y").toArray();
    const xy = new Float64Array(x.length * 2);
    x.forEach((value, i) => {
      xy[i * 2] = value;
      xy[i * 2 + 1] = y[i];
    });
    return {
      origins: xy,
      start: points.valueOffsets,
      coords: points.children[0].values,
      layer: polylines.getChild("layer").toArray(),
    };
  },
};
