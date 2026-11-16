import type { Scene3dResult } from "@zumen-jp/zumen";
import { THREE_REVISION, View3D, View3DError, type ElementHit, type ReviewApplyResult, type ReviewExport, type Scene3DReport, type Snapshot3D, type View3DState, type View3DView } from "../src/index.js";

declare const blob: Uint8Array;
declare const result: Scene3dResult;
const canvas = document.createElement("canvas");
const view: View3DView = {
  id: "main", rect: { x: 0, y: 0, width: 300, height: 200 },
  camera: { projection: "perspective", position: [0, 0, 10], direction: [0, 0, -1], up: [0, 1, 0], fov: 45 },
  clips: [{ point: [0, 0, 0], normal: [1, 0, 0] }], box: { min: [0, 0, 0], max: [1, 1, 1] },
  selected: ["wall"], visible: false, exceptions: ["slab"], colored: [{ color: "#FF0000", ids: ["door"] }],
};
const viewer = new View3D(canvas, { width: 300, height: 200, pixelRatio: 2, shading: "unlit", limits: { gpuBytes: 1 << 20 }, views: [view] });
const document3d = { doc: "a.ifc", format: "ifc", hash: "0" };
const report: Scene3DReport = viewer.setScene({ blob, result, document: document3d });
const none: null = viewer.setScene(null);
viewer.select(document3d, ["wall"]);
viewer.select(document3d, ["wall"], { view: "main" });
viewer.setHidden(document3d, []);
viewer.setColors(document3d, [{ color: "#80FF0000", ids: ["wall"] }], { view: "main" });
const exported: ReviewExport = viewer.toReviewPlace("main", { frame: "contain", hashes: new Map([["wall", "h"]]) });
const applied: ReviewApplyResult = viewer.applyReviewPlace("main", document3d, exported.place);
const shot: Snapshot3D = viewer.snapshot("main");
const firstView = viewer.getState().views[0];
const clipCount: number = firstView.clips.length + (firstView.box === null ? 0 : 6);
viewer.fit("main");
const hits: readonly ElementHit[] = viewer.hitTest("main", { x: 1, y: 2 }, { limit: 10 }).hits;
const ray = viewer.getRay("main", { x: 1, y: 2 });
const state: View3DState = viewer.getState();
const off = viewer.subscribe((value) => value.views[0]?.selected.length);
const offRender = viewer.on("render", (event) => event.views);
const drawn: boolean = viewer.render();
viewer.invalidate();
off(); offRender();
viewer.destroy();
const revision: "187" = THREE_REVISION;
try { viewer.render(); } catch (error) { if (error instanceof View3DError) error.reason satisfies string; }
void [report, none, hits, ray, state, drawn, revision, applied, shot, clipCount];
