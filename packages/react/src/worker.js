import { loadSync } from "@zumen-jp/zumen";
import { findSource } from "@zumen-jp/view2d";

let session, document;
let catalogName;
let converted = 0;
let preview;
const budget = { input_bytes: 64 * 1024 * 1024, decoded_bytes: 256 * 1024 * 1024 };

function describe(entries) {
  if (!entries.length) return [];
  const { texts } = session.call({ v: 1, op: "describe", catalog: catalogName, diagnostics: entries.map(({ diagnostic }) => diagnostic) }).result;
  return entries.map((entry, i) => ({ ...entry, text: texts[i] }));
}

self.onmessage = ({ data }) => {
  const { id, op } = data;
  let stage = "open";
  let converting = false;
  const entries = [];
  try {
    if (op === "source") {
      self.postMessage({ id, value: findSource(session, document, data.element) });
      return;
    }
    if (op === "convert_estimate") {
      stage = "convert_estimate";
      preview = null;
      const to = "converted-" + ++converted + ".dxf";
      converting = true;
      const response = session.call({ v: 1, op, doc: document.doc, format: "dxf", to, limit: data.limit, budget });
      converting = false;
      for (const diagnostic of response.diagnostics) entries.push({ stage, diagnostic });
      preview = { from: document.hash, estimate: response.result.estimate };
      self.postMessage({ id, value: { result: response.result, diagnostics: describe(entries) } });
      return;
    }
    if (op === "convert") {
      // 開いた文書を新しい DXF の文書にし、書いたファイルを取り出してから閉じる（決定 0206、#690）。
      // 元の文書とシーンは変えないので、表示と出所はそのまま使える。
      stage = "convert";
      const expected = data.estimate;
      if (expected && (!preview || preview.from !== document.hash ||
        expected.doc !== preview.estimate.doc || expected.hash !== preview.estimate.hash || expected.elements !== preview.estimate.elements)) {
        throw new DOMException("drawing.estimate.stale", "InvalidStateError");
      }
      const to = expected?.doc ?? "converted-" + ++converted + ".dxf";
      // 失敗した応答の診断は、下の catch で stage を convert にして返す
      converting = true;
      const response = session.call({ v: 1, op: "convert", doc: document.doc, format: "dxf", to, budget });
      for (const diagnostic of response.diagnostics) entries.push({ stage, diagnostic });
      let bytes;
      try { bytes = session.take(0); }
      finally { session.call({ v: 1, op: "close", docs: [to] }); }
      converting = false;
      preview = null;
      if (expected && (response.result.hash !== expected.hash || response.result.elements !== expected.elements)) {
        throw new DOMException("drawing.estimate.changed", "InvalidStateError");
      }
      self.postMessage({ id, value: { bytes, result: response.result, diagnostics: describe(entries) } }, [bytes.buffer]);
      return;
    }
    const { bytes, module, from, catalog } = data;
    preview = null;
    session = loadSync(module).open();
    const canEstimate = session.call({ v: 1, op: "capabilities" }).result.ops.includes("convert_estimate");
    if (catalog) { catalogName = "catalog"; session.push(catalogName, catalog); }
    session.push("input", bytes);
    function call(request) {
      stage = request.op;
      const response = session.call({ v: 1, ...request, budget });
      for (const diagnostic of response.diagnostics) entries.push({ stage, diagnostic });
      return response.result;
    }
    const opened = call({ op: "open", input: "input" });
    const drawing = call({ op: "drawing", doc: opened.doc });
    document = { doc: opened.doc, format: opened.format, hash: drawing.hash };
    const origin = from ?? drawing.spaces[0]?.id ?? drawing.sheets[0]?.id;
    const result = call({ op: "scene", doc: opened.doc, from: origin });
    // take は次の call より先に行い、wasm のメモリから独立したバッファーだけを移す。
    const blob = session.take(result.blob);
    self.postMessage({ id, value: { blob, result, document, drawing, canEstimate, diagnostics: describe(entries) } }, [blob.buffer]);
  } catch (error) {
    for (const diagnostic of error.response?.diagnostics ?? []) entries.push({ stage, diagnostic });
    let diagnostics;
    try { diagnostics = describe(entries); }
    catch { diagnostics = entries.map((entry) => ({ ...entry, text: entry.diagnostic.code })); }
    // 開けなかったときと、変換で応答のない失敗（trap など）をしたときは、セッションを閉じる。続く出所と変換も失敗にする
    if (op === "open" || (converting && !error.response)) { session?.close(); session = null; preview = null; }
    self.postMessage({ id, error: { name: error.name, message: error.message, diagnostics } });
  }
};
