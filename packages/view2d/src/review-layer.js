// review_scene のシーンを、図面の View2D に重ねた別の View2D で描く。表示範囲と大きさは図面に合わせ、
// 当たり判定は結果の sources で文書の項目へ戻し、ReviewStore の選択を重ねたシーンの選択に写す。
import { valid } from "./validate.js";
import { reviewReference } from "./review.js";

const isViewer = (value) => value !== null && typeof value === "object" && typeof value.on === "function" && typeof value.getView === "function";
const isStore = (value) => value !== null && typeof value === "object" && typeof value.on === "function" && typeof value.items === "function";

/** 図面の View2D と、背景を透明にした重ねる View2D を組にする */
export class ReviewLayer {
  #base;
  #overlay;
  #store;
  #sources = new Map();
  #stops = [];
  #destroyed = false;

  constructor(base, overlay, store = null) {
    valid(isViewer(base) && isViewer(overlay) && base !== overlay, "viewer");
    valid(store === null || isStore(store), "store");
    this.#base = base;
    this.#overlay = overlay;
    this.#store = store;
    this.#follow();
    this.#stops.push(base.on("view", () => this.#follow()), base.on("size", () => this.#follow()));
    if (store) this.#stops.push(store.on("selection", () => this.#reflect()));
  }

  #alive() { if (this.#destroyed) throw new Error("ReviewLayer は破棄済みです"); }

  #follow() {
    const size = this.#base.getSize(), current = this.#overlay.getSize();
    if (size.width !== current.width || size.height !== current.height || size.pixelRatio !== current.pixelRatio) {
      this.#overlay.resize(size.width, size.height, size.pixelRatio);
    }
    this.#overlay.jumpTo(this.#base.getView());
  }

  /** 文書の選択のうち、いま描いている項目だけを重ねたシーンで選ぶ */
  #reflect() {
    if (!this.#store) return;
    const ids = new Set();
    for (const ref of this.#store.getState().selection) {
      for (const [id, source] of this.#sources) {
        if (source.ref.kind === ref.kind && source.ref.id === ref.id && (ref.kind === "markup" || source.ref.topic === ref.topic)) ids.add(id);
      }
    }
    this.#overlay.select([...ids].map((id) => ({ id, path: [] })));
  }

  /** review_scene の結果を描く。null で消す */
  setResult(input) {
    this.#alive();
    if (input === null) {
      this.#sources = new Map();
      return this.#overlay.setScene(null);
    }
    valid(input !== null && typeof input === "object" && Array.isArray(input.result?.sources), "result");
    const sources = new Map();
    for (const source of input.result.sources) {
      valid(typeof source.id === "string" && !sources.has(source.id), "result.sources");
      sources.set(source.id, Object.freeze({ source, ref: reviewReference(source) }));
    }
    const report = this.#overlay.setScene(input);
    this.#sources = sources;
    this.#follow();
    this.#reflect();
    return report;
  }

  /** Canvas 左上からの CSS px の位置にある文書の項目。上に描いたものから返す */
  pick(point, tolerance) {
    this.#alive();
    const out = [], seen = new Set();
    for (const hit of this.#overlay.hitTest(point, tolerance)) {
      const found = this.#sources.get(hit.id);
      if (found && !seen.has(hit.id)) { seen.add(hit.id); out.push(found.ref); }
    }
    return Object.freeze(out);
  }

  /** 結果の sources の 1 件。描いていない ID は null */
  sourceOf(sceneId) {
    this.#alive();
    return this.#sources.get(sceneId)?.source ?? null;
  }

  /** 購読を外す。2 つの View2D は利用側が破棄する。冪等 */
  destroy() {
    if (this.#destroyed) return;
    this.#destroyed = true;
    for (const stop of this.#stops) stop();
    this.#stops = [];
    this.#sources = new Map();
  }
}
