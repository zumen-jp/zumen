// 常駐文書 IPC v1。整数と表示の値は little endian のビット列を保つ。
import { ProcessError } from "./process.js";

export const NONE = (1n << 64n) - 1n;
export const HEADER = 24;
export const PREFIX = 136;
export const MAX_PACKET = 1 << 30;
export const KINDS = ["inputBytes", "decodedBytes", "records", "elements", "depth", "segments", "work"];
const DEFAULTS = [2147483648n, 2147483648n, 100000000n, 100000000n, 256n, 100000000n, 10000000000n];
const STATUSES = ["ok", "invalid", "unsupported", "stale", "missing", "rejected", "exceeded", "cancelled", "timedOut", "noDocument", "alreadyOpen", "noDiagnostics"];
const WIDTHS = [8, 1, 2, 7, 1, 2, 8, 1, 2, 9, 3, 1, 2];
const decoder = new TextDecoder("utf-8", { fatal: true });

export function invalid() { return new ProcessError("protocol", "文書 IPC の応答が契約と違う"); }

export function u64(value, name) {
  if ((typeof value !== "bigint" && !Number.isSafeInteger(value)) || value < 0 || value >= NONE) {
    throw new RangeError(`${name} は 0 以上、u64 の最大値未満の整数にする`);
  }
  return BigInt(value);
}

export function frame(command, id, flags, length) {
  const bytes = Buffer.alloc(HEADER + length);
  bytes.write("ZLIP", 0, "ascii");
  bytes.writeUInt16LE(1, 4);
  bytes.writeUInt16LE(command, 6);
  bytes.writeBigUInt64LE(id, 8);
  bytes.writeUInt32LE(length, 16);
  bytes.writeUInt32LE(flags, 20);
  return bytes;
}

export function control(bytes, budget, revision, ack) {
  if (budget === null || typeof budget !== "object" || Array.isArray(budget)) throw new TypeError("budget は単位ごとの上限にする");
  if (Object.keys(budget).some((key) => !KINDS.includes(key))) throw new TypeError("budget に未知の単位がある");
  for (let i = 0; i < KINDS.length; i++) {
    const value = budget[KINDS[i]];
    bytes.writeBigUInt64LE(value === undefined ? DEFAULTS[i] : value === null ? NONE : u64(value, KINDS[i]), HEADER + i * 8);
  }
  bytes.writeBigUInt64LE(revision ?? NONE, HEADER + 56);
  bytes.writeBigUInt64LE(ack ?? NONE, HEADER + 64);
}

export function responseLength(header, pending, maximum) {
  if (header.readUInt32LE(0) !== 0x52494c5a || header.readUInt16LE(4) !== 1
    || header.readUInt16LE(6) !== pending.command || header.readBigUInt64LE(8) !== pending.id
    || header.readUInt32LE(20) !== pending.flags) throw invalid();
  const length = header.readUInt32LE(16);
  if (length < PREFIX) throw invalid();
  if (length > maximum) throw new ProcessError("limit", "応答が maxMessageBytes を超える");
  return length;
}

// 塊ごとの配列は作らない。全体を検証した後で、必要な塊だけを反復する。
function column(bytes, id, rows, stride, begin, end) {
  return Object.freeze({
    id, rows, stride,
    *blocks() {
      for (let position = begin; position < end;) {
        const start = bytes.readUInt32LE(position);
        const count = bytes.readUInt32LE(position + 4);
        position += 8;
        const next = position + count * stride;
        yield { start, rows: count, bytes: bytes.subarray(position, next) };
        position = next;
      }
    },
  });
}

export function displayPacket(bytes) {
  if (bytes.length < 16 || bytes.length > MAX_PACKET || bytes.readUInt32LE(0) !== 0x50444c5a
    || bytes.readUInt16LE(4) !== 1 || bytes.readUInt16LE(6) > 1
    || bytes.readUInt32LE(8) !== bytes.length || bytes.readUInt32LE(12) !== 13) throw invalid();
  const full = bytes.readUInt16LE(6) === 1;
  const columns = [];
  let position = 16, total = 0, previous = 0;
  for (let id = 0; id < 13; id++) {
    if (position + 16 > bytes.length || bytes.readUInt32LE(position) !== id
      || bytes.readUInt32LE(position + 4) !== WIDTHS[id]) throw invalid();
    const rows = bytes.readUInt32LE(position + 8), count = bytes.readUInt32LE(position + 12);
    const stride = WIDTHS[id] * 4, maximum = Math.ceil(rows / 64);
    if ([0, 3, 6, 9].includes(id)) previous = rows;
    total += rows * stride;
    if (rows !== previous || total > MAX_PACKET || count > maximum || (full && count !== maximum)) throw invalid();
    position += 16;
    const begin = position;
    let end = 0;
    for (let block = 0; block < count; block++) {
      if (position + 8 > bytes.length) throw invalid();
      const first = bytes.readUInt32LE(position), size = bytes.readUInt32LE(position + 4);
      if (first >= rows || first % 64 !== 0 || first < end || (full && first !== end)
        || size !== Math.min(rows - first, 64)) throw invalid();
      position += 8 + size * stride;
      if (position > bytes.length) throw invalid();
      end = first + size;
    }
    columns.push(column(bytes, id, rows, stride, begin, position));
  }
  if (position !== bytes.length) throw invalid();
  return Object.freeze({ full, columns: Object.freeze(columns) });
}

const SYSTEMS = [null, "aci", "sxf", "jww"];

// 画層の表: 件数と、値・フラグ・RGB・色番号・名前の長さ（各 u32）と 4 バイト境界まで 0 で埋めた UTF-8 の名前
function layerTable(data, reply) {
  if (data.length < 4 || reply.revision === null) throw invalid();
  const count = data.readUInt32LE(0);
  if (reply.result !== BigInt(count) || count > (data.length - 4) / 20) throw invalid();
  const layers = [], values = new Set();
  let position = 4;
  for (let i = 0; i < count; i++) {
    if (position + 20 > data.length) throw invalid();
    const value = data.readUInt32LE(position), flags = data.readUInt32LE(position + 4);
    const rgb = data.readUInt32LE(position + 8), code = data.readUInt32LE(position + 12), length = data.readUInt32LE(position + 16);
    const end = position + 20 + length, padded = position + 20 + Math.ceil(length / 4) * 4;
    const state = (flags & 1) !== 0, hasRgb = (flags & 32) !== 0, hasCode = (flags & 64) !== 0;
    if (values.has(value) || flags > 0xff || (!state && (flags & 30) !== 0) || rgb > 0xffffff || (!hasRgb && rgb !== 0)
      || (hasCode ? !SYSTEMS[code >>> 24] : code !== 0) || padded > data.length
      || data.subarray(end, padded).some((byte) => byte !== 0)) throw invalid();
    let name;
    try { name = decoder.decode(data.subarray(position + 20, end)); } catch { throw invalid(); }
    values.add(value);
    layers.push(Object.freeze({
      value, name,
      state: state ? Object.freeze({ visible: (flags & 2) !== 0, printable: (flags & 4) !== 0, editable: (flags & 8) !== 0, frozen: (flags & 16) !== 0 }) : null,
      rgb: hasRgb ? Object.freeze([rgb >>> 16, (rgb >>> 8) & 0xff, rgb & 0xff]) : null,
      code: hasCode ? Object.freeze({ system: SYSTEMS[code >>> 24], value: code & 0xffffff }) : null,
      followsPlacement: (flags & 128) !== 0,
    }));
    position = padded;
  }
  if (position !== data.length) throw invalid();
  return Object.freeze(layers);
}

// 線種の表: 件数と、値・フラグ・名前の長さ・説明の長さ・列の数（各 u32）、4 バイト境界まで 0 で埋めた UTF-8 の
// 名前と説明、f64 の列。フラグは bit 0 が定義あり、bit 1 が用紙の mm の列、bit 2 が添え物あり
function lineTypeTable(data, reply) {
  if (data.length < 4 || reply.revision === null) throw invalid();
  const count = data.readUInt32LE(0);
  if (reply.result !== BigInt(count) || count > (data.length - 4) / 20) throw invalid();
  const lineTypes = [], values = new Set();
  let position = 4;
  const text = (length) => {
    const end = position + length, padded = position + Math.ceil(length / 4) * 4;
    if (padded > data.length || data.subarray(end, padded).some((byte) => byte !== 0)) throw invalid();
    let found;
    try { found = decoder.decode(data.subarray(position, end)); } catch { throw invalid(); }
    position = padded;
    return found;
  };
  for (let i = 0; i < count; i++) {
    if (position + 20 > data.length) throw invalid();
    const value = data.readUInt32LE(position), flags = data.readUInt32LE(position + 4);
    const nameLength = data.readUInt32LE(position + 8), descriptionLength = data.readUInt32LE(position + 12);
    const patternLength = data.readUInt32LE(position + 16);
    const defined = (flags & 1) !== 0;
    if (values.has(value) || flags > 7 || (!defined && (flags !== 0 || descriptionLength !== 0 || patternLength !== 0))) throw invalid();
    position += 20;
    const name = text(nameLength), description = text(descriptionLength);
    if (patternLength > (data.length - position) / 8) throw invalid();
    const pattern = [];
    for (let j = 0; j < patternLength; j++) pattern.push(data.readDoubleLE(position + j * 8));
    position += patternLength * 8;
    values.add(value);
    lineTypes.push(Object.freeze({
      value, name,
      definition: defined ? Object.freeze({
        description, pattern: Object.freeze(pattern), basis: (flags & 2) !== 0 ? "paper" : "element", marks: (flags & 4) !== 0,
      }) : null,
    }));
  }
  if (position !== data.length) throw invalid();
  return Object.freeze(lineTypes);
}

// 行の文字: 件数と、種類・内容のバイト数（各 u32）、4 バイト境界まで 0 で埋めた UTF-8 の内容。
// 種類は 0 が文字でない行（内容は空）、1 が文字、2 が書式つきの文字
const TEXT_KINDS = [null, "text", "formatted"];
function textList(data, reply) {
  if (data.length < 4 || reply.revision === null) throw invalid();
  const count = data.readUInt32LE(0);
  if (reply.result !== BigInt(count) || count > (data.length - 4) / 8) throw invalid();
  const texts = [];
  let position = 4;
  for (let i = 0; i < count; i++) {
    if (position + 8 > data.length) throw invalid();
    const kind = data.readUInt32LE(position), length = data.readUInt32LE(position + 4);
    position += 8;
    const end = position + length, padded = position + Math.ceil(length / 4) * 4;
    if (kind >= TEXT_KINDS.length || (kind === 0 && length !== 0) || padded > data.length
      || data.subarray(end, padded).some((byte) => byte !== 0)) throw invalid();
    let content;
    try { content = decoder.decode(data.subarray(position, end)); } catch { throw invalid(); }
    position = padded;
    texts.push(Object.freeze({ kind: TEXT_KINDS[kind], content }));
  }
  if (position !== data.length) throw invalid();
  return Object.freeze(texts);
}

// 記号の定義: 文書の定義の数、要求した定義の数、定義ごとの先頭の行と行の数（各 u32）、全量の表示のパケット。
// 定義の行は要求の順に隙間なく並ぶ
function symbolList(data, reply) {
  if (data.length < 8 || reply.revision === null) throw invalid();
  const total = data.readUInt32LE(0), count = data.readUInt32LE(4);
  if (reply.result !== BigInt(count) || count > (data.length - 8) / 8) throw invalid();
  const definitions = [];
  let next = 0;
  for (let i = 0; i < count; i++) {
    const first = data.readUInt32LE(8 + i * 8), rows = data.readUInt32LE(12 + i * 8);
    if (first !== next || rows > 0xffffffff - first) throw invalid();
    next = first + rows;
    definitions.push(Object.freeze({ first, rows }));
  }
  const display = displayPacket(data.subarray(8 + count * 8));
  if (!display.full) throw invalid();
  return Object.freeze({ total, definitions: Object.freeze(definitions), display });
}

export function response(bytes, pending) {
  if (bytes.length < PREFIX) throw invalid();
  const statusCode = bytes.readUInt32LE(0), displayCode = bytes.readUInt32LE(4);
  const kind = bytes.readUInt32LE(40), length = bytes.readUInt32LE(48);
  if (statusCode >= STATUSES.length || ![0, 1, 6, 7, 8].includes(displayCode)
    || kind > 10 || length !== bytes.length - PREFIX || bytes.readUInt32LE(52) !== 0 || bytes.readUInt32LE(132) !== 0) throw invalid();
  const value = (offset) => { const n = bytes.readBigUInt64LE(offset); return n === NONE ? null : n; };
  const reply = {
    requestId: pending.id, status: STATUSES[statusCode], displayStatus: STATUSES[displayCode],
    revision: value(8), state: value(16), result: bytes.readBigUInt64LE(24), base: value(32),
    diagnosticCount: bytes.readUInt32LE(44), used: {}, stop: null, data: bytes.subarray(PREFIX),
    display: null, target: null, selection: null, layers: null, order: null, lineTypes: null, texts: null, symbols: null,
  };
  for (let i = 0; i < 7; i++) reply.used[KINDS[i]] = bytes.readBigUInt64LE(56 + i * 8);
  const stopKind = bytes.readUInt32LE(112), limit = bytes.readBigUInt64LE(116), requested = bytes.readBigUInt64LE(124);
  const stopped = [6, 7, 8].includes(statusCode) ? statusCode : [6, 7, 8].includes(displayCode) ? displayCode : 0;
  if (stopped === 6 ? stopKind < 1 || stopKind > 7 : stopKind !== (stopped === 7 ? 8 : stopped === 8 ? 9 : 0)) throw invalid();
  if (stopKind === 0 || stopKind >= 8) { if (limit !== 0n || requested !== 0n) throw invalid(); }
  if (stopKind >= 1 && stopKind <= 7) reply.stop = { kind: KINDS[stopKind - 1], limit, requested };
  else if (stopKind > 7) reply.stop = { kind: stopKind === 8 ? "cancelled" : "timedOut" };
  const wantsDisplay = (pending.flags & 1) !== 0 || pending.command === 2;
  const expected = statusCode !== 0 || displayCode !== 0 ? 0 : wantsDisplay ? 1
    : pending.command === 6 ? 2 : pending.command === 7 ? 3 : pending.command === 8 ? 4 : [10, 14, 20, 21, 22, 25].includes(pending.command) ? 5 : pending.command === 11 ? 6 : pending.command === 12 ? 7 : pending.command === 16 ? 8 : pending.command === 17 ? 9 : pending.command === 19 ? 10 : 0;
  if (kind !== expected || (kind === 0 && length !== 0) || ((statusCode !== 0 || !wantsDisplay) && displayCode !== 0)
    || (kind !== 1 && reply.base !== null) || (reply.revision === null) !== (reply.state === null)) throw invalid();
  if (kind === 1) {
    reply.display = displayPacket(reply.data);
    if (reply.display.full !== (reply.base === null) || reply.revision === null
      || (reply.base !== null && reply.base > reply.revision)) throw invalid();
  } else if (kind === 3) {
    if (length < 20 || reply.data.readUInt32LE(0) !== length - 20) throw invalid();
    reply.target = { id: decoder.decode(reply.data.subarray(20)), hash: `v2:${reply.data.subarray(4, 20).toString("hex")}` };
  } else if (kind === 5) {
    if (length < 4 || reply.revision === null) throw invalid();
    const count = reply.data.readUInt32LE(0);
    if (length !== 4 + count * 4 || reply.result !== BigInt(count)) throw invalid();
    let previous = -1;
    for (let i = 0; i < count; i++) {
      const row = reply.data.readUInt32LE(4 + i * 4);
      if (row >= 0x80000000 || row <= previous) throw invalid();
      previous = row;
    }
    // 全件を検査してから借用する。配列の複製とホストのエンディアンへの依存を避ける。
    const data = reply.data;
    reply.selection = Object.freeze({
      length: count,
      *rows() { for (let i = 0; i < count; i++) yield data.readUInt32LE(4 + i * 4); },
    });
  } else if (kind === 6) {
    reply.layers = layerTable(reply.data, reply);
  } else if (kind === 8) {
    reply.lineTypes = lineTypeTable(reply.data, reply);
  } else if (kind === 9) {
    reply.texts = textList(reply.data, reply);
  } else if (kind === 10) {
    reply.symbols = symbolList(reply.data, reply);
  } else if (kind === 7) {
    if (length < 4 || reply.revision === null) throw invalid();
    const count = reply.data.readUInt32LE(0);
    if (length !== 4 + count * 4 || reply.result !== BigInt(count)) throw invalid();
    // 描く順は昇順でないので、重複は集合で確かめる
    const seen = new Set();
    for (let i = 0; i < count; i++) {
      const row = reply.data.readUInt32LE(4 + i * 4);
      if (row >= 0x80000000 || seen.has(row)) throw invalid();
      seen.add(row);
    }
    const data = reply.data;
    reply.order = Object.freeze({
      length: count,
      *rows() { for (let i = 0; i < count; i++) yield data.readUInt32LE(4 + i * 4); },
    });
  }
  return reply;
}
