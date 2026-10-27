// 形式ごとに別のプロセスで 7 回測り、中央値を表にする。
// 使い方: node run.mjs <encode が書いたディレクトリ>
import { execFileSync } from "node:child_process";

const dir = process.argv[2] ?? "out";
const keys = ["send_ms", "parse_ms", "page_ms", "total_ms", "peak_rss_mb"];
const median = (values) => values.sort((a, b) => a - b)[Math.floor(values.length / 2)];
console.log(["format", ...keys, "check"].join("\t"));
for (const format of ["json", "columnar", "flatbuffers", "arrow"]) {
  const runs = [];
  for (let i = 0; i < 7; i++) {
    const out = execFileSync(process.execPath, ["--expose-gc", "page.mjs", format, `${dir}/${format}.bin`], {
      cwd: import.meta.dirname,
    });
    runs.push(JSON.parse(out));
  }
  const row = keys.map((key) => (runs[0][key] === undefined ? "-" : median(runs.map((r) => r[key])).toFixed(1)));
  console.log([format, ...row, runs[0].check].join("\t"));
}
