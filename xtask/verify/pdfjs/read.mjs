// #519: 別に入れた pdf.js で、標準入力の PDF を読む。実装は成果物に入れない。
// getTextContent・getOperatorList: https://mozilla.github.io/pdf.js/api/draft/module-pdfjsLib-PDFPageProxy.html
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

let pdfjs;
try {
  pdfjs = await import("pdfjs-dist/legacy/build/pdf.mjs");
} catch (error) {
  if (error.code === "ERR_MODULE_NOT_FOUND" && error.message.includes("'pdfjs-dist'")) process.exit(77);
  throw error;
}
if (pdfjs.version !== "6.5.438") throw new Error("pdf.js のバージョンが違う");
if (process.argv[2] === "--check") process.exit(0);
const { getDocument, OPS, AnnotationMode } = pdfjs;
const resources = new URL("node_modules/pdfjs-dist/", import.meta.url);
const resource = (name) => fileURLToPath(new URL(name, resources)).replaceAll("\\", "/");
const task = getDocument({
  data: new Uint8Array(readFileSync(0)),
  verbosity: 0,
  isEvalSupported: false,
  useSystemFonts: false,
  standardFontDataUrl: resource("standard_fonts/"),
  cMapUrl: resource("cmaps/"),
  wasmUrl: resource("wasm/"),
});

// 6.5.438 の constructPath は、描画の演算子と DrawOPS の列を渡す。
// DrawOPS: moveTo = 0、lineTo = 1、curveTo = 2、quadraticCurveTo = 3、closePath = 4。
// 線は部分経路ごと、塗りは輪をまとめた 1 つとして数える（PDF 32000-2 の 8.5.2・8.5.3）。
function subpaths(data) {
  let count = 0, drawn = false;
  for (let i = 0; i < data.length;) {
    const op = data[i++];
    if (op === 0) { if (drawn) count++; drawn = false; i += 2; }
    else if (op === 1) { drawn = true; i += 2; }
    else if (op === 2) { drawn = true; i += 6; }
    else if (op === 3) { drawn = true; i += 4; }
    else if (op !== 4) throw new Error(`未対応の DrawOPS: ${op}`);
  }
  return count + Number(drawn);
}

function counts(operators, optional) {
  let paths = 0, images = 0;
  const fill_rules = [];
  const visibility = [true];
  const fills = new Set([OPS.fill, OPS.eoFill, OPS.fillStroke, OPS.eoFillStroke, OPS.closeFillStroke, OPS.closeEOFillStroke]);
  const evenOdd = new Set([OPS.eoFill, OPS.eoFillStroke, OPS.closeEOFillStroke]);
  const strokes = new Set([OPS.stroke, OPS.closeStroke, OPS.fillStroke, OPS.eoFillStroke, OPS.closeFillStroke, OPS.closeEOFillStroke]);
  const singles = new Set([OPS.paintImageXObject, OPS.paintInlineImageXObject, OPS.paintImageMaskXObject, OPS.paintSolidColorImageMask]);
  operators.fnArray.forEach((op, index) => {
    const args = operators.argsArray[index];
    if (op === OPS.beginMarkedContent || op === OPS.beginMarkedContentProps) {
      const visible = op !== OPS.beginMarkedContentProps || args[0] !== "OC" || optional.isVisible(args[1]);
      visibility.push(visibility.at(-1) && visible);
    } else if (op === OPS.endMarkedContent) {
      if (visibility.length <= 1) throw new Error("閉じるマーク付きの内容がない");
      visibility.pop();
    } else if (visibility.at(-1)) {
      if (op === OPS.constructPath) {
        const n = subpaths(args[1]?.[0] ?? []);
        if (fills.has(args[0]) && n > 0) {
          paths++;
          fill_rules.push(evenOdd.has(args[0]) ? "evenodd" : "nonzero");
        }
        if (strokes.has(args[0])) paths += n;
      } else if (singles.has(op)) images++;
      else if (op === OPS.paintImageXObjectRepeat) images += args[3].length / 2;
      else if (op === OPS.paintImageMaskXObjectRepeat) images += args[5].length / 2;
      else if (op === OPS.paintInlineImageXObjectGroup) images += args[1].length;
      else if (op === OPS.paintImageMaskXObjectGroup) images += args[0].length;
    }
  });
  return { paths, images, fill_rules };
}

try {
  const doc = await task.promise;
  const optional = await doc.getOptionalContentConfig({ intent: "display" });
  const pages = [];
  for (let i = 1; i <= doc.numPages; i++) {
    const page = await doc.getPage(i);
    const viewport = page.getViewport({ scale: 1 });
    const content = await page.getTextContent();
    const operators = await page.getOperatorList({ annotationMode: AnnotationMode.DISABLE });
    // pdf.js の表示の座標（Y が下）を、用紙の左下が原点・Y が上の pt にする。
    const texts = content.items.filter(item => item.str?.trim()).map(item => {
      const [x, y] = viewport.convertToViewportPoint(item.transform[4], item.transform[5]);
      return { text: item.str, x, y: viewport.height - y };
    });
    pages.push({ width: viewport.width, height: viewport.height, texts, ...counts(operators, optional) });
    page.cleanup();
  }
  process.stdout.write(JSON.stringify({ version: pdfjs.version, pages }) + "\n");
} finally {
  await task.destroy();
}
