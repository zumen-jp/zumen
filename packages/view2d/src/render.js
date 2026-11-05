import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { end, rows, valid } from "./validate.js";

const decoder = new TextDecoder();
const stringAt = (c, name, row, offset = 0) => {
  const i = row * (name === "point_markers" ? 4 : 2) + offset;
  return decoder.decode(c.strings.subarray(c[name][i], c[name][i] + c[name][i + 1]));
};

function textReason(c, i) {
  if (end(c.text_glyphs, i, c.glyph_shapes.length) > (c.text_glyphs[i] ?? 0)) return undefined;
  const unsupported = V.text_flag.vertical | V.text_flag.formatted | V.text_flag.stack_upper | V.text_flag.stack_lower;
  if ((c.text_flags[i] & unsupported) || c.text_boxes[i] > 0 || c.text_aligns[i * 2] > V.horizontal.middle || c.text_aligns[i * 2 + 1] > V.vertical.top) {
    return "text-layout-unsupported";
  }
  return "browser-text";
}

function pointReason(c, i) {
  if (c.point_markers[i * 4 + 1] || c.point_markers[i * 4 + 3]) return "marker-unresolved";
  if (c.point_style.length && (
    !Object.values(V.point_mark).includes(c.point_style[0]) ||
    !Number.isInteger(c.point_style[1]) || c.point_style[1] < 0 || c.point_style[1] > 3 ||
    !Object.values(V.point_size).includes(c.point_style[2]) || c.point_style[3] < 0
  )) return "point-style-unsupported";
  return undefined;
}

/** 代替と未描画の一覧を描画前に作る。要素 ID と配置の道筋はシーンの値を保つ。 */
export function reportOf(prepared, limits) {
  const { columns: c, order } = prepared;
  const issues = [];
  const sources = new Map();
  const omitted = new Set();
  const drawn = { lines: 0, faces: 0, texts: 0, points: 0, images: 0, glyphs: 0 };
  function issue(kind, index, unit, reason, disposition) {
    valid(issues.length < limits.issues, "limits.issues");
    const entity = c.unit_entities[unit];
    if (!sources.has(entity)) {
      const path = [];
      for (let i = c.entity_paths[entity]; i < end(c.entity_paths, entity, rows(c, "step_ids")); i++) {
        path.push(Object.freeze({ id: stringAt(c, "step_ids", i), column: c.step_cells[i * 2], row: c.step_cells[i * 2 + 1] }));
      }
      sources.set(entity, Object.freeze({ id: stringAt(c, "entity_ids", entity), path: Object.freeze(path) }));
    }
    issues.push(Object.freeze({ element: sources.get(entity), unit, primitive: Object.freeze({ kind, index }), reason, disposition }));
  }
  for (const { indices, locate } of order) for (const ordinal of indices) {
    const [kind, i] = locate(ordinal);
    const unit = c[`${kind}_units`][i];
    const clip = c.unit_clips[unit];
    let reason;
    if (clip !== SCENE_NONE) {
      for (let p = c.clip_polygons[clip]; p < end(c.clip_polygons, clip, c.polygon_vertices.length); p++) {
        if (c.polygon_flags[p] > 1) { reason = "clip-unsupported"; break; }
      }
    }
    reason ??= kind === "image" ? "resource-unresolved" : kind === "text" ? textReason(c, i) : kind === "point" ? pointReason(c, i) : undefined;
    const skip = reason && reason !== "browser-text" && reason !== "marker-unresolved";
    if (reason) issue(kind, i, unit, reason, skip ? "omitted" : "fallback");
    if (skip) {
      omitted.add(`${kind}:${i}`);
      continue;
    }
    const color = c.unit_colors[unit];
    if (color === SCENE_NONE || ((c.color_flags[color] & V.color_flag.unresolved) && !(c.color_flags[color] & V.color_flag.foreground))) {
      issue(kind, i, unit, "color-unresolved", "fallback");
    }
    drawn[`${kind}s`]++;
    if (kind === "text" && c.text_glyphs.length) drawn.glyphs += end(c.text_glyphs, i, c.glyph_shapes.length) - c.text_glyphs[i];
  }
  return { report: Object.freeze({ drawn: Object.freeze(drawn), issues: Object.freeze(issues) }), omitted };
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

export function paint(ctx, prepared, settings, view, size) {
  const { width, height, pixelRatio } = size;
  ctx.setTransform(pixelRatio, 0, 0, pixelRatio, 0, 0);
  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = settings.background;
  ctx.fillRect(0, 0, width, height);
  if (!prepared) return;
  const { columns: c, order, omitted } = prepared;
  ctx.lineCap = "butt";
  ctx.lineJoin = "round";
  ctx.setLineDash([]);

  const absolute = (x, y) => project(view, width, height, x, y);
  function path(xy, start, finish, at, close) {
    for (let v = start; v < finish; v++) {
      const p = at(xy[v * 2], xy[v * 2 + 1]);
      if (v === start) ctx.moveTo(p.x, p.y); else ctx.lineTo(p.x, p.y);
    }
    if (close && finish > start) ctx.closePath();
  }
  function clipUnit(unit) {
    const clip = c.unit_clips[unit];
    if (clip === SCENE_NONE) return;
    for (let p = c.clip_polygons[clip]; p < end(c.clip_polygons, clip, c.polygon_vertices.length); p++) {
      ctx.beginPath();
      if (c.polygon_flags[p] === 1) ctx.rect(0, 0, width, height);
      path(c.clip_xy, c.polygon_vertices[p], end(c.polygon_vertices, p, rows(c, "clip_xy")), absolute, true);
      ctx.clip("evenodd");
    }
  }
  for (let chunk = 0; chunk < order.length; chunk++) {
    const ox = c.chunk_origins[chunk * 2], oy = c.chunk_origins[chunk * 2 + 1];
    const at = (x, y) => project(view, width, height, x, y, ox, oy);
    for (const ordinal of order[chunk].indices) {
      const [kind, i] = order[chunk].locate(ordinal);
      if (omitted.has(`${kind}:${i}`)) continue;
      const unit = c[`${kind}_units`][i];
      ctx.save();
      try {
        clipUnit(unit);
        const color = c.unit_colors[unit];
        const flags = c.color_flags[color] ?? V.color_flag.unresolved;
        const rgba = color * 4;
        const style = (flags & V.color_flag.foreground) ? settings.foreground : (flags & V.color_flag.unresolved) ? settings.unresolvedColor :
          `rgba(${c.color_rgba[rgba]},${c.color_rgba[rgba + 1]},${c.color_rgba[rgba + 2]},${c.color_rgba[rgba + 3] / 255})`;
        ctx.fillStyle = style;
        ctx.strokeStyle = style;
        ctx.lineWidth = Math.max(1 / pixelRatio, c.unit_weights[unit] * settings.pixelsPerMillimeter);
        if (kind === "line") {
          ctx.beginPath();
          path(c.line_xy, c.line_vertices[i], end(c.line_vertices, i, rows(c, "line_xy")), at, false);
          ctx.stroke();
        } else if (kind === "face") {
          ctx.beginPath();
          for (let r = c.face_rings[i]; r < end(c.face_rings, i, c.ring_vertices.length); r++) {
            path(c.face_xy, c.ring_vertices[r], end(c.ring_vertices, r, rows(c, "face_xy")), at, true);
          }
          ctx.fill("evenodd");
        } else if (kind === "text") {
          const first = c.text_glyphs[i] ?? 0;
          const last = c.text_glyphs.length ? end(c.text_glyphs, i, c.glyph_shapes.length) : 0;
          if (last > first) {
            for (let g = first; g < last; g++) {
              const shape = c.glyph_shapes[g];
              const glyphAt = (x, y) => at(c.glyph_axes[g * 4] * x + c.glyph_axes[g * 4 + 2] * y + c.glyph_xy[g * 2],
                c.glyph_axes[g * 4 + 1] * x + c.glyph_axes[g * 4 + 3] * y + c.glyph_xy[g * 2 + 1]);
              ctx.beginPath();
              for (let r = c.shape_rings[shape]; r < end(c.shape_rings, shape, c.outline_vertices.length); r++) {
                path(c.outline_xy, c.outline_vertices[r], end(c.outline_vertices, r, rows(c, "outline_xy")), glyphAt, true);
              }
              ctx.fill("nonzero");
            }
          } else {
            const p = at(c.text_xy[i * 2], c.text_xy[i * 2 + 1]);
            const flags = c.text_flags[i];
            const mx = (flags & V.text_flag.backward) ? -1 : 1;
            const my = (flags & V.text_flag.upside_down) ? -1 : 1;
            const text = stringAt(c, "text_strings", i);
            const horizontal = c.text_aligns[i * 2];
            const vertical = c.text_aligns[i * 2 + 1];
            ctx.font = `${c.text_heights[i]}px ${settings.fallbackFont}`;
            ctx.textAlign = ["left", "center", "right", "center"][horizontal];
            ctx.textBaseline = "alphabetic";
            let offsetY = 0;
            if (horizontal === V.horizontal.middle) {
              // コアの layout::place と同じく、垂直指定によらず外形の中央。位置合わせには斜体を当てない（決定 0246）。
              const metrics = ctx.measureText(text);
              offsetY = (metrics.actualBoundingBoxAscent - metrics.actualBoundingBoxDescent) / 2;
            } else if (vertical !== V.vertical.baseline) {
              // ほかの垂直指定も、Canvas の基準線の差を先に移し、斜体で横へずらさない。
              const ascent = ctx.measureText(text).actualBoundingBoxAscent;
              ctx.textBaseline = ["alphabetic", "bottom", "middle", "top"][vertical];
              offsetY = ascent - ctx.measureText(text).actualBoundingBoxAscent;
              ctx.textBaseline = "alphabetic";
            }
            ctx.translate(p.x, p.y);
            ctx.rotate(-view.rotation - c.text_rotations[i] * Math.PI / 180);
            ctx.scale(view.scale, view.scale);
            ctx.translate(0, my * offsetY);
            // 斜体にも左右反転を当てる。Canvas の文字の Y 軸は下向きなので、せん断の符号を逆にする。
            ctx.transform(c.text_widths[i] * mx, 0, -mx * Math.tan(c.text_obliques[i] * Math.PI / 180), my, 0, 0);
            ctx.fillText(text, 0, 0);
          }
        } else if (kind === "point") {
          const p = at(c.point_xy[i * 2], c.point_xy[i * 2 + 1]);
          const marker = pointReason(c, i) === "marker-unresolved";
          const [mark, frame, sizeKind, value] = marker || !c.point_style.length ? [V.point_mark.dot, 0, V.point_size.length, 0] : c.point_style;
          const diameter = sizeKind === V.point_size.screen ? value * height / 100 : value * view.scale;
          const radius = Math.max(1, diameter / 2);
          ctx.translate(p.x, p.y);
          ctx.rotate(-view.rotation - c.point_rotations[i] * Math.PI / 180);
          ctx.beginPath();
          if (mark === V.point_mark.dot) { ctx.arc(0, 0, 1 / pixelRatio, 0, 2 * Math.PI); ctx.fill(); ctx.beginPath(); }
          if (mark === V.point_mark.plus) { ctx.moveTo(-radius, 0); ctx.lineTo(radius, 0); ctx.moveTo(0, -radius); ctx.lineTo(0, radius); }
          if (mark === V.point_mark.cross) { ctx.moveTo(-radius, -radius); ctx.lineTo(radius, radius); ctx.moveTo(-radius, radius); ctx.lineTo(radius, -radius); }
          if (mark === V.point_mark.tick) { ctx.moveTo(0, 0); ctx.lineTo(0, -radius); }
          if (frame & V.point_frame.circle) { ctx.moveTo(radius, 0); ctx.arc(0, 0, radius, 0, 2 * Math.PI); }
          if (frame & V.point_frame.square) ctx.rect(-radius, -radius, 2 * radius, 2 * radius);
          ctx.stroke();
        }
      } finally {
        ctx.restore();
      }
    }
  }
}
