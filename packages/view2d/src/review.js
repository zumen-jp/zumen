// レビューの文書（決定 0214・0295）の編集の状態。描画と文書全体の検査はコアの review_scene に任せ、
// ここでは文書の値・選択・絞り込み・取り消しの履歴だけを持つ。文言・作者・時刻・ID・保存先は利用側から受け取り、
// 文書をどこへも送らない。
import { valid } from "./validate.js";

const KINDS = ["redline", "cloud", "text", "photo"];
const STATUSES = ["open", "resolved", "closed"];
const EVENTS = ["document", "selection", "filter", "history"];
const MARKUP_KEYS = ["kind", "target", "view", "points", "text", "topic"];
const LOCATION_KEYS = ["target", "view", "pin", "extent"];
const TOPIC_KEYS = ["title", "description", "status", "status_label", "kind", "priority", "labels", "assignee", "due"];
const DEFAULT_LIMITS = Object.freeze({ history: 256, items: 100000 });
const hasOwn = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
const plain = (value) => value !== null && typeof value === "object" && !Array.isArray(value);

/** JSON の値を凍結して写す。利用側の値を保管しない */
function freezeJson(value, name, depth = 0) {
  valid(depth < 64, name);
  if (Array.isArray(value)) return Object.freeze(value.map((item, i) => freezeJson(item, `${name}/${i}`, depth + 1)));
  if (plain(value)) {
    const out = {};
    for (const key of Object.keys(value)) {
      if (value[key] !== undefined) out[key] = freezeJson(value[key], `${name}/${key}`, depth + 1);
    }
    return Object.freeze(out);
  }
  valid(value === null || typeof value === "string" || typeof value === "boolean" || (typeof value === "number" && Number.isFinite(value)), name);
  return value;
}

function text(value, name) { valid(typeof value === "string", name); return value; }
function id(value, name) { valid(typeof value === "string" && value.length > 0, name); return value; }
function pair(value, name) {
  valid(Array.isArray(value) && value.length === 2 && value.every(Number.isFinite), name);
  return value;
}
function points(value, name) {
  valid(Array.isArray(value), name);
  value.forEach((point, i) => pair(point, `${name}/${i}`));
  return value;
}
function extent(value, name) {
  valid(Array.isArray(value) && value.length === 4 && value.every(Number.isFinite), name);
  return value;
}

/** シーンの ID の一部。コアと同じく UTF-8 のバイトを小文字の 16 進にする */
function hex(value) {
  let out = "";
  for (const byte of new TextEncoder().encode(value)) out += byte.toString(16).padStart(2, "0");
  return out;
}

/** 文書の項目を指す参照を、凍結した新しい値にする */
function reference(value, name) {
  valid(plain(value), name);
  if (value.kind === "markup") return Object.freeze({ kind: "markup", id: id(value.id, `${name}.id`) });
  valid(value.kind === "location", `${name}.kind`);
  return Object.freeze({ kind: "location", topic: id(value.topic, `${name}.topic`), id: id(value.id, `${name}.id`) });
}
const keyOf = (ref) => (ref.kind === "markup" ? `m\u0000${ref.id}` : `l\u0000${ref.topic}\u0000${ref.id}`);

/** review_scene の sources の 1 件を、文書の項目の参照にする */
export function reviewReference(source) {
  valid(plain(source), "source");
  if (source.kind === "markup") return Object.freeze({ kind: "markup", id: id(source.markup, "source.markup") });
  valid(source.kind === "location", "source.kind");
  return Object.freeze({ kind: "location", topic: id(source.topic, "source.topic"), id: id(source.location, "source.location") });
}

/** 文書の項目の参照から、review_scene がシーンの entity_ids に付ける ID を作る */
export function reviewSceneId(ref) {
  const checked = reference(ref, "reference");
  return checked.kind === "markup" ? `review/markup/${hex(checked.id)}` : `review/location/${hex(checked.topic)}/${hex(checked.id)}`;
}

/** 文書の形を確かめる。値の詳しい検査（幾何・参照の整合）はコアが行う */
function checkDocument(doc, limits) {
  valid(plain(doc) && doc.format === "zumen-review" && Number.isInteger(doc.version), "document");
  const targets = doc.targets ?? [];
  valid(Array.isArray(targets), "document/targets");
  const topics = doc.topics ?? [], markups = doc.markups ?? [];
  valid(Array.isArray(topics) && Array.isArray(markups), "document");
  let count = targets.length + topics.length + markups.length;
  const seen = new Set();
  targets.forEach((target, i) => { valid(plain(target), `document/targets/${i}`); id(target.id, `document/targets/${i}/id`); });
  topics.forEach((topic, i) => {
    valid(plain(topic), `document/topics/${i}`);
    const key = id(topic.id, `document/topics/${i}/id`);
    valid(!seen.has(`t\u0000${key}`), `document/topics/${i}/id`); seen.add(`t\u0000${key}`);
    const locations = topic.locations ?? [];
    valid(Array.isArray(locations), `document/topics/${i}/locations`);
    count += locations.length;
    const ids = new Set();
    locations.forEach((location, j) => {
      valid(plain(location), `document/topics/${i}/locations/${j}`);
      const at = id(location.id, `document/topics/${i}/locations/${j}/id`);
      valid(!ids.has(at), `document/topics/${i}/locations/${j}/id`); ids.add(at);
    });
  });
  markups.forEach((markup, i) => {
    valid(plain(markup), `document/markups/${i}`);
    const key = id(markup.id, `document/markups/${i}/id`);
    valid(!seen.has(`m\u0000${key}`), `document/markups/${i}/id`); seen.add(`m\u0000${key}`);
  });
  valid(count <= limits.items, "limits.items");
}

function limitsOf(value = {}) {
  valid(plain(value), "limits");
  const out = { ...DEFAULT_LIMITS };
  for (const key of Object.keys(value)) {
    valid(hasOwn(DEFAULT_LIMITS, key) && Number.isInteger(value[key]) && value[key] > 0, `limits.${key}`);
    out[key] = value[key];
  }
  return Object.freeze(out);
}

/** 変える値を、既知のキーなら型を確かめて写す。null は省略できるキーを消す */
function patchOf(patch, keys, required, check, name) {
  valid(plain(patch), name);
  const out = {};
  for (const key of Object.keys(patch)) {
    // 文書にある未知のキーは保つが、編集では既知のキーだけを変える
    valid(keys.includes(key), `${name}.${key}`);
    const value = patch[key];
    if (value === undefined) continue;
    if (value === null) { valid(!required.includes(key), `${name}.${key}`); out[key] = null; continue; }
    out[key] = check(key, value, `${name}.${key}`);
  }
  return out;
}

function apply(item, patch) {
  const out = { ...item };
  for (const key of Object.keys(patch)) {
    if (patch[key] === null) delete out[key];
    else out[key] = patch[key];
  }
  return freezeJson(out, "item");
}

/** 赤入れ・変更雲・注記・写真位置と指摘の場所を編集する状態。描画は review_scene の結果を View2D に渡す */
export class ReviewStore {
  #doc;
  #limits;
  #createId;
  #stamp;
  #undo = [];
  #redo = [];
  #selection = Object.freeze([]);
  #filter = Object.freeze({});
  #revision = 0;
  #snapshot = null;
  #listeners = new Set();
  #notifying = false;
  #batch = null;

  constructor(document, options = {}) {
    valid(plain(options), "options");
    this.#limits = limitsOf(options.limits);
    valid(options.createId === undefined || typeof options.createId === "function", "createId");
    valid(options.stamp === undefined || typeof options.stamp === "function", "stamp");
    this.#createId = options.createId ?? null;
    this.#stamp = options.stamp ?? null;
    this.#doc = this.#read(document);
  }

  #read(document) {
    const value = typeof document === "string" ? JSON.parse(document) : document;
    const doc = freezeJson(value, "document");
    checkDocument(doc, this.#limits);
    return doc;
  }

  #mutable() { valid(!this.#notifying, "state.notification"); }

  #state() {
    return Object.freeze({ document: this.#doc, selection: this.#selection, filter: this.#filter, revision: this.#revision,
      canUndo: this.#undo.length > 0, canRedo: this.#redo.length > 0 });
  }

  #publish(types, change) {
    const next = this.#state();
    this.#snapshot = next;
    const events = {};
    for (const type of types) events[type] = Object.freeze({ type, state: next, ...(type === "document" ? { change } : {}) });
    let failed = false, first;
    this.#notifying = true;
    try {
      for (const entry of [...this.#listeners]) {
        if (!this.#listeners.has(entry) || (entry.type && !events[entry.type])) continue;
        try { entry.listener(entry.type ? events[entry.type] : next); }
        catch (error) { if (!failed) { failed = true; first = error; } }
      }
    } finally { this.#notifying = false; }
    if (failed) throw first;
  }

  #listen(type, listener) {
    valid(typeof listener === "function", "listener");
    const entry = { type, listener };
    this.#listeners.add(entry);
    return () => { this.#listeners.delete(entry); };
  }

  /** 項目が残っている選択だけを残す */
  #prune(doc) {
    const kept = this.#selection.filter((ref) => this.#find(doc, ref) !== null);
    return kept.length === this.#selection.length ? this.#selection : Object.freeze(kept);
  }

  #find(doc, ref) {
    if (ref.kind === "markup") return (doc.markups ?? []).find((m) => m.id === ref.id) ?? null;
    const topic = (doc.topics ?? []).find((t) => t.id === ref.topic);
    return topic ? (topic.locations ?? []).find((l) => l.id === ref.id) ?? null : null;
  }

  /** 1 つの変更を履歴に積み、通知する。batch の中では最後にまとめる */
  #commit(next, reason, items) {
    checkDocument(next, this.#limits);
    if (this.#batch) {
      this.#batch.items.push(...items);
      this.#batch.reasons.add(reason);
      this.#doc = next;
      return;
    }
    const before = this.#doc;
    this.#doc = next;
    this.#undo.push({ before, after: next, items: Object.freeze(items) });
    if (this.#undo.length > this.#limits.history) this.#undo.shift();
    this.#redo = [];
    this.#changed(reason, items);
  }

  #changed(reason, items) {
    this.#revision += 1;
    const selection = this.#prune(this.#doc);
    const types = ["document", "history"];
    if (selection !== this.#selection) { this.#selection = selection; types.push("selection"); }
    this.#publish(types, Object.freeze({ reason, items: Object.freeze([...items]) }));
  }

  #newId(given, name) {
    if (given !== undefined) return id(given, name);
    valid(this.#createId !== null, "createId");
    return id(this.#createId(), "createId()");
  }

  #stamped() {
    if (!this.#stamp) return {};
    const value = this.#stamp();
    valid(plain(value), "stamp()");
    const out = {};
    if (value.author !== undefined) out.author = text(value.author, "stamp().author");
    if (value.time !== undefined) out.time = text(value.time, "stamp().time");
    return out;
  }

  #target(doc, value, name) {
    id(value, name);
    valid((doc.targets ?? []).some((target) => target.id === value), name);
    return value;
  }

  #markupValue(doc) {
    return (key, value, name) => {
      switch (key) {
        case "kind": valid(KINDS.includes(value), name); return value;
        case "target": return this.#target(doc, value, name);
        case "view": return id(value, name);
        case "points": return freezeJson(points(value, name), name);
        case "text": return text(value, name);
        case "topic": valid((doc.topics ?? []).some((t) => t.id === value), name); return value;
        default: return freezeJson(value, name);
      }
    };
  }

  #locationValue(doc) {
    return (key, value, name) => {
      switch (key) {
        case "target": return this.#target(doc, value, name);
        case "view": return id(value, name);
        case "pin": return freezeJson(pair(value, name), name);
        case "extent": {
          extent(value, name);
          valid(value[0] <= value[2] && value[1] <= value[3], name);
          return freezeJson(value, name);
        }
        default: return freezeJson(value, name);
      }
    };
  }

  static #topicValue(key, value, name) {
    switch (key) {
      case "status": valid(STATUSES.includes(value), name); return value;
      case "labels": valid(Array.isArray(value) && value.every((v) => typeof v === "string"), name); return freezeJson(value, name);
      case "title": case "description": case "status_label": case "kind": case "priority": case "assignee": case "due": return text(value, name);
      default: return freezeJson(value, name);
    }
  }

  getState() { return this.#snapshot ?? (this.#snapshot = this.#state()); }
  getDocument() { return this.#doc; }
  subscribe(listener) { return this.#listen(null, listener); }
  on(type, listener) { valid(EVENTS.includes(type), "type"); return this.#listen(type, listener); }

  /** 文書の JSON。保存先への書き込みは利用側が行う */
  toJSON() { return JSON.stringify(this.#doc); }

  /** 文書を読み直す。履歴と選択を消す */
  load(document) {
    this.#mutable();
    valid(!this.#batch, "batch");
    this.#doc = this.#read(document);
    this.#undo = []; this.#redo = [];
    this.#selection = Object.freeze([]);
    this.#revision += 1;
    this.#publish(["document", "history", "selection"], Object.freeze({ reason: "load", items: Object.freeze([]) }));
  }

  /** 複数の変更を 1 回の取り消しにまとめる。fn が例外を投げたら、変更をすべて戻す */
  batch(fn) {
    this.#mutable();
    valid(typeof fn === "function" && !this.#batch, "batch");
    const before = this.#doc;
    this.#batch = { items: [], reasons: new Set() };
    let result, batch;
    try { result = fn(this); }
    catch (error) { this.#doc = before; throw error; }
    finally { batch = this.#batch; this.#batch = null; }
    if (this.#doc === before) return result;
    const after = this.#doc;
    this.#doc = before;
    const reason = batch.reasons.size === 1 ? [...batch.reasons][0] : "batch";
    this.#commit(after, reason, batch.items);
    return result;
  }

  addMarkup(markup) {
    this.#mutable();
    valid(plain(markup), "markup");
    const doc = this.#doc;
    const { id: _, ...rest } = markup;
    const patch = patchOf(rest, MARKUP_KEYS, MARKUP_KEYS, this.#markupValue(doc), "markup");
    valid(["kind", "target", "points"].every((key) => hasOwn(patch, key)), "markup");
    const markupId = this.#newId(markup.id, "markup.id");
    valid(!(doc.markups ?? []).some((m) => m.id === markupId), "markup.id");
    const item = apply({ id: markupId }, patch);
    const ref = Object.freeze({ kind: "markup", id: markupId });
    this.#commit(freezeJson({ ...doc, markups: [...(doc.markups ?? []), item] }, "document"), "add", [ref]);
    return markupId;
  }

  updateMarkup(markupId, patch) {
    this.#mutable();
    const doc = this.#doc;
    const markups = doc.markups ?? [];
    const at = markups.findIndex((m) => m.id === markupId);
    valid(at >= 0, "markup.id");
    const checked = patchOf(patch, MARKUP_KEYS, ["kind", "target", "points"], this.#markupValue(doc), "patch");
    const next = markups.slice();
    next[at] = apply(markups[at], checked);
    this.#commit(freezeJson({ ...doc, markups: next }, "document"), "update", [Object.freeze({ kind: "markup", id: markupId })]);
  }

  removeMarkup(markupId) {
    this.#mutable();
    const doc = this.#doc;
    const markups = doc.markups ?? [];
    valid(markups.some((m) => m.id === markupId), "markup.id");
    this.#commit(freezeJson({ ...doc, markups: markups.filter((m) => m.id !== markupId) }, "document"), "remove", [Object.freeze({ kind: "markup", id: markupId })]);
  }

  addTopic(topic) {
    this.#mutable();
    valid(plain(topic), "topic");
    const doc = this.#doc;
    const { id: _, locations, ...rest } = topic;
    valid(locations === undefined, "topic.locations");
    const patch = patchOf(rest, TOPIC_KEYS, TOPIC_KEYS, ReviewStore.#topicValue, "topic");
    valid(hasOwn(patch, "title") && hasOwn(patch, "status"), "topic");
    const topicId = this.#newId(topic.id, "topic.id");
    valid(!(doc.topics ?? []).some((t) => t.id === topicId), "topic.id");
    const stamp = this.#stamped();
    const stamped = { ...(stamp.author !== undefined ? { author: stamp.author } : {}), ...(stamp.time !== undefined ? { created: stamp.time } : {}) };
    const item = apply({ id: topicId, ...stamped, locations: [] }, patch);
    this.#commit(freezeJson({ ...doc, topics: [...(doc.topics ?? []), item] }, "document"), "add", []);
    return topicId;
  }

  updateTopic(topicId, patch) {
    this.#mutable();
    const doc = this.#doc;
    const topics = doc.topics ?? [];
    const at = topics.findIndex((t) => t.id === topicId);
    valid(at >= 0, "topic.id");
    const checked = patchOf(patch, TOPIC_KEYS, ["title", "status"], ReviewStore.#topicValue, "patch");
    const stamp = this.#stamped();
    if (stamp.author !== undefined) checked.modified_by = stamp.author;
    if (stamp.time !== undefined) checked.modified = stamp.time;
    const next = topics.slice();
    next[at] = apply(topics[at], checked);
    const items = (topics[at].locations ?? []).map((l) => Object.freeze({ kind: "location", topic: topicId, id: l.id }));
    this.#commit(freezeJson({ ...doc, topics: next }, "document"), "update", items);
  }

  /** 図形が参照する指摘は消さない。先に図形の topic を外す */
  removeTopic(topicId) {
    this.#mutable();
    const doc = this.#doc;
    const topics = doc.topics ?? [];
    const topic = topics.find((t) => t.id === topicId);
    valid(topic !== undefined, "topic.id");
    valid(!(doc.markups ?? []).some((m) => m.topic === topicId), "topic.referenced");
    const items = (topic.locations ?? []).map((l) => Object.freeze({ kind: "location", topic: topicId, id: l.id }));
    this.#commit(freezeJson({ ...doc, topics: topics.filter((t) => t.id !== topicId) }, "document"), "remove", items);
  }

  addLocation(topicId, location) {
    this.#mutable();
    valid(plain(location), "location");
    const doc = this.#doc;
    const topics = doc.topics ?? [];
    const at = topics.findIndex((t) => t.id === topicId);
    valid(at >= 0, "topic.id");
    const locations = topics[at].locations ?? [];
    const { id: _, kind, ...rest } = location;
    valid(kind === undefined || kind === "2d", "location.kind");
    const patch = patchOf(rest, LOCATION_KEYS, LOCATION_KEYS, this.#locationValue(doc), "location");
    valid(hasOwn(patch, "target"), "location.target");
    const locationId = this.#newId(location.id, "location.id");
    valid(!locations.some((l) => l.id === locationId), "location.id");
    const item = apply({ id: locationId, target: patch.target, kind: "2d" }, patch);
    const next = topics.slice();
    next[at] = freezeJson({ ...topics[at], locations: [...locations, item] }, "topic");
    this.#commit(freezeJson({ ...doc, topics: next }, "document"), "add", [Object.freeze({ kind: "location", topic: topicId, id: locationId })]);
    return locationId;
  }

  updateLocation(topicId, locationId, patch) {
    this.#mutable();
    const doc = this.#doc;
    const topics = doc.topics ?? [];
    const at = topics.findIndex((t) => t.id === topicId);
    valid(at >= 0, "topic.id");
    const locations = topics[at].locations ?? [];
    const index = locations.findIndex((l) => l.id === locationId);
    valid(index >= 0, "location.id");
    valid(locations[index].kind === "2d", "location.kind");
    const checked = patchOf(patch, LOCATION_KEYS, ["target"], this.#locationValue(doc), "patch");
    const nextLocations = locations.slice();
    nextLocations[index] = apply(locations[index], checked);
    const next = topics.slice();
    next[at] = freezeJson({ ...topics[at], locations: nextLocations }, "topic");
    this.#commit(freezeJson({ ...doc, topics: next }, "document"), "update", [Object.freeze({ kind: "location", topic: topicId, id: locationId })]);
  }

  removeLocation(topicId, locationId) {
    this.#mutable();
    const doc = this.#doc;
    const topics = doc.topics ?? [];
    const at = topics.findIndex((t) => t.id === topicId);
    valid(at >= 0, "topic.id");
    const locations = topics[at].locations ?? [];
    valid(locations.some((l) => l.id === locationId), "location.id");
    const next = topics.slice();
    next[at] = freezeJson({ ...topics[at], locations: locations.filter((l) => l.id !== locationId) }, "topic");
    this.#commit(freezeJson({ ...doc, topics: next }, "document"), "remove", [Object.freeze({ kind: "location", topic: topicId, id: locationId })]);
  }

  undo() {
    this.#mutable();
    valid(!this.#batch, "batch");
    const step = this.#undo.pop();
    if (!step) return false;
    this.#redo.push(step);
    this.#doc = step.before;
    this.#changed("undo", step.items);
    return true;
  }

  redo() {
    this.#mutable();
    valid(!this.#batch, "batch");
    const step = this.#redo.pop();
    if (!step) return false;
    this.#undo.push(step);
    this.#doc = step.after;
    this.#changed("redo", step.items);
    return true;
  }

  /** 選択を置き換える。add・remove・toggle も使える。文書にない参照は RangeError */
  select(refs, mode = "replace") {
    this.#mutable();
    valid(Array.isArray(refs), "refs");
    valid(["replace", "add", "remove", "toggle"].includes(mode), "mode");
    const given = refs.map((ref, i) => reference(ref, `refs/${i}`));
    for (const [i, ref] of given.entries()) valid(this.#find(this.#doc, ref) !== null, `refs/${i}`);
    const current = new Map(mode === "replace" ? [] : this.#selection.map((ref) => [keyOf(ref), ref]));
    for (const ref of given) {
      const key = keyOf(ref);
      if (mode === "remove" || (mode === "toggle" && current.has(key))) current.delete(key);
      else if (!current.has(key)) current.set(key, ref);
    }
    const next = [...current.values()];
    if (next.length === this.#selection.length && next.every((ref, i) => keyOf(ref) === keyOf(this.#selection[i]))) return;
    this.#selection = Object.freeze(next);
    this.#publish(["selection"]);
  }

  /** 一覧と描画の絞り込み。省略した条件は絞らない */
  setFilter(filter) {
    this.#mutable();
    valid(plain(filter), "filter");
    const out = {};
    for (const key of Object.keys(filter)) {
      valid(["targets", "views", "kinds", "topics", "statuses"].includes(key), `filter.${key}`);
      if (filter[key] === undefined) continue;
      valid(Array.isArray(filter[key]) && filter[key].every((v) => typeof v === "string"), `filter.${key}`);
      if (key === "kinds") valid(filter[key].every((v) => KINDS.includes(v) || v === "location"), `filter.${key}`);
      if (key === "statuses") valid(filter[key].every((v) => STATUSES.includes(v)), `filter.${key}`);
      out[key] = Object.freeze([...new Set(filter[key])]);
    }
    this.#filter = Object.freeze(out);
    this.#publish(["filter"]);
  }

  /**
   * 絞り込みに合う 2D の項目を、review_scene の描く順（図形、続いて指摘の場所）に返す。
   * 3D の場所は含めない。移した状態（carried）があれば、そのまま返す
   */
  items() {
    const f = this.#filter, doc = this.#doc;
    const topics = new Map((doc.topics ?? []).map((t) => [t.id, t]));
    const keep = (target, view, kind, topicId) => {
      if (f.targets && !f.targets.includes(target)) return false;
      if (f.views && !f.views.includes(view ?? "")) return false;
      if (f.kinds && !f.kinds.includes(kind)) return false;
      if (f.topics && !(topicId !== undefined && f.topics.includes(topicId))) return false;
      if (f.statuses) {
        const topic = topicId === undefined ? undefined : topics.get(topicId);
        if (!topic || !f.statuses.includes(topic.status)) return false;
      }
      return true;
    };
    const out = [];
    for (const markup of doc.markups ?? []) {
      if (!keep(markup.target, markup.view, markup.kind, markup.topic)) continue;
      out.push(Object.freeze({ ref: Object.freeze({ kind: "markup", id: markup.id }), item: markup, sceneId: `review/markup/${hex(markup.id)}` }));
    }
    for (const topic of doc.topics ?? []) {
      for (const location of topic.locations ?? []) {
        if (location.kind !== "2d" || !keep(location.target, location.view, "location", topic.id)) continue;
        out.push(Object.freeze({ ref: Object.freeze({ kind: "location", topic: topic.id, id: location.id }), item: location, topic,
          sceneId: `review/location/${hex(topic.id)}/${hex(location.id)}` }));
      }
    }
    return Object.freeze(out);
  }

  /**
   * 絞り込みを当てた文書。review_scene に渡すと、絞り込みに合う項目だけを描く。指摘・対象・ID は元のまま保ち、
   * 絞り込みで外した図形と場所だけを除く。保存には toJSON を使う
   */
  filteredJSON() {
    const items = this.items();
    const markups = new Set(items.filter((i) => i.ref.kind === "markup").map((i) => i.ref.id));
    const locations = new Set(items.filter((i) => i.ref.kind === "location").map((i) => keyOf(i.ref)));
    const doc = this.#doc;
    return JSON.stringify({
      ...doc,
      ...(doc.topics ? { topics: doc.topics.map((t) => ({ ...t, locations: (t.locations ?? []).filter((l) => l.kind !== "2d" || locations.has(keyOf({ kind: "location", topic: t.id, id: l.id }))) })) } : {}),
      ...(doc.markups ? { markups: doc.markups.filter((m) => markups.has(m.id)) } : {}),
    });
  }
}
