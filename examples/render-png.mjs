// 図面を PNG にする例。入力と出力、範囲や色、使うフォントは呼ぶ側が明示する。
import { readFileSync, writeFileSync } from "node:fs";
import { load, ZumenError } from "../bindings/js/src/index.js";

const [wasm, drawing, destination, settings, font] = process.argv.slice(2);
if (!wasm || !drawing || !destination || !settings) {
  throw new Error("wasm・図面・出力 PNG・設定 JSON のパスが必要です。末尾にフォントを指定できます。");
}
const options = JSON.parse(readFileSync(settings, "utf8"));
const zumen = await load(readFileSync(wasm));
const session = zumen.open();
try {
  session.push("drawing", readFileSync(drawing));
  const opened = session.call({ v: 1, op: "open", input: "drawing" });
  if (font) {
    session.push("font", readFileSync(font));
    options.fonts = [{ input: "font" }];
  }
  const reply = session.call({ ...options, v: 1, op: "render", doc: opened.result.doc, format: "png" });
  console.error(JSON.stringify(reply.diagnostics));
  writeFileSync(destination, session.take(reply.result.blob));
} catch (error) {
  if (error instanceof ZumenError) console.error(JSON.stringify(error.response?.diagnostics));
  throw error;
} finally {
  session.close();
}
