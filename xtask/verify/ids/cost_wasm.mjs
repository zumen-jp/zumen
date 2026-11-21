// Python がネイティブ CLI に渡した同じ入力と要求を流し、応答の生バイトのハッシュを返す。
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { loadSync, ZumenError } from "../../../bindings/js/src/index.js";

const session = loadSync(readFileSync(process.argv[2])).open();
const hashes = [];
try {
  for (const step of JSON.parse(readFileSync(0, "utf8"))) {
    if (step.push) {
      session.push(step.push, Buffer.from(step.base64, "base64"));
      continue;
    }
    let bytes;
    let status = "ok";
    try {
      bytes = session.callBytes(JSON.stringify(step.call));
    } catch (error) {
      if (!(error instanceof ZumenError) || !error.bytes) throw error;
      bytes = error.bytes;
      status = error.status;
    }
    hashes.push([status, createHash("sha256").update(bytes).digest("hex")]);
  }
} finally {
  session.close();
}
process.stdout.write(JSON.stringify(hashes));
