// シーンの blob のリーダー（決定 0137・0194）。配置は schema/scene.toml にあり、セクションの定義は constants.js に生成する。
//
// ヘッダーは `ZMSC`、メジャーバージョン（u16）、マイナーバージョン（u16）、塊の数（u32）、セクションの数（u32）で、続けてセクションごとに始まりと長さ（u32、
// バイト）を置く。数はリトルエンディアン。知らないセクション（新しいマイナーバージョンで足したもの）は読み飛ばし、ないセクション（古いマイナーバージョン）は
// 空の列にする。メジャーバージョンが違えば読まない。

import { SCENE_MAGIC, SCENE_MAJOR, SCENE_SECTIONS } from "./constants.js";

const ARRAYS = {
  u8: Uint8Array,
  u32: Uint32Array,
  f32: Float32Array,
  f64: Float64Array,
};

const GETTERS = {
  u8: (view, at) => view.getUint8(at),
  u32: (view, at) => view.getUint32(at, true),
  f32: (view, at) => view.getFloat32(at, true),
  f64: (view, at) => view.getFloat64(at, true),
};

const LITTLE_ENDIAN = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;

const decoder = new TextDecoder();

/** 読んだシーン */
export class Scene {
  constructor(major, minor, chunks, sections, columns) {
    /** メジャーバージョン */
    this.major = major;
    /** マイナーバージョン */
    this.minor = minor;
    /** 塊の数 */
    this.chunks = chunks;
    /** blob にあるセクションの数。このラッパーが知らないセクションも数える */
    this.sections = sections;
    /** セクションごとの列（型付き配列） */
    this.columns = columns;
    Object.freeze(this);
  }

  /** セクションの行の数 */
  rows(name) {
    const section = SCENE_SECTIONS.find((entry) => entry.name === name);
    if (!section) {
      throw new RangeError(`知らないセクション: ${name}`);
    }
    return this.columns[name].length / section.width;
  }

  /** strings のセクションから、始まりと長さ（バイト）で文字列を引く */
  text(start, length) {
    const strings = this.columns.strings;
    if (start + length > strings.length) {
      throw new RangeError("strings のセクションの外を指す");
    }
    return decoder.decode(strings.subarray(start, start + length));
  }
}

function toBytes(input) {
  if (input instanceof Uint8Array) {
    return input;
  }
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  if (input instanceof ArrayBuffer) {
    return new Uint8Array(input);
  }
  throw new TypeError("バイト列を渡す");
}

/** 列を作る。境界がそろい、環境がリトルエンディアンなら写さずに見る */
function column(bytes, view, type, start, length) {
  const Column = ARRAYS[type];
  const size = Column.BYTES_PER_ELEMENT;
  const count = length / size;
  if (LITTLE_ENDIAN && (bytes.byteOffset + start) % size === 0) {
    return new Column(bytes.buffer, bytes.byteOffset + start, count);
  }
  const out = new Column(count);
  const get = GETTERS[type];
  for (let i = 0; i < count; i += 1) {
    out[i] = get(view, start + i * size);
  }
  return out;
}

/** シーンの blob を読む。列は blob のバイト列を共有するので、blob を書き換えない */
export function readScene(input) {
  const bytes = toBytes(input);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.length < 16 || decoder.decode(bytes.subarray(0, 4)) !== SCENE_MAGIC) {
    throw new Error("シーンの blob でない");
  }
  const major = view.getUint16(4, true);
  const minor = view.getUint16(6, true);
  if (major !== SCENE_MAJOR) {
    throw new Error(`シーンのメジャーバージョンが ${major}（このラッパーは ${SCENE_MAJOR}）`);
  }
  const chunks = view.getUint32(8, true);
  const sections = view.getUint32(12, true);
  if (16 + 8 * sections > bytes.length) {
    throw new Error("セクションの表が blob に収まらない");
  }
  const columns = {};
  SCENE_SECTIONS.forEach((section, index) => {
    if (index >= sections) {
      columns[section.name] = new ARRAYS[section.type](0);
      return;
    }
    const start = view.getUint32(16 + 8 * index, true);
    const length = view.getUint32(20 + 8 * index, true);
    const row = ARRAYS[section.type].BYTES_PER_ELEMENT * section.width;
    if (start + length > bytes.length || length % row !== 0) {
      throw new Error(`セクション ${section.name} の始まりか長さが合わない`);
    }
    columns[section.name] = column(bytes, view, section.type, start, length);
  });
  return new Scene(major, minor, chunks, sections, Object.freeze(columns));
}
