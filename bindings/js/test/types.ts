// ラッパーの型が、使う形で通ることを確かめる（決定 0194）。コンパイルするだけで、動かさない。

import {
  SCENE_VALUES,
  SCHEMA_MAJOR,
  type Diagnostic,
  type OpenResult,
  type SceneResult,
  type StatusName,
  ZumenError,
  load,
  loadModules,
  readScene,
} from "../src/index.js";
import { loadBundled } from "../src/node.js";
import { wasmUrls } from "../src/wasm.js";

export async function server(bytes: Uint8Array): Promise<string | undefined> {
  const zumen = await loadBundled();
  if (zumen.broken) {
    return undefined;
  }
  const session = zumen.open();
  try {
    session.push("input", bytes);
    const opened = session.call({ v: SCHEMA_MAJOR, op: "open", input: "input", budget: { input_bytes: 100_000_000 } });
    return opened.result?.format;
  } catch (error) {
    if (error instanceof ZumenError && error.status === "panicked") {
      console.log(error.cause);
    }
    throw error;
  } finally {
    session.close();
  }
}

export async function modules(bytes: Uint8Array): Promise<void> {
  const loaded = loadModules({ drawing: () => fetch(wasmUrls.drawing) });
  const opened = await loaded.open("a.p2z", bytes, { doc: "a" }, { "a.saf": "<SxfAttributeXML/>" });
  if (opened) {
    const format: string | undefined = opened.response.result?.format;
    console.log(format, opened.session.call({ v: SCHEMA_MAJOR, op: "drawing", doc: "a" }).result?.hash);
    opened.session.close();
  }
}

export async function sample(bytes: Uint8Array): Promise<void> {
  const zumen = await load(fetch("zumen_abi1.wasm"));
  const session = zumen.open({ v: SCHEMA_MAJOR, engine: "wasm" });
  session.push("a.dxf", bytes);
  try {
    const opened = session.call({ v: SCHEMA_MAJOR, op: "open", input: "a.dxf" });
    const result: OpenResult | undefined = opened.result;
    const response = session.call({ v: SCHEMA_MAJOR, op: "scene", doc: result?.doc ?? "", from: "space/0" });
    const scene: SceneResult | undefined = response.result;
    if (scene) {
      const read = readScene(session.take(scene.blob));
      const origins: Float64Array = read.columns.chunk_origins;
      const aligns: Uint8Array = read.columns.text_aligns;
      const center: 1 = SCENE_VALUES.horizontal.center;
      console.log(origins.length, aligns[0] === center, read.rows("line_xy"));
    }
  } catch (error) {
    if (error instanceof ZumenError) {
      const status: StatusName = error.status;
      const diagnostics: Diagnostic[] = error.response?.diagnostics ?? [];
      console.log(status, diagnostics.map((diagnostic) => diagnostic.code));
    }
  } finally {
    session.close();
  }
}
