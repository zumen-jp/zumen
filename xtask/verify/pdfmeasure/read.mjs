// #520: pdf-lib の公開 COS API で原値を読み、計測の意味や既定値を補わず返す。
import { createRequire } from "node:module";
import { readInput } from "./common.mjs";

const require = createRequire(import.meta.url);
let pdf;
try {
  pdf = require("pdf-lib");
} catch (error) {
  if (error.code === "MODULE_NOT_FOUND" && error.message.includes("'pdf-lib'")) process.exit(77);
  throw error;
}
const version = require("pdf-lib/package.json").version;
if (version !== "1.17.1") throw new Error("pdf-lib のバージョンが違います");
if (process.argv[2] === "--check") process.exit(0);

const { PDFDocument, PDFDict, PDFArray, PDFNumber, PDFName, PDFString, PDFHexString, PDFRef, PDFBool, PDFNull, PDFStream } = pdf;
const fields = {
  viewport: ["Type", "BBox", "Name", "Measure"],
  measure: ["Type", "Subtype", "R", "X", "Y", "D", "A", "T", "S", "O", "CYX"],
  numberFormat: ["Type", "U", "C", "F", "D", "FD", "RT", "RD", "PS", "SS", "O"],
};
const formats = new Set(["X", "Y", "D", "A", "T", "S"]);

try {
  const document = await PDFDocument.load(await readInput(64 * 1024 * 1024), {
    updateMetadata: false,
    capNumbers: false,
    throwOnInvalidObject: true,
  });
  const context = document.context;
  let visited = 0;
  function value(object, mode = "generic", depth = 0, chain = new Set()) {
    if (++visited > 100_000 || depth > 32) throw new Error("計測辞書の項目数または深さが上限を超えました");
    if (object === undefined) return { type: "absent" };
    if (object === PDFNull) return { type: "null" };
    if (object instanceof PDFRef) {
      const id = `${object.objectNumber} ${object.generationNumber}`;
      const result = { type: "ref", object: object.objectNumber, generation: object.generationNumber };
      if (chain.has(id)) return { ...result, value: { type: "cycle" } };
      const next = new Set(chain);
      next.add(id);
      const resolved = context.lookup(object);
      return { ...result, value: resolved === undefined ? { type: "unresolved" } : value(resolved, mode, depth + 1, next) };
    }
    if (object instanceof PDFNumber) return { type: "number", value: object.asNumber() };
    if (object instanceof PDFBool) return { type: "boolean", value: object.asBoolean() };
    if (object instanceof PDFName) return { type: "name", value: object.decodeText(), bytes: Buffer.from(object.asBytes()).toString("hex") };
    if (object instanceof PDFString || object instanceof PDFHexString) {
      const bytes = object.asBytes();
      if (bytes.length > 1024 * 1024) throw new Error("文字列が照合ツールの上限を超えました");
      return {
        type: "string",
        representation: object instanceof PDFHexString ? "hex" : "literal",
        value: object.decodeText(),
        bytes: Buffer.from(bytes).toString("hex"),
      };
    }
    if (object instanceof PDFArray) {
      const items = [];
      for (let i = 0; i < object.size(); i++) items.push(value(object.get(i), mode, depth + 1, chain));
      return { type: "array", value: items };
    }
    if (object instanceof PDFStream) return { type: "stream", dictionary: value(object.dict, mode, depth + 1, chain) };
    if (object instanceof PDFDict) {
      const keys = new Map(object.keys().map(key => [key.decodeText(), key]));
      const names = [...new Set([...(fields[mode] ?? []), ...keys.keys()])].sort();
      const entries = Object.create(null);
      for (const name of names) {
        const key = keys.get(name) ?? PDFName.of(name);
        const childMode = mode === "viewport" && name === "Measure" ? "measure"
          : mode === "measure" && formats.has(name) ? "numberFormat" : "generic";
        entries[name] = value(object.get(key, true), childMode, depth + 1, chain);
      }
      return { type: "dict", value: entries };
    }
    return { type: "unsupported", class: object.constructor.name };
  }

  const pages = document.getPages().map((page, index) => ({
    index,
    MediaBox: value(page.node.get(PDFName.of("MediaBox"), true)),
    CropBox: value(page.node.get(PDFName.of("CropBox"), true)),
    Rotate: value(page.node.get(PDFName.of("Rotate"), true)),
    UserUnit: value(page.node.get(PDFName.of("UserUnit"), true)),
    VP: value(page.node.get(PDFName.of("VP"), true), "viewport"),
  }));
  process.stdout.write(`${JSON.stringify({ oracle: "pdf-lib", version, pages })}\n`);
} catch (error) {
  process.stderr.write(`PDF 原値照合: ${error.message}\n`);
  process.exitCode = 1;
}
