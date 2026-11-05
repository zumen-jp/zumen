import { View2D } from "../src/index.js";
import { bundle, fixture } from "./fixtures.mjs";

export function inputFixture() {
  const columns = fixture();
  columns.entity_paths.fill(0); columns.step_ids = []; columns.step_cells = [];
  return bundle(columns, { unit: "millimeter", extent: { min_x: -45, min_y: -45, max_x: 45, max_y: 45 } });
}

export function testTools({ assert, near, throws }) {
  const canvas = document.createElement("canvas"), input = inputFixture();
  const camera = { center: { x: 0, y: 0 }, scale: 2, rotation: 0 };
  const viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio: 2, view: camera });
  const pointer = (type, x, y, options = {}) => viewer.handleInput({ type, point: { x, y }, pointerId: 1, ...options });
  const click = (x, y, options = {}) => { pointer("pointerdown", x, y, options); return pointer("pointerup", x, y, options); };
  const key = (key, options = {}) => viewer.handleInput({ type: "keydown", key, ...options });
  const wheel = (options = {}) => viewer.handleInput({ type: "wheel", point: { x: 40, y: 70 }, deltaY: -100, ...options });
  const defaultOptions = viewer.getState().input;
  try {
    assert(viewer.getState().tool === "select" && Object.isFrozen(defaultOptions), "既定の選択ツールと凍結した入力設定");
    const emptyState = viewer.getState(); viewer.cancelInput();
    assert(viewer.getState() === emptyState, "中断する操作がなければ同じ状態を保つ");
    viewer.setScene(input);
    pointer("pointermove", 100, 70);
    assert(viewer.getState().hovered?.id === "space/0/h1" && viewer.getState().gesture === null, "ドラッグ前の移動は hover だけを更新する");
    const notifications = [];
    const off = viewer.on("selection", ({ state }) => notifications.push(state));
    assert(click(100, 70), "クリックを共通の入口から選択する");
    assert(viewer.getState().selection[0]?.id === "space/0/h1" && notifications[0].gesture === null, "選択の通知時にはポインター操作が完了している");
    click(100, 70, { ctrlKey: true });
    assert(viewer.getState().selection.length === 0, "Ctrl で選択を切り替える");
    click(100, 70); click(170, 30, { shiftKey: true });
    assert(viewer.getState().selection.length === 2, "Shift で選択に追加する");
    pointer("pointerdown", 30, 65); pointer("pointermove", 165, 75);
    assert(viewer.getState().gesture.type === "select" && Object.isFrozen(viewer.getState().gesture.start), "選択中の範囲は凍結した CSS px");
    pointer("pointerup", 165, 75);
    assert(viewer.getState().selection.some((ref) => ref.id === "space/0/h1"), "ドラッグの四角と交差する配置を選ぶ");
    off();

    viewer.setTool("pan"); viewer.jumpTo({ ...camera, rotation: Math.PI / 3 });
    const before = viewer.toScreen({ x: 7, y: 13 });
    pointer("pointerdown", 100, 100); pointer("pointermove", 117, 93); pointer("pointerup", 117, 93);
    const after = viewer.toScreen({ x: 7, y: 13 });
    near(after.x - before.x, 17, "回転中のドラッグの横移動"); near(after.y - before.y, -7, "回転中のドラッグの縦移動");
    viewer.setInputOptions({ drag: false });
    const noDrag = viewer.getView();
    pointer("pointerdown", 100, 100); pointer("pointermove", 120, 120); pointer("pointerup", 120, 120);
    assert(viewer.getView() === noDrag, "ドラッグを止めると移動しない");
    viewer.setInputOptions({ pointer: false, touch: true, drag: true });
    assert(!pointer("pointerdown", 100, 100), "mouse と pen の入力を個別に止める");
    assert(pointer("pointerdown", 100, 100, { pointerType: "touch" }), "pointer を止めても touch は使える");
    viewer.cancelInput();
    viewer.setInputOptions({ pointer: true, touch: false });
    assert(!pointer("pointerdown", 100, 100, { pointerType: "touch" }) && pointer("pointerdown", 100, 100, { pointerType: "pen" }), "touch を止めても pen は使える");
    viewer.cancelInput(); viewer.setInputOptions(defaultOptions); viewer.jumpTo(camera);
    pointer("pointerdown", 70, 100, { pointerType: "touch" });
    pointer("pointerdown", 130, 100, { pointerType: "touch", pointerId: 2 });
    const anchored = viewer.toScene({ x: 100, y: 100 });
    assert(viewer.getState().gesture.type === "pinch", "二つの touch は共通のピンチ操作になる");
    assert(!pointer("pointerdown", 100, 130, { pointerType: "touch", pointerId: 3 }), "三つ目のポインターを保管しない");
    pointer("pointermove", 160, 100, { pointerType: "touch", pointerId: 2 });
    near(viewer.getView().scale, 3, "ピンチの距離に従う拡大率");
    near(viewer.toScreen(anchored).x, 115, "ピンチは重心の移動も反映する");
    pointer("pointerup", 160, 100, { pointerType: "touch", pointerId: 2 });
    assert(viewer.getState().gesture === null && !pointer("pointerup", 70, 100, { pointerType: "touch" }), "片方を離すと残るポインターも解放する");
    viewer.setInputOptions({ pinch: false });
    pointer("pointerdown", 70, 100, { pointerType: "touch" });
    assert(!pointer("pointerdown", 130, 100, { pointerType: "touch", pointerId: 2 }), "ピンチを個別に止める");
    viewer.cancelInput(); viewer.setInputOptions(defaultOptions); viewer.jumpTo(camera);

    const wheelAnchor = viewer.toScene({ x: 40, y: 70 });
    assert(!wheel() && viewer.getView().scale === 2, "修飾キーなしのホイールはページへ残す");
    assert(wheel({ ctrlKey: true }), "Ctrl ホイールを受ける");
    near(viewer.toScreen(wheelAnchor).x, 40, "ホイールの固定点 X"); near(viewer.toScreen(wheelAnchor).y, 70, "ホイールの固定点 Y");
    viewer.setInputOptions({ wheel: false });
    const stoppedWheel = viewer.getView();
    assert(!wheel({ metaKey: true }) && viewer.getView() === stoppedWheel, "ホイールを個別に止める");
    viewer.setInputOptions({ wheel: true, wheelModifier: "none" }); assert(wheel(), "修飾キーを不要にもできる");
    viewer.jumpTo(camera);
    assert(viewer.handleInput({ type: "dblclick", point: { x: 100, y: 100 } }) && viewer.getView().scale === 4, "ダブルクリックで拡大する");
    viewer.handleInput({ type: "dblclick", point: { x: 100, y: 100 }, shiftKey: true });
    near(viewer.getView().scale, 2, "Shift ダブルクリックで縮小する");
    viewer.setInputOptions({ doubleClick: false });
    assert(!viewer.handleInput({ type: "dblclick", point: { x: 100, y: 100 } }), "ダブルクリックを個別に止める");
    assert(key("+") && viewer.getView().scale === 2.5, "キーボードで拡大する");
    const keyboardCenter = viewer.getView().center;
    assert(key("ArrowRight", { shiftKey: true }), "Shift の大きな移動を受ける");
    near(viewer.getView().center.x - keyboardCenter.x, 80, "キーボードの移動は CSS px");
    assert(!key("+", { ctrlKey: true }) && !key("q") && !key("toString"), "ページのショートカットと未対応のキーを残す");
    viewer.setInputOptions({ keyboard: false });
    assert(!key("Home"), "キーボードを個別に止める");
    viewer.setInputOptions(defaultOptions); viewer.jumpTo(camera); viewer.setTool("select");
    const middle = viewer.getView();
    pointer("pointerdown", 100, 100, { button: 1 }); pointer("pointermove", 110, 100); pointer("pointerup", 110, 100, { button: 1 });
    assert(viewer.getView() !== middle && viewer.getState().tool === "select", "中ボタンは選択ツールのまま移動する");

    viewer.jumpTo(camera); viewer.setTool("measure");
    const measurements = [], stopMeasurement = viewer.on("measurement", ({ state }) => measurements.push(state));
    pointer("pointerdown", 41, 71); pointer("pointermove", 159, 69); pointer("pointerup", 159, 69);
    const measurement = viewer.getState().measurement;
    near(measurement.distance, 60, "計測ツールは吸着した端点の距離を返す");
    assert(measurement.real.status === "known" && measurement.real.unit === "millimeter", "空間の直接の要素と既知の単位なら実寸も返す");
    assert(measurement.start.element.id === "space/0/h1" && measurement.end.element.id === "space/0/h1", "両端の ID と道筋を保持する");
    assert(measurements[0].gesture.type === "measure" && measurements[measurements.length - 1].gesture === null, "計測通知と操作中の状態を同時に更新する");
    stopMeasurement();
    viewer.setInputOptions({ drag: false });
    pointer("pointerdown", 41, 71); pointer("pointermove", 159, 69); pointer("pointerup", 159, 69);
    assert(viewer.getState().measurement.distance === 0, "ドラッグを止めた計測は pointerup でも離れた端点へ延ばさない");
    viewer.setInputOptions({ drag: true });
    const position = viewer.positionAt({ x: 100, y: 100 });
    viewer.setMeasurement({ start: position, end: viewer.positionAt({ x: 110, y: 100 }) });
    assert(viewer.getState().measurement.real.reason === "unbound", "自由な位置を要素に結び付けず実寸を推測しない");
    const measuredPixel = canvas.getContext("2d").getImageData(210, 200, 1, 1).data;
    assert(measuredPixel[0] === 204 && measuredPixel[1] === 0 && measuredPixel[2] === 102, "画素比を使って計測線を図面の上に描く");
    viewer.jumpTo({ ...camera, center: { x: 1e15, y: 1e15 }, rotation: Math.PI / 5 });
    const precise = viewer.toScreen(viewer.positionAt({ x: 100.01, y: 99.99 }));
    near(precise.x, 100.01, "分けた大原点を足さず画面へ戻す X"); near(precise.y, 99.99, "分けた大原点を足さず画面へ戻す Y");
    viewer.jumpTo(camera);
    key("Escape"); assert(viewer.getState().measurement === null, "Escape で計測を消す");
    pointer("pointerdown", 41, 71); viewer.setTool("select");
    assert(viewer.getState().gesture === null && viewer.getState().measurement === null && !pointer("pointerup", 159, 69), "ツール変更でポインターと計測を消す");
    viewer.setTool("measure"); pointer("pointerdown", 41, 71);
    const beforeInvalid = viewer.getState();
    throws(() => viewer.setScene({ ...input, result: { ...input.result, counts: { ...input.result.counts, lines: 99 } } }), "不正なシーンを拒否する");
    assert(viewer.getState() === beforeInvalid && viewer.getState().gesture.type === "measure", "失敗した差し替えでは進行中の操作を保つ");
    viewer.setScene({ ...input, result: { ...input.result, from: "sheet/0" } });
    assert(viewer.getState().measurement === null && viewer.getState().gesture === null, "成功した差し替えでは計測とポインターを消す");
    click(41, 71);
    assert(viewer.getState().measurement.real.reason === "paper", "用紙の距離を実寸としない");
    viewer.cancelInput(); viewer.setScene(input);
    const stable = viewer.getState();
    for (const invalid of [{ type: "unknown" }, { type: "pointerdown", pointerId: 1, point: { x: Infinity, y: 1 } }, { type: "keydown", key: 1 }]) throws(() => viewer.handleInput(invalid), "不正な入力は入口で拒否する");
    throws(() => viewer.setInputOptions({ wheel: 1 }), "入力設定の型を検査する");
    throws(() => viewer.setTool("missing"), "未登録のツールは拒否する");
    assert(viewer.getState() === stable, "拒否した入力と設定では状態を変えない");

    let activates = 0, cleanups = 0, cancels = [], observed = 0, draws = 0, context;
    const removeTool = viewer.addTool("custom", {
      onActivate(value) {
        context = value; activates++;
        value.subscribe(() => { observed++; });
        value.addOverlay((ctx) => { draws++; ctx.fillRect(1, 1, 2, 2); });
        return () => { cleanups++; };
      },
      onInput(event, { viewer }) {
        if (event.type === "pointerdown") { viewer.select(viewer.hitTest(event.point).slice(0, 1)); return true; }
        if (event.type === "pointerup") return true;
      },
      onCancel(reason, value) { assert(value.signal.aborted, "中断の通知より先に signal を止める"); cancels.push(reason); },
    });
    viewer.setTool("custom"); assert(activates === 0, "独自ツールは最初の入力で有効にする");
    const gestures = [], stopSelection = viewer.on("selection", ({ state }) => gestures.push(state.gesture));
    click(100, 70);
    assert(activates === 1 && viewer.getState().selection[0].id === "space/0/h1" && gestures[0].type === "custom", "独自ツールも選択と共通の入力状態を使う");
    assert(Object.isFrozen(context) && observed > 0 && draws > 0, "独自ツールの購読と重ね描きが動く");
    stopSelection();
    for (const [reason, action] of [["cancel", () => viewer.cancelInput()], ["tool", () => viewer.setTool("pan")], ["scene", () => viewer.setScene(input)], ["options", () => viewer.setInputOptions({ wheel: !viewer.getState().input.wheel })]]) {
      viewer.setTool("custom"); click(100, 70);
      const active = context; action();
      assert(active.signal.aborted && cancels[cancels.length - 1] === reason && viewer.getState().gesture === null, `${reason} で独自ツールを中断する`);
      const oldObserved = observed, oldDraws = draws; viewer.panBy(1, 0);
      assert(observed === oldObserved && draws === oldDraws, `${reason} でツールの購読と重ね描きを解除する`);
      throws(() => active.subscribe(() => {}), "終了した context から購読を残さない");
    }
    assert(cleanups === activates, "有効にした回数だけ後始末する");
    viewer.setTool("custom"); pointer("pointermove", 100, 70);
    assert(viewer.getState().gesture === null, "独自ツールの hover ではドラッグを作らない");
    key("Escape"); assert(context.signal.aborted && viewer.getState().gesture === null, "Escape は独自ツールにも共通の中断");
    removeTool(); assert(viewer.getState().tool === "select", "使用中の独自ツールを除くと選択へ戻す");
    removeTool();

    let failedCleanup = 0;
    const error = new Error("synthetic tool error");
    const removeError = viewer.addTool("error", { onActivate: () => () => { failedCleanup++; }, onInput() { throw error; } });
    viewer.setTool("error"); throws(() => click(100, 70), "独自ツールの例外を呼び出し側へ返す");
    assert(failedCleanup === 1 && viewer.getState().gesture === null, "入力の例外でも後始末と状態の消去を済ませる");
    removeError();
    const removeCancel = viewer.addTool("cancel-error", { onActivate: () => () => { failedCleanup++; }, onInput: () => true, onCancel() { throw error; } });
    viewer.setTool("cancel-error"); pointer("pointerdown", 100, 70);
    throws(() => viewer.setTool("select"), "中断の例外を返す");
    assert(failedCleanup === 2 && viewer.getState().tool === "select" && viewer.getState().gesture === null, "中断の例外でも残りの後始末とツール変更を済ませる");
    removeCancel();
    const removeReentrant = viewer.addTool("reentrant", { onInput() { viewer.setTool("pan"); return true; } });
    viewer.setTool("reentrant"); throws(() => click(100, 70), "ツールの callback 中の構成変更を拒否する"); removeReentrant();
    let unhandled = 0;
    const removeUnhandled = viewer.addTool("unhandled", { onInput() { unhandled++; return false; } });
    viewer.setTool("unhandled");
    assert(!pointer("pointerdown", 100, 70) && !pointer("pointerdown", 100, 70) && unhandled === 2, "処理しない down はポインターを残さない");
    removeUnhandled();
    const customMoves = [], removeNoDrag = viewer.addTool("no-drag", { onInput(event) { customMoves.push(event.type); return true; } });
    viewer.setTool("no-drag"); viewer.setInputOptions({ drag: false });
    pointer("pointerdown", 100, 70); pointer("pointermove", 110, 80); pointer("pointerup", 110, 80);
    assert(customMoves.join(",") === "pointerdown,pointerup", "止めたドラッグの移動は独自ツールにも渡さない");
    viewer.setInputOptions(defaultOptions); removeNoDrag();

    let sceneDraws = 0, viewerDraws = 0;
    const removeSceneOverlay = viewer.addOverlay(() => { sceneDraws++; });
    const removeViewerOverlay = viewer.addOverlay((ctx, state) => { viewerDraws++; assert(ctx.getTransform().a === state.size.pixelRatio, "重ね描きは CSS px の座標系"); }, { scope: "viewer" });
    const oldSceneDraws = sceneDraws; viewer.setScene(input);
    assert(sceneDraws === oldSceneDraws && viewerDraws > 1, "scene の重ね描きだけを差し替えで除く");
    removeSceneOverlay(); removeViewerOverlay();
    let failedDraws = 0;
    throws(() => viewer.addOverlay(() => { failedDraws++; throw error; }), "重ね描きの例外を返す");
    viewer.render(); assert(failedDraws === 1, "登録に失敗した重ね描きを残さない");
    throws(() => viewer.addOverlay(() => viewer.panBy(1, 1)), "描画の callback から状態を変更しない");
    const stopInput = viewer.on("input", () => viewer.cancelInput());
    throws(() => pointer("pointerdown", 100, 70), "同期の通知中に入力状態を変えない");
    stopInput(); assert(viewer.getState().gesture === null && !pointer("pointerup", 100, 70), "通知の例外でも進行中のポインターを解放する");
  } finally { viewer.destroy(); }

  const limited = new View2D(document.createElement("canvas"), { width: 100, height: 100, limits: { stateItems: 1 } });
  const tool = limited.addTool("first", { onInput: () => false });
  throws(() => limited.addTool("second", { onInput: () => false }), "独自ツールの登録数に上限を設ける");
  const overlay = limited.addOverlay(() => {});
  throws(() => limited.addOverlay(() => {}), "重ね描きの登録数に上限を設ける");
  tool(); overlay(); limited.destroy();
  if (typeof OffscreenCanvas !== "undefined") {
    const offscreen = new View2D(new OffscreenCanvas(200, 200));
    offscreen.setTool("pan");
    offscreen.handleInput({ type: "pointerdown", pointerId: 1, point: { x: 20, y: 20 } });
    offscreen.handleInput({ type: "pointermove", pointerId: 1, point: { x: 30, y: 40 } });
    assert(offscreen.getView().center.x === -10 && offscreen.getView().center.y === 20, "OffscreenCanvas へ正規化した入力を渡せる");
    offscreen.destroy();
  }
  testDomInput({ assert, near, throws });
}

function testDomInput({ assert, near, throws }) {
  const canvas = document.createElement("canvas");
  canvas.style.cssText = "width:200px;height:200px;border:4px solid;padding:6px;transform:scale(0.75);transform-origin:top left";
  document.body.append(canvas);
  const viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio: 2 });
  let destroyed = false;
  const key = () => { const event = new KeyboardEvent("keydown", { key: "+", bubbles: true, cancelable: true }); canvas.dispatchEvent(event); return event; };
  const wheel = (options = {}) => { const box = canvas.getBoundingClientRect(); const event = new WheelEvent("wheel", { clientX: box.x + 82.5, clientY: box.y + 82.5, deltaY: -1, deltaMode: 1, ctrlKey: true, cancelable: true, ...options }); canvas.dispatchEvent(event); return event; };
  const pointer = (type, x = 100, y = 100, options = {}) => {
    const box = canvas.getBoundingClientRect();
    const event = new PointerEvent(type, { pointerId: 77, pointerType: "mouse", button: 0, clientX: box.x + (x + 10) * .75, clientY: box.y + (y + 10) * .75, bubbles: true, cancelable: true, ...options });
    canvas.dispatchEvent(event); return event;
  };
  try {
    viewer.setScene(inputFixture()); viewer.jumpTo({ center: { x: 0, y: 0 }, scale: 2, rotation: 0 });
    assert(!key().defaultPrevented && viewer.getView().scale === 2, "入力は明示的に接続するまで登録しない");
    const detach = viewer.attachInput();
    assert(canvas.style.touchAction === "" && !canvas.hasAttribute("tabindex"), "touch-action と tabindex は利用側へ残す");
    pointer("pointerdown", 100, 70); pointer("pointerup", 100, 70);
    assert(viewer.getState().selection[0]?.id === "space/0/h1", "CSS の border・padding・拡縮を除いて入力を Canvas の CSS px に直す");
    assert(!wheel({ ctrlKey: false }).defaultPrevented, "ページへ残すホイールは preventDefault しない");
    assert(wheel().defaultPrevented, "処理したホイールだけ preventDefault する");
    near(viewer.getView().scale, 2 * Math.exp(16 * .002), "行単位のホイールを表示用 16 CSS px に直す");
    viewer.setInputOptions({ wheel: false }); assert(!wheel().defaultPrevented, "止めたホイールの既定の動作を残す");
    assert(key().defaultPrevented, "処理したキーの既定の動作を止める");
    pointer("pointerdown"); canvas.dispatchEvent(new Event("blur"));
    assert(viewer.getState().gesture === null, "フォーカスが外れたら操作を中断する");
    pointer("pointerdown"); pointer("pointercancel");
    assert(viewer.getState().gesture === null, "pointercancel で操作を中断する");
    const reattach = viewer.attachInput(); detach();
    assert(key().defaultPrevented, "古い解除関数で新しい接続を解除しない");
    reattach(); assert(!key().defaultPrevented, "接続を解除したらキーのイベントを残さない");
    const host = document.createElement("div"), button = document.createElement("button"), label = document.createElement("span");
    host.style.cssText = "width:200px;height:200px"; host.append(button); button.append(label); document.body.append(host);
    const stopHost = viewer.attachInput(host), beforeForm = viewer.getView();
    const formKey = new KeyboardEvent("keydown", { key: "+", bubbles: true, cancelable: true }); label.dispatchEvent(formKey);
    assert(!formKey.defaultPrevented && viewer.getView() === beforeForm, "フォームの子の入力もページへ残す");
    const composing = new KeyboardEvent("keydown", { key: "+", isComposing: true, bubbles: true, cancelable: true }); host.dispatchEvent(composing);
    assert(!composing.defaultPrevented && viewer.getView() === beforeForm, "文字の変換中のキーをページへ残す");
    stopHost(); host.remove();
    const errors = viewer.on("input", () => { throw new Error("synthetic notification"); });
    throws(() => viewer.attachInput(), "入力接続の通知の例外を返す"); errors();
    assert(!key().defaultPrevented, "接続に失敗したイベントを残さない");
    let cleanup = 0, signal;
    viewer.addTool("dom", { onActivate(context) { signal = context.signal; context.subscribe(() => {}); return () => { cleanup++; }; }, onInput: () => true });
    viewer.setTool("dom"); const stop = viewer.attachInput(); pointer("pointerdown"); stop();
    assert(signal.aborted && cleanup === 1 && viewer.getState().gesture === null, "DOM の接続解除でツールも片づける");
    viewer.attachInput(); pointer("pointerdown"); viewer.destroy(); destroyed = true;
    assert(signal.aborted && cleanup === 2 && !key().defaultPrevented, "destroy で入力と独自ツールを片づける");
    pointer("pointerup"); window.dispatchEvent(new Event("blur"));
    assert(true, "destroy 後に DOM の入力を送っても破棄した表示部品へ届かない");
  } finally { if (!destroyed) viewer.destroy(); canvas.remove(); }
}
