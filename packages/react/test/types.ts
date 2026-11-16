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
  void opened.estimate({ limit: 25 }).then((preview) => {
    void preview.result.estimate.counts[0]?.fidelity;
    return opened.convert(preview);
  });
}
if (opened.status === "error") void opened.diagnostics[0]?.text;

import { useScene3D, useView3D, useView3DEvent, useView3DInput, useView3DState } from "../src/view3d.js";

declare const canvas3d: HTMLCanvasElement | null;
const viewer3d = useView3D(canvas3d, { shading: "unlit" });
useView3DInput(viewer3d, canvas3d, { select: true });
const state3d = useView3DState(viewer3d);
useView3DEvent(viewer3d, "pick", ({ hits }) => void hits[0]?.kind);
const scene = useScene3D(file, { loadModule: (name) => (name === "bim" ? module : module), tolerance: 0.01 });
if (scene.status === "ready" && viewer3d && state3d) {
  viewer3d.setScene(scene.input);
  void scene.elements[0]?.class;
  void scene.getElement(scene.elements[0]?.id ?? "").then((element) => element?.["id"]);
  const { place } = viewer3d.toReviewPlace("main");
  void scene.writeBcf?.({ title: "確認", place, author: "a", date: "2026-11-17T00:00:00Z", version: "2.1" }).then(({ bytes }) =>
    scene.readBcf?.(bytes).then(({ locations }) => locations[0]?.location.camera?.position));
}
if (scene.status === "error") void [scene.error.format, scene.diagnostics[0]?.stage];
