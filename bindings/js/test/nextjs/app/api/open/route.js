// 送られた図面を、要求ごとに新しいインスタンスで開き、形式と要素の数を返す（決定 0259）
import { ZumenError, loadBundled } from "@zumen-jp/zumen/node";

export async function POST(request) {
  const bytes = new Uint8Array(await request.arrayBuffer());
  const zumen = await loadBundled();
  const session = zumen.open();
  try {
    session.push("input", bytes);
    const opened = session.call({ v: 1, op: "open", input: "input", budget: { input_bytes: 10_000_000 } });
    const entities = session.call({ v: 1, op: "entities", doc: opened.result.doc, limit: 1 });
    return Response.json({ format: opened.result.format, total: entities.result.total });
  } catch (error) {
    if (error instanceof ZumenError) {
      return Response.json({ status: error.status, diagnostics: error.response?.diagnostics ?? [] }, { status: 422 });
    }
    throw error;
  } finally {
    session.close();
  }
}
