// #520: 入力の数値だけを渡し、元の PDF4QT の定義を経た formatValue の引数を返す。
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { artifactDirectory, executableName, pin, readInput, validateArtifacts } from "./common.mjs";

try {
  await validateArtifacts();
} catch (error) {
  process.stderr.write(`計測照合: ${error.message}\n`);
  process.exit(error.code === "ENOENT" || error.code === "ENOTDIR" ? 77 : 1);
}
const executable = join(artifactDirectory, executableName);
if (process.argv[2] === "--check") {
  const checked = spawnSync(executable, ["--check"], { stdio: "ignore", timeout: 10_000, windowsHide: true });
  process.exit(checked.status === 0 ? 0 : checked.error?.code === "ENOENT" ? 77 : 1);
}

try {
  const finite = value => {
    if (typeof value !== "number" || !Number.isFinite(value)) throw new Error("有限の JSON number が必要です");
    return Object.is(value, -0) ? "-0" : String(value);
  };
  const factors = value => {
    const array = value === undefined ? [] : Array.isArray(value) ? value : [value];
    if (array.length > 256) throw new Error("書式の数が上限を超えました");
    return [String(array.length), ...array.map(finite)];
  };
  const input = JSON.parse((await readInput(1024 * 1024)).toString("utf8"));
  const cases = Array.isArray(input) ? input : [input];
  if (cases.length > 1024) throw new Error("計測の件数が上限を超えました");
  const lines = cases.map(request => {
    if (!request || typeof request !== "object" || Array.isArray(request)) throw new Error("計測の object が必要です");
    const allowed = new Set(["mode", "delta", "length", "x", "y", "distance", "cyx"]);
    if (Object.keys(request).some(key => !allowed.has(key))) throw new Error("計測に未対応のキーがあります");
    const mode = request.mode === undefined ? "vector" : request.mode;
    let first, second;
    if (mode === "vector") {
      if (!Array.isArray(request.delta) || request.delta.length !== 2) throw new Error("delta に 2 個の数が必要です");
      [first, second] = request.delta;
    } else if (mode === "scalar") {
      first = request.length;
      second = 0;
    } else throw new Error("mode は vector または scalar です");
    return [mode, finite(first), finite(second), finite(request.cyx === undefined ? 1 : request.cyx),
      ...factors(request.x), ...factors(request.y), ...factors(request.distance)].join(" ");
  });
  const calculated = spawnSync(executable, [], {
    input: `${lines.join("\n")}\n`, encoding: "utf8", timeout: 30_000, maxBuffer: 16 * 1024 * 1024,
    windowsHide: true,
  });
  if (calculated.error || calculated.status !== 0) throw new Error(calculated.stderr?.trim() || "算術照合を実行できませんでした");
  const results = calculated.stdout.trim() ? calculated.stdout.trim().split(/\r?\n/).map(line => JSON.parse(line)) : [];
  if (results.length !== cases.length) throw new Error("算術照合の結果の件数が違います");
  const oracle = { oracle: "pdf4qt-arithmetic", pin };
  process.stdout.write(`${JSON.stringify(Array.isArray(input) ? { ...oracle, results } : { ...oracle, ...results[0] })}\n`);
} catch (error) {
  process.stderr.write(`計測照合: ${error.message}\n`);
  process.exitCode = 1;
}
