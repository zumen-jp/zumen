import type { Scene3dResult } from "@zumen-jp/zumen";
import { THREE_REVISION, View3D, View3DError, type ElementHit, type Scene3DReport, type View3DState, type View3DView } from "../src/index.js";

declare const blob: Uint8Array;
declare const result: Scene3dResult;
const canvas = document.createElement("canvas");
const view: View3DView = {
  id: "main", rect: { x: 0, y: 0, width: 300, height: 200 },
  camera: { projection: "perspective", position: [0, 0, 10], direction: [0, 0, -1], up: [0, 1, 0], fov: 45 },
};
const viewer = new View3D(canvas, { width: 300, height: 200, pixelRatio: 2, shading: "unlit", limits: { gpuBytes: 1 << 20 }, views: [view] });
const document3d = { doc: "a.ifc", format: "ifc", hash: "0" };
const report: Scene3DReport = viewer.setScene({ blob, result, document: document3d });
const none: null = viewer.setScene(null);
viewer.select(document3d, ["wall"]);
viewer.setHidden(document3d, []);
viewer.fit("main");
const hits: readonly ElementHit[] = viewer.hitTest("main", { x: 1, y: 2 }, { limit: 10 }).hits;
const ray = viewer.getRay("main", { x: 1, y: 2 });
const state: View3DState = viewer.getState();
const off = viewer.subscribe((value) => value.selection.length);
const offRender = viewer.on("render", (event) => event.views);
const drawn: boolean = viewer.render();
viewer.invalidate();
off(); offRender();
viewer.destroy();
const revision: "187" = THREE_REVISION;
try { viewer.render(); } catch (error) { if (error instanceof View3DError) error.reason satisfies string; }
void [report, none, hits, ray, state, drawn, revision];
