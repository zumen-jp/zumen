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
  openRanged,
  readScene,
  readScene3D,
  type Scene3dResult,
} from "../src/index.js";

function checkScene3DTypes(blob: Uint8Array, result: Scene3dResult): string | undefined {
  const scene = readScene3D(blob, { maxAllocatedBytes: 1024 * 1024, maxWork: 100000 });
  const chunk: Float64Array = scene.chunk(0).origin;
  const mesh: Uint32Array = scene.mesh(0).triangles;
  const id: string = scene.element(scene.instance(0).element).id;
  const row: number | undefined = scene.elementById(id);
  return result.counts.instances > 0 && chunk.length === 3 && mesh.length > 0 && row !== undefined ? id : undefined;
}
void checkScene3DTypes;
import { loadBundled, openProcess, openLiveProcess, ProcessError, type ProcessLimits, type LiveReply } from "../src/node.js";

export async function liveDocument(executable: string, input: Uint8Array): Promise<bigint | null> {
  const live = await openLiveProcess(executable);
  try {
    const opened: LiveReply = await live.open(input, { threads: 4, budget: { work: 100000n } });
    if (opened.revision !== null && opened.display) {
      for (const column of opened.display.columns) {
        for (const block of column.blocks()) {
          const bytes: Uint8Array = block.bytes;
          void bytes;
        }
      }
      live.acknowledge(opened.revision);
      const changed = await live.transform({ revision: opened.revision, rows: [0], matrix: [1, 0, 5, 0, 1, -2] });
      if (changed.revision !== null) await live.undo(changed.revision);
      const target: string | undefined = (await live.target(0, live.revision!)).target?.id;
      void target;
      const selected = await live.select([0, 0, 10, 10], live.revision!);
      const rows: number[] = selected.selection ? [...selected.selection.rows()] : [];
      const names: string[] = (await live.layers(live.revision!)).layers?.map((layer) => layer.name) ?? [];
      void names;
      const drawn: number[] = [...((await live.order(live.revision!)).order?.rows() ?? [])];
      void drawn;
      const copies: number[] = [...((await live.copy({ revision: live.revision!, rows: [0], matrix: [1, 0, 1, 0, 1, 0] })).selection?.rows() ?? [])];
      await live.remove({ revision: live.revision!, rows: copies }, { display: false });
      if (selected.revision !== null) await live.transform({ revision: selected.revision, rows, matrix: [1, 0, 1, 0, 1, 2] });
      await live.diagnostics(changed.requestId, { budget: { decodedBytes: null } });
    }
    return live.revision;
  } finally { await live.close(); }
}
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

export async function processServer(executable: string, bytes: Uint8Array): Promise<string | undefined> {
  const error: ProcessError = new ProcessError("io", "通信の失敗", new Error("原因"));
  void error;
  const limits: ProcessLimits = { timeoutMs: 60_000, maxMessageBytes: 8 * 1024 * 1024 };
  const session = await openProcess(executable, { v: SCHEMA_MAJOR }, limits);
  try {
    await session.push("input", bytes);
    const opened = await session.call({ v: SCHEMA_MAJOR, op: "open", input: "input" });
    const result: OpenResult | undefined = opened.result;
    const blob: Uint8Array = await session.take(0);
    void blob;
    await session.cancel();
    return result?.format;
  } catch (error) {
    if (error instanceof ProcessError) console.log(error.code, session.broken);
    throw error;
  } finally { await session.close(); }
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

export async function ranged(file: Blob): Promise<number | undefined> {
  const zumen = await load(fetch(wasmUrls.pointcloud));
  const session = zumen.open();
  try {
    const opened = await openRanged(session, { v: SCHEMA_MAJOR, input: "a.las", budget: { input_bytes: 1 << 26 } }, file, { head: 4096 });
    const doc: string | undefined = opened.result?.doc;
    await openRanged(session, { v: SCHEMA_MAJOR, input: "b.las" }, (start, end) => new Uint8Array(end - start), { size: 1024 });
    return doc ? session.call({ v: SCHEMA_MAJOR, op: "pointcloud", doc }).result?.points : undefined;
  } finally { session.close(); }
}
