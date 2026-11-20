import type { Scene3dResult } from "@zumen-jp/zumen";
import { attachGround2D, type View2DLike } from "../src/ground2d.js";
import { attachPointCloud, type PointCloudOutcome } from "../src/pointcloud.js";
import { THREE_REVISION, View3D, View3DError, type Hit3D, type ReviewApplyResult, type ReviewExport, type Scene3DReport, type Snapshot3D, type View3DState, type View3DView } from "../src/index.js";

declare const blob: Uint8Array;
declare const result: Scene3dResult;
const canvas = document.createElement("canvas");
const view: View3DView = {
  id: "main", rect: { x: 0, y: 0, width: 300, height: 200 },
  camera: { projection: "perspective", position: [0, 0, 10], direction: [0, 0, -1], up: [0, 1, 0], fov: 45 },
  clips: [{ point: [0, 0, 0], normal: [1, 0, 0] }], box: { min: [0, 0, 0], max: [1, 1, 1] },
  selected: ["wall"], visible: false, exceptions: ["slab"], colored: [{ color: "#FF0000", ids: ["door"] }],
};
const viewer = await View3D.create(canvas, { width: 300, height: 200, pixelRatio: 2, shading: "unlit", limits: { gpuBytes: 1 << 20 }, views: [view] });
const document3d = { doc: "a.ifc", format: "ifc", hash: "0" };
const report: Scene3DReport = viewer.setScene({ blob, result, document: document3d });
const none: null = viewer.setScene(null);
viewer.select(document3d, ["wall"]);
viewer.select(document3d, ["wall"], { view: "main" });
viewer.setHidden(document3d, []);
viewer.setColors(document3d, [{ color: "#80FF0000", ids: ["wall"] }], { view: "main" });
const exported: ReviewExport = viewer.toReviewPlace("main", { frame: "contain", hashes: new Map([["wall", "h"]]) });
const applied: ReviewApplyResult = viewer.applyReviewPlace("main", document3d, exported.place);
const shot: Snapshot3D = await viewer.snapshot("main");
const firstView = viewer.getState().views[0];
const clipCount: number = firstView.clips.length + (firstView.box === null ? 0 : 6);
viewer.fit("main");
const unit = { name: "METRE" };
const registered = viewer.addGeometry([{ kind: "line", points: [[0, 0, 0], [1, 0, 0]] }, { kind: "label", position: [0, 0, 0], image: new ImageData(1, 1), width: 8, height: 8 }], { id: "g", origin: [0, 0, 0], unit, priority: 1 });
const parts: number = registered.parts.length;
const object = viewer.addObject({}, { id: "o", origin: [0, 0, 0], unit, scope: "viewer", clipping: false, dispose: () => {} });
object.signal.addEventListener("abort", () => object.remove());
const detach = viewer.attachInput(canvas, { orbit: false, zoomSpeed: 0.001 });
const handled: boolean = viewer.handleInput({ type: "wheel", point: { x: 1, y: 2 }, deltaY: 10, ctrlKey: true });
viewer.setInputOptions({ touch: false });
viewer.cancelInput();
detach();
const offPick = viewer.on("pick", (event) => event.hits[0]?.kind === "geometry" ? event.hits[0].part : event.view);
offPick();
declare const real2d: import("../../view2d/src/index.js").View2D;
const view2d: View2DLike = real2d;
const ground = attachGround2D(viewer, { id: "ground", view2d, canvas, plane: { origin: [0, 0, 0], xAxis: [1, 0, 0], yAxis: [0, 1, 0] },
  link: { document: document3d, to3d: (ref) => [ref.id], to2d: () => [] } });
const groundHit = ground.hitTest("main", { x: 1, y: 2 });
const groundElements: number = groundHit.status === "hit" ? groundHit.elements.length : 0;
const groundScale: number | null = ground.getReport().scale;
ground.remove();
const element = viewer.getElement(document3d, "wall");
const storey: string | null = element === null ? null : element.storey;
const mapName: string | undefined = viewer.getState().scene?.map?.crs_name;
const cloud = attachPointCloud(viewer, { id: "cloud", input: { blob, result, document: document3d }, color: "intensity",
  load: async ({ signal }) => (signal.aborted ? null : { blob, result, document: document3d }) });
const outcome: Promise<PointCloudOutcome> = cloud.request({ box: [0, 0, 0, 1, 1, 1] });
const pointHit = cloud.hitTest("main", { x: 1, y: 2 }).hits[0];
const pointRow: number | undefined = pointHit?.row;
const cloudDrawn: number = cloud.getReport().drawn.points;
cloud.cancel();
cloud.remove();
const hits: readonly Hit3D[] = viewer.hitTest("main", { x: 1, y: 2 }, { limit: 10 }).hits;
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
void [report, none, hits, ray, state, drawn, revision, applied, shot, clipCount, parts, handled, state.objects.length, groundElements, groundScale, storey, mapName, outcome, pointRow, cloudDrawn];
