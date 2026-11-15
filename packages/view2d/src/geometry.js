import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { end, rows, valid } from "./validate.js";
import { unitVisible } from "./interaction.js";
import { clipRing, clipSegment, diskCoversBox, inside, wideClosedStroke, wideJoin, wideSegment } from "./raster.js";
import { addExact, divideExact, dyadic, multiplyExact, sqrtExact } from "./exact.js";
import { circleArcs } from "./circle-path.js";

const decoder = new TextDecoder();
const noCharge = () => {};
const needsCircleArcs = (p, radius, width, ratio) => [p.x, p.y, radius].every(Number.isFinite) &&
  Math.max(Math.abs(p.x), Math.abs(p.y), radius) * ratio > 1e6 && width * ratio <= 65536;
export const stringAt = (c, name, row, offset = 0) => {
  const i = row * (name === "point_markers" ? 4 : 2) + offset;
  return decoder.decode(c.strings.subarray(c[name][i], c[name][i] + c[name][i + 1]));
};

export function textReason(c, i) {
  if (end(c.text_glyphs, i, c.glyph_shapes.length) > (c.text_glyphs[i] ?? 0)) return undefined;
  const unsupported = V.text_flag.vertical | V.text_flag.formatted | V.text_flag.stack_upper | V.text_flag.stack_lower;
  if ((c.text_flags[i] & unsupported) || c.text_boxes[i] > 0 || c.text_aligns[i * 2] > V.horizontal.middle || c.text_aligns[i * 2 + 1] > V.vertical.top) return "text-layout-unsupported";
  return "browser-text";
}

export function pointReason(c, i) {
  if (c.point_markers[i * 4 + 1] || c.point_markers[i * 4 + 3]) return "marker-unresolved";
  if (c.point_style.length && (
    !Object.values(V.point_mark).includes(c.point_style[0]) ||
    !Number.isInteger(c.point_style[1]) || c.point_style[1] < 0 || c.point_style[1] > 3 ||
    !Object.values(V.point_size).includes(c.point_style[2]) || c.point_style[3] < 0
  )) return "point-style-unsupported";
  return undefined;
}

export function project(view, width, height, x, y, originX = 0, originY = 0) {
  // 原点を足してから引くと、原点が大きいときに塊の相対座標の下位桁が消える。
  const dx = (originX - view.center.x) + x, dy = (originY - view.center.y) + y;
  const cos = Math.cos(view.rotation), sin = Math.sin(view.rotation);
  return { x: width / 2 + view.scale * (cos * dx - sin * dy), y: height / 2 - view.scale * (sin * dx + cos * dy) };
}

export function unproject(view, width, height, x, y) {
  const dx = (x - width / 2) / view.scale, dy = -(y - height / 2) / view.scale;
  const cos = Math.cos(view.rotation), sin = Math.sin(view.rotation);
  return { x: view.center.x + cos * dx + sin * dy, y: view.center.y - sin * dx + cos * dy };
}

/** 近傍へ切る閉輪では、投影の積和を丸める前の値も残す。 */
function projectFill(view, size, x, y, ox = 0, oy = 0, local) {
  const p = project(view, size.width, size.height, x, y, ox, oy);
  if (!local && Math.max(Math.abs(ox - view.center.x), Math.abs(oy - view.center.y), Math.abs(x), Math.abs(y)) * view.scale < 1e6) return p;
  const dx = addExact(addExact(dyadic(ox), dyadic(view.center.x), true), local?.x ?? dyadic(x));
  const dy = addExact(addExact(dyadic(oy), dyadic(view.center.y), true), local?.y ?? dyadic(y));
  const cos = dyadic(Math.cos(view.rotation)), sin = dyadic(Math.sin(view.rotation)), scale = dyadic(view.scale);
  const exact = {
    x: addExact(dyadic(size.width / 2), multiplyExact(scale, addExact(multiplyExact(cos, dx), multiplyExact(sin, dy), true))),
    y: addExact(dyadic(size.height / 2), multiplyExact(scale, addExact(multiplyExact(sin, dx), multiplyExact(cos, dy))), true),
  };
  const one = dyadic(1);
  return { x: divideExact(exact.x, one), y: divideExact(exact.y, one), exact };
}

export const lineWidth = (c, unit, settings, size) => Math.max(1 / size.pixelRatio, c.unit_weights[unit] * settings.pixelsPerMillimeter);

export function unitStyle(c, unit, settings, display) {
  const override = display?.colors.get(c.unit_entities[unit]);
  if (override !== undefined) return override;
  const color = c.unit_colors[unit], flags = c.color_flags[color] ?? V.color_flag.unresolved, rgba = color * 4;
  return flags & V.color_flag.foreground ? settings.foreground : flags & V.color_flag.unresolved ? settings.unresolvedColor :
    `rgba(${c.color_rgba[rgba]},${c.color_rgba[rgba + 1]},${c.color_rgba[rgba + 2]},${c.color_rgba[rgba + 3] / 255})`;
}

/** Canvas が正規化した色の、明示された alpha=0 だけを除く。色を調べるための描画はしない。 */
export function transparentStyle(style) {
  if (style === "transparent") return true;
  if (/^#[\da-f]{8}$/i.test(style)) return style.slice(-2) === "00";
  if (/^#[\da-f]{4}$/i.test(style)) return style.slice(-1) === "0";
  const separator = style.includes("/") ? style.lastIndexOf("/") : /^rgba\(/i.test(style) ? style.lastIndexOf(",") : -1;
  if (separator < 0 || !style.endsWith(")")) return false;
  const alpha = style.slice(separator + 1, -1).trim();
  return /^[+-]?(?:\d*\.)?\d+(?:e[+-]?\d+)?%?$/i.test(alpha) && Number(alpha.replace(/%$/, "")) === 0;
}

/** 描画と当たり判定の元の形を CSS px で作る。輪郭の派生データは問い合わせの間だけ持つ。 */
class Path {
  constructor(collect, charge) {
    this.path = new Path2D(); this.contours = []; this.circles = [];
    this.collect = collect; this.charge = charge; this.points = null;
    this.bounds = { min_x: Infinity, min_y: Infinity, max_x: -Infinity, max_y: -Infinity };
    this.work = 0;
  }
  addPoint(p, radius = 0) {
    this.charge(1); this.work++;
    if (!this.collect) return;
    this.bounds.min_x = Math.min(this.bounds.min_x, p.x - radius);
    this.bounds.min_y = Math.min(this.bounds.min_y, p.y - radius);
    this.bounds.max_x = Math.max(this.bounds.max_x, p.x + radius);
    this.bounds.max_y = Math.max(this.bounds.max_y, p.y + radius);
  }
  flush(closed = false) {
    if (this.points) {
      const points = this.points;
      if (closed && points.length > 1 && equal(points[0], points[points.length - 1])) points.pop();
      this.contours.push({ points, closed }); this.points = null;
    }
  }
  move(p) {
    this.flush(); this.path.moveTo(p.x, p.y); this.addPoint(p);
    if (this.collect) this.points = [p];
  }
  line(p) {
    this.path.lineTo(p.x, p.y); this.addPoint(p);
    if (this.collect && !equal(this.points[this.points.length - 1], p)) this.points.push(p);
  }
  close() { this.path.closePath(); this.flush(true); }
  circle(p, radius, exactRadius) {
    this.flush(); this.path.moveTo(p.x + radius, p.y); this.path.arc(p.x, p.y, radius, 0, 2 * Math.PI);
    this.addPoint(p, radius);
    if (this.collect) this.circles.push({ ...p, radius, exactRadius });
  }
  rectangle(minX, minY, maxX, maxY, at = (x, y) => ({ x, y })) {
    this.move(at(minX, minY)); this.line(at(maxX, minY)); this.line(at(maxX, maxY)); this.line(at(minX, maxY)); this.close();
  }
  finish(fill, width = 0) {
    this.flush();
    return { path: this.path, contours: this.contours, circles: this.circles, bounds: this.bounds, work: this.work, fill, width };
  }
}

/** HTML の line styles / trace a path の butt・round・同一 stroke の union を保つ。
 * https://html.spec.whatwg.org/multipage/canvas.html#line-styles
 * 画面内の領域を変えず、Canvas に渡す直線や塗りの座標を画面近傍へ収める。
 */
class StrokePath extends Path {
  constructor(charge, size, width) {
    super(false, charge);
    this.pixelRatio = size.pixelRatio;
    // round join の半径と 2 画素を外へ残す。作った butt の端と join は可視領域へ入らない。
    const margin = width / 2 + 2 / size.pixelRatio;
    this.box = { min_x: -margin, min_y: -margin, max_x: size.width + margin, max_y: size.height + margin };
    this.previous = null; this.tip = null;
    const padding = 2 / size.pixelRatio;
    this.wideBox = { min_x: -padding, min_y: -padding, max_x: size.width + padding, max_y: size.height + padding };
    this.widePath = new Path2D(); this.strokePoints = null; this.radius = width / 2; this.wideCovered = false;
    // B の L1 直径に由来する幅。全元頂点の両円盤が B を覆う場合だけ、一度の局所 stroke に使う。
    this.localWidth = Math.min(width, 2 * (this.wideBox.max_x - this.wideBox.min_x + this.wideBox.max_y - this.wideBox.min_y));
    this.localStroke = Number.isFinite(this.localWidth); this.widePieces = 0;
  }
  certifyPoint(p) {
    if (this.localStroke && (!inside(p, this.wideBox) || !diskCoversBox(p, this.radius, this.wideBox) ||
      !diskCoversBox(p, this.localWidth / 2, this.wideBox))) this.localStroke = false;
  }
  move(p) {
    this.certifyPoint(p);
    this.flushStroke(); if (this.widePath && !this.wideCovered) this.strokePoints = [p];
    this.previous = p; this.tip = null; this.addPoint(p);
  }
  line(p) {
    this.certifyPoint(p);
    if (this.widePath && !this.wideCovered && !equal(this.strokePoints[this.strokePoints.length - 1], p)) this.strokePoints.push(p);
    const segment = clipSegment(this.previous, p, this.box);
    if (segment) {
      const [a, b] = segment;
      if (!this.tip || !equal(this.tip, a)) this.path.moveTo(a.x, a.y);
      this.path.lineTo(b.x, b.y); this.tip = b;
    } else this.tip = null;
    this.previous = p; this.addPoint(p);
  }
  rectangle(minX, minY, maxX, maxY, at = (x, y) => ({ x, y })) {
    const points = [at(minX, minY), at(maxX, minY), at(maxX, maxY), at(minX, maxY)];
    for (const p of points) this.certifyPoint(p);
    if (this.widePath && !this.wideCovered) {
      const polygon = wideClosedStroke(points, this.radius, this.wideBox);
      // 明示的な閉輪だけを扱う。open な輪の先終端が一致しても closed とはみなさない。
      // 凸片の内部 AA を作らず、全包を証明できる場合だけ単一の近傍矩形を塗る。
      this.widePath = polygon ? new Path2D() : null;
      if (polygon) {
        for (let i = 0; i < polygon.length; i++) this.widePath[i ? "lineTo" : "moveTo"](polygon[i].x, polygon[i].y);
        this.widePath.closePath(); this.wideCovered = true;
      }
    }
    this.strokePoints = null;
    const outside = points.findIndex((p) => !inside(p, this.box));
    if (outside < 0) {
      super.move(points[0]); for (let i = 1; i < points.length; i++) super.line(points[i]); super.close();
    } else {
      // 開始点を画面外へ置けば、見える元の角はすべて途中の頂点として join を保てる。
      this.move(points[outside]);
      for (let i = 1; i <= points.length; i++) this.line(points[(outside + i) % points.length]);
    }
    this.previous = null; this.tip = null;
  }
  circle(p, radius, exactRadius) {
    // 同じ part のマーク・円・四角を一度の stroke にし、重なりの alpha を保つ。
    this.widePath = null; this.strokePoints = null; this.localStroke = false;
    if (needsCircleArcs(p, radius, this.radius * 2, this.pixelRatio)) {
      for (const points of circleArcs(p, radius, this.box, 1 / (64 * this.pixelRatio), this.charge, exactRadius)) {
        for (let i = 0; i < points.length; i++) this.path[i ? "lineTo" : "moveTo"](points[i].x, points[i].y);
        if (points.length > 2 && equal(points[0], points[points.length - 1])) this.path.closePath();
      }
      this.addPoint(p, radius);
    } else super.circle(p, radius, exactRadius);
  }
  flushStroke() {
    const points = this.strokePoints; this.strokePoints = null;
    if (!this.widePath || !points?.length) return;
    const append = (polygon) => {
      if (this.wideCovered) return true;
      if (polygon === null) { this.widePath = null; return false; }
      if (!polygon.length) return true;
      if (polygon.coversBox) {
        this.widePath = new Path2D(); this.wideCovered = true;
      } else if (this.widePieces++) {
        // 複数の凸片の内部境界には backend の AA の差が残る。局所 stroke の証明も不成立なら従来へ戻す。
        this.widePath = null; return false;
      }
      for (let i = 0; i < polygon.length; i++) this.widePath[i ? "lineTo" : "moveTo"](polygon[i].x, polygon[i].y);
      if (polygon.length) this.widePath.closePath();
      return true;
    };
    for (let i = 0; i < points.length - 1; i++) {
      if (!append(wideSegment(points[i], points[i + 1], this.radius, this.wideBox)) || this.wideCovered) return;
    }
    for (let i = 1; i < points.length - 1; i++) {
      if (!append(wideJoin(points[i - 1], points[i], points[i + 1], this.radius, this.wideBox)) || this.wideCovered) return;
    }
  }
  finish(fill, width = 0) {
    this.flushStroke();
    const part = super.finish(fill, width);
    if (!this.wideCovered && this.localStroke) return { ...part, width: this.localWidth };
    return this.widePath ? { ...part, path: this.widePath, fill: "nonzero", width: 0 } : part;
  }
}

/** HTML の fill()/clip() の塗り規則と閉じた輪を保ち、画面近傍へ切る。
 * https://html.spec.whatwg.org/multipage/canvas.html#drawing-paths-to-the-canvas
 * 選択も画面との共通部分を調べるので、同じ輪を使って投影で失う幅を避ける。
 */
class FillPath extends Path {
  constructor(charge, size, collect = false) {
    super(collect, charge);
    const margin = 2 / size.pixelRatio;
    this.box = { min_x: -margin, min_y: -margin, max_x: size.width + margin, max_y: size.height + margin };
  }
  move(p) { this.flush(); this.points = [p]; this.addPoint(p); }
  line(p) { this.points.push(p); this.addPoint(p); }
  close() { this.flush(); }
  flush() {
    if (!this.points) return;
    // 一度に一輪だけ持つ。4回の半平面検査は元頂点ごとの固定上限の作業。
    // 描画前の列・共有字形・clip の使用回数の上限で、この一時確保も制限する。
    const points = clipRing(this.points, this.box);
    for (let i = 0; i < points.length; i++) this.path[i ? "lineTo" : "moveTo"](points[i].x, points[i].y);
    if (points.length) this.path.closePath();
    if (this.collect && points.length) {
      const unique = points.filter((p, i) => i === 0 || !equal(p, points[i - 1]));
      if (unique.length > 1 && equal(unique[0], unique[unique.length - 1])) unique.pop();
      this.contours.push({ points: unique, closed: true });
    }
    this.points = null;
  }
}

const equal = (a, b) => a.x === b.x && a.y === b.y;

function pointShape(c, i, chunk, view, size) {
  const marker = pointReason(c, i) === "marker-unresolved";
  const [mark, frame, sizeKind, value] = marker || !c.point_style.length ? [V.point_mark.dot, 0, V.point_size.length, 0] : c.point_style;
  const x = c.point_xy[i * 2], y = c.point_xy[i * 2 + 1];
  const p = projectFill(view, size, x, y, c.chunk_origins[chunk * 2], c.chunk_origins[chunk * 2 + 1], { x: dyadic(x), y: dyadic(y) });
  const one = dyadic(1), half = dyadic(0.5);
  const requested = sizeKind === V.point_size.screen ? dyadic(value * size.height / 200) : multiplyExact(multiplyExact(dyadic(value), dyadic(view.scale)), half);
  const minimum = addExact(requested, one, true).integer < 0n, exactRadius = minimum ? one : requested;
  // binary64 の回転行列も、二軸の長さは同じ。中心と円へ同じ行列を適用し、
  // cos²+sin² の丸めが巨大な半径との相殺で画面内の移動にならないようにする。
  let circleExactRadius = exactRadius;
  if ((frame & V.point_frame.circle) && !minimum && sizeKind === V.point_size.length && view.rotation !== 0) {
    const cos = dyadic(Math.cos(view.rotation)), sin = dyadic(Math.sin(view.rotation));
    const squared = addExact(multiplyExact(cos, cos), multiplyExact(sin, sin));
    if (addExact(squared, one, true).integer !== 0n) circleExactRadius = multiplyExact(exactRadius, sqrtExact(squared));
  }
  return { p, mark, frame, radius: divideExact(exactRadius, one), exactRadius, circleRadius: divideExact(circleExactRadius, one), circleExactRadius };
}

/** 可変の円弧の分割を描画前に確かめる。ここでの計算と実描画の両方を数える。 */
export function validateCircleWork(prepared, settings, view, size, display, limits) {
  if (!prepared) return;
  const { columns: c, order, omitted } = prepared;
  let work = prepared.work ?? 0;
  const charge = (count) => { work += count * 2; valid(Number.isSafeInteger(work) && work <= limits.work, "limits.work"); };
  if (!(c.point_style[1] & V.point_frame.circle)) return;
  for (let chunk = 0; chunk < order.length; chunk++) for (const ordinal of order[chunk].indices) {
    const [kind, i] = order[chunk].locate(ordinal);
    if (kind !== "point" || omitted.has(`point:${i}`) || !unitVisible(c, c.point_units[i], display) || pointReason(c, i) === "marker-unresolved") continue;
    const { p, circleRadius: radius, circleExactRadius } = pointShape(c, i, chunk, view, size);
    const width = lineWidth(c, c.point_units[i], settings, size);
    if (!needsCircleArcs(p, radius, width, size.pixelRatio)) continue;
    const margin = width / 2 + 2 / size.pixelRatio;
    const box = { min_x: -margin, min_y: -margin, max_x: size.width + margin, max_y: size.height + margin };
    circleArcs(p, radius, box, 1 / (64 * size.pixelRatio), charge, circleExactRadius);
  }
}

function vertices(path, xy, start, finish, at, close) {
  for (let v = start; v < finish; v++) {
    const p = at(xy[v * 2], xy[v * 2 + 1]);
    if (v === start) path.move(p); else path.line(p);
  }
  if (close && finish > start) path.close();
}

export function viewportPart(size, charge = noCharge) {
  const path = new Path(true, charge);
  path.rectangle(0, 0, size.width, size.height);
  return path.finish("evenodd");
}

export function* clipParts(c, unit, view, size, collect = false, charge = noCharge) {
  const clip = c.unit_clips[unit];
  if (clip === SCENE_NONE) return;
  const at = (x, y) => projectFill(view, size, x, y);
  for (let p = c.clip_polygons[clip]; p < end(c.clip_polygons, clip, c.polygon_vertices.length); p++) {
    charge(1);
    const path = new FillPath(charge, size, collect);
    if (c.polygon_flags[p] === 1) path.rectangle(0, 0, size.width, size.height);
    vertices(path, c.clip_xy, c.polygon_vertices[p], end(c.polygon_vertices, p, rows(c, "clip_xy")), at, true);
    yield path.finish("evenodd");
  }
}

function fallbackText(ctx, c, i, settings, view, at, collect, charge) {
  charge(c.text_strings[i * 2 + 1]);
  const text = stringAt(c, "text_strings", i), flags = c.text_flags[i];
  const mx = flags & V.text_flag.backward ? -1 : 1, my = flags & V.text_flag.upside_down ? -1 : 1;
  const horizontal = c.text_aligns[i * 2], vertical = c.text_aligns[i * 2 + 1];
  ctx.font = `${c.text_heights[i]}px ${settings.fallbackFont}`;
  ctx.textAlign = ["left", "center", "right", "center"][horizontal];
  ctx.textBaseline = "alphabetic";
  const measure = () => { charge(c.text_strings[i * 2 + 1]); return ctx.measureText(text); };
  let offsetY = 0;
  if (horizontal === V.horizontal.middle) {
    // 垂直指定によらず外形の中央。位置合わせに斜体を当てない（決定 0246）。
    const metrics = measure(); offsetY = (metrics.actualBoundingBoxAscent - metrics.actualBoundingBoxDescent) / 2;
  } else if (vertical !== V.vertical.baseline) {
    const ascent = measure().actualBoundingBoxAscent;
    ctx.textBaseline = ["alphabetic", "bottom", "middle", "top"][vertical];
    offsetY = ascent - measure().actualBoundingBoxAscent; ctx.textBaseline = "alphabetic";
  }
  const p = at(c.text_xy[i * 2], c.text_xy[i * 2 + 1]), angle = -view.rotation - c.text_rotations[i] * Math.PI / 180;
  const shear = -mx * Math.tan(c.text_obliques[i] * Math.PI / 180), stretch = c.text_widths[i] * mx;
  const apply = (target) => {
    target.translate(p.x, p.y); target.rotate(angle); target.scale(view.scale, view.scale);
    target.translate(0, my * offsetY); target.transform(stretch, 0, shear, my, 0, 0);
  };
  if (!collect) return { text, apply };
  // ブラウザーの文字には輪郭がない。字形外形の矩形を同じ変換で写す近似に限る。
  const m = measure(), path = new Path(true, charge), cos = Math.cos(angle), sin = Math.sin(angle);
  const transform = (x, y) => {
    const dx = view.scale * (stretch * x + shear * y), dy = view.scale * my * (y + offsetY);
    return { x: p.x + cos * dx - sin * dy, y: p.y + sin * dx + cos * dy };
  };
  if (stretch !== 0 && m.actualBoundingBoxLeft + m.actualBoundingBoxRight > 0 && m.actualBoundingBoxAscent + m.actualBoundingBoxDescent > 0) {
    path.rectangle(-m.actualBoundingBoxLeft, -m.actualBoundingBoxAscent, m.actualBoundingBoxRight, m.actualBoundingBoxDescent, transform);
  }
  return path.finish("nonzero");
}

export function* primitiveParts(ctx, c, kind, i, chunk, settings, view, size, collect = false, charge = noCharge) {
  const ox = c.chunk_origins[chunk * 2], oy = c.chunk_origins[chunk * 2 + 1];
  const at = (x, y) => project(view, size.width, size.height, x, y, ox, oy);
  const width = lineWidth(c, c[`${kind}_units`][i], settings, size);
  if (kind === "line") {
    const path = collect ? new Path(true, charge) : new StrokePath(charge, size, width);
    const lineAt = (x, y) => projectFill(view, size, x, y, ox, oy);
    vertices(path, c.line_xy, c.line_vertices[i], end(c.line_vertices, i, rows(c, "line_xy")), lineAt, false);
    yield path.finish(null, width);
  } else if (kind === "face") {
    const path = new FillPath(charge, size, collect);
    const fillAt = (x, y) => projectFill(view, size, x, y, ox, oy);
    for (let r = c.face_rings[i]; r < end(c.face_rings, i, c.ring_vertices.length); r++) {
      charge(1); vertices(path, c.face_xy, c.ring_vertices[r], end(c.ring_vertices, r, rows(c, "face_xy")), fillAt, true);
    }
    yield path.finish("evenodd");
  } else if (kind === "text") {
    const first = c.text_glyphs[i] ?? 0, last = c.text_glyphs.length ? end(c.text_glyphs, i, c.glyph_shapes.length) : 0;
    if (last === first) { yield fallbackText(ctx, c, i, settings, view, at, collect, charge); return; }
    for (let g = first; g < last; g++) {
      charge(1);
      const shape = c.glyph_shapes[g], path = new FillPath(charge, size, collect);
      const glyphAt = (x, y) => {
        const gx = c.glyph_axes[g * 4] * x + c.glyph_axes[g * 4 + 2] * y + c.glyph_xy[g * 2];
        const gy = c.glyph_axes[g * 4 + 1] * x + c.glyph_axes[g * 4 + 3] * y + c.glyph_xy[g * 2 + 1];
        const magnitude = Math.max(Math.abs(c.glyph_axes[g * 4] * x), Math.abs(c.glyph_axes[g * 4 + 2] * y), Math.abs(c.glyph_xy[g * 2]),
          Math.abs(c.glyph_axes[g * 4 + 1] * x), Math.abs(c.glyph_axes[g * 4 + 3] * y), Math.abs(c.glyph_xy[g * 2 + 1]));
        const local = magnitude * view.scale < 1e6 ? null : {
          x: addExact(addExact(multiplyExact(dyadic(c.glyph_axes[g * 4]), dyadic(x)), multiplyExact(dyadic(c.glyph_axes[g * 4 + 2]), dyadic(y))), dyadic(c.glyph_xy[g * 2])),
          y: addExact(addExact(multiplyExact(dyadic(c.glyph_axes[g * 4 + 1]), dyadic(x)), multiplyExact(dyadic(c.glyph_axes[g * 4 + 3]), dyadic(y))), dyadic(c.glyph_xy[g * 2 + 1])),
        };
        return projectFill(view, size, gx, gy, ox, oy, local);
      };
      for (let r = c.shape_rings[shape]; r < end(c.shape_rings, shape, c.outline_vertices.length); r++) {
        charge(1); vertices(path, c.outline_xy, c.outline_vertices[r], end(c.outline_vertices, r, rows(c, "outline_xy")), glyphAt, true);
      }
      yield path.finish("nonzero");
    }
  } else if (kind === "point") {
    const { p, mark, frame, radius, exactRadius, circleRadius, circleExactRadius } = pointShape(c, i, chunk, view, size);
    const angle = -view.rotation - c.point_rotations[i] * Math.PI / 180;
    const cos = Math.cos(angle), sin = Math.sin(angle);
    const pointAt = (x, y) => {
      const dx = multiplyExact(dyadic(Math.sign(x)), exactRadius), dy = multiplyExact(dyadic(Math.sign(y)), exactRadius);
      const exact = { x: addExact(p.exact.x, addExact(multiplyExact(dyadic(cos), dx), multiplyExact(dyadic(sin), dy), true)),
        y: addExact(p.exact.y, addExact(multiplyExact(dyadic(sin), dx), multiplyExact(dyadic(cos), dy))) };
      return { x: divideExact(exact.x, dyadic(1)), y: divideExact(exact.y, dyadic(1)), exact };
    };
    if (mark === V.point_mark.dot) {
      const dot = new Path(collect, charge); dot.circle(p, 1 / size.pixelRatio); yield dot.finish("nonzero");
    }
    const path = collect ? new Path(true, charge) : new StrokePath(charge, size, width);
    const segment = (x1, y1, x2, y2) => { path.move(pointAt(x1, y1)); path.line(pointAt(x2, y2)); };
    if (mark === V.point_mark.plus) { segment(-radius, 0, radius, 0); segment(0, -radius, 0, radius); }
    if (mark === V.point_mark.cross) { segment(-radius, -radius, radius, radius); segment(-radius, radius, radius, -radius); }
    if (mark === V.point_mark.tick) segment(0, 0, 0, -radius);
    if (frame & V.point_frame.circle) path.circle(p, circleRadius, circleExactRadius);
    if (frame & V.point_frame.square) path.rectangle(-radius, -radius, radius, radius, pointAt);
    yield path.finish(null, width);
  }
}
