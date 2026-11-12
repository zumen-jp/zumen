import { valid } from "./validate.js";

const UNITS = new Set(["millimeter", "centimeter", "meter", "inch", "foot", "unknown"]);
const REASONS = new Set(["outside-bounds", "sheet-mismatch", "overflow"]);
const text = (value) => typeof value === "string" && value.length > 0 && value.length <= 4096;

function position(value, field) {
  valid(value && typeof value === "object", field);
  const point = (value, name) => {
    valid(value && typeof value === "object", name);
    const { x, y } = value;
    valid([x, y].every((v) => typeof v === "number" && Number.isFinite(v)), name);
    return { x, y };
  };
  return { origin: point(value.origin, `${field}.origin`), offset: point(value.offset, `${field}.offset`) };
}

/** 分割座標を保ってコアへ渡し、診断と予算停止を含む応答をそのまま返す。 */
export function measureDistance(session, document, calibration, measurement, options = {}) {
  valid(session && typeof session.call === "function", "measurement.session");
  valid(document && ["doc", "format", "hash"].every((key) => text(document[key])), "measurement.document");
  valid(text(calibration), "measurement.calibration");
  valid(measurement && text(measurement.from), "measurement.from");
  valid(options && typeof options === "object" && !Array.isArray(options), "measurement.options");
  const start = position(measurement.start, "measurement.start"), end = position(measurement.end, "measurement.end");
  return session.call({ v: 1, op: "measure_distance", doc: document.doc, from: measurement.from, calibration,
    start, end, expected_hash: document.hash,
    ...(options.budget === undefined ? {} : { budget: options.budget }),
    ...(options.max_places === undefined ? {} : { max_places: options.max_places }) });
}

export function calibrationOf(value) {
  if (value === null) return null;
  valid(value && typeof value === "object", "measurement.calibration");
  const { id, resolve } = value;
  valid(text(id) && typeof resolve === "function", "measurement.calibration");
  return Object.freeze({ id, resolve });
}

function failureOf(diagnostics, id) {
  let reason;
  // 利用側の応答も無制限にたどらず、上限を超えたものは採用しない。
  if (!Array.isArray(diagnostics)) return "failed";
  const count = diagnostics.length;
  if (!Number.isSafeInteger(count) || count < 0 || count > 256) return "failed";
  for (let i = 0; i < count; i++) {
    const diagnostic = diagnostics[i];
    const code = diagnostic?.code;
    if (typeof code !== "string") return "failed";
    if ((code.startsWith("budget.") && code.endsWith(".exceeded")) || code === "call.request.cancelled") return "stopped";
    if (code === "drawing.measurement.failed" && diagnostic.params?.id === id) {
      const value = diagnostic.params.reason;
      reason = REASONS.has(value) ? value : "failed";
    } else if (code === "call.value.invalid") {
      const pointer = diagnostic.params?.pointer;
      reason = pointer === "/expected_hash" || pointer === "/doc" ? "document-mismatch" :
        pointer === "/from" ? "sheet-mismatch" : pointer === "/calibration" ? "calibration-missing" : "failed";
    } else if (diagnostic.severity === "error") reason ??= "failed";
  }
  return reason;
}

function resolve(measurement, document, selection) {
  const unavailable = (reason) => Object.freeze({ status: "unavailable", calibration: selection.id, reason });
  try {
    let response;
    try { response = selection.resolve(measurement, document); }
    catch (error) { response = error?.response; } // Session.call の失敗は ZumenError.response に診断を持つ。
    if (!response || typeof response !== "object") return unavailable("failed");
    if (typeof response.then === "function") {
      // Promise は待たない。実際の Promise の拒否だけを回収し、任意の thenable は呼ばない。
      try { Promise.prototype.then.call(response, () => {}, () => {}); } catch { /* Promise 以外は採用しない。 */ }
      return unavailable("failed");
    }
    const failure = failureOf(response.diagnostics, selection.id);
    if (failure) return unavailable(failure);
    const result = response.result;
    if (response.v !== 1 || !result || typeof result !== "object") return unavailable("failed");
    const { hash, calibration, from, distance, unit, unit_label } = result;
    if (hash !== document.hash) return unavailable("document-mismatch");
    if (calibration !== selection.id) return unavailable("calibration-missing");
    if (from !== measurement.from) return unavailable("sheet-mismatch");
    if (typeof distance !== "number" || distance < 0 || typeof unit !== "string" || unit.length > 10 || !UNITS.has(unit) ||
      typeof unit_label !== "string" || unit_label.length > 4096) return unavailable("failed");
    if (!Number.isFinite(distance)) return unavailable("overflow");
    return Object.freeze({ status: "measured", calibration, from, distance, unit, unit_label });
  } catch { return unavailable("failed"); }
}

/** 元の紙上の測定と real を保ち、利用側の応答は状態へ残さない。 */
export function calibratedMeasurement(measurement, document, selection) {
  if (!measurement) return null;
  const { calibrated: _previous, ...base } = measurement;
  const plain = _previous === undefined ? measurement : Object.freeze(base);
  return selection ? Object.freeze({ ...plain, calibrated: resolve(plain, document, selection) }) : plain;
}
