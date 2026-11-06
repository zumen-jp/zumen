import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { end, valid } from "./validate.js";
import { unitVisible } from "./interaction.js";
import { clipParts, pointReason, primitiveParts, textReason, unitStyle } from "./geometry.js";
export { project, unproject } from "./geometry.js";

/** 代替と未描画の一覧を描画前に作る。要素 ID と配置の道筋はシーンの値を保つ。 */
export function reportOf(prepared, limits, display, elements, underlays) {
  const { columns: c, order } = prepared;
  const issues = [];
  const omitted = new Set();
  const drawn = { lines: 0, faces: 0, texts: 0, points: 0, images: 0, glyphs: 0 };
  function issue(kind, index, unit, reason, disposition) {
    valid(issues.length < limits.issues, "limits.issues");
    const element = elements[c.unit_entities[unit]];
    issues.push(Object.freeze({ element, unit, primitive: Object.freeze({ kind, index }), reason, disposition }));
  }
  for (const { indices, locate } of order) for (const ordinal of indices) {
    const [kind, i] = locate(ordinal);
    const unit = c[`${kind}_units`][i];
    if (!unitVisible(c, unit, display)) continue;
    const clip = c.unit_clips[unit];
    let reason;
    if (clip !== SCENE_NONE) {
      for (let p = c.clip_polygons[clip]; p < end(c.clip_polygons, clip, c.polygon_vertices.length); p++) {
        if (c.polygon_flags[p] > 1) { reason = "clip-unsupported"; break; }
      }
    }
    reason ??= kind === "image" ? (underlays ? underlays.reason(i) : "resource-unresolved") : kind === "text" ? textReason(c, i) : kind === "point" ? pointReason(c, i) : undefined;
    const skip = reason && reason !== "browser-text" && reason !== "marker-unresolved";
    if (reason) issue(kind, i, unit, reason, skip ? "omitted" : "fallback");
    if (skip) {
      omitted.add(`${kind}:${i}`);
      continue;
    }
    const color = c.unit_colors[unit];
    if (kind !== "image" && (color === SCENE_NONE || ((c.color_flags[color] & V.color_flag.unresolved) && !(c.color_flags[color] & V.color_flag.foreground)))) {
      issue(kind, i, unit, "color-unresolved", "fallback");
    }
    drawn[`${kind}s`]++;
    if (kind === "text" && c.text_glyphs.length) drawn.glyphs += end(c.text_glyphs, i, c.glyph_shapes.length) - c.text_glyphs[i];
  }
  return { report: Object.freeze({ drawn: Object.freeze(drawn), issues: Object.freeze(issues) }), omitted };
}

export function paint(ctx, prepared, settings, view, size, display, underlays) {
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

  for (let chunk = 0; chunk < order.length; chunk++) {
    for (const ordinal of order[chunk].indices) {
      const [kind, i] = order[chunk].locate(ordinal);
      if (omitted.has(`${kind}:${i}`)) continue;
      const unit = c[`${kind}_units`][i];
      if (!unitVisible(c, unit, display)) continue;
      ctx.save();
      try {
        for (const part of clipParts(c, unit, view, size)) ctx.clip(part.path, part.fill);
        if (kind === "image") { underlays?.draw(ctx, i); continue; }
        const style = unitStyle(c, unit, settings, display);
        ctx.fillStyle = style;
        ctx.strokeStyle = style;
        for (const part of primitiveParts(ctx, c, kind, i, chunk, settings, view, size)) {
          if (part.text !== undefined) { part.apply(ctx); ctx.fillText(part.text, 0, 0); }
          else if (part.fill) ctx.fill(part.path, part.fill);
          else { ctx.lineWidth = part.width; ctx.stroke(part.path); }
        }
      } finally {
        ctx.restore();
      }
    }
  }
}
