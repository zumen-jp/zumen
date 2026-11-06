// 利用側の例。pdf.js と入力の読み方・ページ名・単位・Canvas の作り方は、利用側が明示する。
// pdf.js の getViewport / render: https://mozilla.github.io/pdf.js/api/draft/module-pdfjsLib-PDFPageProxy.html
const check = (signal) => { if (signal.aborted) throw new Error("中断しました"); };

export function pdfUnderlayRenderer(pdfjs, read, { pageNumber, contentUnitsPerPoint, createCanvas }) {
  if (!Number.isFinite(contentUnitsPerPoint) || contentUnitsPerPoint <= 0) throw new RangeError("contentUnitsPerPoint");
  return async (request) => {
    if (request.kind !== "pdf") throw new Error("PDF ではありません");
    const bytes = await read(request);
    check(request.signal);
    if (!(bytes instanceof Uint8Array) || bytes.byteLength > request.limits.maxBytes) throw new RangeError("maxBytes");
    const number = pageNumber(request.page);
    if (!Number.isSafeInteger(number) || number < 1) throw new RangeError("pageNumber");
    // 入力バイト数と出力 Canvas の上限であり、pdf.js 内部の全確保量を制限するものではない。
    // pdf.js は data を Worker へ転送する。上限を確かめた後、転送する範囲だけを複製する。
    const task = pdfjs.getDocument({ data: new Uint8Array(bytes), useSystemFonts: false, disableFontFace: true,
      isEvalSupported: false, maxImageSize: request.limits.maxPixels, canvasMaxAreaInBytes: request.limits.maxPixels * 4 });
    let closing;
    const close = () => closing ??= task.destroy();
    const aborted = () => { void close().catch(() => {}); };
    request.signal.addEventListener("abort", aborted, { once: true });
    try {
      const doc = await task.promise;
      const page = await doc.getPage(number);
      check(request.signal);
      const viewport = page.getViewport({ scale: 1 }), width = viewport.width * contentUnitsPerPoint, height = viewport.height * contentUnitsPerPoint;
      return {
        width, height,
        async render(value) {
          check(value.signal);
          const source = createCanvas(value.width, value.height), region = value.region;
          source.width = value.width; source.height = value.height;
          const sx = value.width / (region.max_x - region.min_x), sy = value.height / (region.max_y - region.min_y);
          let rendering;
          const cancel = () => rendering?.cancel();
          value.signal.addEventListener("abort", cancel, { once: true });
          try {
            rendering = page.render({ canvasContext: source.getContext("2d"), viewport, background: "rgba(0,0,0,0)",
              transform: [sx * contentUnitsPerPoint, 0, 0, sy * contentUnitsPerPoint, -region.min_x * sx, -(height - region.max_y) * sy] });
            await rendering.promise; check(value.signal);
            return { source, dispose() { source.width = 0; source.height = 0; } };
          } catch (error) { source.width = 0; source.height = 0; throw error; }
          finally { value.signal.removeEventListener("abort", cancel); }
        },
        dispose: close,
      };
    } catch (error) { await close(); throw error; }
    finally { request.signal.removeEventListener("abort", aborted); }
  };
}
