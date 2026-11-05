// ABI を変更せず、同じ描画部品を wasm で走らせてネイティブの出力・診断・予算・停止と照らす。
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";

const [wasm, transcript] = process.argv.slice(2);
const module = new WebAssembly.Module(readFileSync(wasm));
assert.deepEqual(WebAssembly.Module.imports(module), []);
const { exports } = new WebAssembly.Instance(module, {});
const ptr = exports.transcript_ptr();
const size = exports.transcript_len();
const result = Buffer.from(exports.memory.buffer, ptr, size);
assert.deepEqual(result, readFileSync(transcript));
console.log(`SVG の native/wasm 照合: ${size} bytes、sha256 ${createHash("sha256").update(result).digest("hex")}`);
