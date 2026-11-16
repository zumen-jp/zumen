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
