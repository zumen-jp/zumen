// 3D のシーン（ZMS3）を利用側の Canvas へ WebGL2 で描き、要素の ID で選ぶ表示部品（docs/view3d.md）。
// three.js は同梱せず、利用側と同じ ESM の実体を peer dependency として使う。
import * as THREE from "three";
import { readScene3D } from "@zumen-jp/zumen";
import { View3DError, color, cross, dot, fail, finite, invalid, isObject, length, normalize, positive, scale, sub, vector } from "./common.js";
import { Extras, compareHits, sameUnit } from "./extras.js";
import { InputState, bindInput, normalizeInput, readInputOptions } from "./input.js";

/** 検査を通した three.js の r 版。範囲外では互換を推測して続けない。 */
export const THREE_REVISION = "187";

const DEFAULT_LIMITS = Object.freeze({
  instances: 1000000,
  gpuBytes: 536870912,
  work: 100000000,
  canvasPixels: 16777216,
  stateItems: 100000,
  views: 16,
  clips: 16,
  hits: 1000,
  issues: 10000,
  readBytes: 536870912,
  readAllocatedBytes: 268435456,
  readWork: 100000000,
});
const OPTION_KEYS = new Set(["width", "height", "pixelRatio", "background", "unresolvedColor", "selectionColor", "edgeColor",
  "shading", "edges", "antialias", "limits", "views"]);
const ZERO = new Float32Array(16);
const IDENTITY = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);

export { View3DError };

function readLimits(options) {
  if (options === undefined) return DEFAULT_LIMITS;
  if (!isObject(options)) invalid("limits はオブジェクト");
  const limits = { ...DEFAULT_LIMITS };
  for (const [key, value] of Object.entries(options)) {
    if (!(key in DEFAULT_LIMITS)) invalid(`未知の上限: ${key}`);
    if (!Number.isSafeInteger(value) || value <= 0) invalid(`上限は正の整数: ${key}`);
    limits[key] = value;
  }
  return Object.freeze(limits);
}

function readSize(width, height, pixelRatio, limits) {
  positive(width, "幅は正の有限値"); positive(height, "高さは正の有限値");
  if (finite(pixelRatio, "pixelRatio は有限値") <= 0 || pixelRatio > 8) invalid("pixelRatio は 0 より大きく 8 以下");
  if (Math.ceil(width * pixelRatio) * Math.ceil(height * pixelRatio) > limits.canvasPixels) fail("limit", "Canvas の画素数の上限");
  return Object.freeze({ width, height, pixelRatio });
}

function sameDocument(a, b) { return a !== null && b !== null && a.doc === b.doc && a.format === b.format && a.hash === b.hash; }
function readDocument(value) {
  if (!isObject(value)) invalid("document は文書の識別");
  for (const key of ["doc", "format", "hash"]) if (typeof value[key] !== "string" || value[key] === "") invalid(`document.${key} は空でない文字列`);
  return Object.freeze({ doc: value.doc, format: value.format, hash: value.hash });
}

function readCamera(value) {
  if (!isObject(value)) invalid("camera はオブジェクト");
  const position = vector(value.position, "camera.position は有限の 3 値");
  const direction = vector(value.direction, "camera.direction は有限の 3 値");
  const up = vector(value.up, "camera.up は有限の 3 値");
  const d = length(direction), u = length(up);
  if (!(d > 0) || !(u > 0) || !Number.isFinite(d) || !Number.isFinite(u)) invalid("方向と上方向は 0 でない");
  const side = length(cross(scale(direction, 1 / d), scale(up, 1 / u)));
  if (!(side > 1e-9)) invalid("方向と上方向は平行でない");
  if (value.projection === "perspective") {
    const fov = finite(value.fov, "fov は縦の視野角（度）");
    if (fov <= 0 || fov >= 180) invalid("fov は 0 より大きく 180 未満");
    return Object.freeze({ projection: "perspective", position: Object.freeze(position), direction: Object.freeze(direction), up: Object.freeze(up), fov });
  }
  if (value.projection === "orthographic") {
    const width = positive(value.width, "平行投影の width は正の有限値");
    return Object.freeze({ projection: "orthographic", position: Object.freeze(position), direction: Object.freeze(direction), up: Object.freeze(up), width });
  }
  return invalid("projection は perspective か orthographic");
}

/** 面の点と法線。法線の向きの半空間を取り除く。 */
function readPlane(value, message) {
  if (!isObject(value)) invalid(`${message}は { point, normal }`);
  for (const key of Object.keys(value)) if (key !== "point" && key !== "normal") invalid(`${message}の未知の項目: ${key}`);
  const point = vector(value.point, `${message}の point は有限の 3 値`);
  const normal = vector(value.normal, `${message}の normal は有限の 3 値`);
  const n = length(normal);
  if (!(n > 0) || !Number.isFinite(n)) invalid(`${message}の法線は 0 でない`);
  return Object.freeze({ point: Object.freeze(point), normal: Object.freeze(normal) });
}

/** 画面で切る箱。外側を取り除く 6 面と同じ。要素を絞る箱ではない。 */
function readBox(value) {
  if (value === undefined || value === null) return null;
  if (!isObject(value)) invalid("box は { min, max }");
  for (const key of Object.keys(value)) if (key !== "min" && key !== "max") invalid(`box の未知の項目: ${key}`);
  const min = vector(value.min, "box.min は有限の 3 値"), max = vector(value.max, "box.max は有限の 3 値");
  if (min.some((v, k) => !(v <= max[k]))) invalid("box の min は max 以下");
  return Object.freeze({ min: Object.freeze(min), max: Object.freeze(max) });
}

function boxPlanes(box) {
  if (box === null) return [];
  const planes = [];
  for (let k = 0; k < 3; k++) {
    const plus = [0, 0, 0], minus = [0, 0, 0];
    plus[k] = 1; minus[k] = -1;
    planes.push(Object.freeze({ point: box.max, normal: Object.freeze(plus) }), Object.freeze({ point: box.min, normal: Object.freeze(minus) }));
  }
  return planes;
}

/** レビュー文書と同じ #RRGGBB か #AARRGGBB。透明度は保つが描かない。 */
function reviewColor(value, message) {
  if (typeof value !== "string" || !/^#([0-9a-f]{6}|[0-9a-f]{8})$/i.test(value)) invalid(message);
  return value.toUpperCase();
}

function idList(value, message) {
  if (value === undefined) return [];
  if (!Array.isArray(value)) invalid(`${message}は要素の ID の配列`);
  for (const id of value) if (typeof id !== "string") invalid(`${message}の要素の ID は文字列`);
  return value;
}

const VIEW_KEYS = ["id", "rect", "camera", "clips", "box", "selected", "visible", "exceptions", "colored"];

/** ビューの形を検査する。要素の ID はシーンと照らす前の値のまま返す。 */
function readViews(values, limits) {
  if (!Array.isArray(values)) invalid("views は配列");
  if (values.length > limits.views) fail("limit", "ビューの数の上限");
  const ids = new Set(), views = [];
  for (const value of values) {
    if (!isObject(value)) invalid("ビューはオブジェクト");
    for (const key of Object.keys(value)) if (!VIEW_KEYS.includes(key)) invalid(`ビューの未知の項目: ${key}`);
    if (typeof value.id !== "string" || value.id === "" || ids.has(value.id)) invalid("ビューの ID は重ならない空でない文字列");
    ids.add(value.id);
    if (!isObject(value.rect)) invalid("rect は CSS px の四角");
    const rect = Object.freeze({
      x: finite(value.rect.x, "rect.x は有限値"), y: finite(value.rect.y, "rect.y は有限値"),
      width: positive(value.rect.width, "rect.width は正の有限値"), height: positive(value.rect.height, "rect.height は正の有限値"),
    });
    if (value.clips !== undefined && !Array.isArray(value.clips)) invalid("clips は切断面の配列");
    const clips = Object.freeze((value.clips ?? []).map((plane, i) => readPlane(plane, `clips[${i}]`)));
    const box = readBox(value.box);
    if (clips.length + (box === null ? 0 : 6) > limits.clips) fail("limit", "切断面の数の上限");
    if (value.visible !== undefined && typeof value.visible !== "boolean") invalid("visible は真偽値");
    if (value.colored !== undefined && !Array.isArray(value.colored)) invalid("colored は { color, ids } の配列");
    const colored = (value.colored ?? []).map((entry, i) => {
      if (!isObject(entry)) invalid(`colored[${i}] は { color, ids }`);
      for (const key of Object.keys(entry)) if (key !== "color" && key !== "ids") invalid(`colored[${i}] の未知の項目: ${key}`);
      return { color: reviewColor(entry.color, `colored[${i}].color は #RRGGBB か #AARRGGBB`), ids: idList(entry.ids, `colored[${i}].ids `) };
    });
    views.push({
      id: value.id, rect, camera: readCamera(value.camera), clips, box,
      selected: idList(value.selected, "selected "), visible: value.visible ?? true, exceptions: idList(value.exceptions, "exceptions "), colored,
    });
  }
  for (let i = 0; i < views.length; i++) {
    for (let j = i + 1; j < views.length; j++) {
      const a = views[i].rect, b = views[j].rect;
      if (a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height) invalid("ビューの枠は重ならない");
    }
  }
  return views;
}

/** 選択・非表示・色を持たないビューの表示の状態。 */
const PLAIN = Object.freeze({ selected: new Set(), visible: true, exceptions: new Set(), colors: new Map(), key: "v:" });
const isHidden = (display, element) => (display.visible ? display.exceptions.has(element) : !display.exceptions.has(element));
const ANGLE = Math.PI / 180;

/** 行優先の 3 行 4 列が、まとめ描きで保てる相似（反転なし・一様な尺度）か。 */
function similarity(t) {
  const c0 = [t[0], t[4], t[8]], c1 = [t[1], t[5], t[9]], c2 = [t[2], t[6], t[10]];
  const l0 = dot(c0, c0), l1 = dot(c1, c1), l2 = dot(c2, c2);
  const tolerance = 1e-6 * Math.max(l0, l1, l2);
  if (Math.abs(l0 - l1) > tolerance || Math.abs(l0 - l2) > tolerance) return false;
  if (Math.abs(dot(c0, c1)) > tolerance || Math.abs(dot(c0, c2)) > tolerance || Math.abs(dot(c1, c2)) > tolerance) return false;
  return dot(cross(c0, c1), c2) > 0;
}

function columnMajor(t, target, at) {
  target[at] = t[0]; target[at + 1] = t[4]; target[at + 2] = t[8]; target[at + 3] = 0;
  target[at + 4] = t[1]; target[at + 5] = t[5]; target[at + 6] = t[9]; target[at + 7] = 0;
  target[at + 8] = t[2]; target[at + 9] = t[6]; target[at + 10] = t[10]; target[at + 11] = 0;
  target[at + 12] = t[3]; target[at + 13] = t[7]; target[at + 14] = t[11]; target[at + 15] = 1;
}

/** 後始末と通知を全部行い、最初の例外だけを返す。 */
function runAll(functions) {
  let first = null;
  for (const fn of functions) {
    try { fn(); } catch (error) { if (first === null) first = { error }; }
  }
  if (first !== null) throw first.error;
}

export class View3D {
  #canvas; #renderer; #limits; #size; #options;
  #root = new THREE.Scene();
  #state = null;
  #scene = null;
  #views = Object.freeze([]);
  #cameras = new Map();
  /** ビュー ID から、行に解決した選択・非表示・色と切断面 */
  #displays = new Map();
  #subscribers = new Set();
  #listeners = { render: new Set(), contextlost: new Set(), contextrestored: new Set(), pick: new Set(), input: new Set() };
  /** 利用側の追加描画。部品が作った資源と借用した Object3D を分けて持つ */
  #extras;
  #input;
  #binding = null;
  #handling = false;
  #inputCamera = false;
  #frame = 0;
  #lost = false;
  #destroyed = false;
  #onLost; #onRestored;

  constructor(canvas, options = {}) {
    if (THREE.REVISION !== THREE_REVISION) fail("three-version", `three.js r${THREE.REVISION} は検査した r${THREE_REVISION} と異なる`);
    if (!canvas || typeof canvas.getContext !== "function") invalid("Canvas を渡す");
    if (!isObject(options)) invalid("options はオブジェクト");
    for (const key of Object.keys(options)) if (!OPTION_KEYS.has(key)) invalid(`未知の設定: ${key}`);
    this.#limits = readLimits(options.limits);
    const width = options.width ?? (canvas.clientWidth || canvas.width);
    const height = options.height ?? (canvas.clientHeight || canvas.height);
    this.#size = readSize(width, height, options.pixelRatio ?? 1, this.#limits);
    const shading = options.shading ?? "lambert";
    if (shading !== "lambert" && shading !== "unlit") invalid("shading は lambert か unlit");
    if (options.edges !== undefined && typeof options.edges !== "boolean") invalid("edges は真偽値");
    if (options.antialias !== undefined && typeof options.antialias !== "boolean") invalid("antialias は真偽値");
    this.#options = Object.freeze({
      background: color(options.background ?? "#ffffff", "background は #rrggbb"),
      unresolved: color(options.unresolvedColor ?? "#b4b4b4", "unresolvedColor は #rrggbb"),
      selection: color(options.selectionColor ?? "#ff8800", "selectionColor は #rrggbb"),
      edge: color(options.edgeColor ?? "#202020", "edgeColor は #rrggbb"),
      shading, edges: options.edges ?? true,
    });
    const views = options.views === undefined ? null : this.#resolveViews(readViews(options.views, this.#limits));
    const context = canvas.getContext("webgl2", { antialias: options.antialias ?? true, alpha: false, depth: true, stencil: false, preserveDrawingBuffer: false });
    if (!context) fail("webgl2-unavailable", "WebGL2 を使えない");
    try {
      this.#renderer = new THREE.WebGLRenderer({ canvas, context });
    } catch (error) {
      fail("webgl2-unavailable", "WebGL2 の描画を始められない", error);
    }
    this.#canvas = canvas;
    this.#extras = new Extras(this.#limits, canvas.ownerDocument ?? null);
    this.#root.add(this.#extras.group);
    this.#input = new InputState({
      view: (id) => this.#views.find((v) => v.id === id) ?? null,
      viewAt: (p) => this.#views.find((v) => p.x >= v.rect.x && p.x <= v.rect.x + v.rect.width && p.y >= v.rect.y && p.y <= v.rect.y + v.rect.height) ?? null,
      hitTest: (id, p) => this.hitTest(id, p),
      getRay: (id, p) => this.getRay(id, p),
      center: () => this.#scene?.extent?.center ?? null,
      setCamera: (id, camera) => this.#setCamera(id, camera),
      select: (ids) => { if (this.#scene !== null) this.select(this.#scene.document, ids); },
      emit: (type, event) => this.#emit(type, event),
    });
    if (views !== null) { this.#views = views.views; this.#displays = views.displays; }
    this.#renderer.autoClear = false;
    this.#renderer.setPixelRatio(this.#size.pixelRatio);
    this.#renderer.setSize(this.#size.width, this.#size.height, false);
    if (shading === "lambert") {
      this.#root.add(new THREE.AmbientLight(0xffffff, 1.2));
      const light = new THREE.DirectionalLight(0xffffff, 1.6);
      light.position.set(0.4, 0.6, 1);
      this.#root.add(light);
    }
    this.#onLost = (event) => {
      event.preventDefault();
      this.#lost = true;
      this.#cancelFrame();
      this.#changed();
      this.#emit("contextlost", { type: "contextlost" });
    };
    this.#onRestored = () => {
      this.#lost = false;
      this.#changed();
      this.#emit("contextrestored", { type: "contextrestored" });
      this.invalidate();
    };
    canvas.addEventListener("webglcontextlost", this.#onLost);
    canvas.addEventListener("webglcontextrestored", this.#onRestored);
  }

  #alive() { if (this.#destroyed) fail("destroyed", "破棄した表示部品"); }

  #changed() { this.#state = null; this.#notify(); }

  #notify() {
    const state = this.getState();
    runAll([...this.#subscribers].map((fn) => () => fn(state)));
  }

  #emit(type, event) { runAll([...this.#listeners[type]].map((fn) => () => fn(event))); }

  #cancelFrame() {
    if (this.#frame !== 0 && typeof cancelAnimationFrame === "function") cancelAnimationFrame(this.#frame);
    this.#frame = 0;
  }

  /** 状態が変わるまで同じ読み取り専用の snapshot。three.js のオブジェクトは含めない。 */
  getState() {
    if (this.#state !== null) return this.#state;
    const scene = this.#scene;
    this.#state = Object.freeze({
      document: scene?.document ?? null,
      scene: scene === null ? null : Object.freeze({
        unit: scene.result.unit, tolerance: scene.result.tolerance, counts: scene.counts,
        map: scene.result.map ?? null, heightCrs: scene.result.height_crs ?? null,
      }),
      views: this.#views,
      objects: this.#extras.state(),
      size: this.#size,
      contextLost: this.#lost,
      destroyed: this.#destroyed,
    });
    return this.#state;
  }

  subscribe(listener) {
    this.#alive();
    if (typeof listener !== "function") invalid("購読は関数");
    if (this.#subscribers.size >= this.#limits.stateItems) fail("limit", "購読の数の上限");
    const entry = (state) => listener(state);
    this.#subscribers.add(entry);
    return () => { this.#subscribers.delete(entry); };
  }

  on(type, listener) {
    this.#alive();
    if (!Object.prototype.hasOwnProperty.call(this.#listeners, type)) invalid(`未知の通知: ${type}`);
    if (typeof listener !== "function") invalid("通知は関数");
    if (this.#listeners[type].size >= this.#limits.stateItems) fail("limit", "通知の数の上限");
    const entry = (event) => listener(event);
    this.#listeners[type].add(entry);
    return () => { this.#listeners[type].delete(entry); };
  }

  setSize(width, height, pixelRatio = this.#size.pixelRatio) {
    this.#alive();
    this.#size = readSize(width, height, pixelRatio, this.#limits);
    this.#renderer.setPixelRatio(this.#size.pixelRatio);
    this.#renderer.setSize(this.#size.width, this.#size.height, false);
    this.#changed();
    this.invalidate();
  }

  /** ID・CSS px の枠・カメラ・切断面・要素の状態を一括で検査し、成功時だけ置き換える。 */
  setViews(views) {
    this.#alive();
    const resolved = this.#resolveViews(readViews(views, this.#limits));
    // 利用側がビューを差し替えたら操作を中断する。操作が動かすカメラは #setCamera を通る
    const cleanup = this.#inputCamera ? { failed: false } : this.#input.reset("views");
    this.#commitViews(resolved);
    if (cleanup.failed) throw cleanup.error;
  }

  /** 入力の操作がカメラを動かす。ビューの他の状態は保つ。 */
  #setCamera(viewId, camera) {
    this.#inputCamera = true;
    try { this.setViews(this.#views.map((v) => (v.id === viewId ? { ...v, camera } : v))); }
    finally { this.#inputCamera = false; }
  }

  /** 要素の ID を表示中のシーンの行に解決する。シーンがなければ ID を持てない。 */
  #resolveViews(views, scene = this.#scene) {
    const displays = new Map(), snapshots = [];
    for (const view of views) {
      const items = view.selected.length + view.exceptions.length + view.colored.reduce((n, entry) => n + entry.ids.length, 0);
      if (items > this.#limits.stateItems) fail("limit", "状態の件数の上限");
      if (items > 0 && scene === null) fail("no-scene", "シーンがないビューに要素を指定した");
      const rows = (ids) => {
        const out = new Set();
        for (const id of ids) {
          const row = scene.data.elementById(id);
          if (row === undefined) fail("unknown-element", `シーンにない要素: ${id}`);
          out.add(row);
        }
        return out;
      };
      const sorted = (set) => [...set].sort((a, b) => a - b);
      const selected = rows(view.selected), exceptions = rows(view.exceptions), colors = new Map(), groups = new Map();
      for (const entry of view.colored) {
        const paint = new THREE.Color(Number.parseInt(entry.color.slice(-6), 16));
        for (const row of rows(entry.ids)) {
          if (colors.has(row)) invalid(`同じ要素に 2 つの色: ${scene.data.element(row).id}`);
          colors.set(row, paint);
          let group = groups.get(entry.color);
          if (group === undefined) groups.set(entry.color, (group = []));
          group.push(row);
        }
      }
      const idOf = (row) => scene.data.element(row).id;
      const key = `${view.visible ? "v" : "h"}:${sorted(exceptions).join(",")}`;
      const planes = [...view.clips, ...boxPlanes(view.box)].map((plane) => ({ point: plane.point, normal: normalize(plane.normal) }));
      displays.set(view.id, Object.freeze({ selected, visible: view.visible, exceptions, colors, key, planes }));
      snapshots.push(Object.freeze({
        id: view.id, rect: view.rect, camera: view.camera, clips: view.clips, box: view.box,
        selected: Object.freeze(sorted(selected).map(idOf)), visible: view.visible, exceptions: Object.freeze(sorted(exceptions).map(idOf)),
        colored: Object.freeze([...groups].map(([color, members]) => Object.freeze({ color, ids: Object.freeze(sorted(members).map(idOf)) }))),
      }));
    }
    return { views: Object.freeze(snapshots), displays };
  }

  /** 稜線を先に用意し、成功したときだけビューを置き換える。 */
  #commitViews(resolved) {
    if (this.#scene !== null) this.#prepareEdges(this.#scene, resolved.displays);
    this.#views = resolved.views;
    this.#displays = resolved.displays;
    this.#cameras.clear();
    if (this.#scene !== null) this.#pruneEdges(this.#scene);
    this.#changed();
    this.invalidate();
  }

  /** 要素の状態を消したビュー。シーンを差し替えたときに使う。 */
  #plainViews() {
    return this.#views.map((view) => ({ ...view, selected: [], visible: true, exceptions: [], colored: [] }));
  }

  /**
   * blob・同じ要求の応答・同じ版の文書識別の組か null。検査と資源の作成に成功したときだけ差し替える。
   * 選択と非表示は新しい文書へ引き継がない。
   */
  setScene(input) {
    this.#alive();
    if (input === null) {
      const old = this.#scene;
      this.#scene = null;
      const resolved = this.#resolveViews(this.#plainViews(), null);
      this.#views = resolved.views; this.#displays = resolved.displays;
      const tasks = this.#sceneChanged(null);
      if (old !== null) tasks.unshift(() => this.#dispose(old));
      try { runAll(tasks); } finally {
        this.#changed();
        this.invalidate();
      }
      return null;
    }
    const built = this.#build(input);
    const old = this.#scene;
    const resolved = this.#resolveViews(this.#plainViews(), built);
    this.#scene = built;
    this.#views = resolved.views; this.#displays = resolved.displays;
    const tasks = this.#sceneChanged(built.result.unit);
    if (old !== null) tasks.unshift(() => this.#dispose(old));
    try { runAll(tasks); } finally {
      this.#changed();
      this.invalidate();
    }
    return built.report;
  }

  /**
   * シーンの差し替えに伴う後始末。操作を中断し、scope が scene の追加描画と、新しいシーンと単位の違う viewer の追加描画を外す。
   * viewer の追加描画は残すが、古い文書の要素の参照は引き継がない。
   */
  #sceneChanged(unit) {
    const tasks = [() => { const cleanup = this.#input.reset("scene"); if (cleanup.failed) throw cleanup.error; }];
    for (const entry of this.#extras.entries()) {
      if (entry.scope === "scene") tasks.push(...this.#extras.remove(entry, "scene"));
      else if (unit !== null && !sameUnit(entry.unit, unit)) tasks.push(...this.#extras.remove(entry, "unit"));
    }
    return tasks;
  }

  #build(input) {
    if (!isObject(input)) invalid("シーンの入力は { blob, result, document }");
    for (const key of Object.keys(input)) if (!["blob", "result", "document"].includes(key)) invalid(`シーンの入力の未知の項目: ${key}`);
    const document = readDocument(input.document);
    const result = input.result;
    if (!isObject(result) || !isObject(result.counts)) invalid("result は scene3d の応答の result");
    if (result.doc !== document.doc) invalid("応答と文書の名前が違う");
    const limits = this.#limits;
    let data;
    try {
      data = readScene3D(input.blob, { maxBytes: limits.readBytes, maxAllocatedBytes: limits.readAllocatedBytes, maxWork: limits.readWork });
    } catch (error) {
      fail(error instanceof RangeError && /上限/.test(error.message) ? "limit" : "invalid-scene", `シーンを読めない: ${error.message}`, error);
    }
    const counts = Object.freeze({
      chunks: data.chunks, meshes: data.rows("mesh_vertices"), instances: data.rows("instance_meshes"), elements: data.rows("element_ids"),
      colors: data.rows("color_rgba"), vertices: data.rows("vertex_xyz"), triangles: data.rows("triangle_vertices"), edges: data.rows("edge_vertices"),
      points: data.rows("point_xyz"),
    });
    for (const key of ["chunks", "meshes", "instances", "elements", "colors", "vertices", "triangles", "edges"]) {
      if (result.counts[key] !== counts[key]) invalid(`応答と blob の ${key} の数が違う`);
    }
    if ((result.counts.points ?? 0) !== counts.points) invalid("応答と blob の points の数が違う");
    if (counts.instances > limits.instances) fail("limit", "インスタンスの数の上限");
    let work = 0;
    const spend = (n) => { work += n; if (work > limits.work) fail("limit", "作業量の上限"); };
    const c = data.columns;
    const options = this.#options;
    // 色ごとの線形の RGB と不透明度。未解決は利用側の代替色
    const colors = new Float32Array(counts.colors * 4);
    for (let row = 0; row < counts.colors; row++) {
      spend(1);
      const rgba = c.color_rgba.subarray(row * 4, row * 4 + 4), unresolved = (c.color_flags[row] & 1) !== 0;
      const value = unresolved ? options.unresolved : new THREE.Color().setRGB(rgba[0], rgba[1], rgba[2], THREE.SRGBColorSpace);
      colors.set([value.r, value.g, value.b, rgba[3]], row * 4);
    }
    // 確保の前に、まとめ描きの組と GPU へ渡すバイト数を数える
    const groups = new Map(), meshUsed = new Uint8Array(counts.meshes);
    const batchOf = new Int32Array(counts.instances).fill(-1);
    let individual = 0, transparent = 0, edgeRows = 0, gpuBytes = 0;
    const unresolvedElements = new Set();
    for (let chunk = 0; chunk < data.chunks; chunk++) {
      const begin = c.chunk_instances[chunk], end = c.chunk_instances[chunk + 1] ?? counts.instances;
      for (let row = begin; row < end; row++) {
        spend(1);
        const mesh = c.instance_meshes[row], colorRow = c.instance_colors[row];
        if ((c.color_flags[colorRow] & 1) !== 0) unresolvedElements.add(c.instance_elements[row]);
        const alpha = colors[colorRow * 4 + 3];
        if (alpha === 0) { transparent++; continue; }
        const transform = c.instance_transforms.subarray(row * 12, row * 12 + 12);
        const shared = similarity(transform);
        const key = shared ? `${chunk}/${mesh}/${alpha}` : `i${row}`;
        let group = groups.get(key);
        if (group === undefined) {
          group = { chunk, mesh, alpha, shared, rows: [] };
          groups.set(key, group);
          gpuBytes += 128;
        }
        if (!shared) individual++;
        group.rows.push(row);
        gpuBytes += 64 + 12;
        meshUsed[mesh] = 1;
        if (options.edges) {
          const edges = (c.mesh_edges[mesh + 1] ?? counts.edges) - c.mesh_edges[mesh];
          edgeRows += edges;
          gpuBytes += edges * 24;
        }
      }
    }
    for (let mesh = 0; mesh < counts.meshes; mesh++) {
      if (meshUsed[mesh] === 0) continue;
      spend(1);
      const vertices = (c.mesh_vertices[mesh + 1] ?? counts.vertices) - c.mesh_vertices[mesh];
      const triangles = (c.mesh_triangles[mesh + 1] ?? counts.triangles) - c.mesh_triangles[mesh];
      gpuBytes += vertices * 12 + triangles * 12;
    }
    if (gpuBytes + this.#extras.bytes((entry) => entry.scope === "viewer" && sameUnit(entry.unit, result.unit)) > limits.gpuBytes) fail("limit", "GPU へ渡すバイト数の上限");
    spend(edgeRows * 2);

    // ここから資源を作る。失敗したら作った分を解放し、前のシーンを保つ
    const built = {
      data, result, document, counts, colors, batchOf, slotOf: new Uint32Array(counts.instances),
      batches: [], geometries: new Map(), materials: new Map(), chunks: [], bounds: new Map(),
      edgeSets: new Map(), edgeMaterial: null, edgeBytes: 0, applied: PLAIN,
      elementInstances: null, extent: null, report: null,
    };
    try {
      const geometry = (mesh) => {
        let value = built.geometries.get(mesh);
        if (value === undefined) {
          const view = data.mesh(mesh);
          value = new THREE.BufferGeometry();
          // 型と配置が合う列は blob のビューを共有する。GPU への転送は別の確保
          value.setAttribute("position", new THREE.BufferAttribute(view.vertices, 3));
          value.setIndex(new THREE.BufferAttribute(view.triangles, 1));
          built.geometries.set(mesh, value);
        }
        return value;
      };
      const material = (alpha) => {
        let value = built.materials.get(alpha);
        if (value === undefined) {
          const parameters = { color: 0xffffff, side: THREE.DoubleSide, transparent: alpha < 1, opacity: alpha, depthWrite: alpha >= 1 };
          // 面の法線は画面上の微分から求める。反転・非一様な尺度でも法線の列を作らない
          value = options.shading === "lambert" ? new THREE.MeshLambertMaterial({ ...parameters, flatShading: true }) : new THREE.MeshBasicMaterial(parameters);
          built.materials.set(alpha, value);
        }
        return value;
      };
      for (let chunk = 0; chunk < data.chunks; chunk++) {
        const group = new THREE.Group();
        group.matrixAutoUpdate = false;
        built.chunks.push(group);
      }
      const matrix = new THREE.Matrix4(), paint = new THREE.Color();
      for (const group of groups.values()) {
        const mesh = new THREE.InstancedMesh(geometry(group.mesh), material(group.alpha), group.rows.length);
        mesh.frustumCulled = false;
        mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
        const batch = built.batches.length;
        if (!group.shared) {
          // まとめ描きで表せない変換は、個別の Mesh の変換で保つ
          const t = c.instance_transforms.subarray(group.rows[0] * 12, group.rows[0] * 12 + 12);
          columnMajor(t, matrix.elements, 0);
          mesh.matrixAutoUpdate = false;
          mesh.matrix.copy(matrix);
        }
        for (const [slot, row] of group.rows.entries()) {
          spend(1);
          built.batchOf[row] = batch; built.slotOf[row] = slot;
          if (group.shared) columnMajor(c.instance_transforms.subarray(row * 12, row * 12 + 12), mesh.instanceMatrix.array, slot * 16);
          else mesh.instanceMatrix.array.set(IDENTITY, slot * 16);
          const colorRow = c.instance_colors[row];
          paint.setRGB(colors[colorRow * 4], colors[colorRow * 4 + 1], colors[colorRow * 4 + 2]);
          mesh.setColorAt(slot, paint);
        }
        mesh.userData.zumen = { batch };
        built.batches.push({ mesh, rows: group.rows, shared: group.shared, chunk: group.chunk });
        built.chunks[group.chunk].add(mesh);
      }
      // 要素から配置を引く索引
      const starts = new Uint32Array(counts.elements + 1), order = new Uint32Array(counts.instances);
      for (let row = 0; row < counts.instances; row++) starts[c.instance_elements[row] + 1]++;
      for (let e = 0; e < counts.elements; e++) starts[e + 1] += starts[e];
      const fill = starts.slice(0, counts.elements);
      for (let row = 0; row < counts.instances; row++) order[fill[c.instance_elements[row]]++] = row;
      built.elementInstances = { starts, order };
      // 範囲は f64 の原点と f32 の相対値から求める
      let min = null, max = null;
      for (let chunk = 0; chunk < data.chunks; chunk++) {
        const view = data.chunk(chunk);
        const low = [0, 1, 2].map((k) => view.origin[k] + view.bounds[k]), high = [0, 1, 2].map((k) => view.origin[k] + view.bounds[k + 3]);
        min = min === null ? low : min.map((v, k) => Math.min(v, low[k]));
        max = max === null ? high : max.map((v, k) => Math.max(v, high[k]));
      }
      if (min !== null) {
        const center = [0, 1, 2].map((k) => min[k] + (max[k] - min[k]) / 2);
        built.extent = { min, max, center, radius: Math.max(length(sub(max, min)) / 2, 1e-9) };
      }
      if (options.edges) {
        built.edgeMaterial = new THREE.LineBasicMaterial({ color: options.edge });
        const lines = [];
        built.edgeSets.set(PLAIN.key, lines);
        for (let chunk = 0; chunk < data.chunks; chunk++) lines.push(this.#edges(built, chunk, PLAIN));
      }
      const issues = [];
      let issueCount = 0;
      const issue = (value) => { issueCount++; if (issues.length < limits.issues) issues.push(Object.freeze(value)); };
      for (const row of [...unresolvedElements].sort((a, b) => a - b)) {
        issue({ reason: "color-unresolved", disposition: "substituted", element: data.element(row).id });
      }
      if (counts.points > 0) issue({ reason: "points-unsupported", disposition: "omitted", count: counts.points });
      built.report = Object.freeze({
        drawn: Object.freeze({ instances: counts.instances - transparent, batches: built.batches.length, individual, edges: edgeRows }),
        skipped: Object.freeze({ transparent }),
        gpuBytes,
        issues: Object.freeze(issues),
        truncatedIssues: issueCount - issues.length,
      });
      return built;
    } catch (error) {
      this.#dispose(built);
      throw error;
    }
  }

  /** 非表示の状態で描く稜線の数。 */
  #edgeRows(built, display) {
    const c = built.data.columns;
    let rows = 0;
    for (let row = 0; row < built.counts.instances; row++) {
      if (built.batchOf[row] < 0 || isHidden(display, c.instance_elements[row])) continue;
      const mesh = c.instance_meshes[row];
      rows += (c.mesh_edges[mesh + 1] ?? built.counts.edges) - c.mesh_edges[mesh];
    }
    return rows;
  }

  /** ビューが使う非表示の状態ごとの稜線を用意する。資源を作る前にバイト数を数える。 */
  #prepareEdges(scene, displays) {
    if (!this.#options.edges) return;
    const missing = new Map();
    for (const display of displays.values()) if (!scene.edgeSets.has(display.key)) missing.set(display.key, display);
    let bytes = scene.report.gpuBytes + this.#extras.bytes();
    for (const [key, lines] of scene.edgeSets) if (key !== PLAIN.key) for (const line of lines) bytes += line.geometry.attributes.position.array.byteLength;
    for (const display of missing.values()) bytes += this.#edgeRows(scene, display) * 24;
    if (bytes > this.#limits.gpuBytes) fail("limit", "GPU へ渡すバイト数の上限（ビューごとの稜線）");
    const made = [];
    try {
      for (const [key, display] of missing) {
        const lines = [];
        made.push(lines);
        for (let chunk = 0; chunk < scene.chunks.length; chunk++) lines.push(this.#edges(scene, chunk, display));
        scene.edgeSets.set(key, lines);
      }
    } catch (error) {
      for (const [key] of missing) scene.edgeSets.delete(key);
      for (const lines of made) for (const line of lines) { line.removeFromParent(); line.geometry.dispose(); }
      throw error;
    }
  }

  /** どのビューも使わない稜線を解放する。非表示のない稜線は残す。 */
  #pruneEdges(scene) {
    const used = new Set([PLAIN.key, ...[...this.#displays.values()].map((d) => d.key)]);
    for (const [key, lines] of scene.edgeSets) {
      if (used.has(key)) continue;
      scene.edgeSets.delete(key);
      for (const line of lines) { line.removeFromParent(); line.geometry.dispose(); }
    }
  }

  /** 塊の稜線を、描く配置だけから塊の原点に対する f32 の座標へ展開する。 */
  #edges(built, chunk, display) {
    const { data } = built, c = data.columns;
    const begin = c.chunk_instances[chunk], end = c.chunk_instances[chunk + 1] ?? built.counts.instances;
    let rows = 0;
    for (let row = begin; row < end; row++) {
      if (built.batchOf[row] < 0 || isHidden(display, c.instance_elements[row])) continue;
      rows += data.mesh(c.instance_meshes[row]).edges.length / 2;
    }
    const positions = new Float32Array(rows * 6);
    let at = 0;
    for (let row = begin; row < end; row++) {
      if (built.batchOf[row] < 0 || isHidden(display, c.instance_elements[row])) continue;
      const mesh = data.mesh(c.instance_meshes[row]), t = c.instance_transforms.subarray(row * 12, row * 12 + 12);
      for (const vertex of mesh.edges) {
        const x = mesh.vertices[vertex * 3], y = mesh.vertices[vertex * 3 + 1], z = mesh.vertices[vertex * 3 + 2];
        positions[at++] = t[0] * x + t[1] * y + t[2] * z + t[3];
        positions[at++] = t[4] * x + t[5] * y + t[6] * z + t[7];
        positions[at++] = t[8] * x + t[9] * y + t[10] * z + t[11];
      }
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    const lines = new THREE.LineSegments(geometry, built.edgeMaterial);
    lines.frustumCulled = false;
    lines.visible = false;
    built.chunks[chunk].add(lines);
    return lines;
  }

  #dispose(built) {
    const tasks = [];
    for (const geometry of built.geometries.values()) tasks.push(() => geometry.dispose());
    for (const material of built.materials.values()) tasks.push(() => material.dispose());
    for (const batch of built.batches) tasks.push(() => batch.mesh.dispose());
    for (const lines of built.edgeSets.values()) for (const line of lines) tasks.push(() => line.geometry.dispose());
    const edgeMaterial = built.edgeMaterial;
    if (edgeMaterial !== null) tasks.push(() => edgeMaterial.dispose());
    for (const group of built.chunks) tasks.push(() => { if (group.parent !== null) group.parent.remove(group); });
    built.batches = []; built.edgeSets = new Map(); built.edgeMaterial = null; built.geometries = new Map(); built.materials = new Map(); built.chunks = [];
    runAll(tasks);
  }

  /** 表示の状態を GPU の配置へ当てる。前に当てた状態との差の要素だけを塗り直す。 */
  #apply(scene, display) {
    const before = scene.applied;
    if (before === display) return;
    let changed = null;
    if (before.visible === display.visible) {
      changed = new Set();
      for (const row of before.exceptions) if (!display.exceptions.has(row)) changed.add(row);
      for (const row of display.exceptions) if (!before.exceptions.has(row)) changed.add(row);
      for (const row of before.selected) if (!display.selected.has(row)) changed.add(row);
      for (const row of display.selected) if (!before.selected.has(row)) changed.add(row);
      for (const [row, paint] of before.colors) if (display.colors.get(row) !== paint) changed.add(row);
      for (const row of display.colors.keys()) if (!before.colors.has(row)) changed.add(row);
    }
    this.#repaint(scene, changed, display);
    scene.applied = display;
  }

  /** 要素の配置を、選択・非表示・色の状態に合わせて塗り直す。rows が null なら全部。 */
  #repaint(scene, rows, display) {
    const { starts, order } = scene.elementInstances, c = scene.data.columns;
    const paint = new THREE.Color(), touched = new Set();
    const elements = rows ?? Array.from({ length: scene.counts.elements }, (_, e) => e);
    for (const element of elements) {
      const hidden = isHidden(display, element);
      for (let i = starts[element]; i < starts[element + 1]; i++) {
        const row = order[i], batch = scene.batchOf[row];
        if (batch < 0) continue;
        const { mesh, shared } = scene.batches[batch], slot = scene.slotOf[row];
        if (shared) {
          if (hidden) mesh.instanceMatrix.array.set(ZERO, slot * 16);
          else columnMajor(c.instance_transforms.subarray(row * 12, row * 12 + 12), mesh.instanceMatrix.array, slot * 16);
        } else mesh.visible = !hidden;
        if (display.selected.has(element)) paint.copy(this.#options.selection);
        else if (display.colors.has(element)) paint.copy(display.colors.get(element));
        else {
          const colorRow = c.instance_colors[row];
          paint.setRGB(scene.colors[colorRow * 4], scene.colors[colorRow * 4 + 1], scene.colors[colorRow * 4 + 2]);
        }
        mesh.setColorAt(slot, paint);
        touched.add(mesh);
      }
    }
    for (const mesh of touched) {
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor !== null) mesh.instanceColor.needsUpdate = true;
    }
  }

  /** 対象のビュー。view を省けばすべて。 */
  #targets(options) {
    if (!isObject(options)) invalid("options はオブジェクト");
    for (const key of Object.keys(options)) if (key !== "view") invalid(`未知の設定: ${key}`);
    if (options.view === undefined) return null;
    return this.#view(options.view).id;
  }

  #update(document, options, change) {
    const scene = this.#scene;
    if (scene === null) fail("no-scene", "シーンがない");
    if (!sameDocument(readDocument(document), scene.document)) fail("other-document", "表示中と別の文書の参照");
    const target = this.#targets(options);
    const views = this.#views.map((view) => (target === null || view.id === target ? { ...view, ...change } : view));
    this.#commitViews(this.#resolveViews(readViews(views, this.#limits)));
  }

  /** 表示中の文書の要素の ID・クラス（地物では型）・名前・所属階の ID。未知の ID は null。 */
  getElement(document, id) {
    this.#alive();
    const scene = this.#scene;
    if (scene === null) fail("no-scene", "シーンがない");
    if (!sameDocument(readDocument(document), scene.document)) fail("other-document", "表示中と別の文書の参照");
    if (typeof id !== "string") invalid("要素 ID は文字列");
    const row = scene.data.elementById(id);
    if (row === undefined) return null;
    const element = scene.data.element(row);
    const storey = element.storey === null ? null : scene.data.element(element.storey).id;
    return Object.freeze({ id: element.id, class: element.class, name: element.name, storey, row });
  }

  /** 表示中の文書の要素 ID で選ぶ。別文書・未知の ID は読み替えずに断り、状態を変えない。 */
  select(document, ids, options = {}) {
    this.#alive();
    this.#update(document, options, { selected: idList(ids, "選択") });
  }

  /** 非表示の要素。非表示の要素は描画からも当たり判定からも除く。 */
  setHidden(document, ids, options = {}) {
    this.#alive();
    this.#update(document, options, { visible: true, exceptions: idList(ids, "非表示") });
  }

  /** 要素の色。#RRGGBB か #AARRGGBB で、透明度は保つが描かない。選択の色が優先する。 */
  setColors(document, colored, options = {}) {
    this.#alive();
    this.#update(document, options, { colored });
  }

  #view(viewId) {
    const view = this.#views.find((v) => v.id === viewId);
    if (view === undefined) fail("unknown-view", `ビューがない: ${String(viewId)}`);
    return view;
  }

  /** ビューのカメラを、向き・投影を保ったままシーンの範囲へ合わせる。 */
  fit(viewId, margin = 1.05) {
    this.#alive();
    const view = this.#view(viewId);
    positive(margin, "margin は正の有限値");
    const extent = this.#scene?.extent;
    if (!extent) fail("no-scene", "範囲を持つシーンがない");
    const camera = view.camera, forward = normalize(camera.direction), aspect = view.rect.width / view.rect.height;
    let next;
    if (camera.projection === "perspective") {
      const half = (camera.fov * Math.PI) / 360, horizontal = Math.atan(Math.tan(half) * aspect);
      const distance = (extent.radius * margin) / Math.sin(Math.min(half, horizontal));
      next = { ...camera, position: sub(extent.center, scale(forward, distance)) };
    } else {
      const distance = extent.radius * 2 * margin;
      next = { ...camera, position: sub(extent.center, scale(forward, distance)), width: 2 * extent.radius * margin * Math.max(1, aspect) };
    }
    this.setViews(this.#views.map((v) => (v.id === viewId ? { ...v, camera: next } : v)));
  }

  #basis(camera) {
    const forward = normalize(camera.direction);
    const right = normalize(cross(forward, camera.up));
    return { forward, right, up: cross(right, forward) };
  }

  #clipRange(camera) {
    const extent = this.#scene?.extent;
    if (!extent) return { near: 0.1, far: 1000 };
    const distance = length(sub(extent.center, camera.position)), radius = extent.radius;
    const far = (distance + radius) * 1.01 + 1e-6;
    const near = camera.projection === "orthographic" ? 0 : Math.max(far * 1e-6, (distance - radius) * 0.5);
    return { near, far };
  }

  #cameraFor(view) {
    const { camera, rect } = view, aspect = rect.width / rect.height, { near, far } = this.#clipRange(camera);
    let value = this.#cameras.get(view.id);
    if (value === undefined) {
      value = camera.projection === "perspective" ? new THREE.PerspectiveCamera() : new THREE.OrthographicCamera();
      this.#cameras.set(view.id, value);
    }
    if (camera.projection === "perspective") {
      value.fov = camera.fov; value.aspect = aspect;
    } else {
      const half = camera.width / 2;
      value.left = -half; value.right = half; value.top = half / aspect; value.bottom = -half / aspect;
    }
    value.near = near; value.far = far;
    // 共有の DEFAULT_UP は変えず、このカメラの上方向だけを決める。表示原点はカメラの位置
    value.up.set(camera.up[0], camera.up[1], camera.up[2]);
    value.position.set(0, 0, 0);
    value.lookAt(camera.direction[0], camera.direction[1], camera.direction[2]);
    value.updateProjectionMatrix();
    value.updateMatrixWorld(true);
    return value;
  }

  /** 次のフレームで描く。シーンや状態を変えた後に呼ぶ。 */
  invalidate() {
    this.#alive();
    if (this.#frame !== 0 || this.#lost || typeof requestAnimationFrame !== "function") return;
    this.#frame = requestAnimationFrame(() => { this.#frame = 0; if (!this.#destroyed) this.render(); });
  }

  /** すぐに描く。context を失っている間は描かずに false を返す。 */
  render() {
    this.#alive();
    this.#cancelFrame();
    if (this.#lost) return false;
    const renderer = this.#renderer, size = this.#size, scene = this.#scene;
    renderer.setScissorTest(true);
    renderer.setViewport(0, 0, size.width, size.height);
    renderer.setScissor(0, 0, size.width, size.height);
    renderer.setClearColor(this.#options.background, 1);
    renderer.clear(true, true, false);
    if (scene !== null) for (const group of scene.chunks) if (group.parent === null) this.#root.add(group);
    const excluded = [];
    for (const view of this.#views) {
      const { rect, camera } = view, origin = camera.position;
      if (scene !== null) {
        for (let chunk = 0; chunk < scene.chunks.length; chunk++) {
          // GPU の座標は (c - o) + t + A p。先に絶対座標を f32 にしない
          const c = scene.data.columns.chunk_origins;
          const group = scene.chunks[chunk];
          group.position.set(c[chunk * 3] - origin[0], c[chunk * 3 + 1] - origin[1], c[chunk * 3 + 2] - origin[2]);
          group.updateMatrix();
        }
      }
      const display = this.#displays.get(view.id) ?? PLAIN;
      if (scene !== null) {
        this.#apply(scene, display);
        for (const [key, lines] of scene.edgeSets) for (const line of lines) line.visible = key === display.key;
      }
      for (const id of this.#extras.prepare(camera, rect, display.planes)) excluded.push(Object.freeze({ view: view.id, id, reason: "clipping-unsupported" }));
      // 切断面は表示原点からの相対座標で渡す。three.js は符号付き距離が負の側を切るので法線を反転する
      renderer.clippingPlanes = display.planes.map(({ point, normal }) => new THREE.Plane(
        new THREE.Vector3(-normal[0], -normal[1], -normal[2]), dot(normal, sub(point, origin))));
      const y = size.height - rect.y - rect.height;
      renderer.setViewport(rect.x, y, rect.width, rect.height);
      renderer.setScissor(rect.x, y, rect.width, rect.height);
      renderer.clear(true, true, false);
      renderer.render(this.#root, this.#cameraFor(view));
    }
    renderer.clippingPlanes = [];
    renderer.setScissorTest(false);
    this.#emit("render", Object.freeze({ type: "render", views: this.#views.length, excluded: Object.freeze(excluded) }));
    return true;
  }

  /** CSS px の点から、カメラの位置（f64）を原点とする光線を返す。 */
  getRay(viewId, point) {
    this.#alive();
    const view = this.#view(viewId);
    if (!isObject(point)) invalid("点は { x, y }");
    const x = finite(point.x, "点の x は有限値"), y = finite(point.y, "点の y は有限値"), { rect, camera } = view;
    if (x < rect.x || x > rect.x + rect.width || y < rect.y || y > rect.y + rect.height) fail("outside-view", "点がビューの枠の外");
    const ndcX = ((x - rect.x) / rect.width) * 2 - 1, ndcY = 1 - ((y - rect.y) / rect.height) * 2, aspect = rect.width / rect.height;
    const { forward, right, up } = this.#basis(camera);
    if (camera.projection === "perspective") {
      const t = Math.tan((camera.fov * Math.PI) / 360);
      const direction = normalize([0, 1, 2].map((k) => forward[k] + right[k] * ndcX * t * aspect + up[k] * ndcY * t));
      return Object.freeze({ origin: Object.freeze([...camera.position]), offset: Object.freeze([0, 0, 0]), direction: Object.freeze(direction) });
    }
    const half = camera.width / 2;
    const offset = [0, 1, 2].map((k) => right[k] * ndcX * half + up[k] * ndcY * (half / aspect));
    return Object.freeze({ origin: Object.freeze([...camera.position]), offset: Object.freeze(offset), direction: Object.freeze(forward) });
  }

  /**
   * 光線に当たる要素を距離の近い順に返す。非表示・透明度 0 は除く。
   * 位置は光線の f64 原点と相対位置、距離は文書単位。同じ要素の当たりはもっとも近いものにまとめる。
   */
  hitTest(viewId, point, options = {}) {
    this.#alive();
    const ray = this.getRay(viewId, point);
    if (!isObject(options)) invalid("options はオブジェクト");
    const limit = options.limit ?? this.#limits.hits;
    if (!Number.isSafeInteger(limit) || limit <= 0 || limit > this.#limits.hits) invalid("limit は上限以下の正の整数");
    const scene = this.#scene, view = this.#view(viewId);
    const { near } = this.#clipRange(view.camera);
    const display = this.#displays.get(viewId) ?? PLAIN;
    let work = 0;
    const spend = (n) => { work += n; if (work > this.#limits.work) fail("limit", "当たり判定の作業量の上限"); };
    const extras = this.#extras.size === 0 ? [] : this.#extras.hit(ray, view.camera, this.#cameraFor(view), view.rect, display.planes, near, spend);
    if (scene === null) {
      extras.sort(compareHits);
      return Object.freeze({ hits: Object.freeze(extras.slice(0, limit)), truncated: extras.length > limit });
    }
    const { data } = scene, c = data.columns, d = ray.direction;
    // 切断面で除く条件 n · (q + o - p0) > 0 を、光線の係数 t の一次式 a + b t にする（f64）
    const cuts = display.planes.map(({ point, normal }) => [dot(normal, sub(ray.origin, point)) + dot(normal, ray.offset), dot(normal, d)]);
    const accept = (t) => cuts.every(([a, b]) => !(a + b * t > 0));
    const best = new Map();
    for (let chunk = 0; chunk < data.chunks; chunk++) {
      const begin = c.chunk_instances[chunk], end = c.chunk_instances[chunk + 1] ?? scene.counts.instances;
      // 光線の原点を塊の原点からの相対値にする（f64）
      const relative = [0, 1, 2].map((k) => ray.origin[k] - c.chunk_origins[chunk * 3 + k] + ray.offset[k]);
      for (let row = begin; row < end; row++) {
        spend(1);
        const element = c.instance_elements[row];
        if (scene.batchOf[row] < 0 || isHidden(display, element)) continue;
        const t = c.instance_transforms.subarray(row * 12, row * 12 + 12);
        const inverse = invert(t);
        const p = sub(relative, [t[3], t[7], t[11]]);
        const localOrigin = apply(inverse, p), localDirection = apply(inverse, d);
        const mesh = c.instance_meshes[row];
        const box = this.#meshBounds(scene, mesh, spend);
        if (!rayBox(localOrigin, localDirection, box)) continue;
        const view = data.mesh(mesh);
        const hit = nearest(view, localOrigin, localDirection, near, accept, spend);
        if (hit === null) continue;
        const previous = best.get(element);
        if (previous === undefined || hit < previous.distance || (hit === previous.distance && row < previous.instance)) {
          best.set(element, { distance: hit, instance: row });
        }
      }
    }
    const hits = [...best.entries()].map(([element, { distance, instance }]) => Object.freeze({
      kind: "element",
      document: scene.document,
      id: data.element(element).id,
      element,
      instance,
      distance,
      position: Object.freeze({ origin: ray.origin, offset: Object.freeze([0, 1, 2].map((k) => ray.offset[k] + d[k] * distance)) }),
    }));
    hits.push(...extras);
    hits.sort(compareHits);
    return Object.freeze({ hits: Object.freeze(hits.slice(0, limit)), truncated: hits.length > limit });
  }

  #meshBounds(scene, mesh, spend) {
    let box = scene.bounds.get(mesh);
    if (box === undefined) {
      const vertices = scene.data.mesh(mesh).vertices;
      spend(vertices.length / 3);
      box = [Infinity, Infinity, Infinity, -Infinity, -Infinity, -Infinity];
      for (let i = 0; i < vertices.length; i += 3) {
        for (let k = 0; k < 3; k++) { box[k] = Math.min(box[k], vertices[i + k]); box[k + 3] = Math.max(box[k + 3], vertices[i + k]); }
      }
      scene.bounds.set(mesh, box);
    }
    return box;
  }

  /** 文書単位が m か。BCF の厳密な往復はこの場合に限る。 */
  #metreIssues(scene) {
    const unit = scene.result.unit;
    if (isObject(unit) && unit.name === "METRE" && unit.prefix === undefined && unit.scale === undefined) return [];
    return [Object.freeze({ reason: "unit-not-metre", disposition: "kept", unit: Object.freeze({ ...unit }) })];
  }

  /**
   * ビューをレビュー文書の 3D の場所（kind が 3d の Place）にする。BCF の互換枠は 1:1 で、縦横比が違う枠は
   * frame: "contain" を指定したときだけ、枠を含む正方形へ広げて調整した値を返す。保存先・作者・時刻は持たない。
   */
  toReviewPlace(viewId, options = {}) {
    this.#alive();
    const view = this.#view(viewId), scene = this.#scene;
    if (scene === null) fail("no-scene", "シーンがない");
    if (!isObject(options)) invalid("options はオブジェクト");
    for (const key of Object.keys(options)) if (key !== "frame" && key !== "hashes") invalid(`未知の設定: ${key}`);
    const frame = options.frame ?? "exact";
    if (frame !== "exact" && frame !== "contain") invalid("frame は exact か contain");
    const hashes = readHashes(options.hashes);
    const { camera, rect } = view, aspect = rect.width / rect.height, adjustments = [];
    const projection = camera.projection === "perspective"
      ? { projection: "perspective", field_of_view: camera.fov }
      : { projection: "orthographic", width: camera.width };
    if (Math.abs(aspect - 1) > 1e-9) {
      if (frame === "exact") fail("aspect-ratio", "BCF の互換枠は 1:1。縦横比の違う枠は frame を contain にして広げる");
      if (camera.projection === "perspective") {
        const half = Math.tan((camera.fov * ANGLE) / 2), fov = (2 * Math.atan(Math.max(half, half * aspect))) / ANGLE;
        projection.field_of_view = fov;
        adjustments.push(Object.freeze({ field: "camera.field_of_view", from: camera.fov, to: fov, reason: "square-frame" }));
      } else {
        const width = Math.max(camera.width, camera.width / aspect);
        projection.width = width;
        adjustments.push(Object.freeze({ field: "camera.width", from: camera.width, to: width, reason: "square-frame" }));
      }
    }
    const reference = (id) => { const hash = hashes?.get(id); return Object.freeze(hash === undefined ? { id } : { id, hash }); };
    const place = Object.freeze({
      kind: "3d",
      camera: Object.freeze({ ...projection, position: camera.position, direction: camera.direction, up: camera.up }),
      selected: Object.freeze(view.selected.map(reference)),
      visible: view.visible,
      exceptions: Object.freeze(view.exceptions.map(reference)),
      colored: Object.freeze(view.colored.map((entry) => Object.freeze({ color: entry.color, elements: Object.freeze(entry.ids.map(reference)) }))),
      clips: Object.freeze([...view.clips, ...boxPlanes(view.box)].map((plane) => Object.freeze({ point: plane.point, normal: plane.normal }))),
    });
    const issues = Object.freeze(this.#metreIssues(scene));
    return Object.freeze({ place, strict: issues.length === 0 && adjustments.length === 0, adjustments: Object.freeze(adjustments), issues });
  }

  /**
   * レビュー文書の 3D の場所をビューへ当てる。別文書は断り、見つからない要素・ハッシュの違う要素は採用せずに報告する。
   * 1:1 のカメラは枠の中の正方形（frame）に収め、縦横比が違えば調整した値を返す。元のレビュー文書には書かない。
   */
  applyReviewPlace(viewId, document, place, options = {}) {
    this.#alive();
    const view = this.#view(viewId), scene = this.#scene;
    if (scene === null) fail("no-scene", "シーンがない");
    if (!sameDocument(readDocument(document), scene.document)) fail("other-document", "表示中と別の文書の場所");
    if (!isObject(place) || place.kind !== "3d") invalid("place は kind が 3d のレビューの場所");
    if (!isObject(options)) invalid("options はオブジェクト");
    for (const key of Object.keys(options)) if (key !== "hashes") invalid(`未知の設定: ${key}`);
    const hashes = readHashes(options.hashes), limits = this.#limits;
    const missing = [], changed = [];
    let unchecked = 0, items = 0, overridden = 0;
    const references = (value, message) => {
      if (value === undefined) return [];
      if (!Array.isArray(value)) invalid(`${message}は参照の配列`);
      items += value.length;
      if (items > limits.stateItems) fail("limit", "状態の件数の上限");
      const ids = [];
      for (const reference of value) {
        if (!isObject(reference) || typeof reference.id !== "string") invalid(`${message}の参照は { id, hash? }`);
        if (reference.hash !== undefined && typeof reference.hash !== "string") invalid(`${message}の hash は文字列`);
        if (scene.data.elementById(reference.id) === undefined) { missing.push(reference.id); continue; }
        if (reference.hash !== undefined) {
          if (hashes === undefined) unchecked++;
          else if (hashes.get(reference.id) !== reference.hash) { changed.push(reference.id); continue; }
        }
        ids.push(reference.id);
      }
      return ids;
    };
    const { rect } = view, aspect = rect.width / rect.height, adjustments = [];
    let camera = view.camera;
    if (place.camera !== undefined) {
      const value = place.camera;
      if (!isObject(value)) invalid("camera はオブジェクト");
      const base = { position: value.position, direction: value.direction, up: value.up };
      if (value.projection === "perspective") {
        const f = finite(value.field_of_view, "field_of_view は有限値");
        let fov = f;
        if (aspect < 1 - 1e-9) {
          fov = (2 * Math.atan(Math.tan((f * ANGLE) / 2) / aspect)) / ANGLE;
          adjustments.push(Object.freeze({ field: "camera.fov", from: f, to: fov, reason: "square-frame" }));
        }
        camera = readCamera({ projection: "perspective", ...base, fov });
      } else if (value.projection === "orthographic") {
        const w = positive(value.width, "平行投影の width は正の有限値");
        let width = w;
        if (aspect > 1 + 1e-9) {
          width = w * aspect;
          adjustments.push(Object.freeze({ field: "camera.width", from: w, to: width, reason: "square-frame" }));
        }
        camera = readCamera({ projection: "orthographic", ...base, width });
      } else invalid("projection は perspective か orthographic");
    }
    if (place.clips !== undefined && !Array.isArray(place.clips)) invalid("clips は切断面の配列");
    const clips = (place.clips ?? []).map((plane, i) => readPlane(plane, `clips[${i}]`));
    if (place.visible !== undefined && typeof place.visible !== "boolean") invalid("visible は真偽値");
    if (place.colored !== undefined && !Array.isArray(place.colored)) invalid("colored は配列");
    // 同じ要素の色が重なれば後のものを採る
    const colorOf = new Map();
    for (const [i, entry] of (place.colored ?? []).entries()) {
      if (!isObject(entry)) invalid(`colored[${i}] は { color, elements }`);
      const color = reviewColor(entry.color, `colored[${i}].color は #RRGGBB か #AARRGGBB`);
      for (const id of references(entry.elements, `colored[${i}].elements `)) {
        if (colorOf.has(id)) { overridden++; colorOf.delete(id); }
        colorOf.set(id, color);
      }
    }
    const groups = new Map();
    for (const [id, color] of colorOf) { if (!groups.has(color)) groups.set(color, []); groups.get(color).push(id); }
    const next = {
      ...view, camera, clips, box: null,
      selected: references(place.selected, "selected "), visible: place.visible ?? true, exceptions: references(place.exceptions, "exceptions "),
      colored: [...groups].map(([color, ids]) => ({ color, ids })),
    };
    this.#commitViews(this.#resolveViews(readViews(this.#views.map((v) => (v.id === view.id ? next : v)), limits)));
    const issues = [...this.#metreIssues(scene)];
    const sample = (ids) => Object.freeze(ids.slice(0, limits.issues));
    if (missing.length > 0) issues.push(Object.freeze({ reason: "element-missing", disposition: "omitted", count: missing.length, elements: sample(missing) }));
    if (changed.length > 0) issues.push(Object.freeze({ reason: "element-changed", disposition: "omitted", count: changed.length, elements: sample(changed) }));
    if (unchecked > 0) issues.push(Object.freeze({ reason: "hash-unchecked", disposition: "kept", count: unchecked }));
    if (overridden > 0) issues.push(Object.freeze({ reason: "color-overridden", disposition: "substituted", count: overridden }));
    const side = Math.min(rect.width, rect.height);
    const square = Object.freeze({ x: rect.x + (rect.width - side) / 2, y: rect.y + (rect.height - side) / 2, width: side, height: side });
    return Object.freeze({
      strict: issues.length === 0 && adjustments.length === 0, adjustments: Object.freeze(adjustments), issues: Object.freeze(issues), frame: square,
    });
  }

  /** 利用側の明示した要求で、ビューの枠を描いて RGBA の画素（上の行から）を返す。保存・送信はしない。 */
  /** 登録の扱い。remove は何回呼んでも後始末を重ねず、signal は解除の理由を reason に持つ。 */
  #handle(entry) {
    const remove = () => {
      if (this.#destroyed || !entry.active) return;
      const tasks = this.#extras.remove(entry, "removed");
      try { runAll(tasks); } finally {
        this.#changed();
        this.invalidate();
      }
    };
    const handle = { id: entry.id, remove, signal: entry.controller.signal };
    if (entry.parts !== undefined) handle.parts = entry.parts;
    return Object.freeze(handle);
  }

  #registrationContext() {
    return { unit: this.#scene?.result.unit ?? null, usedBytes: (this.#scene?.report.gpuBytes ?? 0) + this.#extras.bytes() };
  }

  /**
   * 利用側の Object3D を借用して描く。部品は親のない root を wrapper に接続するだけで、位置・材質・userData を書き換えず、
   * 資源を解放しない。登録解除・scope の終了・破棄で接続を外し、渡した dispose を一度だけ呼ぶ。
   */
  addObject(root, options) {
    this.#alive();
    const entry = this.#extras.addObject(root, options, this.#registrationContext());
    this.#changed();
    this.invalidate();
    return this.#handle(entry);
  }

  /** 線・範囲・円・寸法線・ラベルの記述から部品が資源を作って描く。資源は部品が所有する。 */
  addGeometry(items, options) {
    this.#alive();
    const entry = this.#extras.addGeometry(items, options, this.#registrationContext());
    this.#changed();
    this.invalidate();
    return this.#handle(entry);
  }

  /** DOM の入力を接続する。1 つの要素だけを接続し、別の要素を渡すと前の接続を外す。 */
  attachInput(element = this.#canvas, options) {
    this.#alive();
    const next = readInputOptions(options, this.#input.options);
    const binding = bindInput(this, element);
    const cleanup = this.#input.reset("attach");
    const previous = this.#binding;
    this.#binding = binding;
    this.#input.options = next;
    try { previous?.disconnect(); } finally { if (cleanup.failed) throw cleanup.error; }
    return () => {
      if (this.#destroyed || this.#binding !== binding) return;
      this.#binding = null;
      const stopped = this.#input.reset("detach");
      binding.disconnect();
      if (stopped.failed) throw stopped.error;
    };
  }

  /** 標準の割り当てを個別に有効・無効にする。操作中なら中断する。 */
  setInputOptions(options) {
    this.#alive();
    const next = readInputOptions(options, this.#input.options);
    const cleanup = this.#input.reset("options");
    this.#input.options = next;
    if (cleanup.failed) throw cleanup.error;
  }

  /** 正規化した入力を扱い、扱ったかを返す。例外では操作を中断して元の例外を返す。通知の中から呼ぶと reentrant。 */
  handleInput(value) {
    this.#alive();
    if (this.#handling) fail("reentrant", "入力の処理中に入力を渡した");
    const input = normalizeInput(value);
    this.#handling = true;
    try {
      return this.#input.handle(input);
    } catch (error) {
      try { this.#input.reset("error"); } catch { /* 元の例外を返す */ }
      throw error;
    } finally {
      this.#handling = false;
    }
  }

  /** 操作を中断する。移動・回転・2 本指の操作は元のカメラへ戻す。 */
  cancelInput(reason = "cancel") {
    this.#alive();
    if (typeof reason !== "string" || reason === "" || reason.length > 64) invalid("中断の理由は 64 文字以下の文字列");
    const cleanup = this.#input.reset(reason);
    if (cleanup.failed) throw cleanup.error;
  }

  snapshot(viewId) {
    this.#alive();
    const view = this.#view(viewId);
    if (this.#lost) fail("context-lost", "context を失っている");
    const ratio = this.#size.pixelRatio, { rect } = view;
    const x = Math.round(rect.x * ratio), y = Math.round((this.#size.height - rect.y - rect.height) * ratio);
    const width = Math.round(rect.width * ratio), height = Math.round(rect.height * ratio);
    if (width * height > this.#limits.canvasPixels) fail("limit", "Canvas の画素数の上限");
    this.render();
    const gl = this.#renderer.getContext(), raw = new Uint8Array(width * height * 4);
    gl.readPixels(x, y, width, height, gl.RGBA, gl.UNSIGNED_BYTE, raw);
    const pixels = new Uint8Array(raw.length), stride = width * 4;
    for (let row = 0; row < height; row++) pixels.set(raw.subarray((height - 1 - row) * stride, (height - row) * stride), row * stride);
    return Object.freeze({ view: view.id, rect, pixelRatio: ratio, width, height, pixels });
  }

  /** 観測用の数。GPU の実際の使用量ではなく、three.js が持つ資源の件数。 */
  getStats() {
    this.#alive();
    const info = this.#renderer.info;
    return Object.freeze({
      geometries: info.memory.geometries, textures: info.memory.textures, programs: info.programs?.length ?? 0,
      calls: info.render.calls, triangles: info.render.triangles, lines: info.render.lines,
      gpuBytes: this.#scene?.report.gpuBytes ?? 0,
      subscribers: this.#subscribers.size,
      listeners: Object.values(this.#listeners).reduce((n, set) => n + set.size, 0),
      frame: this.#frame !== 0,
    });
  }

  /** 予約した描画・購読・接続と、部品が作った資源を終える。Canvas は取り除かない。 */
  destroy() {
    if (this.#destroyed) return;
    this.#destroyed = true;
    const input = this.#input, binding = this.#binding;
    this.#binding = null;
    const tasks = [
      () => { const cleanup = input.reset("destroy"); if (cleanup.failed) throw cleanup.error; },
      () => binding?.disconnect(),
      ...this.#extras.entries().flatMap((entry) => this.#extras.remove(entry, "destroy")),
      () => this.#cancelFrame(),
      () => this.#canvas.removeEventListener("webglcontextlost", this.#onLost),
      () => this.#canvas.removeEventListener("webglcontextrestored", this.#onRestored),
    ];
    const scene = this.#scene;
    this.#scene = null;
    if (scene !== null) tasks.push(() => this.#dispose(scene));
    tasks.push(() => this.#renderer.dispose());
    tasks.push(() => { this.#cameras.clear(); });
    this.#state = null;
    const state = this.getState(), subscribers = [...this.#subscribers];
    this.#subscribers.clear();
    for (const set of Object.values(this.#listeners)) set.clear();
    tasks.push(...subscribers.map((fn) => () => fn(state)));
    runAll(tasks);
  }
}

/** 要素 ID から要素のハッシュへの Map。省けばハッシュを付けず、照らさない。 */
function readHashes(value) {
  if (value === undefined) return undefined;
  if (!(value instanceof Map)) invalid("hashes は要素 ID から要素のハッシュへの Map");
  return value;
}

/** 行優先の 3 行 4 列の線形部分の逆行列（行優先の 3 行 3 列）。 */
function invert(t) {
  const [a, b, c, , d, e, f, , g, h, i] = t;
  const A = e * i - f * h, B = -(d * i - f * g), C = d * h - e * g;
  const det = a * A + b * B + c * C;
  return [A / det, -(b * i - c * h) / det, (b * f - c * e) / det,
    B / det, (a * i - c * g) / det, -(a * f - c * d) / det,
    C / det, -(a * h - b * g) / det, (a * e - b * d) / det];
}
function apply(m, v) {
  return [m[0] * v[0] + m[1] * v[1] + m[2] * v[2], m[3] * v[0] + m[4] * v[1] + m[5] * v[2], m[6] * v[0] + m[7] * v[1] + m[8] * v[2]];
}
function rayBox(o, d, box) {
  let low = -Infinity, high = Infinity;
  for (let k = 0; k < 3; k++) {
    if (d[k] === 0) { if (o[k] < box[k] || o[k] > box[k + 3]) return false; continue; }
    const a = (box[k] - o[k]) / d[k], b = (box[k + 3] - o[k]) / d[k];
    low = Math.max(low, Math.min(a, b)); high = Math.min(high, Math.max(a, b));
  }
  return high >= Math.max(low, 0);
}
/** 両面の三角形とのもっとも近い交点の光線の係数。局所の光線は元の光線と同じ係数を持つ。 */
function nearest(mesh, o, d, near, accept, spend) {
  const v = mesh.vertices, tri = mesh.triangles;
  spend(tri.length / 3);
  let best = null;
  for (let i = 0; i < tri.length; i += 3) {
    const p0 = [v[tri[i] * 3], v[tri[i] * 3 + 1], v[tri[i] * 3 + 2]];
    const e1 = sub([v[tri[i + 1] * 3], v[tri[i + 1] * 3 + 1], v[tri[i + 1] * 3 + 2]], p0);
    const e2 = sub([v[tri[i + 2] * 3], v[tri[i + 2] * 3 + 1], v[tri[i + 2] * 3 + 2]], p0);
    const p = cross(d, e2), det = dot(e1, p);
    if (det === 0) continue;
    const s = sub(o, p0), u = dot(s, p) / det;
    if (u < 0 || u > 1) continue;
    const q = cross(s, e1), w = dot(d, q) / det;
    if (w < 0 || u + w > 1) continue;
    const t = dot(e2, q) / det;
    if (t >= near && (best === null || t < best) && accept(t)) best = t;
  }
  return best;
}
