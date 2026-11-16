import wasmUrl from "scene3d-module?url";
import catalog from "../../catalog/ja.json?raw";

let compiled;
export { catalog };
/** 3D のページは全部入りの wasm を 1 つだけ取り、IFC・地物・点群と地面の図面で共有する。 */
export async function loadModule(_name, signal) {
  if (compiled) return compiled;
  const response = await fetch(wasmUrl, { signal, redirect: "error" });
  if (!response.ok) throw new Error("wasm.load");
  const module = await WebAssembly.compileStreaming(response);
  if (!signal.aborted) compiled = module;
  return module;
}
export const loadDrawingModule = (signal) => loadModule("drawing", signal);
