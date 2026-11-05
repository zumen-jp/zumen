import type { Scene, SceneResult } from "@zumen-jp/zumen";
import { View2D, type Camera, type RenderIssue } from "../src/index.js";

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
