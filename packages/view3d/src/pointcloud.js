// 点群の 3D のシーン（ZMS3 1.1 の点の区画）を、表示中の建物・地物と同じビューに重ねる任意のアダプター（docs/view3d.md#地面の-2d地物点群）。
// 列は元の blob から借用し、八分木の節（塊）を単位に表示範囲と予算で描く節を選ぶ。点ごとの永続 ID は仮定しない。
import * as THREE from "three";
import { readScene3D } from "@zumen-jp/zumen";
import { cross, dot, fail, finite, invalid, isObject, normalize, sub } from "./common.js";

const KEYS = new Set(["id", "input", "load", "color", "pointSize", "maxPoints", "maxWork", "scope", "priority", "readLimits"]);
const COLORS = new Set(["auto", "rgba", "intensity", "classification", "none"]);
/** 分類の表示色。表示だけの既定で、利用側は color を none にして別の描き方を選べる。 */
const CLASS_COLORS = [
  [160, 160, 160], [200, 200, 200], [170, 120, 70], [110, 190, 90], [60, 150, 60], [30, 110, 30], [230, 90, 70],
  [255, 0, 255], [255, 220, 0], [60, 120, 230], [140, 140, 140], [90, 90, 90], [255, 160, 0],
];
const NONE_COLOR = 0x888888;
/** 頂点色の sRGB を線形へ戻す。点群の色は sRGB の値として持つ。 */
const SRGB_TO_LINEAR = `#include <color_vertex>
#if defined( USE_COLOR_ALPHA ) || defined( USE_COLOR )
  vColor.rgb = mix( vColor.rgb / 12.92, pow( ( vColor.rgb + 0.055 ) / 1.055, vec3( 2.4 ) ), step( vec3( 0.04045 ), vColor.rgb ) );
#endif`;

/** EPSG の 3 つの書き方を番号にそろえる。ほかの名前はそのまま比べる。 */
function crsKey(name) {
  if (typeof name !== "string" || name === "") return null;
  const match = /^(?:EPSG:|urn:ogc:def:crs:EPSG::|http:\/\/www\.opengis\.net\/def\/crs\/EPSG\/0\/)(\d+)$/.exec(name);
  return match === null ? `name:${name}` : `EPSG:${match[1]}`;
}

const isMetre = (unit) => isObject(unit) && unit.name === "METRE" && unit.prefix === undefined && unit.scale === undefined;

/** 点群の地図の座標（m）を表示中の文書の座標へ写す。地図への変換は docs/formats/crs.md の MapConversion の逆。 */
function placement(viewerScene, result) {
  if (viewerScene === null) return { reason: "no-scene" };
  if (!isMetre(result.unit)) return { reason: "unit-unknown" };
  if (!isMetre(viewerScene.unit)) return { reason: "unit-not-metre" };
  const cloudMap = result.map, map = viewerScene.map;
  if (!isObject(cloudMap) || crsKey(cloudMap.crs_name) === null) return { reason: "crs-unknown" };
  if (cloudMap.eastings !== 0 || cloudMap.northings !== 0 || cloudMap.height !== 0 || (cloudMap.scale ?? 1) !== 1
    || (cloudMap.x_axis_abscissa ?? 1) !== 1 || (cloudMap.x_axis_ordinate ?? 0) !== 0) return { reason: "map-unsupported" };
  if (!isObject(map)) return { reason: "map-unknown" };
  if (crsKey(map.crs_name) === null) return { reason: "crs-unknown" };
  if (crsKey(map.crs_name) !== crsKey(cloudMap.crs_name)) return { reason: "crs-mismatch" };
  const heightCloud = result.height_crs ?? null, heightViewer = viewerScene.heightCrs ?? null;
  if (heightCloud !== null && heightViewer !== null && heightCloud !== heightViewer) return { reason: "height-mismatch" };
  const scale = map.scale ?? 1;
  const abscissa = map.x_axis_abscissa ?? 1, ordinate = map.x_axis_ordinate ?? 0, norm = Math.hypot(abscissa, ordinate);
  if (!(scale > 0) || !Number.isFinite(scale) || !(norm > 0) || !Number.isFinite(norm)) return { reason: "map-unsupported" };
  const a = abscissa / norm, b = ordinate / norm;
  // 地図 = scale·R·p + t、R は (a, b) の向きへの反時計回り。逆は p = Rᵀ(地図 − t) / scale。
  const linear = [[a / scale, b / scale, 0], [-b / scale, a / scale, 0], [0, 0, 1 / scale]];
  const translation = [map.eastings, map.northings, map.height];
  const apply = (m) => [0, 1, 2].map((r) => dot(linear[r], m));
  return { reason: null, linear, toViewer: (mapPoint) => apply(sub(mapPoint, translation)), apply,
    height: heightCloud !== null && heightViewer !== null ? "same" : "unverified" };
}

function readCount(value, fallback, message, max = Number.MAX_SAFE_INTEGER) {
  if (value === undefined) return fallback;
  if (!Number.isSafeInteger(value) || value <= 0 || value > max) invalid(message);
  return value;
}

function readInput(value) {
  if (!isObject(value) || !isObject(value.result) || !isObject(value.document)) invalid("input は { blob, result, document }");
  if (typeof value.document.doc !== "string" || value.document.doc !== value.result.doc) invalid("応答と文書の食い違い");
  return value;
}

/** 表示中の建物・地物の地図の座標系に合わせて点群を重ねる。単位・座標系・高さの基準が分からないか食い違えば描かず理由を返す。 */
export function attachPointCloud(viewer, options) {
  if (!isObject(options)) invalid("options はオブジェクト");
  for (const key of Object.keys(options)) if (!KEYS.has(key)) invalid(`未知の項目: ${key}`);
  if (typeof options.id !== "string") invalid("id は文字列");
  if (options.load !== undefined && typeof options.load !== "function") invalid("load は関数");
  const colorMode = options.color ?? "auto";
  if (!COLORS.has(colorMode)) invalid("color は auto・rgba・intensity・classification・none");
  const pointSize = options.pointSize === undefined ? 2 : finite(options.pointSize, "pointSize は有限値");
  if (pointSize <= 0 || pointSize > 64) invalid("pointSize は 0 より大きく 64 以下の CSS px");
  const maxPoints = readCount(options.maxPoints, 2000000, "maxPoints は正の整数");
  const maxWork = readCount(options.maxWork, 10000000, "maxWork は正の整数");
  const scene3d = viewer.getState().scene;
  if (scene3d === null) fail("no-scene", "点群を重ねるには 3D のシーンが要る");
  const unit = { name: scene3d.unit.name };
  if (scene3d.unit.prefix !== undefined) unit.prefix = scene3d.unit.prefix;
  if (scene3d.unit.scale !== undefined) unit.scale = scene3d.unit.scale;
  const root = new THREE.Group();
  const material = new THREE.PointsMaterial({ size: pointSize, sizeAttenuation: false, vertexColors: colorMode !== "none" });
  material.onBeforeCompile = (shader) => { shader.vertexShader = shader.vertexShader.replace("#include <color_vertex>", SRGB_TO_LINEAR); };
  material.customProgramCacheKey = () => "zumen-points-srgb";
  if (colorMode === "none") material.color.setHex(NONE_COLOR);

  let alive = true, current = null, place = null, reason = "empty", controller = null, generation = 0;
  let drawn = { points: 0, chunks: 0 }, skipped = { outside: 0, budget: 0 }, colorsUsed = null, displayBytes = 0;
  const requests = { applied: 0, stale: 0, cancelled: 0, failed: 0 };
  let lastViews = null, lastScene = null, lastSize = null, firstFailure = null;
  const unsubscribes = [];

  const clear = () => {
    for (const child of [...root.children]) { root.remove(child); child.geometry.dispose(); }
    current = null; drawn = { points: 0, chunks: 0 }; skipped = { outside: 0, budget: 0 }; colorsUsed = null; displayBytes = 0;
  };

  const release = () => {
    if (!alive) return;
    alive = false;
    controller?.abort("removed");
    const errors = [];
    for (const off of unsubscribes.splice(0)) { try { off(); } catch (error) { errors.push(error); } }
    clear();
    material.dispose();
    if (errors.length > 0) throw errors[0];
  };

  /** 塊の行列を、点群の局所座標から登録の原点の相対座標へ置き直す。 */
  const placeChunks = () => {
    for (const child of root.children) {
      const { mapOrigin } = child.userData.zumen;
      const linear = place.linear;
      const at = place.toViewer(mapOrigin);
      child.matrix.set(
        linear[0][0], linear[0][1], linear[0][2], at[0],
        linear[1][0], linear[1][1], linear[1][2], at[1],
        linear[2][0], linear[2][1], linear[2][2], at[2],
        0, 0, 0, 1);
      child.matrixWorldNeedsUpdate = true;
    }
  };

  /** ビューの枠に塊の外接球が入るかを、カメラの基底で見る。three.js のカメラは使わない。 */
  const visibleIn = (view, center, radius) => {
    const { camera, rect } = view;
    const forward = normalize(camera.direction), right = normalize(cross(forward, camera.up)), up = cross(right, forward);
    const rel = sub(center, camera.position);
    const depth = dot(rel, forward), x = dot(rel, right), y = dot(rel, up), aspect = rect.width / rect.height;
    if (camera.projection === "perspective") {
      const t = Math.tan((camera.fov * Math.PI) / 360), slack = radius * Math.hypot(1, t * aspect);
      if (depth + radius <= 0) return false;
      return Math.abs(x) <= depth * t * aspect + slack && Math.abs(y) <= depth * t + radius * Math.hypot(1, t);
    }
    const half = camera.width / 2;
    return Math.abs(x) <= half + radius && Math.abs(y) <= half / aspect + radius;
  };

  /** 粗い段から近い順に、表示範囲の塊を予算まで描く。 */
  const choose = () => {
    if (!alive) return;
    const state = viewer.getState();
    material.size = pointSize * state.size.pixelRatio;
    if (place === null || current === null) { viewer.invalidate(); return; }
    const candidates = [];
    let outside = 0;
    for (const child of root.children) {
      const info = child.userData.zumen;
      const center = place.toViewer(info.mapCenter);
      const radius = info.radius * place.linear[2][2];
      const views = state.views.filter((view) => visibleIn(view, center, radius));
      if (views.length === 0) { outside++; child.visible = false; continue; }
      const depth = Math.min(...views.map((view) => dot(sub(center, view.camera.position), normalize(view.camera.direction))));
      candidates.push({ child, level: info.level, depth, chunk: info.chunk });
    }
    candidates.sort((a, b) => a.level - b.level || a.depth - b.depth || a.chunk - b.chunk);
    let points = 0, chunks = 0, budget = 0;
    for (const { child } of candidates) {
      const count = child.userData.zumen.count;
      if (points + count > maxPoints) { child.visible = false; budget++; continue; }
      child.visible = true; points += count; chunks++;
    }
    drawn = { points, chunks }; skipped = { outside, budget };
    viewer.invalidate();
  };

  const resolve = () => {
    place = null;
    if (current === null) { reason = "empty"; return; }
    const state = viewer.getState();
    const next = placement(state.scene, current.input.result);
    if (next.reason !== null) { reason = next.reason; root.visible = false; return; }
    reason = null;
    place = next;
    root.visible = true;
    placeChunks();
    choose();
  };

  /** 表示用の色。rgba は元の列を借用し、強度（点群の最大で正規化）・分類は表示用の 8 ビットの色を作る。 */
  const colorAttribute = (points, mode, peak) => {
    const count = points.end - points.begin;
    if (mode === "rgba") return new THREE.BufferAttribute(points.rgba, 4, true);
    const out = new Uint8Array(count * 3);
    if (mode === "intensity") for (let i = 0; i < count; i++) out.fill(peak === 0 ? 0 : Math.round((points.intensity[i] * 255) / peak), i * 3, i * 3 + 3);
    else for (let i = 0; i < count; i++) out.set(CLASS_COLORS[points.classification[i]] ?? CLASS_COLORS[0], i * 3);
    displayBytes += out.byteLength;
    return new THREE.BufferAttribute(out, 3, true);
  };

  /** 検査に通ったときだけ差し替える。 */
  const setInput = (value) => {
    if (!alive) fail("destroyed", "解除した点群");
    if (value === null) { clear(); resolve(); viewer.invalidate(); return getReport(); }
    const input = readInput(value);
    const scene = readScene3D(input.blob, options.readLimits);
    if (scene.sections <= 20 || scene.rows("instance_meshes") > 0) invalid("点群のシーンではない");
    const columns = scene.columns, total = scene.rows("point_xyz");
    if ((input.result.counts?.points ?? 0) !== total) invalid("応答と blob の points の数が違う");
    const mode = colorMode !== "auto" ? colorMode
      : columns.point_rgba.length > 0 ? "rgba" : columns.point_intensity.length > 0 ? "intensity" : columns.point_classification.length > 0 ? "classification" : "none";
    if (mode === "rgba" && columns.point_rgba.length === 0) invalid("点群に色の列がない");
    if (mode === "intensity" && columns.point_intensity.length === 0) invalid("点群に強度の列がない");
    if (mode === "classification" && columns.point_classification.length === 0) invalid("点群に分類の列がない");
    let peak = 0;
    if (mode === "intensity") for (const value of columns.point_intensity) if (value > peak) peak = value;
    clear();
    material.vertexColors = mode !== "none";
    material.needsUpdate = true;
    for (let row = 0; row < scene.chunks; row++) {
      const points = scene.points(row), count = points.end - points.begin;
      if (count === 0) continue;
      const chunk = scene.chunk(row);
      const mapOrigin = [...chunk.origin], bounds = chunk.bounds;
      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute("position", new THREE.BufferAttribute(points.positions, 3));
      if (mode !== "none") geometry.setAttribute("color", colorAttribute(points, mode, peak));
      const middle = [0, 1, 2].map((k) => (bounds[k] + bounds[k + 3]) / 2);
      const radius = Math.hypot(bounds[3] - bounds[0], bounds[4] - bounds[1], bounds[5] - bounds[2]) / 2;
      geometry.boundingSphere = new THREE.Sphere(new THREE.Vector3(...middle), radius);
      geometry.boundingBox = new THREE.Box3(new THREE.Vector3(bounds[0], bounds[1], bounds[2]), new THREE.Vector3(bounds[3], bounds[4], bounds[5]));
      const object = new THREE.Points(geometry, material);
      object.matrixAutoUpdate = false;
      object.visible = false;
      object.userData.zumen = { chunk: row, count, level: points.node === null ? 0 : points.node[0], node: points.node, points,
        mapOrigin, mapCenter: [0, 1, 2].map((k) => mapOrigin[k] + middle[k]), radius };
      root.add(object);
    }
    current = { input, scene };
    colorsUsed = mode;
    resolve();
    viewer.invalidate();
    return getReport();
  };

  const onViewer = (state) => {
    if (!alive) return;
    if (state.scene !== lastScene) { lastScene = state.scene; lastViews = state.views; lastSize = state.size; resolve(); return; }
    if (state.views !== lastViews || state.size !== lastSize) { lastViews = state.views; lastSize = state.size; choose(); }
  };

  /** 要求の箱・間隔・上限で読み直す。新しい要求か解除で前の要求を中断し、古い結果は捨てる。 */
  const request = (query = {}) => {
    if (!alive) fail("destroyed", "解除した点群");
    if (options.load === undefined) invalid("load を渡していない");
    if (!isObject(query)) invalid("要求はオブジェクト");
    controller?.abort("superseded");
    const own = new AbortController(), ticket = ++generation;
    controller = own;
    return Promise.resolve()
      // 始まる前に次の要求か解除が来れば load を呼ばない
      .then(() => (own.signal.aborted ? Promise.reject(own.signal.reason) : options.load({ ...query, signal: own.signal })))
      .then((input) => {
        if (!alive || ticket !== generation || own.signal.aborted) { requests.stale++; return Object.freeze({ outcome: "stale" }); }
        controller = null;
        setInput(input);
        requests.applied++;
        return Object.freeze({ outcome: "applied", report: getReport() });
      }, (error) => {
        if (!alive || ticket !== generation || own.signal.aborted) { requests.cancelled++; return Object.freeze({ outcome: "cancelled" }); }
        controller = null;
        requests.failed++;
        firstFailure ??= error;
        return Object.freeze({ outcome: "failed", error });
      });
  };

  const cancel = (why = "cancel") => { controller?.abort(why); controller = null; };

  /** 描いている点のうち、点の近くを通る光線の点を距離の順に返す。手前の要素に隠れた点は除く。 */
  const hitTest = (viewId, point, hitOptions = {}) => {
    if (!alive) return Object.freeze({ hits: Object.freeze([]), truncated: false, reason: "removed" });
    if (!isObject(hitOptions)) invalid("options はオブジェクト");
    const tolerance = hitOptions.tolerance === undefined ? 4 : finite(hitOptions.tolerance, "tolerance は有限値");
    if (tolerance < 0 || tolerance > 64) invalid("tolerance は 0〜64 の CSS px");
    const limit = readCount(hitOptions.limit, 16, "limit は 1〜1000 の整数", 1000);
    const ray = viewer.getRay(viewId, point);
    if (place === null) return Object.freeze({ hits: Object.freeze([]), truncated: false, reason });
    const view = viewer.getState().views.find((item) => item.id === viewId);
    const { camera, rect } = view;
    const start = [0, 1, 2].map((k) => ray.origin[k] + ray.offset[k]), direction = ray.direction;
    const perPixel = camera.projection === "perspective"
      ? (depth) => (depth * 2 * Math.tan((camera.fov * Math.PI) / 360)) / rect.height
      : () => camera.width / rect.width;
    const front = viewer.hitTest(viewId, point, { limit: 1 }).hits.find((hit) => hit.kind === "element");
    const found = [];
    let work = 0, truncated = false;
    for (const child of root.children) {
      if (!child.visible) continue;
      const info = child.userData.zumen;
      const base = sub(place.toViewer(info.mapOrigin), start);
      const center = sub(place.toViewer(info.mapCenter), start), radius = info.radius * place.linear[2][2];
      const along = dot(center, direction), perpendicular = Math.sqrt(Math.max(0, dot(center, center) - along * along));
      if (along + radius < 0 || perpendicular - radius > tolerance * perPixel(Math.max(along + radius, 0))) continue;
      const { positions } = info.points;
      for (let i = 0; i < info.count; i++) {
        if (++work > maxWork) { truncated = true; break; }
        const local = [positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]];
        const q = [0, 1, 2].map((k) => base[k] + dot(place.linear[k], local));
        const distance = dot(q, direction);
        if (distance < 0) continue;
        const off = Math.sqrt(Math.max(0, dot(q, q) - distance * distance));
        if (off > tolerance * perPixel(distance)) continue;
        if (front !== undefined && distance > front.distance) continue;
        const absolute = [0, 1, 2].map((k) => start[k] + q[k]);
        if (view.clips.some((clip) => dot(clip.normal, sub(absolute, clip.point)) > 0)) continue;
        if (view.box !== null && [0, 1, 2].some((k) => absolute[k] < view.box.min[k] || absolute[k] > view.box.max[k])) continue;
        found.push({ distance, off, chunk: info.chunk, row: i, info, q });
      }
      if (truncated) break;
    }
    found.sort((a, b) => a.distance - b.distance || a.chunk - b.chunk || a.row - b.row);
    if (found.length > limit) truncated = true;
    const { columns } = current.scene;
    const hits = found.slice(0, limit).map(({ distance, chunk, row, info, q }) => {
      const index = info.points.begin + row;
      return Object.freeze({
        kind: "point", document: current.input.document, chunk, row, node: info.node === null ? null : Object.freeze([...info.node]), distance,
        position: Object.freeze({ origin: Object.freeze([...start]), offset: Object.freeze([...q]) }),
        rgba: columns.point_rgba.length > 0 ? Object.freeze([...columns.point_rgba.subarray(index * 4, index * 4 + 4)]) : null,
        intensity: columns.point_intensity.length > 0 ? columns.point_intensity[index] : null,
        classification: columns.point_classification.length > 0 ? columns.point_classification[index] : null,
      });
    });
    return Object.freeze({ hits: Object.freeze(hits), truncated, occluder: front ?? null });
  };

  const getReport = () => Object.freeze({
    status: !alive ? "removed" : current === null ? "empty" : place === null ? "unavailable" : "ready",
    reason: !alive ? "removed" : reason,
    document: current?.input.document ?? null,
    height: place?.height ?? null,
    points: current === null ? 0 : current.scene.rows("point_xyz"),
    chunks: root.children.length,
    drawn: Object.freeze({ ...drawn }),
    skipped: Object.freeze({ ...skipped }),
    truncated: current?.input.result.truncated ?? false,
    tolerance: current?.input.result.tolerance ?? null,
    color: colorsUsed,
    displayBytes,
    pending: controller !== null,
    requests: Object.freeze({ ...requests, firstFailure }),
  });

  const registration = viewer.addObject(root, {
    id: options.id, origin: [0, 0, 0], unit, scope: options.scope, priority: options.priority,
    pickable: false, clipping: true, dispose: release,
  });
  // 登録の原点は 0 にし、塊の行列に文書の座標を f64 で置く。部品はカメラの位置を引いた行列を f64 で合成してから f32 にする。
  try {
    unsubscribes.push(viewer.subscribe(onViewer));
    lastScene = viewer.getState().scene; lastViews = viewer.getState().views; lastSize = viewer.getState().size;
    if (options.input !== undefined) setInput(options.input);
  } catch (error) {
    registration.remove();
    throw error;
  }

  return Object.freeze({
    id: registration.id, signal: registration.signal, remove: registration.remove,
    setInput, request, cancel, hitTest, getReport,
  });
}
