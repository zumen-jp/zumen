import type { Scene, SceneResult } from "@zumen-jp/zumen";
import { View2D, type Camera, type RenderIssue, type View2DState, type View2DEvent } from "../src/index.js";

declare const scene: Scene;
declare const result: SceneResult;
const canvas = document.createElement("canvas");
const viewer = new View2D(canvas, { width: 300, height: 200, pixelRatio: 2 });
const report = viewer.setScene({ scene, result, document: { doc: "a.dxf", format: "dxf", hash: "v2:0" } });
const issues: readonly RenderIssue[] = report.issues;
const view: Camera = viewer.getView();
viewer.fit();
viewer.zoomBy(2, { x: 10, y: 20 });
viewer.rotateBy(Math.PI / 2);
viewer.panBy(20, 30);
viewer.jumpTo(view);
viewer.toScene(viewer.toScreen({ x: 1, y: 2 }));
viewer.resize(600, 400);
const state: View2DState = viewer.getState();
viewer.setUnderlayRenderer(async (request) => {
  const name: string = request.name, page: string = request.page, limit: number = request.limits.maxBytes;
  const signal: AbortSignal = request.signal;
  void name; void page; void limit; void signal; void request.element.path;
  return {
    width: 100, height: 50,
    render(part) {
      const source = document.createElement("canvas"); source.width = part.width; source.height = part.height;
      const bottom: number = part.region.min_y;
      void bottom;
      return { source, dispose() { source.width = 0; } };
    },
    async dispose() {},
  };
});
const stopUnderlay = viewer.on("underlay", ({ state }) => { void state.underlays[0]?.status; });
viewer.refreshUnderlays(); viewer.cancelUnderlays(); viewer.setUnderlayRenderer(null); stopUnderlay();
// @ts-expect-error 資源にも解放する関数が要る
viewer.setUnderlayRenderer(() => ({ width: 1, height: 1, render() { return { source: canvas, dispose() {} }; } }));
// @ts-expect-error Blob は復号した画素ではない
viewer.setUnderlayRenderer(() => ({ width: 1, height: 1, render() { return { source: new Blob(), dispose() {} }; }, dispose() {} }));
// @ts-expect-error 画像の状態の一覧を直接変更しない
state.underlays.push();
const unsubscribe = viewer.subscribe((next) => { const same: View2DState = next; void same; });
const off = viewer.on("selection", (event) => { const same: View2DEvent = event; void same; });
viewer.setHiddenLayers(viewer.getLayers());
viewer.setLayersVisible(viewer.getLayers(), true);
const refs = viewer.hitTest({ x: 100, y: 80 }, 4);
viewer.select(refs, "toggle");
viewer.setHovered(refs[0] ?? null);
viewer.setHighlight("search", viewer.hitTestRect({ min_x: 0, min_y: 0, max_x: 200, max_y: 100 }), "#ff0000");
const detach = viewer.attachInput(canvas, { wheel: false, touch: true });
viewer.setInputOptions({ keyboard: false });
viewer.setTool("measure");
viewer.handleInput({ type: "pointerdown", point: { x: 5, y: 6 }, pointerId: 1, pointerType: "touch" });
viewer.handleInput({ type: "cancel" });
const start = viewer.snapAt({ x: 0, y: 0 })?.position ?? viewer.positionAt({ x: 0, y: 0 });
viewer.toScreen(start);
viewer.setMeasurement({ start, end: viewer.positionAt({ x: 3, y: 4 }) });
const removeTool = viewer.addTool("custom", {
  onActivate(context) {
    context.on("measurement", ({ state }) => { void state.measurement?.real; });
    context.addOverlay((ctx, state) => { ctx.strokeRect(0, 0, state.size.width, state.size.height); });
    return () => { void context.signal.aborted; };
  },
  onInput(input, { viewer }) {
    if (input.type === "pointerup") { viewer.select(viewer.hitTest(input.point)); return true; }
  },
});
const removeOverlay = viewer.addOverlay((ctx) => ctx.fillRect(2, 2, 2, 2), { scope: "viewer" });
viewer.invalidate(); viewer.cancelInput(); removeTool(); removeOverlay(); detach();
unsubscribe(); off();
viewer.setScene(null);
viewer.destroy();
new View2D(new OffscreenCanvas(300, 200));
void issues;
// @ts-expect-error 文書の hash が要る
viewer.setScene({ scene, result, document: { doc: "a", format: "dxf" } });
// @ts-expect-error カメラの中心を直接変更しない
view.center.x = 1;
// @ts-expect-error 保持したシーンの件数を直接変更しない
viewer.getScene()!.result.counts.lines = 1;
// @ts-expect-error 未定義のフォントの取得を指定しない
new View2D(canvas, { fallbackFont: "https://example.com/font.ttf" });
// @ts-expect-error 状態の選択を直接変更しない
state.selection.push({ id: "space/0/h1", path: [] });
// @ts-expect-error 配置の格子の列・行を省かない
viewer.select([{ id: "space/1/h1", path: [{ id: "space/0/h2" }] }]);
// @ts-expect-error 未定義の種類を購読しない
viewer.on("pointer", () => {});
// @ts-expect-error 入力には画面の点とポインターの番号が要る
viewer.handleInput({ type: "pointerdown" });
// @ts-expect-error カメラと同じく計測の位置も直接変更しない
start.offset.x = 0;
// @ts-expect-error OffscreenCanvas を DOM の入力先にはしない
viewer.attachInput(new OffscreenCanvas(100, 100));
// @ts-expect-error 独自ツールには入力の関数が要る
viewer.addTool("invalid", {});
