// 使い方: node main.mjs <入力> [wasm]
import { readFile } from "node:fs/promises";
import { createSceneJob } from "./job.mjs";

const [inputPath, wasmPath] = process.argv.slice(2);
if (!inputPath) {
  console.error("使い方: node main.mjs <入力> [wasm]");
  process.exit(2);
}
// サーバーでは、この Promise を要求の外で 1 度だけ作る。
const module = await WebAssembly.compile(await readFile(wasmPath ?? new URL(import.meta.resolve("@zumen-jp/zumen/zumen_abi1.wasm"))));
// readFile の Buffer は共有プールを持つことがあるので、専有するバッファーに写す。
// HTTP の request.arrayBuffer() なら、そのまま渡せる。
const input = Uint8Array.from(await readFile(inputPath)).buffer;
if (input.byteLength > 64 * 1024 * 1024) throw new RangeError("入力は 64 MiB まで");
const { result } = createSceneJob(module, input, {
  budget: { input_bytes: 64 * 1024 * 1024, decoded_bytes: 512 * 1024 * 1024 },
  signal: AbortSignal.timeout(30_000),
});
const { response, blob } = await result;
console.log(JSON.stringify({ counts: response.result.counts, bytes: blob.byteLength }));
