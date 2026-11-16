// Node.js の子プロセスの入口を、正常な応答と通信の失敗で検査する。
// node bindings/js/test/process.mjs <zumen の実行ファイル>
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { ProcessError, ProcessSession, ZumenError, openProcess } from "../src/node.js";
import { runConformance } from "./run.mjs";

const executable = process.argv[2];
const fixture = fileURLToPath(new URL("./fixtures/process-child.mjs", import.meta.url));
async function fake(mode, limits = {}) {
  const child = spawn(process.execPath, [fixture, mode], { stdio: ["pipe", "pipe", "ignore"], windowsHide: true });
  const settings = { timeoutMs: 2000, maxMessageBytes: 1024 * 1024, ...limits };
  const timeout = settings.timeoutMs;
  // 短い応答期限のテストに、OS がプロセスを起動する時間を含めない。
  settings.timeoutMs = 10_000;
  const session = await ProcessSession.create(child, undefined, settings);
  settings.timeoutMs = timeout;
  return { child, session };
}
const status = (value) => (error) => error instanceof ZumenError && error.status === value;
const transport = (value) => (error) => error instanceof ProcessError && error.code === value;
const ended = (child) => assert(child.exitCode !== null || child.signalCode !== null);

test("文字列・別のビューの範囲・任意バイト列と大きい blob を保つ", async () => {
  const { child, session } = await fake("echo");
  try {
    const bytes = Uint8Array.of(0, 255, 1, 254);
    assert.deepEqual(await session.callBytes(bytes.subarray(1, 3)), bytes.slice(1, 3));
    const small = await session.callBytes(bytes);
    assert.equal(small.byteOffset, 0);
    assert.equal(small.buffer.byteLength, small.byteLength);
    assert.deepEqual(await session.call({ text: "日本語 😀" }), { text: "日本語 😀" });
    const blob = await session.take(0);
    assert.equal(blob.length, 256 * 1024);
    assert(blob.every((byte) => byte === 165));
    await session.cancel();
  } finally { await session.close(); }
  ended(child);
  await session.close();
  assert(session.closed);
  await assert.rejects(session.callBytes("{}"), status("invalid"));
});

test("1 バイトずつ届く応答を読み、送信上限の後も再利用できる", async () => {
  const { child, session } = await fake("fragmented", { maxMessageBytes: 1024 });
  try {
    await assert.rejects(session.push("input", new Uint8Array(1024)), transport("limit"));
    assert(!session.broken);
    assert.deepEqual(await session.call({ text: "日本語 😀" }), { text: "日本語 😀" });
  } finally { await session.close(); }
  ended(child);
});

test("処理中の重複を busy にし、中断の応答と call を混同しない", async () => {
  const { child, session } = await fake("hold");
  try {
    const result = session.callBytes("{}");
    const rejected = assert.rejects(result, (error) => status("failed")(error) && error.response.cancelled);
    await assert.rejects(session.push("input", "x"), status("busy"));
    await assert.rejects(session.take(0), status("busy"));
    await Promise.all([session.cancel(), session.cancel()]);
    await rejected;
    assert(!session.broken);
  } finally { await session.close(); }
  ended(child);
});

test("call が先に終わっても、中断の応答を次の呼び出しへ持ち越さない", async () => {
  const { child, session } = await fake("hold-reverse");
  try {
    const rejected = assert.rejects(session.callBytes("{}"), status("failed"));
    const cancelled = session.cancel();
    await rejected;
    await assert.rejects(session.push("input", "x"), status("busy"));
    await cancelled;
    await session.push("input", "x");
  } finally { await session.close(); }
  ended(child);
});

for (const [mode, code, method] of [
  ["exit", "io", "callBytes"], ["truncated", "io", "callBytes"],
  ["invalid-utf8", "protocol", "callBytes"], ["json-error", "protocol", "callBytes"],
  ["wrong-id", "protocol", "callBytes"], ["unknown-status", "protocol", "callBytes"],
  ["no-response", "protocol", "callBytes"], ["bad-base64", "protocol", "take"],
  ["oversized", "limit", "callBytes"],
]) {
  test(`${mode} で待っている要求を失敗にし、壊れたプロセスを捨てる`, async () => {
    const { child, session } = await fake(mode, { maxMessageBytes: 1024 });
    try {
      await assert.rejects(session[method](method === "take" ? 0 : "{}"), transport(code));
      assert(session.broken);
      await assert.rejects(session.callBytes("{}"), transport(code));
      await session.cancel();
    } finally { await session.close(); }
    ended(child);
  });
}

test("応答の期限と、中断に応じない子プロセスを止める", async () => {
  for (const cancel of [false, true]) {
    const { child, session } = await fake("silent", { timeoutMs: cancel ? 5000 : 200 });
    try {
      const result = session.callBytes("{}");
      const rejected = assert.rejects(result, transport("timeout"));
      if (cancel) await session.cancel();
      await rejected;
      assert(session.broken);
    } finally { await session.close(); }
    ended(child);
  }
});

test("処理中に close しても Promise を残さず、終了まで待つ", async () => {
  const { child, session } = await fake("hold");
  const rejected = assert.rejects(session.callBytes("{}"), transport("closed"));
  await session.close();
  await rejected;
  ended(child);
});

test("コアの panicked の応答と後続の状態を保ち、後片付けできる", async () => {
  const { child, session } = await fake("panicked");
  try {
    await assert.rejects(session.callBytes("{}"), (error) => status("panicked")(error) && error.response.panic);
    assert(session.broken);
    await assert.rejects(session.take(0), status("panicked"));
    await session.cancel();
  } finally { await session.close(); }
  ended(child);
});

test("起動失敗と通信上限の不正値を返す", async () => {
  await assert.rejects(openProcess(fileURLToPath(new URL("./fixtures/no-such-engine", import.meta.url))), transport("spawn"));
  await assert.rejects(openProcess(process.execPath, undefined, { timeoutMs: 0 }), RangeError);
  await assert.rejects(openProcess(process.execPath, undefined, { maxMessageBytes: 1 }), RangeError);
});

test("適合検査の追加の確認で失敗しても、開いた子プロセスを閉じる", async () => {
  for (const failAt of [1, 2]) {
    const opened = [];
    try {
      await assert.rejects(runConformance({
        zumen: { open: async () => {
          const value = await fake(opened.length + 1 === failAt ? "reject-capabilities" : "capabilities");
          opened.push(value);
          return value.session;
        } },
        cases: [], readSample: () => undefined, sha256: () => "",
      }), status("failed"));
      assert.equal(opened.length, failAt);
      for (const { child, session } of opened) { assert(session.closed); ended(child); }
    } finally {
      for (const { session } of opened) await session.close();
    }
  }
});

test("ネイティブの 2 セッションの入力と寿命を分ける", { skip: !executable }, async () => {
  const a = await openProcess(executable);
  let b;
  try {
    b = await openProcess(executable);
    await a.push("input.dxf", "0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n");
    const response = await a.call({ v: 1, op: "open", input: "input.dxf" });
    assert.equal(response.result.format, "dxf");
    await assert.rejects(b.call({ v: 1, op: "open", input: "input.dxf" }), (error) => status("failed")(error) && error.response.diagnostics.length > 0);
    await a.close();
    assert.equal((await b.call({ v: 1, op: "capabilities" })).result.abi, 1);
  } finally { await a.close(); await b?.close(); }
});
