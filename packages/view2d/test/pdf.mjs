import { SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { pdfUnderlayRenderer } from "../example/pdf.js";
import { imageScene } from "./underlays.mjs";
import { bundle } from "./fixtures.mjs";

const canvasOf = (width, height) => { const canvas = document.createElement("canvas"); canvas.width = width; canvas.height = height; return canvas; };
async function settled(viewer) {
  if (viewer.getState().underlays.every((value) => value.status !== "resource-pending")) return;
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => { off(); reject(new Error("PDF の完了通知を待てませんでした")); }, 15000);
    const off = viewer.on("underlay", ({ state }) => {
      if (state.underlays.some((value) => value.status === "resource-pending")) return;
      clearTimeout(timeout); off(); resolve();
    });
  });
}

function twoPages() {
  const objects = ["<< /Type /Catalog /Pages 2 0 R >>", "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 80 40] /Resources << >> /Contents 5 0 R >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 80 40] /Rotate 90 /Resources << >> /Contents 6 0 R >>"];
  for (const color of ["1 0 0", "0 0 1"]) { const stream = `${color} rg 0 0 80 40 re f\n`; objects.push(`<< /Length ${stream.length} >>\nstream\n${stream}endstream`); }
  let text = "%PDF-1.7\n", offsets = [0];
  for (let i = 0; i < objects.length; i++) { offsets.push(text.length); text += `${i + 1} 0 obj\n${objects[i]}\nendobj\n`; }
  const xref = text.length;
  text += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n${offsets.slice(1).map((offset) => `${String(offset).padStart(10, "0")} 00000 n \n`).join("")}trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return new TextEncoder().encode(text);
}

export async function testPdfUnderlays({ assert, near }) {
  const pdfjs = await import("/xtask/verify/pdfjs/node_modules/pdfjs-dist/build/pdf.mjs");
  assert(pdfjs.version === "6.5.438", "PDF の検査は既存の固定版を使う");
  pdfjs.GlobalWorkerOptions.workerSrc = "/xtask/verify/pdfjs/node_modules/pdfjs-dist/build/pdf.worker.mjs";
  const data = new Uint8Array(await (await fetch("/samples/pdf/pdfium-smask-blend.pdf")).arrayBuffer());
  const direct = pdfjs.getDocument({ data: data.slice(), useSystemFonts: false, disableFontFace: true, isEvalSupported: false });
  const doc = await direct.promise, page = await doc.getPage(1), viewport = page.getViewport({ scale: 1 });
  const reference = canvasOf(viewport.width, viewport.height);
  await page.render({ canvasContext: reference.getContext("2d"), viewport }).promise;
  const canvas = canvasOf(200, 200), viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio: 2 });
  const allocated = [], requests = [], disposals = [];
  const base = pdfUnderlayRenderer(pdfjs, () => Promise.resolve(data.slice()), { pageNumber: (name) => name === "front" ? 1 : Number(name), contentUnitsPerPoint: 1,
    createCanvas(width, height) { const source = canvasOf(width, height); allocated.push(source); return source; } });
  viewer.setUnderlayRenderer(async (request) => {
    requests.push(request);
    const resource = await base(request);
    return { ...resource, async dispose() { await resource.dispose(); disposals.push(request.index); } };
  });
  viewer.setScene(bundle(imageScene({ kind: V.image_kind.pdf, at: [-50, -50], edges: [100, 0, 0, 100], pages: ["front"] })));
  await settled(viewer);
  assert(viewer.getReport().drawn.images === 1 && viewer.getState().underlays[0].status === "ready", "許諾済みの公開 PDF を利用側の描画関数で表示する");
  near(viewer.getState().underlays[0].width, viewport.width, "PDF のページ幅");
  const expected = reference.getContext("2d").getImageData(0, 0, 200, 200).data;
  const actual = canvas.getContext("2d").getImageData(100, 100, 200, 200).data;
  let wrong = 0, painted = 0;
  for (let i = 0; i < expected.length; i++) { if (Math.abs(expected[i] - actual[i]) > 3) wrong++; if (i % 4 !== 3 && expected[i] < 240) painted++; }
  assert(wrong === 0 && painted > 100, "公開 PDF の画素を直接の pdf.js 描画と配置・画素比まで照らす");
  viewer.setScene(bundle(imageScene({ kind: V.image_kind.pdf, at: [-50, -50], edges: [100, 0, 0, 100], pages: ["front"],
    clips: [{ xy: [-20, -20, 20, -20, 20, 20, -20, 20], inverted: true }] })));
  await settled(viewer);
  const middle = canvas.getContext("2d").getImageData(200, 200, 1, 1).data;
  assert(middle[0] === 255 && middle[1] === 255 && middle[2] === 255, "公開 PDF に反転した切り抜きを当てる");
  viewer.setScene(bundle(imageScene({ kind: V.image_kind.pdf, pages: ["2"] }))); await settled(viewer);
  assert(viewer.getReport().issues[0].reason === "resource-resolve-failed", "公開 PDF にないページを別ページへ代替しない");
  const synthetic = pdfUnderlayRenderer(pdfjs, () => Promise.resolve(twoPages()), { pageNumber: Number, contentUnitsPerPoint: 2, createCanvas: canvasOf });
  viewer.setUnderlayRenderer(synthetic);
  viewer.setScene(bundle(imageScene({ kind: V.image_kind.pdf, frame: V.image_frame.unit, at: [-20, -40], edges: [.5, 0, 0, .5], pages: ["2"] }))); await settled(viewer);
  const blue = canvas.getContext("2d").getImageData(200, 200, 1, 1).data;
  assert(blue[0] === 0 && blue[2] === 255, "合成 PDF の 2 ページ目を描く");
  assert(viewer.getState().underlays[0].width === 80 && viewer.getState().underlays[0].height === 160, "PDF のページ回転と利用側が指定した内容の単位を保つ");
  viewer.destroy(); await direct.destroy();
  for (let n = 0; n < 100 && disposals.length < 2; n++) await new Promise((resolve) => setTimeout(resolve, 10));
  assert(allocated.every((source) => source.width === 0 && source.height === 0) && disposals.length === 2, "PDF の画素と非同期の文書解放を完了する");
}
