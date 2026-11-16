// 利用側の追加描画（docs/view3d.md#追加描画と所有権）。借用する Object3D と、幾何の記述から部品が作る資源を分けて持つ。
// 位置は f64 の原点と局所の変換で受け、描画と当たり判定のたびにカメラの位置からの相対値にしてから three.js へ渡す。
import * as THREE from "three";
import { color, cross, dot, fail, finite, invalid, isObject, length, normalize, positive, scale, sub, vector } from "./common.js";

const COMMON_KEYS = ["id", "origin", "matrix", "unit", "scope", "priority", "pickable", "pickTolerance"];
const OBJECT_KEYS = new Set([...COMMON_KEYS, "clipping", "dispose"]);
const GEOMETRY_KEYS = new Set(COMMON_KEYS);
/** 同じ距離の当たりの種類の順。要素・幾何の記述・利用側の Object3D。 */
export const KIND_RANK = Object.freeze({ element: 0, geometry: 1, object: 2 });
const MAX_SEGMENTS = 65536;
const MAX_LABEL_PX = 4096;

/** 長さの単位の名前・接頭辞・尺度が同じか。 */
export function sameUnit(a, b) {
  return isObject(a) && isObject(b) && a.name === b.name && a.prefix === b.prefix && a.scale === b.scale;
}

function readUnit(value) {
  if (!isObject(value) || typeof value.name !== "string" || value.name === "") invalid("unit はシーンと同じ { name, prefix?, scale? }");
  if (value.prefix !== undefined && typeof value.prefix !== "string") invalid("unit.prefix は文字列");
  if (value.scale !== undefined) positive(value.scale, "unit.scale は正の有限値");
  const unit = { name: value.name };
  if (value.prefix !== undefined) unit.prefix = value.prefix;
  if (value.scale !== undefined) unit.scale = value.scale;
  return Object.freeze(unit);
}

/** 列優先の 4 行 4 列（three.js の Matrix4.elements と同じ順）。アフィンで逆を持つものだけ受ける。 */
function readMatrix(value) {
  if (value === undefined) return null;
  if ((!Array.isArray(value) && !ArrayBuffer.isView(value)) || value.length !== 16) invalid("matrix は列優先の 16 個の有限値");
  for (let i = 0; i < 16; i++) finite(value[i], "matrix は列優先の 16 個の有限値");
  if (value[3] !== 0 || value[7] !== 0 || value[11] !== 0 || value[15] !== 1) invalid("matrix はアフィン変換（最後の行が 0 0 0 1）");
  const matrix = new THREE.Matrix4().fromArray(Array.from(value));
  const det = matrix.determinant();
  if (!Number.isFinite(det) || det === 0) invalid("matrix は逆を持つ");
  return matrix;
}

function readRegistration(options, keys, context) {
  if (!isObject(options)) invalid("登録の設定はオブジェクト");
  for (const key of Object.keys(options)) if (!keys.has(key)) invalid(`登録の未知の項目: ${key}`);
  const { id } = options;
  if (typeof id !== "string" || id === "" || id.length > 256) invalid("登録の ID は 256 文字以下の空でない文字列");
  if (context.ids.has(id)) invalid(`登録の ID が重なる: ${id}`);
  const scope = options.scope ?? "scene";
  if (scope !== "scene" && scope !== "viewer") invalid("scope は scene か viewer");
  const unit = readUnit(options.unit);
  if (context.unit === null) {
    if (scope === "scene") fail("no-scene", "scope が scene の登録にはシーンが要る");
  } else if (!sameUnit(unit, context.unit)) invalid("unit がシーンの長さの単位と違う");
  const priority = options.priority ?? 0;
  if (!Number.isSafeInteger(priority) || Math.abs(priority) > 1000000) invalid("priority は ±1000000 以内の整数");
  for (const key of ["pickable", "clipping"]) if (options[key] !== undefined && typeof options[key] !== "boolean") invalid(`${key} は真偽値`);
  const pickTolerance = options.pickTolerance ?? 3;
  if (positive(pickTolerance, "pickTolerance は CSS px の正の有限値") > 64) invalid("pickTolerance は 64 CSS px 以下");
  if (options.dispose !== undefined && typeof options.dispose !== "function") invalid("dispose は関数");
  const matrix = readMatrix(options.matrix);
  const columns = matrix === null ? [1, 1] : [0, 1].map((k) => length(matrix.elements.slice(k * 4, k * 4 + 3)));
  return {
    id, scope, unit, priority, pickTolerance, matrix, columns,
    origin: Object.freeze(vector(options.origin, "origin は有限の 3 値")),
    pickable: options.pickable ?? true, clipping: options.clipping ?? true, dispose: options.dispose ?? null,
  };
}

function points(value, minimum, message) {
  if (!Array.isArray(value) || value.length < minimum) invalid(`${message}は ${minimum} 個以上の点`);
  return value.map((p) => vector(p, `${message}の点は有限の 3 値`));
}

/** 表示部品が所有する資源と GPU のバイト数を数える。 */
class Owned {
  geometries = []; materials = []; textures = []; bytes = 0;
  geometry(values) {
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.Float32BufferAttribute(values, 3));
    this.geometries.push(geometry);
    this.bytes += values.length * 4;
    return geometry;
  }
  material(value) { this.materials.push(value); return value; }
  dispose() {
    for (const list of [this.geometries, this.materials, this.textures]) for (const item of list) item.dispose();
  }
}

/** 平面の多角形を三角形に分ける。平面から許容差より離れた点と自己交差は断る。 */
function region(list, tolerance, owned, rgb, opacity) {
  let normal = [0, 0, 0];
  for (let i = 0; i < list.length; i++) {
    const a = list[i], b = list[(i + 1) % list.length];
    normal = [normal[0] + (a[1] - b[1]) * (a[2] + b[2]), normal[1] + (a[2] - b[2]) * (a[0] + b[0]), normal[2] + (a[0] - b[0]) * (a[1] + b[1])];
  }
  if (!(length(normal) > 0)) invalid("範囲の面積が 0");
  const n = normalize(normal);
  const helper = Math.abs(n[0]) < 0.9 ? [1, 0, 0] : [0, 1, 0];
  const u = normalize(cross(helper, n)), v = cross(n, u);
  const flat = [];
  for (const p of list) {
    const d = sub(p, list[0]);
    if (Math.abs(dot(d, n)) > tolerance) invalid("範囲の点が同じ平面にない");
    flat.push(new THREE.Vector2(dot(d, u), dot(d, v)));
  }
  const faces = THREE.ShapeUtils.triangulateShape(flat, []);
  if (faces.length !== list.length - 2) invalid("範囲は自己交差しない単純な多角形");
  const values = [];
  for (const face of faces) for (const k of face) values.push(...list[k]);
  const material = owned.material(new THREE.MeshBasicMaterial({ color: rgb, side: THREE.DoubleSide, transparent: opacity < 1, opacity, depthWrite: opacity === 1 }));
  return new THREE.Mesh(owned.geometry(values), material);
}

function circlePoints(center, radius, normal, tolerance) {
  if (!(tolerance < radius)) invalid("円の許容差は半径より小さい");
  const segments = Math.max(8, Math.ceil(Math.PI / Math.acos(1 - tolerance / radius)));
  if (segments > MAX_SEGMENTS) fail("limit", "円の分割数の上限");
  const n = normalize(normal), helper = Math.abs(n[0]) < 0.9 ? [1, 0, 0] : [0, 1, 0];
  const u = normalize(cross(helper, n)), v = cross(n, u), out = [];
  for (let i = 0; i < segments; i++) {
    const a = (2 * Math.PI * i) / segments;
    out.push([0, 1, 2].map((k) => center[k] + radius * (Math.cos(a) * u[k] + Math.sin(a) * v[k])));
  }
  return out;
}

/** 画像を部品の Canvas へ写す。元の画像は改変せず、URL やフォントを取りに行かない。 */
function copyImage(image, ownerDocument, limits) {
  let width, height;
  const isData = typeof ImageData !== "undefined" && image instanceof ImageData;
  if (isData) { width = image.width; height = image.height; }
  else if (isObject(image) && typeof image.width === "number") {
    if (typeof HTMLImageElement !== "undefined" && image instanceof HTMLImageElement) {
      if (!image.complete || image.naturalWidth === 0) invalid("画像は読み込み済み");
      width = image.naturalWidth; height = image.naturalHeight;
    } else { width = image.width; height = image.height; }
  } else invalid("image は ImageData・ImageBitmap・Canvas・読み込み済みの画像");
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width <= 0 || height <= 0) invalid("画像の寸法は正の整数");
  if (width * height > limits.canvasPixels) fail("limit", "画像の画素数の上限");
  const canvas = ownerDocument !== null ? ownerDocument.createElement("canvas") : new OffscreenCanvas(width, height);
  canvas.width = width; canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) fail("webgl2-unavailable", "画像を写す 2D の Canvas を使えない");
  try {
    if (isData) context.putImageData(image, 0, 0); else context.drawImage(image, 0, 0);
    context.getImageData(0, 0, 1, 1);
  } catch (error) {
    invalid(`画像を読めない（別オリジンの画像は使えない）: ${error?.name ?? error}`);
  }
  return { canvas, width, height };
}

function readLabel(value, position, owned, context) {
  if (!isObject(value)) invalid("ラベルは { image, width, height, anchor? }");
  const width = positive(value.width, "ラベルの width は CSS px の正の有限値"), height = positive(value.height, "ラベルの height は CSS px の正の有限値");
  if (width > MAX_LABEL_PX || height > MAX_LABEL_PX) invalid("ラベルは 4096 CSS px 以下");
  const anchor = value.anchor ?? [0.5, 0.5];
  if (!Array.isArray(anchor) || anchor.length !== 2 || anchor.some((a) => typeof a !== "number" || !(a >= 0 && a <= 1))) invalid("anchor は 0〜1 の 2 値");
  const copy = copyImage(value.image, context.ownerDocument, context.limits);
  const texture = new THREE.CanvasTexture(copy.canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  owned.textures.push(texture);
  owned.bytes += copy.width * copy.height * 4;
  const material = owned.material(new THREE.SpriteMaterial({ map: texture, sizeAttenuation: false, transparent: true }));
  // Sprite の geometry は three.js が共有する。最後のラベルを外すときに GPU の写しだけを解放する（remove）
  const sprite = new THREE.Sprite(material);
  sprite.center.set(anchor[0], anchor[1]);
  sprite.position.set(position[0], position[1], position[2]);
  return { sprite, width, height };
}

const ITEM_KEYS = {
  line: ["kind", "points", "closed", "color"],
  region: ["kind", "points", "color", "opacity", "tolerance"],
  circle: ["kind", "center", "radius", "normal", "tolerance", "filled", "color", "opacity"],
  dimension: ["kind", "from", "to", "offset", "color", "label"],
  label: ["kind", "position", "image", "width", "height", "anchor"],
};

function buildItem(item, index, owned, labels, context) {
  if (!isObject(item) || !Object.prototype.hasOwnProperty.call(ITEM_KEYS, item.kind)) invalid(`items[${index}] の kind は ${Object.keys(ITEM_KEYS).join("・")}`);
  for (const key of Object.keys(item)) if (!ITEM_KEYS[item.kind].includes(key)) invalid(`items[${index}] の未知の項目: ${key}`);
  const rgb = item.kind === "label" ? null : color(item.color ?? "#000000", `items[${index}].color は #rrggbb`);
  const opacity = item.opacity ?? 1;
  if (typeof opacity !== "number" || !(opacity > 0 && opacity <= 1)) invalid(`items[${index}].opacity は 0 より大きく 1 以下`);
  const spend = (n) => { context.work += n; if (context.work > context.limits.work) fail("limit", "幾何の記述の作業量の上限"); };
  const lineOf = (list, closed) => {
    spend(list.length);
    const values = list.flat();
    const material = owned.material(new THREE.LineBasicMaterial({ color: rgb }));
    return closed ? new THREE.LineLoop(owned.geometry(values), material) : new THREE.Line(owned.geometry(values), material);
  };
  const toleranceOf = (list) => {
    if (item.tolerance !== undefined) return positive(item.tolerance, `items[${index}].tolerance は文書単位の正の有限値`);
    let size = 0;
    for (const p of list) size = Math.max(size, length(sub(p, list[0])));
    return Math.max(size * 1e-9, Number.MIN_VALUE);
  };
  switch (item.kind) {
    case "line": {
      if (item.closed !== undefined && typeof item.closed !== "boolean") invalid(`items[${index}].closed は真偽値`);
      return { kind: "line", objects: [lineOf(points(item.points, 2, `items[${index}].points `), item.closed ?? false)] };
    }
    case "region": {
      const list = points(item.points, 3, `items[${index}].points `);
      spend(list.length);
      return { kind: "region", objects: [region(list, toleranceOf(list), owned, rgb, opacity)] };
    }
    case "circle": {
      const center = vector(item.center, `items[${index}].center は有限の 3 値`);
      const radius = positive(item.radius, `items[${index}].radius は正の有限値`);
      const normal = vector(item.normal ?? [0, 0, 1], `items[${index}].normal は有限の 3 値`);
      if (!(length(normal) > 0)) invalid(`items[${index}].normal は 0 でない`);
      if (item.filled !== undefined && typeof item.filled !== "boolean") invalid(`items[${index}].filled は真偽値`);
      const tolerance = item.tolerance === undefined ? radius * 1e-3 : positive(item.tolerance, `items[${index}].tolerance は文書単位の正の有限値`);
      const list = circlePoints(center, radius, normal, tolerance);
      spend(list.length);
      const object = item.filled ? region(list, Math.max(tolerance, radius * 1e-9), owned, rgb, opacity) : lineOf(list, true);
      return { kind: "circle", objects: [object], segments: list.length };
    }
    case "dimension": {
      const from = vector(item.from, `items[${index}].from は有限の 3 値`), to = vector(item.to, `items[${index}].to は有限の 3 値`);
      const offset = vector(item.offset ?? [0, 0, 0], `items[${index}].offset は有限の 3 値`);
      const a = [0, 1, 2].map((k) => from[k] + offset[k]), b = [0, 1, 2].map((k) => to[k] + offset[k]);
      spend(6);
      const material = owned.material(new THREE.LineBasicMaterial({ color: rgb }));
      const objects = [new THREE.LineSegments(owned.geometry([...from, ...a, ...to, ...b, ...a, ...b]), material)];
      if (item.label !== undefined) {
        const label = readLabel(item.label, scale([0, 1, 2].map((k) => a[k] + b[k]), 0.5), owned, context);
        labels.push(label);
        objects.push(label.sprite);
      }
      // 寸法の長さは文書単位。線とラベルの画面上の寸法（CSS px）とは別に返す
      return { kind: "dimension", objects, length: length(sub(to, from)) };
    }
    default: {
      const label = readLabel(item, vector(item.position, `items[${index}].position は有限の 3 値`), owned, context);
      labels.push(label);
      return { kind: "label", objects: [label.sprite] };
    }
  }
}

function materialDrawn(material) {
  return material !== undefined && material !== null && material.visible !== false && !(material.transparent && material.opacity === 0);
}

/** 光線の当たりの順。距離、登録の優先度（大きいほう）、種類、元の行か ID、部分の番号の順に固定する。 */
export function compareHits(a, b) {
  if (a.distance !== b.distance) return a.distance - b.distance;
  const pa = a.priority ?? 0, pb = b.priority ?? 0;
  if (pa !== pb) return pb - pa;
  if (a.kind !== b.kind) return KIND_RANK[a.kind] - KIND_RANK[b.kind];
  if (a.kind === "element") return a.element - b.element;
  if (a.id !== b.id) return a.id < b.id ? -1 : 1;
  return (a.part ?? 0) - (b.part ?? 0);
}

/** 登録した追加描画の一覧。描画では Group にまとめ、ビューごとにカメラの位置からの相対の変換を置く。 */
export class Extras {
  group = new THREE.Group();
  #entries = new Map();
  #limits; #ownerDocument;

  constructor(limits, ownerDocument) {
    this.#limits = limits;
    this.#ownerDocument = ownerDocument;
    this.group.matrixAutoUpdate = false;
  }

  get size() { return this.#entries.size; }
  entries() { return [...this.#entries.values()]; }
  /** 部品が作った資源の GPU のバイト数。filter で残る登録だけを数える。 */
  bytes(filter = () => true) {
    let total = 0;
    for (const entry of this.#entries.values()) if (filter(entry)) total += entry.owned?.bytes ?? 0;
    return total;
  }

  state() {
    return Object.freeze(this.entries().map((e) => Object.freeze({ id: e.id, kind: e.kind, scope: e.scope, priority: e.priority, pickable: e.pickable, clipping: e.clipping })));
  }

  #wrap(registration, kind, child) {
    const wrapper = new THREE.Group();
    wrapper.matrixAutoUpdate = false;
    wrapper.add(child);
    const controller = new AbortController();
    const entry = { ...registration, kind, wrapper, controller, active: true };
    return entry;
  }

  /** 利用側の Object3D を借用する。親のない root だけを受け、位置・材質・userData を書き換えない。 */
  addObject(root, options, context) {
    if (this.#entries.size >= this.#limits.stateItems) fail("limit", "追加描画の数の上限");
    if (!(root instanceof THREE.Object3D)) invalid(root?.isObject3D ? "root は表示部品と同じ three.js の実体の Object3D" : "root は Object3D");
    if (root.parent !== null) invalid("root は親のない Object3D（既存の親から奪わない）");
    if (root.isScene) invalid("root に Scene は使えない");
    const registration = readRegistration(options, OBJECT_KEYS, { ...context, ids: this.#entries });
    let nodes = 0;
    root.traverse(() => { if (++nodes > this.#limits.work) fail("limit", "Object3D の節の数の上限"); });
    const entry = this.#wrap(registration, "object", root);
    entry.root = root;
    entry.disposed = false;
    this.#entries.set(entry.id, entry);
    this.group.add(entry.wrapper);
    return entry;
  }

  /** 幾何の記述から部品が資源を作る。資源は登録解除・scope の終了・破棄で解放する。 */
  addGeometry(items, options, context) {
    if (this.#entries.size >= this.#limits.stateItems) fail("limit", "追加描画の数の上限");
    if (isObject(options) && options.clipping !== undefined) invalid("幾何の記述は部品の材質で切るので clipping を受けない");
    const registration = readRegistration(options, GEOMETRY_KEYS, { ...context, ids: this.#entries });
    if (!Array.isArray(items) || items.length === 0 || items.length > this.#limits.stateItems) invalid("items は 1 個以上の幾何の記述の配列");
    const owned = new Owned(), labels = [], parts = [], partOf = new Map();
    const group = new THREE.Group();
    const build = { limits: this.#limits, ownerDocument: this.#ownerDocument, work: 0 };
    try {
      items.forEach((item, index) => {
        const part = buildItem(item, index, owned, labels, build);
        for (const object of part.objects) { group.add(object); partOf.set(object, index); }
        const info = { part: index, kind: part.kind };
        if (part.length !== undefined) info.length = part.length;
        if (part.segments !== undefined) info.segments = part.segments;
        parts.push(Object.freeze(info));
      });
      if (owned.bytes + context.usedBytes > this.#limits.gpuBytes) fail("limit", "GPU へ渡すバイト数の上限（追加描画）");
    } catch (error) {
      owned.dispose();
      throw error;
    }
    const entry = this.#wrap(registration, "geometry", group);
    Object.assign(entry, { root: group, owned, labels, partOf, parts: Object.freeze(parts) });
    this.#entries.set(entry.id, entry);
    this.group.add(entry.wrapper);
    return entry;
  }

  /** 登録を外し、部品の資源を解放して signal を通知する。後始末の 1 つが失敗しても残りを行い、最初の例外を返す。 */
  remove(entry, reason) {
    if (!entry.active) return [];
    entry.active = false;
    this.#entries.delete(entry.id);
    const tasks = [() => { this.group.remove(entry.wrapper); }];
    if (entry.kind === "object") {
      tasks.push(() => { entry.wrapper.remove(entry.root); });
      if (entry.dispose !== null) tasks.push(() => { if (!entry.disposed) { entry.disposed = true; entry.dispose(); } });
    } else {
      tasks.push(() => entry.owned.dispose());
      // 共有の geometry の dispose は GPU の写しを消すだけで、次にラベルを描くときに three.js が転送し直す
      const shared = entry.labels[0]?.sprite.geometry;
      if (shared && ![...this.#entries.values()].some((other) => other.labels?.length > 0)) tasks.push(() => shared.dispose());
    }
    tasks.push(() => entry.controller.abort(reason));
    return tasks;
  }

  /** ビューのカメラの位置からの相対の変換を置く。clipping を宣言しない物は切断面のあるビューから除く。 */
  prepare(camera, rect, planes) {
    const excluded = [];
    for (const entry of this.#entries.values()) {
      this.#place(entry, camera.position);
      const visible = entry.clipping || planes.length === 0;
      entry.wrapper.visible = visible;
      if (!visible) excluded.push(entry.id);
      if (entry.labels) {
        // sizeAttenuation のない Sprite の尺度。透視投影は奥行き 1 あたり、平行投影は文書単位
        const perPixel = camera.projection === "perspective" ? (2 * Math.tan((camera.fov * Math.PI) / 360)) / rect.height : camera.width / rect.width;
        for (const label of entry.labels) label.sprite.scale.set((label.width * perPixel) / entry.columns[0], (label.height * perPixel) / entry.columns[1], 1);
      }
    }
    return excluded;
  }

  #place(entry, position) {
    const o = entry.origin;
    const matrix = new THREE.Matrix4().makeTranslation(o[0] - position[0], o[1] - position[1], o[2] - position[2]);
    if (entry.matrix !== null) matrix.multiply(entry.matrix);
    entry.wrapper.matrix.copy(matrix);
    entry.wrapper.matrixWorldNeedsUpdate = true;
  }

  /**
   * ビューの光線に当たる追加描画。three.js の raycast を節ごとに呼び、呼ぶ前に作業量を数える。
   * 非表示・材質の透明度 0・切断面の外を除き、登録（幾何の記述は部分）ごとにもっとも近い当たりにまとめる。
   */
  hit(ray, camera, threeCamera, rect, planes, near, spend) {
    const raycaster = new THREE.Raycaster();
    raycaster.camera = threeCamera;
    raycaster.near = near;
    raycaster.far = Infinity;
    raycaster.ray.origin.set(ray.offset[0], ray.offset[1], ray.offset[2]);
    raycaster.ray.direction.set(ray.direction[0], ray.direction[1], ray.direction[2]);
    // n · (p + o - p0) > 0 の側を除く。p は光線の原点（カメラの位置）からの相対位置
    const cuts = planes.map(({ point, normal }) => [normal, dot(normal, sub(ray.origin, point))]);
    const kept = (p) => cuts.every(([n, c]) => !(n[0] * p.x + n[1] * p.y + n[2] * p.z + c > 0));
    const forward = normalize(camera.direction);
    const perPixel = (depth) => (camera.projection === "perspective" ? (2 * Math.tan((camera.fov * Math.PI) / 360) * Math.max(depth, near)) / rect.height : camera.width / rect.width);
    const center = new THREE.Vector3(), out = [];
    for (const entry of this.#entries.values()) {
      if (!entry.pickable || (!entry.clipping && planes.length > 0)) continue;
      this.#place(entry, camera.position);
      entry.wrapper.updateMatrixWorld(true);
      const best = new Map();
      const stack = [entry.root];
      while (stack.length > 0) {
        const node = stack.pop();
        spend(1);
        if (!node.visible) continue;
        for (const child of node.children) stack.push(child);
        if (!(node.isMesh || node.isLine || node.isPoints || node.isSprite) || !node.layers.test(threeCamera.layers)) continue;
        const geometry = node.geometry;
        const position = geometry?.attributes?.position;
        const count = node.isSprite ? 1 : (geometry?.index?.count ?? position?.count ?? 0) * (node.isInstancedMesh ? node.count : 1);
        spend(count);
        if (node.isLine || node.isPoints) {
          if (geometry.boundingSphere === null) geometry.computeBoundingSphere();
          center.copy(geometry.boundingSphere.center).applyMatrix4(node.matrixWorld);
          const threshold = entry.pickTolerance * perPixel(center.x * forward[0] + center.y * forward[1] + center.z * forward[2]);
          raycaster.params.Line.threshold = threshold;
          raycaster.params.Points.threshold = threshold;
        }
        const found = [];
        node.raycast(raycaster, found);
        for (const hit of found) {
          const material = Array.isArray(node.material) ? node.material[hit.face?.materialIndex ?? 0] : node.material;
          if (!materialDrawn(material) || !kept(hit.point) || !(hit.distance >= near)) continue;
          const part = entry.kind === "geometry" ? entry.partOf.get(node) : undefined;
          const key = part ?? 0;
          const previous = best.get(key);
          if (previous === undefined || hit.distance < previous.distance) best.set(key, { distance: hit.distance, node, part });
        }
      }
      for (const { distance, node, part } of best.values()) {
        const hit = { kind: entry.kind, id: entry.id, priority: entry.priority, distance,
          position: Object.freeze({ origin: ray.origin, offset: Object.freeze([0, 1, 2].map((k) => ray.offset[k] + ray.direction[k] * distance)) }) };
        if (part !== undefined) hit.part = part; else hit.object = node;
        out.push(Object.freeze(hit));
      }
    }
    return out;
  }
}
