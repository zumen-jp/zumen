// プレイグラウンドに入れる公開サンプル（#689）。出所とライセンスは samples/ledger.toml から読み、配布物に入れる。
// 選び方: 中身の違う DXF と、DWG の R2000・R2004・R2018 を 1 つずつ。どれも再配布を認めるライセンスのもの（決定 0171）。
// SXF・JWW は、再配布できる公開サンプルがないので入れない（samples/README.md）。
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { normalizePath } from "vite";

export const SAMPLE_PATHS = [
  "dxf/text_alignments.dxf",
  "dxf/opendxfviewer-latin1.dxf",
  "dwg/zumen-layout-r2000.dwg",
  "dwg/zumen-minsert-r2004.dwg",
  "dwg/zumen-xclip-r2018.dwg",
];

const FIELDS = ["path", "title", "publisher", "url", "license", "license-url", "attribution"];
// 欠けてもよい欄。変えたことの表示を求めるライセンス（CC BY など）のために出す
const OPTIONAL = ["changes"];
// 本文の同梱を求めるライセンス。表示の文に licenses/ の本文の道筋がなければ、組み立てを止める
const NEEDS_TEXT = /^(MIT|BSD-[23]-Clause|Apache-2\.0|ISC|Zlib)$/;

/** 台帳の `[[samples]]` の項目から、文字列の値だけを読む。台帳は `cargo xtask codegen --check` が形を確かめる */
export function readLedger(text) {
  return text.split(/^\[\[samples\]\]\s*$/m).slice(1).map((block) => {
    const entry = {};
    for (const line of block.split(/\r?\n/)) {
      const match = /^([\w-]+) = ("(?:[^"\\]|\\.)*")\s*$/.exec(line);
      if (match && (FIELDS.includes(match[1]) || OPTIONAL.includes(match[1]))) entry[match[1]] = JSON.parse(match[2]);
    }
    return entry;
  });
}

/** 入れるサンプルの出所と、同梱を求めるライセンスの本文のファイル（`licenses/`） */
export function selectSamples(root) {
  const ledger = readLedger(readFileSync(resolve(root, "samples/ledger.toml"), "utf8"));
  return SAMPLE_PATHS.map((path) => {
    const entry = ledger.find((item) => item.path === path);
    if (!entry) throw new Error("samples/ledger.toml にないサンプル: " + path);
    for (const field of FIELDS) if (!entry[field]) throw new Error(path + " の " + field + " がない");
    const text = /licenses\/[\w.-]+\.txt/.exec(entry.attribution)?.[0] ?? null;
    if (!text && NEEDS_TEXT.test(entry.license)) throw new Error(path + " は本文の同梱を求めるが、表示の文に licenses/ の本文がない");
    return { ...entry, file: resolve(root, "samples", path), text: text && resolve(root, text) };
  });
}

/** サンプルの出所を、画面に渡す仮想のモジュール（`virtual:samples`）と、配布物の `samples.md` にする Vite のプラグイン */
export function samplesPlugin(root) {
  const id = "virtual:samples", resolved = "\0" + id;
  const samples = selectSamples(root);
  return {
    name: "zumen-samples",
    resolveId(source) { return source === id ? resolved : null; },
    load(source) {
      if (source !== resolved) return null;
      const imports = [], items = [];
      samples.forEach((sample, index) => {
        imports.push(`import file${index} from ${JSON.stringify(normalizePath(sample.file) + "?url")};`);
        if (sample.text) imports.push(`import text${index} from ${JSON.stringify(normalizePath(sample.text) + "?url")};`);
        items.push(`{ path: ${JSON.stringify(sample.path)}, title: ${JSON.stringify(sample.title)}, publisher: ${JSON.stringify(sample.publisher)}, source: ${JSON.stringify(sample.url)}, license: ${JSON.stringify(sample.license)}, licenseUrl: ${JSON.stringify(sample["license-url"])}, attribution: ${JSON.stringify(sample.attribution)}, changes: ${JSON.stringify(sample.changes ?? null)}, file: file${index}, text: ${sample.text ? "text" + index : "null"} }`);
      });
      return imports.join("\n") + "\nexport const samples = Object.freeze([\n" + items.join(",\n") + "\n]);\n";
    },
    generateBundle(_, bundle) {
      // 配布物の中の名前（ハッシュつき）を、元のファイルから引く
      const emitted = (file) => Object.values(bundle).find((item) => item.type === "asset"
        && (item.originalFileNames ?? []).some((name) => normalizePath(file).endsWith("/" + normalizePath(name).replace(/^(\.\.\/)+/, ""))))?.fileName;
      const lines = ["# 同梱した公開サンプル", "",
        "プレイグラウンドの配布物に入れた公開サンプルと、その出所・ライセンスです（samples/ledger.toml から作りました）。",
        "これらのファイルは、プレイグラウンドのライセンス（MIT OR Apache-2.0）ではなく、それぞれのライセンスに従います（決定 0171）。", ""];
      for (const sample of samples) {
        lines.push("## " + sample.path, "", "- 配布物のファイル: " + (emitted(sample.file) ?? "（見つからない）"), "- 題: " + sample.title,
          "- 公開した者: " + sample.publisher, "- 出所: " + sample.url,
          "- ライセンス: " + sample.license, "- ライセンスの URL: " + sample["license-url"], "- 表示: " + sample.attribution);
        if (sample.changes) lines.push("- 変えたこと: " + sample.changes);
        if (sample.text) lines.push("- 本文のファイル: " + (emitted(sample.text) ?? "（見つからない）"));
        lines.push("");
        if (sample.text) lines.push("```text", readFileSync(sample.text, "utf8").trimEnd(), "```", "");
      }
      this.emitFile({ type: "asset", fileName: "samples.md", source: lines.join("\n") });
    },
  };
}
