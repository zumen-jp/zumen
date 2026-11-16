// View2D に描かせた Canvas を 3D の地面の平面に敷く任意のアダプター（docs/view3d.md#地面の-2d地物点群）。
// View2D を読み込まず、利用側が作った View2D と Canvas を受け取る。2D の列や描画を作り直さず、Canvas をそのまま texture にする。
import * as THREE from "three";
import { cross, dot, fail, finite, invalid, isObject, length, normalize, positive, vector } from "./common.js";

/** 2D のシーンの単位の、1 単位あたりの m。 */
const METRES_2D = Object.freeze({ millimeter: 0.001, centimeter: 0.01, meter: 1, inch: 0.0254, foot: 0.3048 });
/** IFC の SI の接頭辞。 */
const PREFIXES = Object.freeze({
  EXA: 1e18, PETA: 1e15, TERA: 1e12, GIGA: 1e9, MEGA: 1e6, KILO: 1e3, HECTO: 1e2, DECA: 1e1,
  DECI: 1e-1, CENTI: 1e-2, MILLI: 1e-3, MICRO: 1e-6, NANO: 1e-9, PICO: 1e-12, FEMTO: 1e-15, ATTO: 1e-18,
});
const KEYS = new Set(["id", "view2d", "canvas", "plane", "scale", "side", "scope", "priority", "maxPixels", "maxSide", "link"]);
const PLANE_KEYS = new Set(["origin", "xAxis", "yAxis", "anchor"]);
const LINK_KEYS = new Set(["document", "view", "to3d", "to2d"]);
/** 直交の許容差。軸は正規化した後の内積で見る。 */
const ORTHOGONAL = 1e-9;
/** 換算の比の許容差。明示した scale と単位からの値を比べる。 */
const RATIO = 1e-9;

/** 3D のシーンの長さの単位の、1 単位あたりの m。分からなければ null。 */
function metres3D(unit) {
  if (!isObject(unit) || typeof unit.name !== "string") return null;
  if (unit.name === "METRE") return unit.prefix === undefined ? 1 : (PREFIXES[unit.prefix] ?? null);
  return typeof unit.scale === "number" && Number.isFinite(unit.scale) && unit.scale > 0 ? unit.scale : null;
}

/** 実寸の空間でない 2D は重ねない。判定は View2D の実寸の距離と同じ。 */
function reason2D(result) {
  if (result.from.startsWith("sheet/") || result.view !== undefined) return "paper";
  if (!result.from.startsWith("space/")) return "definition";
  return null;
}

const refKey = (ref) => JSON.stringify([ref.id, ref.path.map((step) => [step.id, step.column, step.row])]);

function readPlane(value) {
  if (!isObject(value)) invalid("plane は { origin, xAxis, yAxis, anchor? }");
  for (const key of Object.keys(value)) if (!PLANE_KEYS.has(key)) invalid(`plane の未知の項目: ${key}`);
  const origin = vector(value.origin, "plane.origin は有限の 3 値");
  const x = vector(value.xAxis, "plane.xAxis は有限の 3 値"), y = vector(value.yAxis, "plane.yAxis は有限の 3 値");
  if (length(x) === 0 || length(y) === 0) invalid("plane の軸は 0 でない");
  const u = normalize(x), v = normalize(y);
  if (Math.abs(dot(u, v)) > ORTHOGONAL) invalid("plane の 2 軸は直交する");
  const anchor = value.anchor ?? { x: 0, y: 0 };
  if (!isObject(anchor)) invalid("plane.anchor は { x, y }");
  const point = { x: finite(anchor.x, "plane.anchor.x は有限値"), y: finite(anchor.y, "plane.anchor.y は有限値") };
  return Object.freeze({ origin, u, v, normal: normalize(cross(u, v)), anchor: point });
}

function readLink(value) {
  if (value === undefined) return null;
  if (!isObject(value)) invalid("link は { document, to3d, to2d, view? }");
  for (const key of Object.keys(value)) if (!LINK_KEYS.has(key)) invalid(`link の未知の項目: ${key}`);
  if (!isObject(value.document)) invalid("link.document は 3D の文書識別");
  if (typeof value.to3d !== "function" || typeof value.to2d !== "function") invalid("link.to3d・link.to2d は関数");
  if (value.view !== undefined && typeof value.view !== "string") invalid("link.view はビュー ID");
  return Object.freeze({ document: value.document, view: value.view, to3d: value.to3d, to2d: value.to2d });
}

function readSide(value) {
  if (value === undefined || value === "double") return "double";
  if (value === "front") return "front";
  return invalid("side は front か double");
}

function readCount(value, fallback, message) {
  if (value === undefined) return fallback;
  if (!Number.isSafeInteger(value) || value <= 0) invalid(message);
  return value;
}

/**
 * View2D の Canvas を 3D の平面に敷く。2D の起点の座標 anchor を平面の origin に、2D の +x・+y を xAxis・yAxis に写す。
 * 尺度は 2D と 3D の単位から決め、scale を渡したときは単位と照らす。紙の上の 2D と単位の分からない組は重ねず理由を返す。
 */
export function attachGround2D(viewer, options) {
  if (!isObject(options)) invalid("options はオブジェクト");
  for (const key of Object.keys(options)) if (!KEYS.has(key)) invalid(`未知の項目: ${key}`);
  const { view2d, canvas } = options;
  if (!isObject(view2d) || typeof view2d.getState !== "function" || typeof view2d.toScreen !== "function") invalid("view2d は View2D");
  if (!isObject(canvas) || !Number.isSafeInteger(canvas.width) || !Number.isSafeInteger(canvas.height)) invalid("canvas は View2D の Canvas");
  if (typeof options.id !== "string") invalid("id は文字列");
  const plane = readPlane(options.plane);
  const explicit = options.scale === undefined ? null : positive(options.scale, "scale は正の有限値");
  const side = readSide(options.side);
  const link = readLink(options.link);
  const maxPixels = readCount(options.maxPixels, 16777216, "maxPixels は正の整数");
  const maxSide = readCount(options.maxSide, 4096, "maxSide は正の整数");
  const scene3d = viewer.getState().scene;
  if (scene3d === null) fail("no-scene", "地面の 2D には 3D のシーンが要る");
  const unit = { name: scene3d.unit.name };
  if (scene3d.unit.prefix !== undefined) unit.prefix = scene3d.unit.prefix;
  if (scene3d.unit.scale !== undefined) unit.scale = scene3d.unit.scale;

  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(12), 3));
  // Canvas の左上・右上・右下・左下。texture は上下を反転して読む。
  geometry.setAttribute("uv", new THREE.BufferAttribute(new Float32Array([0, 1, 1, 1, 1, 0, 0, 0]), 2));
  geometry.setIndex([3, 2, 1, 3, 1, 0]);
  const material = new THREE.MeshBasicMaterial({
    transparent: true, depthWrite: false, toneMapped: false, side: side === "front" ? THREE.FrontSide : THREE.DoubleSide,
  });
  const mesh = new THREE.Mesh(geometry, material);
  mesh.visible = false;
  const root = new THREE.Group();
  root.add(mesh);

  let texture = null, status = "unavailable", reason = "no-scene", factor = null, frame = null, alive = true;
  let syncing = false, lastSelection2D = null, lastSelection3D = null, unmatched = 0, failures = 0, firstFailure = null;
  const unsubscribes = [];

  const release = () => {
    if (!alive) return;
    alive = false;
    const errors = [];
    for (const off of unsubscribes.splice(0)) { try { off(); } catch (error) { errors.push(error); } }
    texture?.dispose();
    texture = null;
    material.map = null;
    material.dispose();
    geometry.dispose();
    if (errors.length > 0) throw errors[0];
  };

  const resolve = () => {
    const state2d = view2d.getState(), state3d = viewer.getState();
    if (state3d.scene === null) return ["no-scene", null];
    if (state2d.scene === null) return ["no-scene-2d", null];
    const paper = reason2D(state2d.scene.result);
    if (paper !== null) return [paper, null];
    const m2 = METRES_2D[state2d.scene.result.unit] ?? null, m3 = metres3D(state3d.scene.unit);
    const derived = m2 !== null && m3 !== null ? m2 / m3 : null;
    if (explicit !== null) {
      if (derived !== null && Math.abs(explicit / derived - 1) > RATIO) return ["unit-mismatch", null];
      return [null, explicit];
    }
    return derived === null ? ["unit-unknown", null] : [null, derived];
  };

  /** 状態・texture・四隅を当て直す。描画は 3D の次の描画で 1 回だけ転送する。 */
  const refresh = () => {
    if (!alive) return;
    const [why, value] = resolve();
    const size = view2d.getSize();
    let next = why;
    if (next === null && (Math.abs(canvas.width - size.width * size.pixelRatio) > 1 || Math.abs(canvas.height - size.height * size.pixelRatio) > 1)) next = "canvas-mismatch";
    if (next === null && (canvas.width > maxSide || canvas.height > maxSide || canvas.width * canvas.height > maxPixels)) next = "limit";
    if (next === null && (canvas.width === 0 || canvas.height === 0)) next = "canvas-mismatch";
    if (next !== null) {
      status = "unavailable"; reason = next; factor = null; frame = null; mesh.visible = false;
      texture?.dispose(); texture = null; material.map = null; material.needsUpdate = true;
      viewer.invalidate();
      return;
    }
    status = "ready"; reason = null; factor = value;
    if (texture === null || texture.image.width !== canvas.width || texture.image.height !== canvas.height
      || frame === null || frame.textureWidth !== canvas.width || frame.textureHeight !== canvas.height) {
      // 寸法が変われば texture を作り直す。three.js は確保済みの texture の寸法の変更を扱わない。
      texture?.dispose();
      texture = new THREE.CanvasTexture(canvas);
      texture.colorSpace = THREE.SRGBColorSpace;
      texture.generateMipmaps = false;
      texture.minFilter = THREE.LinearFilter;
      material.map = texture;
      material.needsUpdate = true;
    }
    texture.needsUpdate = true;
    const { width, height } = size;
    const center = view2d.toScene({ x: width / 2, y: height / 2 });
    const corners = [[0, 0], [width, 0], [width, height], [0, height]].map(([x, y]) => view2d.toScene({ x, y }));
    // 2D の差を先に取り、大きな原点を足す丸めを避ける。
    const toPlane = (dx, dy) => [0, 1, 2].map((k) => factor * (dx * plane.u[k] + dy * plane.v[k]));
    const middle = toPlane(center.x - plane.anchor.x, center.y - plane.anchor.y);
    const positions = geometry.getAttribute("position");
    corners.forEach((corner, i) => {
      const p = toPlane(corner.x - center.x, corner.y - center.y);
      positions.setXYZ(i, p[0], p[1], p[2]);
    });
    positions.needsUpdate = true;
    // View2D は CSS px に pixel ratio を掛けて切り上げた画素を持つ。端の余りの画素は枠の外なので読まない。
    const right = Math.min(1, (width * size.pixelRatio) / canvas.width), bottom = 1 - Math.min(1, (height * size.pixelRatio) / canvas.height);
    const uvs = geometry.getAttribute("uv");
    uvs.setXY(1, right, 1); uvs.setXY(2, right, bottom); uvs.setXY(3, 0, bottom);
    uvs.needsUpdate = true;
    geometry.computeBoundingSphere();
    geometry.computeBoundingBox();
    mesh.position.set(middle[0], middle[1], middle[2]);
    mesh.visible = true;
    const view = view2d.getView();
    frame = Object.freeze({ textureWidth: canvas.width, textureHeight: canvas.height, scenePerPixel: 1 / (view.scale * size.pixelRatio) });
    viewer.invalidate();
  };

  const guard = (fn) => {
    if (syncing) return;
    syncing = true;
    try { fn(); } catch (error) { failures++; firstFailure ??= error; } finally { syncing = false; }
  };
  const linkView = (state) => (link.view === undefined ? state.views[0] : state.views.find((view) => view.id === link.view));
  const sync2DTo3D = (state2d) => guard(() => {
    const ids = [], seen = new Set();
    for (const ref of state2d.selection) {
      const mapped = link.to3d(ref);
      if (!Array.isArray(mapped)) throw new TypeError("link.to3d は ID の配列を返す");
      for (const id of mapped) if (typeof id === "string" && !seen.has(id)) { seen.add(id); ids.push(id); }
    }
    viewer.select(link.document, ids, link.view === undefined ? {} : { view: link.view });
    lastSelection3D = linkView(viewer.getState())?.selected ?? null;
  });
  const sync3DTo2D = (selected) => guard(() => {
    const known = new Map(view2d.getElements().map((ref) => [refKey(ref), ref]));
    const refs = [], seen = new Set();
    for (const id of selected) {
      const mapped = link.to2d(id);
      if (!Array.isArray(mapped)) throw new TypeError("link.to2d は ElementRef の配列を返す");
      for (const ref of mapped) {
        const key = isObject(ref) && typeof ref.id === "string" && Array.isArray(ref.path) ? refKey(ref) : null;
        const found = key === null ? undefined : known.get(key);
        if (found === undefined) { unmatched++; continue; }
        if (!seen.has(key)) { seen.add(key); refs.push(found); }
      }
    }
    view2d.select(refs, "replace");
    lastSelection2D = view2d.getState().selection;
  });

  let last2D = null, last3D = null;
  const on2D = (state) => {
    if (!alive) return;
    const changed = last2D === null || state.scene !== last2D.scene || state.view !== last2D.view || state.size !== last2D.size;
    last2D = state;
    if (changed) refresh();
    else if (texture !== null) { texture.needsUpdate = true; viewer.invalidate(); }
    if (link !== null && state.selection !== lastSelection2D) { lastSelection2D = state.selection; sync2DTo3D(state); }
  };
  const on3D = (state) => {
    if (!alive) return;
    if (last3D === null || state.scene !== last3D.scene) { last3D = state; refresh(); }
    last3D = state;
    if (link === null) return;
    const view = linkView(state);
    if (view !== undefined && view.selected !== lastSelection3D) { lastSelection3D = view.selected; sync3DTo2D(view.selected); }
  };

  refresh();
  const registration = viewer.addObject(root, {
    id: options.id, origin: plane.origin, unit, scope: options.scope, priority: options.priority, pickable: false, clipping: true, dispose: release,
  });
  try {
    unsubscribes.push(view2d.subscribe(on2D));
    unsubscribes.push(viewer.subscribe(on3D));
    last2D = view2d.getState();
    last3D = viewer.getState();
    if (link !== null) {
      lastSelection2D = last2D.selection;
      lastSelection3D = linkView(last3D)?.selected ?? null;
    }
  } catch (error) {
    registration.remove();
    throw error;
  }

  /** 光線と平面の交点を 2D の座標と CSS px に戻し、View2D の当たり判定へ渡す。 */
  const hitTest = (viewId, point, hitOptions = {}) => {
    if (!alive) return Object.freeze({ status: "miss", reason: "removed" });
    if (status !== "ready") return Object.freeze({ status: "miss", reason });
    if (!isObject(hitOptions)) invalid("options はオブジェクト");
    const tolerance = hitOptions.tolerance === undefined ? 3 : finite(hitOptions.tolerance, "tolerance は有限値");
    if (tolerance < 0 || tolerance > 64) invalid("tolerance は 0〜64 の CSS px");
    const ray = viewer.getRay(viewId, point);
    const start = [0, 1, 2].map((k) => ray.origin[k] - plane.origin[k] + ray.offset[k]);
    const denominator = dot(plane.normal, ray.direction);
    if (Math.abs(denominator) < 1e-12) return Object.freeze({ status: "miss", reason: "parallel" });
    const t = -dot(plane.normal, start) / denominator;
    if (t < 0) return Object.freeze({ status: "miss", reason: "behind" });
    if (side === "front" && denominator > 0) return Object.freeze({ status: "miss", reason: "back" });
    const offset = [0, 1, 2].map((k) => start[k] + t * ray.direction[k]);
    const position = Object.freeze({ origin: Object.freeze([...plane.origin]), offset: Object.freeze(offset) });
    const view = viewer.getState().views.find((item) => item.id === viewId);
    const clipped = view.clips.some((clip) => dot(clip.normal, [0, 1, 2].map((k) => plane.origin[k] - clip.point[k] + offset[k])) > 0)
      || (view.box !== null && [0, 1, 2].some((k) => plane.origin[k] + offset[k] < view.box.min[k] || plane.origin[k] + offset[k] > view.box.max[k]));
    if (clipped) return Object.freeze({ status: "miss", reason: "clipped", position });
    const scene = Object.freeze({
      origin: Object.freeze({ ...plane.anchor }),
      offset: Object.freeze({ x: dot(offset, plane.u) / factor, y: dot(offset, plane.v) / factor }),
    });
    const screen = view2d.toScreen(scene);
    const size = view2d.getSize();
    if (!(screen.x >= 0 && screen.x <= size.width && screen.y >= 0 && screen.y <= size.height)) {
      return Object.freeze({ status: "miss", reason: "outside", position, scene, screen });
    }
    const front = viewer.hitTest(viewId, point, { limit: 1 }).hits[0];
    if (front !== undefined && front.distance < t) return Object.freeze({ status: "occluded", occluder: front, distance: t, position, scene, screen });
    const elements = view2d.hitTest(screen, tolerance);
    return Object.freeze({ status: "hit", distance: t, position, scene, screen: Object.freeze({ ...screen }), elements });
  };

  const getReport = () => {
    const issues = alive ? view2d.getReport().issues : [];
    return Object.freeze({
      status: alive ? status : "removed",
      reason: alive ? reason : "removed",
      scale: factor,
      texture: frame === null ? null : Object.freeze({ width: frame.textureWidth, height: frame.textureHeight }),
      // 2D の起点の単位と 3D の文書単位で、texture の 1 画素の辺の長さ。描画の近似の目安。
      resolution: frame === null ? null : Object.freeze({ scene: frame.scenePerPixel, document: frame.scenePerPixel * factor }),
      issues,
      link: link === null ? null : Object.freeze({ unmatched, failures, firstFailure }),
    });
  };

  return Object.freeze({
    id: registration.id,
    signal: registration.signal,
    remove: registration.remove,
    /** View2D の render を状態の変化なしに呼んだときに texture の転送を求める。 */
    update() { if (alive) { refresh(); } },
    hitTest,
    getReport,
  });
}
