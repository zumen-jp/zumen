import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { loadBundled } from "@zumen-jp/zumen/node";
import { parse } from "smol-toml";
import { drawingOutputs, formats, releaseVersion } from "../src/data/site.ts";

const metadata = JSON.parse(readFileSync(new URL("../node_modules/@zumen-jp/zumen/package.json", import.meta.url)));
assert.equal(metadata.version, releaseVersion, "サイトの対象版と公開パッケージの版");

// 公開版を更新するときはタグを取得したチェックアウトで --tag を付ける。
if (process.argv.includes("--tag")) {
  const ledger = parse(execFileSync("git", ["show", `v${releaseVersion}:formats/ledger.toml`], { encoding: "utf8" })).formats;
  for (const row of formats) {
    for (const id of row.ids) {
      assert.ok(ledger[id], `形式台帳に ${id} がない`);
      for (const direction of ["read", "write"]) {
        const supported = ledger[id].place === "core" && Boolean(ledger[id][direction]?.level);
        assert.equal(row[direction] === "○", supported, `${id} の ${direction}`);
      }
    }
  }
}

const core = await loadBundled();
const session = core.open();
try {
  const stb = formats.find((row) => row.ids.includes("stbridge"));
  session.push("frame.stb", '<?xml version="1.0"?><ST_BRIDGE version="2.1.0"><StbModel/></ST_BRIDGE>');
  const openStb = () => session.call({ v: 1, op: "open", input: "frame.stb" });
  if (stb.read === "○") assert.equal(openStb().result.format, "stbridge");
  else assert.throws(openStb, (error) => error.response?.diagnostics.some((d) => d.code === "call.format.unsupported"));

  const line = "0\nLINE\n5\n10\n8\n0\n10\n1\n20\n2\n11\n11\n21\n12\n";
  session.push("line.dxf", "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1021\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n" + line + "0\nENDSEC\n0\nEOF\n");
  session.call({ v: 1, op: "open", input: "line.dxf" });
  for (const { format, extension } of drawingOutputs) {
    const response = session.call({ v: 1, op: "convert", doc: "line.dxf", format, to: `out.${extension}` });
    const bytes = session.take(response.blobs[0].id);
    const reopened = core.open();
    try {
      const input = `saved.${extension}`;
      reopened.push(input, bytes);
      assert.equal(reopened.call({ v: 1, op: "open", input }).result.format, format);
      const entities = reopened.call({ v: 1, op: "entities", doc: input, fields: ["geometry"] });
      assert.deepEqual(entities.result.items.map((item) => item.geometry), [{ shape: "line", start: [1, 2], end: [11, 12] }], extension);
    } finally {
      reopened.close();
    }
  }
} finally {
  session.close();
}
console.log(`v${releaseVersion} の ST-Bridge と ${drawingOutputs.map((output) => output.label).join("・")} の変換・再読を確認`);
