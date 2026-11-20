// 通信の検査用の独立した応答器。実装の符号化器・読み手は使わない。
const mode = process.argv[2];
let input = Buffer.alloc(0), stored = Buffer.alloc(0), waiting;
const none = (1n << 64n) - 1n;

function reply(request, status = 0, kind = 0, data = Buffer.alloc(0)) {
  const out = Buffer.alloc(160 + data.length);
  out.write("ZLIR");
  out.writeUInt16LE(1, 4);
  out.writeUInt16LE(request.command, 6);
  out.writeBigUInt64LE(request.id, 8);
  out.writeUInt32LE(136 + data.length, 16);
  out.writeUInt32LE(request.flags, 20);
  out.writeUInt32LE(status, 24);
  out.writeBigUInt64LE(none, 56);
  out.writeUInt32LE(kind, 64);
  out.writeUInt32LE(data.length, 72);
  if (status === 7) out.writeUInt32LE(8, 136);
  data.copy(out, 160);
  return out;
}

async function send(bytes) {
  if (mode === "fragmented" || mode === "selection-fragmented") {
    for (const byte of bytes) {
      process.stdout.write(Buffer.from([byte]));
      await new Promise((resolve) => setImmediate(resolve));
    }
  } else process.stdout.write(bytes);
}

function handle(request, body) {
  if (request.command === 9) { process.exit(0); return; }
  if (request.command === 0) {
    if (mode === "hold" && waiting && waiting.id === request.id) {
      send(reply(waiting, 7));
      waiting = undefined;
    }
    return;
  }
  if (request.command === 1) {
    stored = Buffer.from(body.subarray(80));
    send(reply(request));
    return;
  }
  if (mode === "hold" || mode === "silent") { waiting = request; return; }
  if (mode === "exit") { process.exit(7); return; }
  if (request.command === 14 && (mode === "copy-legacy" || mode === "copy-large")) {
    const count = body.readUInt32LE(72);
    const end = 128 + count * 4;
    if (body.readUInt32LE(76) !== 0 || count > 65536) { process.exit(6); return; }
    if (mode === "copy-legacy") {
      if (body.length > 128 + 65536 * 4) { process.exit(6); return; }
      if (body.length !== end) { send(reply(request, 1)); return; }
    } else if (body.length !== end + 12 || body.readUInt32LE(end) !== 7 ||
               body.toString("utf8", end + 4, end + 11) !== "space/0" || body[end + 11] !== 0) {
      process.exit(6); return;
    }
    const data = Buffer.alloc(4 + count * 4);
    data.writeUInt32LE(count);
    for (let i = 0; i < count; i++) data.writeUInt32LE(i + 1, 4 + i * 4);
    const out = reply(request, 0, 5, data);
    out.writeBigUInt64LE(count ? 1n : 0n, 32);
    out.writeBigUInt64LE(BigInt(count), 48);
    send(out);
    return;
  }
  if (request.command === 10) {
    if (body.length !== 104 || [1, 2, 3, 4].some((value, i) => body.readDoubleLE(72 + i * 8) !== value)) { process.exit(6); return; }
    const data = Buffer.alloc(12);
    data.writeUInt32LE(2, 0);
    data.writeUInt32LE(2, 4);
    data.writeUInt32LE(65, 8);
    if (mode === "selection-duplicate") data.writeUInt32LE(2, 8);
    if (mode === "selection-descending") data.writeUInt32LE(1, 8);
    if (mode === "selection-virtual") data.writeUInt32LE(0x80000000, 8);
    if (mode === "selection-count") data.writeUInt32LE(0xffffffff, 0);
    let payload = mode === "selection-truncated" ? data.subarray(0, 11) : data;
    if (mode === "selection-extra") payload = Buffer.concat([data, Buffer.from([0])]);
    const out = reply(request, 0, 5, payload);
    out.writeBigUInt64LE(body.readBigUInt64LE(56), 32);
    out.writeBigUInt64LE(mode === "selection-result" ? 3n : 2n, 48);
    if (mode === "selection-no-revision") { out.writeBigUInt64LE(none, 32); out.writeBigUInt64LE(none, 40); }
    send(out);
    return;
  }
  if (request.command === 12) {
    // 描く順は昇順に限らない
    const data = Buffer.alloc(16);
    data.writeUInt32LE(3, 0);
    for (const [i, row] of [5, 1, 9].entries()) data.writeUInt32LE(row, 4 + i * 4);
    if (mode === "order-duplicate") data.writeUInt32LE(5, 12);
    if (mode === "order-virtual") data.writeUInt32LE(0x80000000, 8);
    const out = reply(request, 0, 7, mode === "order-truncated" ? data.subarray(0, 14) : data);
    out.writeBigUInt64LE(body.readBigUInt64LE(56), 32);
    out.writeBigUInt64LE(mode === "order-result" ? 2n : 3n, 48);
    send(out);
    return;
  }
  if (request.command === 16) {
    // 線種 2 件: 用紙の mm の列を持つ "DASH"、定義のない "線"（UTF-8 3 バイト、1 バイト埋める）
    const name = Buffer.from("線");
    const data = Buffer.alloc(68);
    data.writeUInt32LE(2, 0);
    for (const [i, word] of [2, 1 | 2, 4, 0, 2].entries()) data.writeUInt32LE(word, 4 + i * 4);
    data.write("DASH", 24);
    data.writeDoubleLE(1.5, 28);
    data.writeDoubleLE(-0.5, 36);
    for (const [i, word] of [9, 0, name.length, 0, 0].entries()) data.writeUInt32LE(word, 44 + i * 4);
    name.copy(data, 64);
    if (mode === "line-types-duplicate") data.writeUInt32LE(2, 44);
    if (mode === "line-types-flags") data.writeUInt32LE(8 | 1, 8);
    if (mode === "line-types-undefined") data.writeUInt32LE(0, 8);
    if (mode === "line-types-padding") data[67] = 1;
    if (mode === "line-types-name") data[64] = 0xff;
    if (mode === "line-types-count") data.writeUInt32LE(0xffffffff, 0);
    if (mode === "line-types-pattern") data.writeUInt32LE(0x10000000, 20);
    let payload = mode === "line-types-truncated" ? data.subarray(0, 60) : data;
    if (mode === "line-types-extra") payload = Buffer.concat([data, Buffer.alloc(4)]);
    const out = reply(request, 0, 8, payload);
    out.writeBigUInt64LE(body.readBigUInt64LE(56), 32);
    out.writeBigUInt64LE(0n, 40);
    out.writeBigUInt64LE(mode === "line-types-result" ? 3n : 2n, 48);
    send(out);
    return;
  }
  if (request.command === 19) {
    // 記号の定義 2 件（文書には 3 件）。どちらも行がなく、表示は空の全量のパケット
    const packet = Buffer.alloc(16 + 13 * 16);
    packet.writeUInt32LE(0x50444c5a, 0);
    packet.writeUInt16LE(1, 4);
    packet.writeUInt16LE(mode === "symbols-delta" ? 0 : 1, 6);
    packet.writeUInt32LE(packet.length, 8);
    packet.writeUInt32LE(13, 12);
    const widths = [8, 1, 2, 7, 1, 2, 8, 1, 2, 9, 3, 1, 2];
    for (const [id, width] of widths.entries()) {
      packet.writeUInt32LE(id, 16 + id * 16);
      packet.writeUInt32LE(width, 20 + id * 16);
    }
    const data = Buffer.concat([Buffer.alloc(24), packet]);
    data.writeUInt32LE(3, 0);
    data.writeUInt32LE(2, 4);
    if (mode === "symbols-first") data.writeUInt32LE(1, 16);
    if (mode === "symbols-count") data.writeUInt32LE(0xffffffff, 4);
    let payload = mode === "symbols-truncated" ? data.subarray(0, 20) : data;
    if (mode === "symbols-extra") payload = Buffer.concat([data, Buffer.alloc(4)]);
    const out = reply(request, 0, 10, payload);
    out.writeBigUInt64LE(body.readBigUInt64LE(56), 32);
    out.writeBigUInt64LE(0n, 40);
    out.writeBigUInt64LE(mode === "symbols-result" ? 3n : 2n, 48);
    send(out);
    return;
  }
  if (request.command === 17) {
    // 文字 3 件: 文字でない行、"AB"（2 バイト埋める）、書式つきの "線"（UTF-8 3 バイト、1 バイト埋める）
    const name = Buffer.from("線");
    const data = Buffer.alloc(4 + 8 + 8 + 4 + 8 + 4);
    data.writeUInt32LE(3, 0);
    for (const [i, word] of [0, 0, 1, 2].entries()) data.writeUInt32LE(word, 4 + i * 4);
    data.write("AB", 20);
    for (const [i, word] of [2, name.length].entries()) data.writeUInt32LE(word, 24 + i * 4);
    name.copy(data, 32);
    if (mode === "texts-kind") data.writeUInt32LE(3, 12);
    if (mode === "texts-empty") data.writeUInt32LE(1, 8);
    if (mode === "texts-padding") data[35] = 1;
    if (mode === "texts-content") data[32] = 0xff;
    if (mode === "texts-count") data.writeUInt32LE(0xffffffff, 0);
    if (mode === "texts-length") data.writeUInt32LE(0x10000000, 16);
    let payload = mode === "texts-truncated" ? data.subarray(0, 30) : data;
    if (mode === "texts-extra") payload = Buffer.concat([data, Buffer.alloc(4)]);
    const out = reply(request, 0, 9, payload);
    out.writeBigUInt64LE(body.readBigUInt64LE(56), 32);
    out.writeBigUInt64LE(0n, 40);
    out.writeBigUInt64LE(mode === "texts-result" ? 2n : 3n, 48);
    send(out);
    return;
  }
  if (request.command === 11) {
    // 画層 2 件: 状態と RGB を持つ "0"、色番号だけの "壁"（UTF-8 3 バイト、1 バイト埋める）
    const name = Buffer.from("壁");
    const data = Buffer.alloc(4 + 24 + 24);
    data.writeUInt32LE(2, 0);
    for (const [i, word] of [7, 1 | 2 | 8 | 32 | 128, 0x123456, 0, 1].entries()) data.writeUInt32LE(word, 4 + i * 4);
    data.write("0", 24);
    for (const [i, word] of [3, 64, 0, (3 << 24) | 5, name.length].entries()) data.writeUInt32LE(word, 28 + i * 4);
    name.copy(data, 48);
    if (mode === "layers-duplicate") data.writeUInt32LE(7, 28);
    if (mode === "layers-flags") data.writeUInt32LE(256, 32);
    if (mode === "layers-state") data.writeUInt32LE(2, 32);
    if (mode === "layers-rgb") data.writeUInt32LE(0x1000000, 12);
    if (mode === "layers-system") data.writeUInt32LE(4 << 24, 40);
    if (mode === "layers-padding") data[51] = 1;
    if (mode === "layers-name") data[48] = 0xff;
    if (mode === "layers-count") data.writeUInt32LE(0xffffffff, 0);
    let payload = mode === "layers-truncated" ? data.subarray(0, 50) : data;
    if (mode === "layers-extra") payload = Buffer.concat([data, Buffer.alloc(4)]);
    const out = reply(request, 0, 6, payload);
    out.writeBigUInt64LE(body.readBigUInt64LE(56), 32);
    out.writeBigUInt64LE(0n, 40);
    out.writeBigUInt64LE(mode === "layers-result" ? 3n : 2n, 48);
    send(out);
    return;
  }
  let out = reply(request, 0, 2, stored);
  if (mode === "wrong-id") out.writeBigUInt64LE(request.id + 1n, 8);
  if (mode === "wrong-magic") out[0] = 0;
  if (mode === "wrong-flags") out.writeUInt32LE(1, 20);
  if (mode === "wrong-command") out.writeUInt16LE(4, 6);
  if (mode === "short-prefix") { out.writeUInt32LE(135, 16); out = out.subarray(0, 24); }
  if (mode === "oversized") { out.writeUInt32LE(0xffffffff, 16); out = out.subarray(0, 24); }
  if (mode === "unknown-status") out.writeUInt32LE(999, 24);
  if (mode === "reserved") out.writeUInt32LE(1, 156);
  if (mode === "bad-length") out.writeUInt32LE(stored.length + 1, 72);
  if (mode === "unexpected-display") out.writeUInt32LE(1, 64);
  if (mode === "unexpected-stop") out.writeUInt32LE(8, 136);
  if (mode === "truncated") { process.stdout.write(out.subarray(0, 30), () => process.exit(0)); return; }
  send(out);
}

process.stdin.on("data", (bytes) => {
  input = Buffer.concat([input, bytes]);
  while (input.length >= 24) {
    const length = input.readUInt32LE(16);
    if (input.length < 24 + length) return;
    const request = { id: input.readBigUInt64LE(8), command: input.readUInt16LE(6), flags: input.readUInt32LE(20) };
    const body = input.subarray(24, 24 + length);
    input = input.subarray(24 + length);
    handle(request, body);
  }
});
