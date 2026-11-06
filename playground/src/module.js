import wasmUrl from "drawing-module?url";
import catalog from "../../catalog/ja.json?raw";

let compiled;
export { catalog };
export async function loadModule(signal) {
  if (compiled) return compiled;
  const response = await fetch(wasmUrl, { signal, redirect: "error" });
  if (!response.ok) throw new Error("wasm.load");
  const module = await WebAssembly.compileStreaming(response);
  if (!signal.aborted) compiled = module;
  return module;
}
