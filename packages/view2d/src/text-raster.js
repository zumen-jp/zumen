// 代替文字（browser-text）と重ね描きの文字計測・グリフ画像化用の内部補助。
// 利用側の描画先 Canvas には一切触れず、内部の OffscreenCanvas だけで OS フォントの寸法と字形を得る。

let measureCanvas = null;
let measureCtx = null;

function createScratchCanvas(width, height) {
  if (typeof OffscreenCanvas !== "undefined") return new OffscreenCanvas(width, height);
  if (typeof document !== "undefined") {
    const c = document.createElement("canvas");
    c.width = width;
    c.height = height;
    return c;
  }
  return null;
}

/** 文字計測専用の内部コンテキストを返す。save / restore と font / textAlign / textBaseline / measureText を持つ。 */
export function getTextMeasureContext() {
  if (measureCtx) return measureCtx;
  measureCanvas = createScratchCanvas(4, 4);
  measureCtx = measureCanvas?.getContext("2d") ?? null;
  return measureCtx;
}

/**
 * 変換行列 [a, b, c, d, e, f]（device px への写像）で文字列を内部 OffscreenCanvas へ描き、
 * 画面内に収まる切り出し矩形と画像ソースを返す。画面外なら null。
 */
export function rasterizeTransformedText({
  text,
  font,
  textAlign = "left",
  textBaseline = "alphabetic",
  fillStyle,
  matrix,
  canvasWidth,
  canvasHeight,
}) {
  if (!text) return null;
  const mctx = getTextMeasureContext();
  if (!mctx) return null;
  mctx.save();
  let metrics;
  try {
    mctx.font = font;
    mctx.textAlign = textAlign;
    mctx.textBaseline = textBaseline;
    metrics = mctx.measureText(text);
  } finally {
    mctx.restore();
  }
  const left = -(metrics.actualBoundingBoxLeft ?? 0) - 2;
  const right = (metrics.actualBoundingBoxRight ?? metrics.width ?? 0) + 2;
  const top = -(metrics.actualBoundingBoxAscent ?? 16) - 2;
  const bottom = (metrics.actualBoundingBoxDescent ?? 4) + 2;
  if (!(right > left && bottom > top)) return null;

  const [a, b, c, d, e, f] = matrix;
  const corners = [
    [left, top],
    [right, top],
    [right, bottom],
    [left, bottom],
  ];
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const [ lx, ly ] of corners) {
    const dx = a * lx + c * ly + e;
    const dy = b * lx + d * ly + f;
    if (!Number.isFinite(dx) || !Number.isFinite(dy)) return null;
    minX = Math.min(minX, dx);
    minY = Math.min(minY, dy);
    maxX = Math.max(maxX, dx);
    maxY = Math.max(maxY, dy);
  }
  const x0 = Math.max(0, Math.floor(minX));
  const y0 = Math.max(0, Math.floor(minY));
  const x1 = Math.min(canvasWidth, Math.ceil(maxX));
  const y1 = Math.min(canvasHeight, Math.ceil(maxY));
  const width = x1 - x0;
  const height = y1 - y0;
  if (width <= 0 || height <= 0) return null;

  const patch = createScratchCanvas(width, height);
  const pctx = patch?.getContext("2d");
  if (!pctx) return null;
  pctx.font = font;
  pctx.textAlign = textAlign;
  pctx.textBaseline = textBaseline;
  pctx.fillStyle = fillStyle;
  pctx.setTransform(a, b, c, d, e - x0, f - y0);
  pctx.fillText(text, 0, 0);
  return { source: patch, x0, y0, width, height };
}
