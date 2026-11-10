import { bounds, finite, valid } from "./validate.js";
import { unitVisible } from "./interaction.js";
import { clipParts, primitiveParts, transparentStyle, unitStyle, viewportPart } from "./geometry.js";

const EMPTY = Object.freeze([]);
const EPSILON = 16 * Number.EPSILON;
const lessEqual = (a, b, error = 0) => a <= b || a - b <= error + EPSILON * Math.max(1, Math.abs(a), Math.abs(b));
const between = (n, a, b, error = 0) => lessEqual(a, n, error) && lessEqual(n, b, error);
const pointInBox = (p, b) => between(p.x, b.min_x, b.max_x) && between(p.y, b.min_y, b.max_y);
const overlap = (a, b) => a.min_x <= b.max_x && b.min_x <= a.max_x && a.min_y <= b.max_y && b.min_y <= a.max_y;
const segmentBounds = (a, b) => ({ min_x: Math.min(a.x, b.x), min_y: Math.min(a.y, b.y), max_x: Math.max(a.x, b.x), max_y: Math.max(a.y, b.y) });
const circleBounds = ({ x, y, radius }) => ({ min_x: x - radius, min_y: y - radius, max_x: x + radius, max_y: y + radius });
const sign = (value, scale = 1) => Math.abs(value) <= EPSILON * scale ? 0 : Math.sign(value);
const coordinateSign = (first, second, secondScale) => sign(first) || sign(second, secondScale);

function compareToward(value, boundary, first, second, secondScale, error = 0) {
  return lessEqual(value, boundary, error) && lessEqual(boundary, value, error) ? coordinateSign(first, second, secondScale) : Math.sign(value - boundary);
}

function circleToward(p, center, radius, direction) {
  const dx = p.x - center.x, dy = p.y - center.y, distance = Math.hypot(dx, dy);
  if (!(lessEqual(distance, radius) && lessEqual(radius, distance))) return Math.sign(distance - radius);
  if (distance === 0) return 1;
  const first = dx / distance * direction.x + dy / distance * direction.y;
  if (sign(first)) return sign(first);
  const acceleration = dx / distance * direction.ax + dy / distance * direction.ay;
  const curvature = (direction.x * direction.x + direction.y * direction.y - first * first) / (2 * distance);
  return sign(acceleration + curvature, Math.abs(acceleration) + Math.abs(curvature));
}

function crossAt(a, b, p) {
  const ax = a.x - p.x, ay = a.y - p.y, bx = b.x - p.x, by = b.y - p.y;
  const scale = Math.max(Math.abs(ax), Math.abs(ay), Math.abs(bx), Math.abs(by));
  if (scale === 0) return 0;
  const left = ax / scale * (by / scale), right = ay / scale * (bx / scale);
  const cross = left - right;
  return Math.abs(cross) <= EPSILON * (Math.abs(left) + Math.abs(right)) ? 0 : cross;
}

function segmentContains(a, b, p, radius, direction) {
  const dx = b.x - a.x, dy = b.y - a.y, scale = Math.max(Math.abs(dx), Math.abs(dy));
  if (scale === 0) return false; // butt の端だけを持つ線は描かれない。
  const nx = dx / scale, ny = dy / scale, px = p.x - a.x, py = p.y - a.y;
  const tx = px / scale * nx, ty = py / scale * ny, squaredLength = nx * nx + ny * ny;
  const t = (tx + ty) / squaredLength;
  const length = Math.hypot(nx, ny), normal = (px * ny - py * nx) / length;
  // stroke の角を CSS 座標へ足す 1 回の丸めは ε/2、法線 offset の
  // 正規化と積には既存の 16ε の余裕を使い、各軸から距離へ伝播する。
  // offset がゼロの軸には加算がない。t は線分長で割るので距離の誤差も割る。
  const offsetX = Math.abs(ny) / length * radius, offsetY = Math.abs(nx) / length * radius;
  const errorX = offsetX === 0 ? 0 : Number.EPSILON / 2 * Math.max(Math.abs(a.x), Math.abs(b.x), Math.abs(p.x)) + EPSILON * offsetX;
  const errorY = offsetY === 0 ? 0 : Number.EPSILON / 2 * Math.max(Math.abs(a.y), Math.abs(b.y), Math.abs(p.y)) + EPSILON * offsetY;
  // 内積・外積の 2 積が相殺するときも、結果ではなく各項から丸めを数える。
  const errorT = ((Math.abs(nx) * errorX + Math.abs(ny) * errorY) / scale + Number.EPSILON * (Math.abs(tx) + Math.abs(ty))) / squaredLength;
  const errorNormal = (Math.abs(ny) * errorX + Math.abs(nx) * errorY + Number.EPSILON * (Math.abs(px * ny) + Math.abs(py * nx))) / length;
  if (!direction) return between(t, 0, 1, errorT) && lessEqual(Math.abs(normal), radius, errorNormal);
  const along = direction.x * nx + direction.y * ny, across = (direction.x * ny - direction.y * nx) / length;
  const secondAlong = direction.ax * nx + direction.ay * ny, secondAcross = (direction.ax * ny - direction.ay * nx) / length;
  const secondScale = Math.max(Math.abs(direction.ax), Math.abs(direction.ay));
  return compareToward(t, 0, along, secondAlong, secondScale, errorT) > 0 && compareToward(t, 1, along, secondAlong, secondScale, errorT) < 0 &&
    compareToward(normal, -radius, across, secondAcross, secondScale, errorNormal) > 0 && compareToward(normal, radius, across, secondAcross, secondScale, errorNormal) < 0;
}

function roundJoin(previous, center, next) {
  const ix = center.x - previous.x, iy = center.y - previous.y, ox = next.x - center.x, oy = next.y - center.y;
  const incomingLength = Math.hypot(ix, iy), outgoingLength = Math.hypot(ox, oy);
  if (incomingLength === 0 || outgoingLength === 0) return null;
  const incoming = { x: ix / incomingLength, y: iy / incomingLength }, outgoing = { x: ox / outgoingLength, y: oy / outgoingLength };
  // 同じ向きの共線の線分には、足す扇形がない。折返しでは前方の半円を残す。
  if (incoming.x * outgoing.y === incoming.y * outgoing.x && incoming.x * outgoing.x + incoming.y * outgoing.y > 0) return null;
  return { center, incoming, outgoing };
}

function joinContains({ center, incoming, outgoing }, p, radius, direction) {
  const dx = p.x - center.x, dy = p.y - center.y;
  const before = dx * incoming.x + dy * incoming.y, after = dx * outgoing.x + dy * outgoing.y;
  // 円全体では、短い線分の butt の端からはみ出す。前の線の先かつ次の線の
  // 手前だけが round join の外側の扇形で、残りは線分の矩形が担う。
  if (!direction) return lessEqual(0, before) && lessEqual(after, 0) && lessEqual(Math.hypot(dx, dy), radius);
  const secondScale = Math.max(Math.abs(direction.ax), Math.abs(direction.ay));
  return compareToward(before, 0, direction.x * incoming.x + direction.y * incoming.y,
    direction.ax * incoming.x + direction.ay * incoming.y, secondScale) > 0 &&
    compareToward(after, 0, direction.x * outgoing.x + direction.y * outgoing.y,
      direction.ax * outgoing.x + direction.ay * outgoing.y, secondScale) < 0 && circleToward(p, center, radius, direction) < 0;
}

/** 輪を塗る規則と butt/round の stroke を、描画と同じ座標から調べる。 */
function compile(part, charge) {
  const segments = [], joins = [];
  for (const { points, closed } of part.contours) {
    charge(1 + points.length);
    if (part.fill && (points.length < 3 || !points.slice(2).some((p) => crossAt(points[0], points[1], p) !== 0))) continue;
    const count = closed || part.fill ? points.length : Math.max(0, points.length - 1);
    for (let i = 0; i < count; i++) segments.push({ a: points[i], b: points[(i + 1) % points.length] });
    if (!part.fill && points.length > 1) {
      for (let i = closed ? 0 : 1; i < (closed ? points.length : points.length - 1); i++) {
        charge(1);
        const join = roundJoin(points[(i + points.length - 1) % points.length], points[i], points[(i + 1) % points.length]);
        if (join) joins.push(join);
      }
    }
  }
  const radius = part.width / 2, work = 1 + segments.length + joins.length + part.circles.length;
  const box = { min_x: part.bounds.min_x - radius, min_y: part.bounds.min_y - radius, max_x: part.bounds.max_x + radius, max_y: part.bounds.max_y + radius };
  const contains = (p, direction) => {
    charge(work);
    if (!pointInBox(p, box)) return false;
    if (!part.fill) {
      for (const { a, b } of segments) if (segmentContains(a, b, p, radius, direction)) return true;
      for (const join of joins) if (joinContains(join, p, radius, direction)) return true;
      for (const circle of part.circles) {
        if (direction ? circleToward(p, circle, circle.radius + radius, direction) < 0 &&
          (circle.radius <= radius || circleToward(p, circle, circle.radius - radius, direction) > 0) :
          lessEqual(Math.abs(Math.hypot(p.x - circle.x, p.y - circle.y) - circle.radius), radius)) return true;
      }
      return false;
    }
    let winding = 0;
    const secondScale = direction ? Math.max(Math.abs(direction.ax), Math.abs(direction.ay)) : 0;
    const compareY = (y) => y < p.y ? -1 : y > p.y ? 1 : direction ? -coordinateSign(direction.y, direction.ay, secondScale) : 0;
    for (const { a, b } of segments) {
      let cross = crossAt(a, b, p);
      if (cross === 0) {
        if (!direction && pointInBox(p, segmentBounds(a, b))) return true;
        if (direction) {
          const ex = b.x - a.x, ey = b.y - a.y, scale = Math.max(Math.abs(ex), Math.abs(ey));
          cross = scale === 0 ? 0 : coordinateSign(ex / scale * direction.y - ey / scale * direction.x,
            ex / scale * direction.ay - ey / scale * direction.ax, secondScale);
        }
      }
      const ay = compareY(a.y), by = compareY(b.y);
      if (ay <= 0 && by > 0 && cross > 0) winding++;
      else if (by <= 0 && ay > 0 && cross < 0) winding--;
    }
    for (const circle of part.circles) if (direction ? circleToward(p, circle, circle.radius, direction) < 0 : lessEqual(Math.hypot(p.x - circle.x, p.y - circle.y), circle.radius)) winding++;
    return part.fill === "evenodd" ? winding % 2 !== 0 : winding !== 0;
  };
  function* edges() {
    if (part.fill) {
      for (const edge of segments) { charge(1); yield edge; }
      for (const circle of part.circles) { charge(1); yield circle; }
      return;
    }
    for (const { a, b } of segments) {
      charge(4);
      const dx = b.x - a.x, dy = b.y - a.y, length = Math.hypot(dx, dy);
      if (length === 0) continue;
      const nx = -dy / length * radius, ny = dx / length * radius;
      const corners = [{ x: a.x + nx, y: a.y + ny }, { x: b.x + nx, y: b.y + ny }, { x: b.x - nx, y: b.y - ny }, { x: a.x - nx, y: a.y - ny }];
      for (let i = 0; i < 4; i++) yield { a: corners[i], b: corners[(i + 1) % 4] };
    }
    // 円弧の候補は全円から出し、contains で扇形に絞る。半径方向の二辺は
    // 隣接する線分の端辺に含まれ、query/clip との交点もその列から調べる。
    for (const { center } of joins) { charge(1); yield { ...center, radius }; }
    for (const circle of part.circles) {
      charge(2); yield { ...circle, radius: circle.radius + radius };
      if (circle.radius > radius) yield { ...circle, radius: circle.radius - radius };
    }
  }
  return { bounds: box, contains, edges };
}

function rectangle(rect) {
  const points = [{ x: rect.min_x, y: rect.min_y }, { x: rect.max_x, y: rect.min_y }, { x: rect.max_x, y: rect.max_y }, { x: rect.min_x, y: rect.max_y }];
  return { bounds: rect, center: { x: rect.min_x + (rect.max_x - rect.min_x) / 2, y: rect.min_y + (rect.max_y - rect.min_y) / 2 },
    contains: (p) => pointInBox(p, rect), *edges() { for (let i = 0; i < 4; i++) yield { a: points[i], b: points[(i + 1) % 4] }; } };
}

function circleQuery(point, radius) {
  const circle = { ...point, radius };
  return { bounds: circleBounds(circle), center: point, contains: (p) => lessEqual(Math.hypot(p.x - point.x, p.y - point.y), radius), *edges() { yield circle; } };
}

function intersectSegments(first, second, accept) {
  const rx = first.b.x - first.a.x, ry = first.b.y - first.a.y, sx = second.b.x - second.a.x, sy = second.b.y - second.a.y;
  const qx = second.a.x - first.a.x, qy = second.a.y - first.a.y;
  const scale = Math.max(Math.abs(rx), Math.abs(ry), Math.abs(sx), Math.abs(sy), Math.abs(qx), Math.abs(qy));
  if (scale === 0) return accept(first.a);
  const rX = rx / scale, rY = ry / scale, sX = sx / scale, sY = sy / scale, qX = qx / scale, qY = qy / scale;
  const cross = rX * sY - rY * sX;
  if (cross === 0) return false; // 共線の重なりは、端点かほかの境界との交点で調べられる。
  const t = (qX * sY - qY * sX) / cross, u = (qX * rY - qY * rX) / cross;
  return between(t, 0, 1) && between(u, 0, 1) && accept({ x: first.a.x + t * rx, y: first.a.y + t * ry });
}

function intersectCircleSegment(circle, edge, accept) {
  const ax = edge.a.x - circle.x, ay = edge.a.y - circle.y, dx = edge.b.x - edge.a.x, dy = edge.b.y - edge.a.y;
  const scale = Math.max(Math.abs(ax), Math.abs(ay), Math.abs(dx), Math.abs(dy), circle.radius);
  if (scale === 0) return accept(edge.a);
  const x = ax / scale, y = ay / scale, vx = dx / scale, vy = dy / scale, r = circle.radius / scale;
  const length = vx * vx + vy * vy;
  if (length === 0) return false;
  const center = -(x * vx + y * vy) / length;
  const cross = x * vy - y * vx, gap = r * r - cross * cross / length;
  if (gap < -EPSILON * r * r) return false;
  const offset = Math.sqrt(Math.max(0, gap) / length);
  for (const t of [center - offset, center + offset]) {
    if (between(t, 0, 1) && accept({ x: edge.a.x + t * dx, y: edge.a.y + t * dy })) return true;
  }
  return false;
}

function intersectCircles(first, second, accept) {
  const dx = second.x - first.x, dy = second.y - first.y, distance = Math.hypot(dx, dy);
  if (distance === 0 || !lessEqual(distance, first.radius + second.radius) || !lessEqual(Math.abs(first.radius - second.radius), distance)) return false;
  const scale = Math.max(first.radius, second.radius, distance), r = first.radius / scale, s = second.radius / scale, d = distance / scale;
  const along = (r * r - s * s + d * d) / (2 * d), height = Math.sqrt(Math.max(0, r * r - along * along)) * scale;
  const x = first.x + dx / distance * along * scale, y = first.y + dy / distance * along * scale;
  return accept({ x: x - dy / distance * height, y: y + dx / distance * height }) || accept({ x: x + dy / distance * height, y: y - dx / distance * height });
}

/** 問い合わせとの接触は残し、形と全 clip に共通する可視領域のない境界だけを除く。 */
function visibleNear(point, constraints, charge) {
  const angles = [], circles = [];
  for (const item of constraints) for (const edge of item.edges()) {
    charge(1);
    if (edge.a) {
      if (crossAt(edge.a, edge.b, point) !== 0 || !pointInBox(point, segmentBounds(edge.a, edge.b))) continue;
      for (const vertex of [edge.a, edge.b]) if (vertex.x !== point.x || vertex.y !== point.y) angles.push(Math.atan2(vertex.y - point.y, vertex.x - point.x));
    } else {
      const distance = Math.hypot(point.x - edge.x, point.y - edge.y);
      if (!(lessEqual(distance, edge.radius) && lessEqual(edge.radius, distance)) || edge.radius === 0) continue;
      const tangent = Math.atan2(point.x - edge.x, -(point.y - edge.y));
      angles.push(tangent, tangent > 0 ? tangent - Math.PI : tangent + Math.PI); circles.push(edge);
    }
  }
  if (angles.length === 0) return true;
  charge(angles.length * Math.max(1, Math.ceil(Math.log2(angles.length + 1))));
  angles.sort((a, b) => a - b);
  const inside = (angle, bend = 0) => {
    const x = Math.cos(angle), y = Math.sin(angle), direction = { x, y, ax: -y * bend, ay: x * bend };
    return constraints.every((item) => item.contains(point, direction));
  };
  // p + εd の winding と stroke の不等式を符号で評価する。固定の距離だけ
  // 座標をずらさないので、狭い領域を飛び越えず、相殺した輪も拾わない。
  for (let i = 0; i < angles.length; i++) {
    const begin = angles[i], finish = i + 1 < angles.length ? angles[i + 1] : angles[0] + 2 * Math.PI;
    if (begin !== finish && inside(begin + (finish - begin) / 2)) return true;
  }
  // 円の穴に接する直線の切り抜きでは、可視側が接線方向に狭まることがある。
  // p + εd + ε²kn の二次の項も調べ、円の外側だけが残る接点を落とさない。
  if (circles.length) for (const angle of angles) {
    charge(1);
    const dx = Math.cos(angle), dy = Math.sin(angle), bends = [0];
    for (const circle of circles) {
      charge(1);
      const nx = (point.x - circle.x) / circle.radius, ny = (point.y - circle.y) / circle.radius;
      if (sign(nx * dx + ny * dy) === 0) bends.push(-1 / (2 * circle.radius * (-nx * dy + ny * dx)));
    }
    charge(bends.length * Math.max(1, Math.ceil(Math.log2(bends.length + 1))));
    bends.sort((a, b) => a - b);
    const first = bends[0], last = bends[bends.length - 1];
    if (inside(angle, first - Math.max(1, Math.abs(first))) || inside(angle, last + Math.max(1, Math.abs(last)))) return true;
    for (let i = 1; i < bends.length; i++) if (bends[i - 1] !== bends[i] && inside(angle, bends[i - 1] + (bends[i] - bends[i - 1]) / 2)) return true;
  }
  return false;
}

function intersects(shape, clips, query, charge) {
  const constraints = [shape, ...clips], box = { ...query.bounds };
  for (const item of constraints) {
    charge(1);
    box.min_x = Math.max(box.min_x, item.bounds.min_x); box.min_y = Math.max(box.min_y, item.bounds.min_y);
    box.max_x = Math.min(box.max_x, item.bounds.max_x); box.max_y = Math.min(box.max_y, item.bounds.max_y);
  }
  if (box.min_x > box.max_x || box.min_y > box.max_y) return false;
  const accept = (point) => {
    charge(1);
    return Number.isFinite(point.x) && Number.isFinite(point.y) && pointInBox(point, box) && query.contains(point) &&
      constraints.every((item) => item.contains(point)) && visibleNear(point, constraints, charge);
  };
  if (accept(query.center)) return true;
  const corners = [{ x: box.min_x, y: box.min_y }, { x: box.max_x, y: box.min_y }, { x: box.max_x, y: box.max_y }, { x: box.min_x, y: box.max_y }];
  for (const point of corners) if (accept(point)) return true;
  const edges = [];
  // 穴と、複数の内外の切り抜きも含む境界の配置を調べる。外接矩形だけで当たりにしない。
  for (const item of [...constraints, query]) for (const edge of item.edges()) {
    charge(1);
    const edgeBox = edge.a ? segmentBounds(edge.a, edge.b) : circleBounds(edge);
    if (!overlap(edgeBox, box)) continue;
    if (edge.a) {
      if (accept(edge.a) || accept(edge.b)) return true;
    } else {
      for (const [dx, dy] of [[1, 0], [0, 1], [-1, 0], [0, -1]]) {
        if (accept({ x: edge.x + dx * edge.radius, y: edge.y + dy * edge.radius })) return true;
      }
    }
    edges.push({ edge, bounds: edgeBox });
  }
  for (let i = 0; i < edges.length; i++) for (let j = 0; j < i; j++) {
    charge(1);
    const first = edges[i], second = edges[j];
    if (!overlap(first.bounds, second.bounds)) continue;
    const a = first.edge, b = second.edge;
    if (a.a && b.a ? intersectSegments(a, b, accept) : a.a ? intersectCircleSegment(b, a, accept) : b.a ? intersectCircleSegment(a, b, accept) : intersectCircles(a, b, accept)) return true;
  }
  return false;
}

function hit(ctx, prepared, settings, view, size, display, index, limits, query) {
  if (!prepared) return EMPTY;
  let work = 0;
  const charge = (count) => { work += count; valid(Number.isSafeInteger(work) && work <= limits.work, "limits.work"); };
  const viewport = compile(viewportPart(size, charge), charge), result = [], seen = new Set();
  const { columns: c, order, omitted } = prepared;
  ctx.save();
  try {
    for (let chunk = order.length - 1; chunk >= 0; chunk--) {
      const group = order[chunk];
      for (let ordinal = group.indices.length - 1; ordinal >= 0; ordinal--) {
        charge(1);
        const [kind, i] = group.locate(group.indices[ordinal]), unit = c[`${kind}_units`][i];
        const element = index.elements[c.unit_entities[unit]];
        if (seen.has(element) || omitted.has(`${kind}:${i}`) || !unitVisible(c, unit, display) || transparentStyle(unitStyle(c, unit, settings, display))) continue;
        let clips;
        for (const part of primitiveParts(ctx, c, kind, i, chunk, settings, view, size, true, charge)) {
          const shape = compile(part, charge);
          if (!overlap(shape.bounds, query.bounds) || !overlap(shape.bounds, viewport.bounds)) continue;
          if (!clips) clips = [viewport, ...Array.from(clipParts(c, unit, view, size, true, charge), (clip) => compile(clip, charge))];
          if (intersects(shape, clips, query, charge)) {
            valid(result.length < limits.stateItems, "limits.stateItems");
            result.push(element); seen.add(element); break;
          }
        }
      }
    }
  } finally { ctx.restore(); }
  return Object.freeze(result);
}

/** 吸着の候補点が、描画と同じ形とすべての切り抜きに残るかを調べる。 */
export function visiblePointTest(ctx, prepared, settings, view, size, display, kind, i, chunk, charge) {
  const { columns: c, omitted } = prepared, unit = c[`${kind}_units`][i];
  if (omitted.has(`${kind}:${i}`) || !unitVisible(c, unit, display) || transparentStyle(unitStyle(c, unit, settings, display))) return null;
  let groups;
  return (point) => {
    // 半径内の候補が現れるまで、輪郭と切り抜きの派生データを作らない。
    if (!groups) {
      const clips = [compile(viewportPart(size, charge), charge), ...Array.from(clipParts(c, unit, view, size, true, charge), (part) => compile(part, charge))];
      groups = Array.from(primitiveParts(ctx, c, kind, i, chunk, settings, view, size, true, charge), (part) => {
        charge(1 + clips.length);
        return [compile(part, charge), ...clips];
      });
    }
    for (const constraints of groups) {
      charge(constraints.length);
      if (constraints.every((item) => pointInBox(point, item.bounds) && item.contains(point)) && visibleNear(point, constraints, charge)) return true;
    }
    return false;
  };
}

export function hitPoint(ctx, prepared, settings, view, size, display, index, limits, point, tolerance) {
  finite(point?.x, "point.x"); finite(point?.y, "point.y"); finite(tolerance, "tolerance"); valid(tolerance >= 0, "tolerance");
  return hit(ctx, prepared, settings, view, size, display, index, limits, circleQuery(point, tolerance));
}

export function hitRect(ctx, prepared, settings, view, size, display, index, limits, rect) {
  return hit(ctx, prepared, settings, view, size, display, index, limits, rectangle(bounds(rect)));
}
