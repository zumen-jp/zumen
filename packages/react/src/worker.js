import { loadSync } from "@zumen-jp/zumen";
import { findSource } from "@zumen-jp/view2d";

let session, document;
let catalogName;
const budget = { input_bytes: 64 * 1024 * 1024, decoded_bytes: 256 * 1024 * 1024 };

function describe(entries) {
  if (!entries.length) return [];
  const { texts } = session.call({ v: 1, op: "describe", catalog: catalogName, diagnostics: entries.map(({ diagnostic }) => diagnostic) }).result;
  return entries.map((entry, i) => ({ ...entry, text: texts[i] }));
}

self.onmessage = ({ data }) => {
  const { id, op } = data;
  let stage = "open";
  const entries = [];
  try {
    if (op === "source") {
      self.postMessage({ id, value: findSource(session, document, data.element) });
      return;
    }
    const { bytes, module, from, catalog } = data;
    session = loadSync(module).open();
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
    self.postMessage({ id, value: { blob, result, document, drawing, diagnostics: describe(entries) } }, [blob.buffer]);
  } catch (error) {
    for (const diagnostic of error.response?.diagnostics ?? []) entries.push({ stage, diagnostic });
    let diagnostics;
    try { diagnostics = describe(entries); }
    catch { diagnostics = entries.map((entry) => ({ ...entry, text: entry.diagnostic.code })); }
    if (op === "open") { session?.close(); session = null; }
    self.postMessage({ id, error: { name: error.name, message: error.message, diagnostics } });
  }
};
