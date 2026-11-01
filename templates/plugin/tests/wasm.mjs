// プラグインを束ねた wasm のモジュールを、JavaScript の包みの loadModules で呼ぶ（#399、決定 0243）。
//
// 使い方: node templates/plugin/tests/wasm.mjs <zumen_plugin_template.wasm> <drawing.wasm>
//
// 包みは、コアのどの形式でもない入力を、plugin.json の判定の規則で試し、プラグインのモジュールを読み込む。コアの
// 形式の入力は、コアのモジュールで読む。

import { readFileSync } from "node:fs";

import { MODULES, loadModules } from "../../../bindings/js/src/index.js";

const [pluginPath, drawingPath] = process.argv.slice(2);
if (!pluginPath || !drawingPath) {
  console.error("使い方: node templates/plugin/tests/wasm.mjs <zumen_plugin_template.wasm> <drawing.wasm>");
  process.exit(2);
}

const failures = [];
const check = (ok, message) => {
  if (!ok) {
    failures.push(message);
  }
};

const description = JSON.parse(readFileSync(new URL("../plugin.json", import.meta.url), "utf8"));
const plugin = { ...description, ops: MODULES[description.module].ops, load: () => readFileSync(pluginPath) };
const modules = loadModules({ drawing: () => readFileSync(drawingPath) }, [plugin]);

const sample = new TextEncoder().encode("ZUMEN-LINES 1\n0 0 1000 0\n1000 0 1000 500\n");
const detection = modules.detect(sample);
check(detection?.module === "zumen_lines" && detection?.format === "zumen_lines", `判定が ${JSON.stringify(detection)}`);
const zumen = await modules.forInput(sample);
check(zumen === (await modules.get("zumen_lines")), "プラグインのモジュールを 2 度読み込んだ");
check(zumen !== (await modules.get("drawing")), "プラグインの入力を、コアのモジュールで読んだ");

const session = zumen.open();
session.push("a.txt", sample);
const open = session.call({ v: 1, op: "open", input: "a.txt" });
check(open.result?.format === "zumen_lines", `open の結果が ${JSON.stringify(open)}`);
const entities = session.call({ v: 1, op: "entities", doc: "a.txt" });
const ids = entities.result?.items?.map((item) => item.id);
check(JSON.stringify(ids) === JSON.stringify(["space/0/r0", "space/0/r1"]), `図形の ID が ${JSON.stringify(ids)}`);
const capabilities = session.call({ v: 1, op: "capabilities" });
check(
  capabilities.result?.formats?.some((format) => format.id === "zumen_lines" && format.read && !format.write),
  "capabilities にプラグインの形式がない",
);
session.close();

// コアの図面のモジュールは、プラグインの形式を読まない
const core = (await modules.get("drawing")).open();
core.push("a.txt", sample);
try {
  core.call({ v: 1, op: "open", input: "a.txt" });
  failures.push("コアのモジュールが、プラグインの形式を読んだ");
} catch {
  // 形式が分からないので失敗する
}
core.close();

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log("プラグインのモジュールを、包みから呼べた");
