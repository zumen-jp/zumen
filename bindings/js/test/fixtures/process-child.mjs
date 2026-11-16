// 子プロセスとの通信だけを検査する、自作の JSON-RPC 応答器。
import { createInterface } from "node:readline";

const mode = process.argv[2];
let waiting;
const reply = (id, result) => JSON.stringify({ jsonrpc: "2.0", id, result }) + "\n";
const write = (id, result) => process.stdout.write(reply(id, result));
createInterface({ input: process.stdin }).on("line", (line) => {
  const { id, method, params } = JSON.parse(line);
  if (method === "session_new" || method === "push") { write(id, { status: "ok" }); return; }
  if (method === "cancel") {
    if (mode === "hold-reverse" && waiting !== undefined) {
      write(waiting, { status: "failed", response: '{"cancelled":true}' });
      setTimeout(() => write(id, {}), 100);
      waiting = undefined;
      return;
    }
    if (mode === "hold" && waiting !== undefined) {
      process.stdout.write(reply(id, {}) + reply(waiting, { status: "failed", response: '{"cancelled":true}' }));
      waiting = undefined;
    } else write(id, {});
    return;
  }
  if (mode === "exit") process.exit(7);
  if (mode === "truncated") { process.stdout.write('{"jsonrpc":', () => process.exit(0)); return; }
  if (mode === "invalid-utf8") { process.stdout.write(Buffer.from([255, 10])); return; }
  if (mode === "json-error") { process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id, error: { code: -32600 } }) + "\n"); return; }
  if (mode === "wrong-id") { write(id + 1, { status: "ok" }); return; }
  if (mode === "oversized") { process.stdout.write("x".repeat(2048)); return; }
  if (mode === "unknown-status") { write(id, { status: "unknown" }); return; }
  if (mode === "no-response") { write(id, { status: "ok" }); return; }
  if (mode === "bad-base64") { write(id, { status: "ok", base64: "AA=A" }); return; }
  if (mode === "panicked") { write(id, { status: "panicked", response: '{"panic":true}' }); return; }
  if (mode === "hold" || mode === "hold-reverse" || mode === "silent") { waiting = id; return; }
  if (mode === "reject-capabilities") { write(id, { status: "failed", response: '{"diagnostics":[{}]}' }); return; }
  if (mode === "capabilities") {
    const request = JSON.parse(Buffer.from(params.request_base64, "base64"));
    write(id, request.op === "capabilities"
      ? { status: "ok", response: '{"v":1,"result":{"package":"test","abi":1}}' }
      : { status: "failed", response: '{"diagnostics":[{}]}' });
    return;
  }
  if (method === "take") { write(id, { status: "ok", base64: Buffer.alloc(256 * 1024, 165).toString("base64") }); return; }
  const result = { status: "ok", response_base64: params.request_base64 };
  if (mode === "fragmented") {
    const data = Buffer.from(reply(id, result));
    let offset = 0;
    const part = () => {
      if (offset < data.length) { process.stdout.write(data.subarray(offset, ++offset)); setImmediate(part); }
    };
    part();
  } else write(id, result);
});
