import { measureDistance, View2D } from "../src/index.js";
import { readScene } from "@zumen-jp/zumen";
import { bundle, fixture } from "./fixtures.mjs";

export function testCalibration({ assert, throws }) {
  const canvas = document.createElement("canvas"), input = bundle(fixture(), { from: "sheet/0", unit: "millimeter" });
  const viewer = new View2D(canvas, { width: 200, height: 200, view: { center: { x: 0, y: 0 }, scale: 10, rotation: 0 } });
  const start = { origin: { x: 0, y: 0 }, offset: { x: 0, y: 0 }, element: { id: "unrelated", path: [] } };
  const end = { origin: { x: 0, y: 0 }, offset: { x: 3, y: 4 } };
  const measurement = { from: "sheet/0", start, end };
  const response = (changes = {}) => ({ v: 1, diagnostics: [], blobs: [], result: {
    hash: input.document.hash, from: "sheet/0", calibration: "cal/first", distance: 123, unit: "unknown", unit_label: "原寸尺", ...changes,
  } });
  const diagnostic = (code, params = {}) => ({ v: 1, diagnostics: [{ code, severity: "error", count: 1, at: [], params }], blobs: [] });
  let request;
  const raw = response(), session = { call(value) { request = value; return raw; } };
  const budget = { work: 12, decoded_bytes: 2048 };
  assert(measureDistance(session, input.document, "cal/first", measurement, { budget, max_places: 0 }) === raw, "helper は診断も含め元の応答を返す");
  assert(request.v === 1 && request.op === "measure_distance" && request.doc === input.document.doc && request.from === "sheet/0" &&
    request.calibration === "cal/first" && request.expected_hash === input.document.hash && request.budget === budget && request.max_places === 0, "明示した ID・文書の版・予算をコアへ渡す");
  assert(request.start !== start && request.start.origin !== start.origin && request.start.offset !== start.offset && request.start.element === undefined &&
    Object.keys(request.start).join() === "origin,offset" && request.end.offset.x === 3, "helper は origin/offset だけを写し、参照や紙距離を送らない");
  const split = { from: "sheet/0", start: { origin: { x: 0, y: 0 }, offset: { x: 1, y: 0 } },
    end: { origin: { x: 1e16, y: 0 }, offset: { x: -1e16 + 2, y: 0 } } };
  measureDistance(session, input.document, "cal/first", split);
  assert(request.start.offset.x === 1 && request.end.origin.x === 1e16 && request.end.offset.x === -1e16 + 2, "コアへ渡す前に分割座標を足さない");
  const stopped = diagnostic("budget.work.exceeded");
  assert(measureDistance({ call: () => stopped }, input.document, "cal/first", split) === stopped, "helper は停止応答も加工しない");
  const rejected = Object.assign(new Error("stopped"), { response: stopped });
  let caught;
  try { measureDistance({ call() { throw rejected; } }, input.document, "cal/first", split); } catch (error) { caught = error; }
  assert(caught === rejected, "helper は Session.call の例外とその応答も変えない");

  try {
    viewer.setScene(input);
    const paper = { start: { origin: start.origin, offset: start.offset }, end };
    viewer.setMeasurement(paper);
    const ordinary = viewer.getState().measurement;
    assert(ordinary.distance === 5 && ordinary.real.reason === "paper" && ordinary.calibrated === undefined, "未選択なら紙上の通常測定だけを返す");
    const pixels = () => canvas.getContext("2d").getImageData(0, 0, 200, 200).data;
    const beforePixels = pixels();
    let calls = 0, lastMeasurement, lastDocument, current = response();
    const resolve = (value, document) => { calls++; lastMeasurement = value; lastDocument = document; return current; };
    const choice = { id: "cal/first", resolve };
    viewer.setMeasurementCalibration(choice);
    const selected = viewer.getState().measurement;
    assert(calls === 1 && lastMeasurement.distance === 5 && lastMeasurement.calibrated === undefined && lastDocument.hash === input.document.hash &&
      Object.isFrozen(lastMeasurement) && Object.isFrozen(lastDocument), "明示選択時に既存の測定と凍結した文書識別を同期で渡す");
    assert(selected.distance === ordinary.distance && selected.real === ordinary.real && selected.calibrated.status === "measured" &&
      selected.calibrated.distance === 123 && selected.calibrated.unit === "unknown" && selected.calibrated.unit_label === "原寸尺", "未知単位と元ラベルを保ち、紙距離と real を変えない");
    assert(Object.isFrozen(selected) && Object.isFrozen(selected.calibrated) && selected.result === undefined && selected.diagnostics === undefined, "校正結果だけを複製・凍結し、raw 応答は保管しない");
    choice.id = "changed"; choice.resolve = () => { throw new Error("changed"); }; current.result.distance = 999;
    assert(selected.calibrated.distance === 123 && selected.calibrated.calibration === "cal/first", "利用側の応答と選択の書き換えから状態を分離する");
    current = response({ distance: 42 }); viewer.setMeasurement(paper);
    assert(viewer.getState().measurement.calibrated.distance === 42 && calls === 2, "手動測定も保管した明示 ID で解決する");

    const failures = [
      [diagnostic("drawing.measurement.failed", { id: "cal/first", reason: "outside-bounds" }), "outside-bounds"],
      [diagnostic("drawing.measurement.failed", { id: "cal/first", reason: "sheet-mismatch" }), "sheet-mismatch"],
      [diagnostic("drawing.measurement.failed", { id: "cal/first", reason: "overflow" }), "overflow"],
      [diagnostic("call.value.invalid", { pointer: "/calibration" }), "calibration-missing"],
      [diagnostic("call.value.invalid", { pointer: "/from" }), "sheet-mismatch"],
      [diagnostic("call.value.invalid", { pointer: "/expected_hash" }), "document-mismatch"],
      [stopped, "stopped"], [diagnostic("call.request.cancelled"), "stopped"],
      [diagnostic("call.op.unsupported"), "failed"],
      [response({ hash: "different" }), "document-mismatch"], [response({ from: "sheet/1" }), "sheet-mismatch"],
      [response({ calibration: "cal/other" }), "calibration-missing"], [response({ distance: Infinity }), "overflow"],
      [response({ distance: NaN }), "overflow"], [response({ distance: -1 }), "failed"],
      [response({ unit: 3 }), "failed"], [response({ unit_label: null }), "failed"],
      [response({ unit_label: "尺".repeat(4097) }), "failed"],
      [{ ...response(), diagnostics: Array(257).fill({ code: "x" }) }, "failed"],
    ];
    for (const [value, reason] of failures) {
      current = value; viewer.setMeasurement(paper);
      const measured = viewer.getState().measurement;
      assert(measured.distance === 5 && measured.real.reason === "paper" && measured.calibrated.status === "unavailable" &&
        measured.calibrated.reason === reason && measured.calibrated.calibration === "cal/first", `${reason} でも紙距離と線を保持する`);
    }
    assert(pixels().every((value, i) => value === beforePixels[i]), "校正の失敗で紙上の計測線と描画を変えない");
    current = response({ unit_label: "尺".repeat(4096) }); viewer.setMeasurement(paper);
    assert(viewer.getState().measurement.calibrated.unit_label.length === 4096, "元ラベルの保持上限ちょうどは受け付ける");
    current = response();
    viewer.setMeasurementCalibration({ id: "cal/first", resolve() { throw new Error("resolver"); } });
    assert(viewer.getState().measurement.calibrated.reason === "failed", "解決関数の例外でも紙上の測定を保つ");
    viewer.setMeasurementCalibration({ id: "cal/first", resolve() { throw { response: diagnostic("drawing.measurement.failed", { id: "cal/first", reason: "outside-bounds" }) }; } });
    assert(viewer.getState().measurement.calibrated.reason === "outside-bounds", "Session.call の例外の応答からも理由を読み取る");
    viewer.setMeasurementCalibration({ id: "cal/first", resolve: () => Promise.reject(new Error("asynchronous")) });
    assert(viewer.getState().measurement.calibrated.reason === "failed", "Promise は待たず拒否する");
    viewer.setMeasurementCalibration({ id: "cal/first", resolve: () => Promise.resolve(response()) });
    assert(viewer.getState().measurement.calibrated.reason === "failed", "成功する Promise も同期の結果として採用しない");
    let thenCalled = false;
    viewer.setMeasurementCalibration({ id: "cal/first", resolve: () => ({ then() { thenCalled = true; } }) });
    assert(!thenCalled && viewer.getState().measurement.calibrated.reason === "failed", "任意の thenable を実行しない");

    const stableScene = viewer.getScene(), stableView = viewer.getView(), stableSize = viewer.getSize();
    viewer.setMeasurementCalibration({ id: "cal/first", resolve() { viewer.setScene(null); return response(); } });
    assert(viewer.getScene() === stableScene && viewer.getState().measurement.distance === 5 && viewer.getState().measurement.calibrated.reason === "failed", "再入したシーン変更を拒み、測定を確定する");
    let notified = 0;
    const off = viewer.on("measurement", () => { notified++; });
    viewer.setMeasurementCalibration({ id: "cal/first", resolve() {
      for (const change of [() => viewer.setMeasurement(null), () => viewer.setMeasurementCalibration(null), () => viewer.jumpTo({ ...stableView, scale: 2 }),
        () => viewer.resize(300, 300), () => viewer.render(), () => viewer.destroy(), () => viewer.subscribe(() => {}), off]) throws(change, "解決中の変更を拒否");
      return response();
    } });
    assert(notified === 1 && viewer.getView() === stableView && viewer.getSize() === stableSize && viewer.getScene() === stableScene &&
      viewer.getState().measurement.calibrated.status === "measured", "解決後に一度だけ通知し、再入で状態を変えない");
    off();
    const previous = viewer.getState();
    throws(() => viewer.setMeasurementCalibration({ id: "", resolve }), "空の校正 ID を拒否");
    assert(viewer.getState() === previous, "不正な選択で状態を変えない");
    viewer.setMeasurementCalibration(null);
    assert(viewer.getState().measurement.distance === 5 && viewer.getState().measurement.calibrated === undefined, "解除は紙上測定だけを残す");

    calls = 0; viewer.setMeasurementCalibration({ id: "cal/first", resolve });
    viewer.setMeasurement(null); viewer.setMeasurement(paper);
    assert(calls === 2, "計測線を消しても同じシーンの校正選択を保つ");
    viewer.setTool("measure");
    viewer.handleInput({ type: "pointerdown", point: { x: 95, y: 95 }, pointerId: 1 });
    viewer.handleInput({ type: "pointerup", point: { x: 120, y: 95 }, pointerId: 1 });
    assert(calls >= 4 && viewer.getState().measurement.calibrated.status === "measured", "用意した measure ツールも setMeasurement を通して校正する");
    const count = calls; viewer.setScene(input); viewer.setMeasurement(paper);
    assert(calls === count && viewer.getState().measurement.calibrated === undefined, "同じシーンの再設定でも校正を解除する");
    viewer.setMeasurementCalibration({ id: "cal/first", resolve });
    viewer.setScene(null); viewer.setScene(input); viewer.setMeasurement(paper);
    assert(viewer.getState().measurement.calibrated === undefined, "シーンの消去でも校正を解除する");
    viewer.destroy(); viewer.destroy();
    throws(() => viewer.setMeasurementCalibration(null), "破棄後に校正を設定しない");
  } finally { viewer.destroy(); }
}

/** 適合テストと同じ校正入り ZMDR を実際の wasm の Session へ渡す。 */
export function testCalibrationCore(zumen, source, { assert }) {
  const session = zumen.open(), viewer = new View2D(document.createElement("canvas"), { width: 200, height: 200 });
  const at = (x, y, dx = 0, dy = 0) => ({ origin: { x, y }, offset: { x: dx, y: dy } });
  try {
    session.push(source.name, Uint8Array.from(source.hex.match(/../g), (s) => parseInt(s, 16)));
    const opened = session.call({ v: 1, op: "open", input: source.name }).result;
    const drawing = session.call({ v: 1, op: "drawing", doc: source.name }).result;
    const identity = { doc: source.name, format: opened.format, hash: drawing.hash };
    const list = session.call({ v: 1, op: "calibrations", doc: source.name, expected_hash: drawing.hash }).result;
    assert(list.hash === "v2:11111111111111111111111111111111" && list.items.map((value) => value.id).join() === "metric,local,overlap", "ZMDR の保存された元 hash と校正の順を実 API から受け取る");
    const scene = (from) => {
      const result = session.call({ v: 1, op: "scene", doc: source.name, from, tolerance: 0.01 }).result;
      viewer.setScene({ scene: readScene(session.take(result.blob)), result, document: identity });
    };
    const choose = (id, options, document = identity) => viewer.setMeasurementCalibration({ id,
      resolve: (measurement) => measureDistance(session, document, id, measurement, options) });
    scene("sheet/hAA");
    viewer.setMeasurement({ start: at(1e16, 0, 0.25), end: at(1e16, 0, 0.75) });
    choose("metric");
    let measured = viewer.getState().measurement;
    assert(measured.distance === 0.5 && measured.calibrated.status === "measured" && measured.calibrated.distance === 1 &&
      measured.calibrated.unit === "meter" && measured.calibrated.unit_label === "m", "実 core の非単位行列を使い、紙距離は変えない");
    choose("overlap");
    assert(viewer.getState().measurement.calibrated.distance === 0.5, "重なる領域を明示した ID で選び分ける");
    choose("metric");
    viewer.setMeasurement({ start: at(1e16, 0, 0.25), end: at(1e16, 0, 5) });
    measured = viewer.getState().measurement;
    assert(measured.distance === 4.75 && measured.calibrated.reason === "outside-bounds", "実 core の閉領域外でも紙距離を保持する");
    viewer.setMeasurement({ start: at(1e16, 0, 0.25), end: at(1e16, 0, 0.75) });
    choose("metric", { budget: { work: 0 } });
    assert(viewer.getState().measurement.calibrated.reason === "stopped", "実 Session.call の予算停止を unavailable にする");
    choose("metric", undefined, { ...identity, hash: "different" });
    assert(viewer.getState().measurement.calibrated.reason === "document-mismatch", "実 core の元 hash 照合の失敗を返す");
    choose("missing");
    assert(viewer.getState().measurement.calibrated.reason === "calibration-missing", "実 core の校正 ID 不在を返す");
    scene("sheet/hAA/1");
    viewer.setMeasurement({ start: at(0, 0), end: at(3, 4) });
    choose("metric");
    assert(viewer.getState().measurement.calibrated.reason === "sheet-mismatch", "同じ元 handle の別用紙も区別する");
    choose("local");
    measured = viewer.getState().measurement;
    assert(measured.distance === 5 && measured.calibrated.distance === 5 && measured.calibrated.unit === "unknown" && measured.calibrated.unit_label === "尺", "実 core の未知単位と元ラベルを表示へ接続する");
    viewer.setMeasurement({ start: at(0, 0, 1), end: at(1e16, 0, -1e16 + 2) });
    assert(viewer.getState().measurement.delta.x === 1 && viewer.getState().measurement.calibrated.distance === 1, "異なる原点の正確な和差を実 core と照合する");
    viewer.setMeasurement({ start: at(Number.MAX_VALUE, 0, -Number.MAX_VALUE), end: at(-Number.MAX_VALUE, 0, Number.MAX_VALUE) });
    assert(viewer.getState().measurement.distance === 0 && viewer.getState().measurement.calibrated.distance === 0, "巨大な有限成分の相殺も実 core と一致する");
  } finally { viewer.destroy(); session.close(); }
}
