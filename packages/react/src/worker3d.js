// 3D のシーンを開く Worker。1 回の読み込みに 1 つで、終了すると wasm のメモリとセッションも消える。
import { loadSync, readScene3D } from "@zumen-jp/zumen";

let session, document, kind, catalogName, serial = 0;
const listLimit = 1000;
const budget = { input_bytes: 256 * 1024 * 1024, decoded_bytes: 1024 * 1024 * 1024 };

function describe(entries) {
  if (!entries.length) return [];
  const { texts } = session.call({ v: 1, op: "describe", catalog: catalogName, diagnostics: entries.map(({ diagnostic }) => diagnostic) }).result;
  return entries.map((entry, i) => ({ ...entry, text: texts[i] }));
}

async function sha256(bytes) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function fileOf(response) {
  const blob = response.blobs.find((item) => item.kind === "file");
  if (!blob) throw new Error("scene3d.bcf.file");
  return session.take(blob.id);
}

self.onmessage = async ({ data }) => {
  const { id, op } = data;
  let stage = op;
  const entries = [];
  const call = (request) => {
    stage = request.op;
    const response = session.call({ v: 1, ...request, budget });
    for (const diagnostic of response.diagnostics) entries.push({ stage, diagnostic });
    return response;
  };
  try {
    if (op === "element") {
      if (kind !== "building") { self.postMessage({ id, value: null }); return; }
      const { elements } = call({ op: "building_elements", doc: document.doc, ids: [data.element], components: ["identity", "class", "spatial", "parent", "properties"], limit: 1 }).result;
      self.postMessage({ id, value: elements[0] ?? null });
      return;
    }
    if (op === "bcf_write") {
      if (kind !== "building") throw new DOMException("scene3d.bcf.unsupported", "NotSupportedError");
      const name = `review-${++serial}.json`;
      session.push(name, JSON.stringify({ format: "zumen-review", version: 1,
        targets: [{ id: "model", format: document.format, hash: document.hash, name: document.doc }],
        topics: [{ id: "t-1", title: data.title, status: "open", locations: [{ id: "t-1/l1", target: "model", ...data.place }] }] }));
      const written = call({ op: "bcf_write", review: name, target: "model", kind: "Issue", author: data.author, date: data.date, version: data.version });
      const bytes = fileOf(written);
      self.postMessage({ id, value: { bytes, diagnostics: describe(entries) } }, [bytes.buffer]);
      return;
    }
    if (op === "bcf_read") {
      if (kind !== "building") throw new DOMException("scene3d.bcf.unsupported", "NotSupportedError");
      const name = `input-${++serial}.bcf`;
      session.push(name, data.bytes);
      const read = call({ op: "bcf_read", doc: document.doc, input: name, target: "model" });
      const review = JSON.parse(new TextDecoder().decode(fileOf(read)));
      const locations = review.topics.flatMap((topic) => topic.locations.map((location) => ({ topic: topic.title, location })));
      self.postMessage({ id, value: { locations, diagnostics: describe(entries) } });
      return;
    }
    const { bytes, module, catalog, tolerance } = data;
    session = loadSync(module).open();
    if (catalog) { catalogName = "catalog"; session.push(catalogName, catalog); }
    const digest = await sha256(bytes);
    session.push("input", bytes);
    const opened = call({ op: "open", input: "input" }).result;
    let hash = digest;
    if (opened.format === "ifc") hash = call({ op: "building", doc: opened.doc }).result.hash;
    const result = call({ op: "scene3d", doc: opened.doc, ...(tolerance === undefined ? {} : { tolerance }) }).result;
    kind = opened.format === "ifc" ? "building" : (result.counts.points ?? 0) > 0 ? "pointcloud" : "feature";
    document = { doc: opened.doc, format: opened.format, hash };
    // take は次の call より先に行い、wasm のメモリから独立したバッファーだけを移す。
    const blob = session.take(result.blob);
    // 一覧の最初の listLimit 件。表示部品と同じ読み手で検査し、描く前に壊れた blob を見つける。
    const scene = readScene3D(blob);
    const total = scene.rows("element_ids");
    const elements = Array.from({ length: Math.min(total, listLimit) }, (_, row) => {
      const { id, class: className, name } = scene.element(row);
      return { id, class: className, name };
    });
    // 塊の原点と範囲を足した文書の座標の範囲。視点・寸法・地面の位置の目安で、要素の厳密な範囲ではない。
    let bounds = null;
    for (let row = 0; row < scene.chunks; row++) {
      const chunk = scene.chunk(row);
      const min = [0, 1, 2].map((k) => chunk.origin[k] + chunk.bounds[k]), max = [0, 1, 2].map((k) => chunk.origin[k] + chunk.bounds[k + 3]);
      bounds = bounds === null ? { min, max } : { min: bounds.min.map((v, k) => Math.min(v, min[k])), max: bounds.max.map((v, k) => Math.max(v, max[k])) };
    }
    self.postMessage({ id, value: { blob, result, document, kind, elements, total, bounds, diagnostics: describe(entries) } }, [blob.buffer]);
  } catch (error) {
    for (const diagnostic of error.response?.diagnostics ?? []) entries.push({ stage, diagnostic });
    let diagnostics;
    try { diagnostics = session ? describe(entries) : []; }
    catch { diagnostics = entries.map((entry) => ({ ...entry, text: entry.diagnostic.code })); }
    if (op === "open") { session?.close(); session = null; }
    self.postMessage({ id, error: { name: error.name, message: error.message, diagnostics } });
  }
};
