import { useDrawing, useDrawingViewer, useViewerEvent, useViewerState } from "../src/index.js";

declare const canvas: HTMLCanvasElement | null, file: File | null, module: WebAssembly.Module;
const viewer = useDrawingViewer(canvas);
const state = useViewerState(viewer);
useViewerEvent(viewer, "selection", ({ state: next }) => void next.selection);
const opened = useDrawing(file, { loadModule: () => module, from: "sheet/0" });
if (opened.status === "ready" && state?.selection[0]) {
  void opened.getSource(state.selection[0]).then((entity) => entity?.source?.record);
}
if (opened.status === "ready") {
  void opened.convert().then(({ bytes, result, diagnostics }) => [bytes.byteLength, result.elements, diagnostics[0]?.stage]);
}
if (opened.status === "error") void opened.diagnostics[0]?.text;
