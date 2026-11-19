// 3D のプレイグラウンド（#818）。選んだファイルは Worker の wasm で開き、外へ送らない。保存は明示操作で行う。
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useDrawing, useDrawingViewer } from "@zumen-jp/react";
import { useScene3D, useView3D, useView3DEvent, useView3DInput, useView3DState } from "@zumen-jp/react/view3d";
import { attachGround2D } from "@zumen-jp/view3d/ground2d";
import { attachPointCloud } from "@zumen-jp/view3d/pointcloud";
import { catalog, loadDrawingModule, loadModule } from "./scene3d-module.js";

const WIDTH = 800, HEIGHT = 480;
const viewOptions = Object.freeze({
  width: WIDTH, height: HEIGHT, selectionColor: "#c74700",
  views: [{ id: "main", rect: { x: 0, y: 0, width: WIDTH, height: HEIGHT },
    camera: { projection: "perspective", position: [10, -10, 10], direction: norm([-1, 1, -1]), up: [0, 0, 1], fov: 50 } }],
});
const inputOptions = Object.freeze({ select: true, pan: true, orbit: true, zoom: true, touch: true });
const groundOptions = Object.freeze({ width: 512, height: 512, foreground: "#333333" });
const sceneOptions = Object.freeze({ loadModule, catalog });
const drawingOptions = Object.freeze({ loadModule: loadDrawingModule, catalog });
const kinds = { building: "建物", feature: "地物", pointcloud: "点群" };
const reasons = {
  "color-unresolved": "色を決められない要素を既定の色で描きました", "points-unsupported": "点は点群の重ね合わせで描きます",
  "no-scene": "3D のシーンがありません", "no-scene-2d": "地面の図面にシーンがありません", paper: "用紙は地面に置けません",
  definition: "定義の空間は地面に置けません", "unit-unknown": "単位が分かりません", "unit-mismatch": "単位を合わせられません",
  "unit-not-metre": "表示中のシーンの単位がメートルではありません", "canvas-mismatch": "図面の Canvas が合いません", limit: "上限を超えました",
  "crs-unknown": "座標系が分かりません", "crs-mismatch": "座標系が違います", "map-unknown": "表示中のシーンが地図の変換を持ちません",
  "map-unsupported": "点群の地図の変換が恒等ではありません", "height-mismatch": "高さの基準が違います", "not-pointcloud": "点群のファイルではありません",
  empty: "点群がありません", removed: "外しました",
};

function norm(v) { const l = Math.hypot(...v); return v.map((x) => x / l); }
function cross(a, b) { return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]; }
const add = (a, b, s = 1) => a.map((v, k) => v + b[k] * s);
const round = (v) => Math.round(v * 100) / 100;

function changeCamera(viewer, change) {
  const { views } = viewer.getState();
  viewer.setViews(views.map((view) => (view.id === "main" ? { ...view, camera: change(view.camera) } : view)));
}
/** 範囲の全体が入る視点にする。範囲がなければ表示部品の fit に任せる。 */
function frame(viewer, bounds) {
  if (!bounds) { viewer.fit("main"); return; }
  const center = bounds.min.map((v, k) => (v + bounds.max[k]) / 2);
  const radius = Math.max(Math.hypot(...bounds.max.map((v, k) => v - bounds.min[k])) / 2, 1e-6);
  const direction = norm([-1, 1, -1]), distance = radius / Math.sin((25 * Math.PI) / 180) * 1.1;
  changeCamera(viewer, () => ({ projection: "perspective", position: add(center, direction, -distance), direction, up: [0, 0, 1], fov: 50 }));
}
function zoom(viewer, bounds, factor) {
  changeCamera(viewer, (camera) => {
    if (camera.projection === "orthographic") return { ...camera, width: camera.width * factor };
    const center = bounds ? bounds.min.map((v, k) => (v + bounds.max[k]) / 2) : add(camera.position, camera.direction);
    const distance = Math.max(Math.hypot(...center.map((v, k) => v - camera.position[k])), 1e-6);
    return { ...camera, position: add(camera.position, camera.direction, distance * (1 - factor)) };
  });
}
function pan(viewer, bounds, dx, dy) {
  changeCamera(viewer, (camera) => {
    const right = norm(cross(camera.direction, camera.up)), up = norm(cross(right, camera.direction));
    const center = bounds ? bounds.min.map((v, k) => (v + bounds.max[k]) / 2) : camera.position;
    const step = (camera.projection === "orthographic" ? camera.width : Math.hypot(...center.map((v, k) => v - camera.position[k]))) * 0.1;
    return { ...camera, position: add(add(camera.position, right, dx * step), up, dy * step) };
  });
}
function download(bytes, name) {
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" }));
  const link = Object.assign(document.createElement("a"), { href: url, download: name });
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
function unitLabel(unit) {
  if (unit?.name !== "METRE") return unit?.name ?? "";
  return { MILLI: "mm", CENTI: "cm", KILO: "km" }[unit.prefix] ?? "m";
}
/** 寸法の文字を描いた Canvas。表示部品が写しを作るので、フォントや URL は取りに行かない。 */
function label(text) {
  const canvas = document.createElement("canvas");
  canvas.width = 320; canvas.height = 64;
  const context = canvas.getContext("2d");
  context.fillStyle = "#ffffff"; context.fillRect(0, 0, 320, 64);
  context.fillStyle = "#c74700"; context.font = "bold 36px sans-serif"; context.textAlign = "center"; context.textBaseline = "middle";
  context.fillText(text, 160, 32);
  return { image: canvas, width: 160, height: 32 };
}
const baseName = (file) => (file?.name ?? "model").replace(/\.[^.]*$/, "") || "model";

function Diagnostics({ entries }) {
  if (!entries?.length) return null;
  return <section className="diagnostics" aria-labelledby="diagnostics-title">
    <h2 id="diagnostics-title">診断</h2>
    <ul className="rows">{entries.map((entry, i) => <li key={i}>{entry.stage}: {entry.text}<span className="detail">{entry.diagnostic.code}</span></li>)}</ul>
  </section>;
}

function Inspector({ viewer, opened, selected }) {
  const [details, setDetails] = useState(null);
  useEffect(() => {
    if (opened.status !== "ready" || !selected) { setDetails(null); return; }
    let live = true;
    setDetails({ id: selected, status: "loading" });
    // 選択が変わった後に届いた前の要素の応答は画面へ出さない。
    opened.getElement(selected).then(
      (value) => { if (live) setDetails({ id: selected, status: "ready", value }); },
      (error) => { if (live) setDetails({ id: selected, status: "error", message: error.message }); });
    return () => { live = false; };
  }, [opened, selected]);
  const element = viewer && opened.status === "ready" && selected ? viewer.getElement(opened.input.document, selected) : null;
  return <section aria-labelledby="inspector-title" className="inspector">
    <h2 id="inspector-title">選んだ要素</h2>
    <div role="status" aria-live="polite">
      {!element ? <p className="quiet">表示か一覧で要素を選んでください。</p> : <>
        <dl className="properties">
          <dt>ID</dt><dd>{element.id}</dd>
          <dt>クラス</dt><dd>{element.class}</dd>
          <dt>名前</dt><dd>{element.name || "（なし）"}</dd>
          <dt>階</dt><dd>{element.storey ?? "（なし）"}</dd>
        </dl>
        {details?.id === element.id && details.status === "loading" && <p className="quiet">建物モデルの属性を読んでいます…</p>}
        {details?.id === element.id && details.status === "ready" && details.value && <details>
          <summary>建物モデルの属性</summary><pre>{JSON.stringify(details.value, null, 2)}</pre>
        </details>}
        {details?.id === element.id && details.status === "error" && <p>属性を読めませんでした: {details.message}</p>}
      </>}
    </div>
  </section>;
}

export function Viewer3D() {
  const [file, setFile] = useState(null), [cloudFile, setCloudFile] = useState(null), [groundFile, setGroundFile] = useState(null);
  const [generation, setGeneration] = useState(0);
  const [canvas, setCanvas] = useState(null), [groundCanvas, setGroundCanvas] = useState(null);
  const [dimension, setDimension] = useState(false);
  const [messages, setMessages] = useState([]);
  const [viewerError, setViewerError] = useState(null);
  const opened = useScene3D(file, sceneOptions);
  const cloudOpened = useScene3D(cloudFile, sceneOptions);
  const drawing = useDrawing(groundFile, drawingOptions);
  const viewer = useView3D(canvas, viewOptions, setViewerError);
  const groundViewer = useDrawingViewer(groundCanvas, groundOptions);
  useLayoutEffect(() => { setViewerError(null); }, [canvas]);
  useLayoutEffect(() => { if (viewer) setViewerError(null); }, [viewer]);
  useView3DInput(viewer, canvas, inputOptions);
  const state = useView3DState(viewer);
  // 資源の数は描いた後の値を出す。
  const [stats, setStats] = useState(null);
  useView3DEvent(viewer, "render", () => setStats(viewer.getStats()));
  const [sceneReport, setSceneReport] = useState(null);
  const [cloud, setCloud] = useState(null), [ground, setGround] = useState(null), [sceneCloud, setSceneCloud] = useState(null);
  const current = useRef(opened);
  current.current = opened;
  const bounds = opened.status === "ready" ? opened.bounds : null;

  // 開いたシーンを描き、全体を見せる。点群のファイルは同じシーンに点群の重ね合わせで点を描く。
  useLayoutEffect(() => {
    if (!viewer) return;
    setMessages([]);
    if (opened.status !== "ready") { viewer.setScene(null); setSceneReport(null); return; }
    setSceneReport(viewer.setScene(opened.input));
    frame(viewer, opened.bounds);
    if (opened.kind !== "pointcloud") return;
    const layer = attachPointCloud(viewer, { id: "scene-points", input: opened.input, scope: "scene" });
    setSceneCloud(layer);
    return () => { layer.remove(); setSceneCloud(null); };
  }, [viewer, opened]);

  // 別に選んだ点群を、表示中のシーンの地図の座標に重ねる。単位の違うシーンでは表示部品が外すので、シーンごとに置き直す。
  useLayoutEffect(() => {
    if (!viewer || cloudOpened.status !== "ready" || opened.status !== "ready") return;
    if (cloudOpened.kind !== "pointcloud") { setCloud({ getReport: () => ({ status: "unavailable", reason: "not-pointcloud" }) }); return () => setCloud(null); }
    const layer = attachPointCloud(viewer, { id: "cloud", input: cloudOpened.input, scope: "scene" });
    setCloud(layer);
    return () => { layer.remove(); setCloud(null); };
  }, [viewer, cloudOpened, opened]);

  // 地面の図面は見えない Canvas の View2D で描き、その Canvas を 3D の地面に貼る。
  useLayoutEffect(() => {
    if (!groundViewer) return;
    groundViewer.setScene(drawing.status === "ready" ? drawing.input : null);
    if (drawing.status === "ready") groundViewer.fit();
  }, [groundViewer, drawing]);
  // 3D のシーンの範囲の最小の角を 2D の起点に合わせる。シーンを替えると置き直す。
  useLayoutEffect(() => {
    if (!viewer || !groundViewer || drawing.status !== "ready" || opened.status !== "ready") return;
    let layer;
    try {
      layer = attachGround2D(viewer, {
        id: "ground", view2d: groundViewer, canvas: groundCanvas, scope: "scene",
        plane: { origin: bounds ? bounds.min : [0, 0, 0], xAxis: [1, 0, 0], yAxis: [0, 1, 0] },
      });
    } catch (error) {
      setGround({ getReport: () => ({ status: "unavailable", reason: error.message }) });
      return () => setGround(null);
    }
    setGround(layer);
    return () => { layer.remove(); setGround(null); };
  }, [viewer, groundViewer, groundCanvas, drawing, opened, bounds]);

  // 利用側の追加描画の例：範囲の X の幅の寸法。シーンを替えると表示部品が外す。
  useLayoutEffect(() => {
    if (!viewer || !dimension || opened.status !== "ready" || !opened.bounds) return;
    const { min, max } = opened.bounds, width = max[0] - min[0];
    if (!(width > 0)) return;
    const { name, prefix, scale } = opened.input.result.unit;
    const registration = viewer.addGeometry([{
      kind: "dimension", from: [0, 0, 0], to: [width, 0, 0], offset: [0, -width * 0.1, 0], color: "#c74700",
      label: label(round(width) + " " + unitLabel(opened.input.result.unit)),
    }], { id: "dimension", origin: min, unit: { name, ...(prefix && { prefix }), ...(scale && { scale }) }, scope: "scene" });
    return () => registration.remove();
  }, [viewer, dimension, opened]);

  function say(text) { setMessages((list) => [...list.slice(-9), text]); }
  async function saveBcf(version) {
    const target = opened;
    try {
      const exported = viewer.toReviewPlace("main", { frame: "contain" });
      const { bytes } = await target.writeBcf({
        title: "プレイグラウンドの視点", place: exported.place, author: "zumen-playground",
        date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"), version,
      });
      if (current.current !== target) return;
      download(bytes, baseName(file) + "-" + version + ".bcf");
      say(`BCF ${version} を保存しました（厳密な往復: ${exported.strict ? "はい" : "いいえ"}・調整 ${exported.adjustments.length} 件・報告 ${exported.issues.length} 件）。`);
    } catch (error) {
      if (current.current === target) say("BCF を保存できませんでした: " + error.message);
    }
  }
  async function readBcf(input) {
    const target = opened;
    if (!input || target.status !== "ready") return;
    try {
      const { locations } = await target.readBcf(await input.arrayBuffer());
      if (current.current !== target) return;
      if (!locations.length) { say("BCF に 3D の場所がありません。"); return; }
      const { id: _id, target: _target, ...place } = locations[0].location;
      const applied = viewer.applyReviewPlace("main", target.input.document, place);
      say(`BCF の「${locations[0].topic}」の視点を当てました（厳密: ${applied.strict ? "はい" : "いいえ"}・調整 ${applied.adjustments.length} 件・報告 ${applied.issues.length} 件）。`);
    } catch (error) {
      if (current.current === target) say("BCF を読めませんでした: " + error.message);
    }
  }
  function onKeyDown(event) {
    if (!viewer || event.altKey || event.ctrlKey || event.metaKey) return;
    const actions = {
      Home: () => frame(viewer, bounds), "+": () => zoom(viewer, bounds, 0.8), "=": () => zoom(viewer, bounds, 0.8), "-": () => zoom(viewer, bounds, 1.25),
      ArrowLeft: () => pan(viewer, bounds, 1, 0), ArrowRight: () => pan(viewer, bounds, -1, 0), ArrowUp: () => pan(viewer, bounds, 0, -1), ArrowDown: () => pan(viewer, bounds, 0, 1),
    };
    if (!actions[event.key]) return;
    event.preventDefault();
    actions[event.key]();
  }
  function close() {
    setFile(null); setCloudFile(null); setGroundFile(null); setGeneration((n) => n + 1);
  }

  const view = state?.views[0];
  const selected = view?.selected[0] ?? null;
  const notices = [];
  const issues = new Map();
  for (const issue of sceneReport?.issues ?? []) issues.set(issue.reason, (issues.get(issue.reason) ?? 0) + (issue.count ?? 1));
  for (const [reason, count] of issues) notices.push(`表示: ${reasons[reason] ?? reason}（${count} 件）`);
  const sceneCloudReport = sceneCloud?.getReport();
  if (sceneCloudReport) notices.push(sceneCloudReport.status === "ready" ? `点群: ${sceneCloudReport.drawn.points} 点を描きました` : `点群: ${reasons[sceneCloudReport.reason] ?? sceneCloudReport.reason}`);
  const cloudReport = cloud?.getReport();
  if (cloudReport) notices.push(cloudReport.status === "ready" ? `重ねた点群: ${cloudReport.drawn.points} 点（高さの基準 ${cloudReport.height === "same" ? "一致" : "未確認"}）` : `重ねた点群: ${reasons[cloudReport.reason] ?? cloudReport.reason}`);
  if (cloudOpened.status === "error") notices.push("重ねた点群: 開けませんでした（" + cloudOpened.error.message + "）");
  const groundReport = ground?.getReport();
  if (groundReport) notices.push(groundReport.status === "ready" ? `地面の図面: 貼りました（未描画・代替 ${groundReport.issues.length} 件）` : `地面の図面: ${reasons[groundReport.reason] ?? groundReport.reason}`);
  if (drawing.status === "error") notices.push("地面の図面: 開けませんでした（" + drawing.error.message + "）");
  let status = "3D のファイル（IFC・CityGML・LAS など）を選んでください。";
  if (viewerError) status = "表示できません: " + (viewerError.reason === "webgpu-unavailable" ? "このブラウザーでは WebGPU を使えません" : viewerError.message);
  else if (opened.status === "loading" || (opened.status === "ready" && (!viewer || !sceneReport))) status = "読み込んでいます…";
  else if (opened.status === "error") status = "表示できません: " + (opened.error.format ? opened.error.format + " は 3D に対応していません" : opened.error.message);
  else if (opened.status === "ready") {
    const { counts } = opened.input.result;
    status = `表示しました。 ${opened.input.document.format.toUpperCase()} ／ ${kinds[opened.kind]} ／ 要素 ${opened.total} 件` + (counts.points ? ` ／ 点 ${counts.points} 点` : "");
  }

  return <>
    <a className="skip" href="#main">本文へ移動</a>
    <div className="page-header"><div className="wrap">
      <h1>3D を試す</h1>
      <p className="page-lead">IFC・CityGML・点群をこの端末の中で開きます。ファイルは外へ送りません。BCF の保存はボタンを押したときだけ行います。</p>
    </div></div>
    <main id="main" className="wrap">
      <div className="file-bar">
        <label htmlFor="model">3D ファイル</label>
        <input id="model" key={"m" + generation} type="file" onChange={(event) => setFile(event.target.files?.[0] ?? null)} />
        <label htmlFor="cloud">重ねる点群</label>
        <input id="cloud" key={"c" + generation} type="file" onChange={(event) => setCloudFile(event.target.files?.[0] ?? null)} />
        <label htmlFor="ground">地面の図面</label>
        <input id="ground" key={"g" + generation} type="file" onChange={(event) => setGroundFile(event.target.files?.[0] ?? null)} />
        <button type="button" onClick={close}>閉じる</button>
      </div>
      <div className="workspace">
        <div className="drawing-pane">
          <div className="toolbar" role="toolbar" aria-label="表示の操作">
            <button type="button" disabled={!viewer} onClick={() => frame(viewer, bounds)}>全体を表示</button>
            <button type="button" disabled={!viewer} onClick={() => zoom(viewer, bounds, 0.8)}>拡大</button>
            <button type="button" disabled={!viewer} onClick={() => zoom(viewer, bounds, 1.25)}>縮小</button>
            <label><input type="checkbox" checked={dimension} onChange={(event) => setDimension(event.target.checked)} /> 幅の寸法を表示</label>
          </div>
          <div className="paper">
            <canvas ref={setCanvas} className="view3d-canvas" width={WIDTH} height={HEIGHT} tabIndex={0} onKeyDown={onKeyDown}
              aria-label="3D の表示" aria-describedby="keys" />
            <p className="status" role="status">{status}</p>
          </div>
          <p id="keys" className="quiet">表示を選んで、Home で全体、+ と − で拡大・縮小、矢印キーで移動します。マウスは中ボタンで移動、Shift と中ボタンで回転、Ctrl とホイールで拡大します。</p>
          {view && <p className="camera" aria-label="視点">視点 {view.camera.position.map(round).join(", ")}</p>}
          <section aria-labelledby="notices-title">
            <h2 id="notices-title">表示の通知</h2>
            <ul className="notices" aria-live="polite">{[...notices, ...messages].map((text, i) => <li key={i}>{text}</li>)}</ul>
          </section>
          <section aria-labelledby="bcf-title" className="export">
            <h2 id="bcf-title">BCF</h2>
            <p>IFC を開いているとき、いまの視点と選択を BCF に保存し、BCF の視点を当てられます。</p>
            <button type="button" disabled={!viewer || opened.status !== "ready" || !opened.writeBcf} onClick={() => saveBcf("3.0")}>BCF 3.0 で保存</button>
            <button type="button" disabled={!viewer || opened.status !== "ready" || !opened.writeBcf} onClick={() => saveBcf("2.1")}>BCF 2.1 で保存</button>
            <label htmlFor="bcf">BCF を読む</label>
            <input id="bcf" key={"b" + generation} type="file" disabled={opened.status !== "ready" || !opened.readBcf}
              onChange={(event) => { void readBcf(event.target.files?.[0]); event.target.value = ""; }} />
          </section>
          <details>
            <summary>表示の資源</summary>
            {stats && <dl className="properties" aria-label="表示の資源">
              <dt>geometry</dt><dd>{stats.geometries}</dd><dt>texture</dt><dd>{stats.textures}</dd>
              <dt>描画の呼び出し</dt><dd>{stats.calls}</dd><dt>GPU の見積もり</dt><dd>{stats.gpuBytes} バイト</dd>
            </dl>}
          </details>
        </div>
        <aside className="drawing-pane">
          <section aria-labelledby="elements-title">
            <h2 id="elements-title">要素</h2>
            {opened.status === "ready" && <>
              <ul className="rows elements">{opened.elements.map((item) => <li key={item.id}>
                <button type="button" aria-pressed={item.id === selected}
                  onClick={() => viewer?.select(opened.input.document, item.id === selected ? [] : [item.id])}>
                  {item.id}<span>{item.class}{item.name && " ・ " + item.name}</span>
                </button>
              </li>)}</ul>
              {opened.total > opened.elements.length && <p className="quiet">先頭 {opened.elements.length} 件を表示しています（全 {opened.total} 件）。</p>}
            </>}
          </section>
          <Inspector viewer={viewer} opened={opened} selected={selected} />
        </aside>
      </div>
      <Diagnostics entries={opened.status === "ready" || opened.status === "error" ? opened.diagnostics : []} />
      <canvas ref={setGroundCanvas} className="ground-source" width={512} height={512} aria-hidden="true" />
    </main>
  </>;
}
