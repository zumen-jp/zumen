// 適合試験の組を、言語に依らない形（CLI・HTTP・WASI）で流し、hashes.txt と照らす（#403、決定 0006・0013・0200）。
//
// 使い方:
//   node bindings/conformance/cli.mjs stdio <zumen の実行ファイル>
//   node bindings/conformance/cli.mjs http <zumen の実行ファイル>
//   node bindings/conformance/cli.mjs call <zumen の実行ファイル>
//   node bindings/conformance/cli.mjs wasi <wasm32-wasip1 の zumen.wasm>
//
// 階層 3 の言語（Ruby・PHP・R・Julia など）は、自分の言語から子プロセスか HTTP で CLI を使う。ここでは、その言語の
// 代わりに Node.js から同じやり取りをする。
//
// - stdio: `zumen serve --stdio` の JSON-RPC。すべての組を流す
// - http: `zumen serve --http` の JSON-RPC と、本文のままの push・take。すべての組を流す
// - wasi: WASI のランタイム（Node.js の node:wasi、wasi.mjs）の上の `zumen serve --stdio`。すべての組を流す
// - call: `zumen call`。1 回の呼び出しが 1 つのセッションなので、セッションの最初の call と、その直後の take だけを
//   流し、その行が hashes.txt の行と一致することを確かめる

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { request as httpRequest } from "node:http";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

import { ACTIONS, bytesOf, checkResponse, loadCases, pushOf, requestOf } from "./cases.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const STATUS = ["ok", "failed", "invalid", "busy", "panicked"];
const decoder = new TextDecoder();

/** UTF-8 として読めれば文字列、読めなければ undefined */
function utf8(bytes) {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return undefined;
  }
}

const sha256 = (data) => createHash("sha256").update(data).digest("hex");
const base64 = (data) => Buffer.from(data).toString("base64");

/** 行ごとの JSON-RPC を話す子プロセス */
class LineRpc {
  #child;
  #next = 1;
  #pending = new Map();

  constructor(command, args) {
    this.#child = spawn(command, args, { stdio: ["pipe", "pipe", "inherit"] });
    createInterface({ input: this.#child.stdout }).on("line", (line) => {
      const message = JSON.parse(line);
      const pending = this.#pending.get(message.id);
      if (pending !== undefined) {
        this.#pending.delete(message.id);
        pending(message);
      }
    });
  }

  request(method, params) {
    const id = this.#next++;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, (message) =>
        message.error ? reject(new Error(`${method} が誤りを返した: ${JSON.stringify(message.error)}`)) : resolve(message.result),
      );
      this.#child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    });
  }

  close() {
    this.#child.stdin.end();
    this.#child.kill();
  }
}

/** `serve --stdio` の JSON-RPC で、1 つのセッションを流す */
function stdioEngine(rpc) {
  return {
    async session(options) {
      return (await rpc.request("session_new", { options_base64: base64(options) })).status;
    },
    async push(name, data) {
      return (await rpc.request("push", { name_base64: base64(name), base64: base64(data) })).status;
    },
    async call(request) {
      const result = await rpc.request("call", { request_base64: base64(request) });
      return [result.status, result.response === undefined ? null : new TextEncoder().encode(result.response)];
    },
    async take(blob) {
      const result = await rpc.request("take", { blob });
      return [result.status, result.base64 === undefined ? null : new Uint8Array(Buffer.from(result.base64, "base64"))];
    },
    async close() {},
  };
}

/** `serve --http` を起こし、待ち受けたアドレスを読む */
async function httpServer(zumen) {
  const child = spawn(zumen, ["serve", "--http", "--listen", "127.0.0.1:0"], { stdio: ["ignore", "pipe", "inherit"] });
  const line = await new Promise((resolve) => createInterface({ input: child.stdout }).once("line", resolve));
  const url = new URL(line.trim());
  const send = (method, path, type, body) =>
    new Promise((resolve, reject) => {
      const req = httpRequest(
        { host: url.hostname, port: url.port, method, path, headers: { "Content-Type": type, "Content-Length": body.length } },
        (res) => {
          const chunks = [];
          res.on("data", (chunk) => chunks.push(chunk));
          res.on("end", () => resolve([res.statusCode, res.headers["content-type"] ?? "", new Uint8Array(Buffer.concat(chunks))]));
        },
      );
      req.on("error", reject);
      req.end(Buffer.from(body));
    });
  const rpc = async (method, params) => {
    const body = new TextEncoder().encode(JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }));
    const [code, , reply] = await send("POST", "/", "application/json", body);
    const message = JSON.parse(decoder.decode(reply));
    if (code !== 200 || message.error) {
      throw new Error(`${method} が誤りを返した: ${code} ${JSON.stringify(message.error)}`);
    }
    return message.result;
  };
  return { child, send, rpc };
}

/** `%XX` にする。`/` と英数字などはそのまま */
function percent(bytes) {
  return [...bytes]
    .map((byte) => (/[A-Za-z0-9\-._~/]/.test(String.fromCharCode(byte)) ? String.fromCharCode(byte) : `%${byte.toString(16).toUpperCase().padStart(2, "0")}`))
    .join("");
}

/** `serve --http` で、1 つのセッションを流す */
function httpEngine(server) {
  let session = null;
  const close = async () => {
    if (session !== null) {
      await server.rpc("session_close", { session });
      session = null;
    }
  };
  return {
    async session(options) {
      await close();
      const result = await server.rpc("session_new", { options_base64: base64(options) });
      session = result.session ?? null;
      return result.status;
    },
    async push(name, data) {
      if (session === null) {
        return "invalid";
      }
      const [, , reply] = await server.send("POST", `/sessions/${session}/inputs/${percent(name)}`, "application/octet-stream", data);
      return JSON.parse(decoder.decode(reply)).status;
    },
    async call(request) {
      if (session === null) {
        return ["invalid", null];
      }
      const result = await server.rpc("call", { session, request_base64: base64(request) });
      return [result.status, result.response === undefined ? null : new TextEncoder().encode(result.response)];
    },
    async take(blob) {
      if (session === null) {
        return ["invalid", null];
      }
      const [code, type, reply] = await server.send("GET", `/sessions/${session}/blobs/${blob}`, "application/json", new Uint8Array());
      if (code === 200 && type === "application/octet-stream") {
        return ["ok", reply];
      }
      return [JSON.parse(decoder.decode(reply)).status, null];
    },
    close,
  };
}

/** 組を流し、ハッシュの行を返す。`engine` は、組ごとに作る */
async function runCases(cases, makeEngine, failures) {
  const hashes = [];
  for (const { name, steps } of cases) {
    const engine = makeEngine();
    let started = false;
    for (const [index, step] of steps.entries()) {
      const at = `${name} の手順 ${index}`;
      const action = ACTIONS.find((key) => Object.hasOwn(step, key));
      const expected = step.status ?? "ok";
      const input = step[action];
      if (action !== "session" && !started) {
        await engine.session(new Uint8Array());
      }
      started = true;
      let status;
      let output = null;
      if (action === "session") {
        status = await engine.session(bytesOf(input, "options", "options_text"));
      } else if (action === "push") {
        const [pushName, data] = pushOf(input);
        status = await engine.push(pushName, data);
      } else if (action === "take") {
        const [taken, data] = await engine.take(input);
        status = taken;
        output = data && ["blob", data];
      } else {
        const [called, response] = await engine.call(requestOf(action, input));
        status = called;
        output = response && ["response", response];
      }
      if (status !== expected) {
        failures.push(`${at}: 状態が ${status}（期待は ${expected}）`);
      }
      if (output) {
        const [kind, data] = output;
        hashes.push(`${name} ${index} ${kind} ${sha256(data)}`);
        if (kind === "response") {
          checkResponse(at, step, JSON.parse(decoder.decode(data)), failures);
        }
      }
    }
    await engine.close();
  }
  return hashes;
}

/** `zumen call` で、セッションの最初の call と、その直後の take を流す */
function runCalls(cases, zumen, failures) {
  const hashes = [];
  const work = mkdtempSync(join(tmpdir(), "zumen-call-"));
  let files = 0;
  try {
    for (const { name, steps } of cases) {
      // セッションの設定と、足した入力。call を 1 度呼んだか、表せない手順があれば、次の session まで飛ばす
      let options = "";
      let inputs = [];
      let done = false;
      let blobs = null;
      for (const [index, step] of steps.entries()) {
        const action = ACTIONS.find((key) => Object.hasOwn(step, key));
        const expected = step.status ?? "ok";
        const input = step[action];
        if (action === "session") {
          const bytes = bytesOf(input, "options", "options_text");
          options = decoder.decode(bytes);
          inputs = [];
          blobs = null;
          // 作れない設定は、call の引数の誤りになる
          done = expected !== "ok" || !new TextEncoder().encode(options).every((byte, i) => byte === bytes[i]);
          continue;
        }
        if (action === "take") {
          if (blobs !== null && blobs.has(input)) {
            hashes.push(`${name} ${index} blob ${sha256(blobs.get(input))}`);
            blobs.delete(input);
          } else {
            blobs = null;
          }
          continue;
        }
        if (done) {
          continue;
        }
        if (action === "push") {
          const [nameBytes, data] = pushOf(input);
          const pushName = utf8(nameBytes);
          if (pushName === undefined || expected !== "ok") {
            done = expected === "ok";
            continue;
          }
          const file = join(work, `${files++}`);
          writeFileSync(file, data);
          inputs.push(`${pushName}=${file}`);
          continue;
        }
        const request = join(work, `${files++}.json`);
        writeFileSync(request, requestOf(action, input));
        const dir = join(work, `${files++}`);
        const args = ["call", ...inputs.flatMap((value) => ["--input", value]), "--blobs", dir];
        if (options !== "") {
          args.push("--options", options);
        }
        const run = spawnSync(zumen, [...args, request], { maxBuffer: 1 << 30 });
        const status = STATUS[run.status] ?? `exit ${run.status}`;
        if (status !== expected) {
          failures.push(`${name} の手順 ${index}: 状態が ${status}（期待は ${expected}）${decoder.decode(run.stderr)}`);
        }
        done = true;
        if (run.stdout.length === 0) {
          continue;
        }
        const response = run.stdout.subarray(0, run.stdout.length - 1);
        hashes.push(`${name} ${index} response ${sha256(response)}`);
        const parsed = JSON.parse(decoder.decode(response));
        checkResponse(`${name} の手順 ${index}`, step, parsed, failures);
        blobs = new Map((parsed.blobs ?? []).map(({ id, kind }) => [id, readFileSync(join(dir, `${id}.${kind}`))]));
      }
    }
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
  return hashes;
}

const [mode, path] = process.argv.slice(2);
if (!["stdio", "http", "call", "wasi"].includes(mode) || !path) {
  console.error("使い方: node bindings/conformance/cli.mjs <stdio | http | call | wasi> <zumen の実行ファイル | zumen.wasm>");
  process.exit(2);
}

const cases = loadCases();
const failures = [];
const expected = readFileSync(join(here, "hashes.txt"), "utf8")
  .split("\n")
  .filter((line) => line !== "" && !line.startsWith("#"));
let hashes;
if (mode === "call") {
  hashes = runCalls(cases, path, failures);
  const missing = hashes.filter((line) => !expected.includes(line));
  if (missing.length > 0) {
    failures.push(`hashes.txt にない行:\n${missing.join("\n")}`);
  }
} else {
  let rpc = null;
  let server = null;
  let makeEngine;
  if (mode === "http") {
    server = await httpServer(path);
    makeEngine = () => httpEngine(server);
  } else {
    // 組ごとに、新しいセッションで始める（session_new が前のセッションを閉じる）
    rpc = mode === "wasi" ? new LineRpc(process.execPath, ["--no-warnings", join(here, "wasi.mjs"), path, "serve", "--stdio"]) : new LineRpc(path, ["serve", "--stdio"]);
    makeEngine = () => stdioEngine(rpc);
  }
  try {
    hashes = await runCases(cases, makeEngine, failures);
  } finally {
    rpc?.close();
    server?.child.kill();
  }
  if (JSON.stringify(hashes) !== JSON.stringify(expected)) {
    const lost = expected.filter((line) => !hashes.includes(line)).map((line) => `- ${line}`);
    const extra = hashes.filter((line) => !expected.includes(line)).map((line) => `+ ${line}`);
    failures.push(`ハッシュの一覧が hashes.txt と合わない:\n${[...lost, ...extra].join("\n")}`);
  }
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log(`${mode}: 試験の組 ${cases.length} 件、ハッシュ ${hashes.length} 件が hashes.txt と一致した`);
