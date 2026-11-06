import React, { useLayoutEffect, useRef, useState } from "react";
import { useDrawing, useDrawingViewer, useViewerState } from "@zumen-jp/react";
import { VIEW_SCALE_LIMITS } from "@zumen-jp/view2d";
import { catalog, loadModule } from "./module.js";
import { Diagnostics, Inspector } from "./inspector.jsx";

const viewOptions = Object.freeze({ foreground: "#172d36", selectionColor: "#b35413" });
function resize(viewer, canvas) {
  viewer.resize(Math.max(1, canvas.clientWidth), Math.max(1, canvas.clientHeight), Math.min(devicePixelRatio || 1, 2));
}
function zoom(viewer, factor) {
  const camera = viewer.getView();
  const scale = Math.max(VIEW_SCALE_LIMITS.min, Math.min(VIEW_SCALE_LIMITS.max, camera.scale * factor));
  viewer.jumpTo({ ...camera, scale });
}
export function Playground() {
  const [canvas, setCanvas] = useState(null);
  const [file, setFile] = useState(null);
  const [place, setPlace] = useState(null);
  const picker = useRef(null);
  const viewer = useDrawingViewer(canvas, viewOptions);
  const state = useViewerState(viewer);
  const opened = useDrawing(file, { from: place?.id, loadModule, catalog });
  const [renderError, setRenderError] = useState(null);
  const input = opened.status === "ready" ? opened.input : null;
  const ready = Boolean(input && state?.scene?.scene === input.scene && !renderError);
  useLayoutEffect(() => {
    if (!viewer || !canvas) return;
    const detach = viewer.attachInput();
    const observer = new ResizeObserver(() => resize(viewer, canvas));
    observer.observe(canvas);
    resize(viewer, canvas);
    return () => { observer.disconnect(); detach(); };
  }, [viewer, canvas]);
  useLayoutEffect(() => {
    if (!viewer) return;
    setRenderError(null);
    try { viewer.setScene(input); }
    catch (error) { viewer.setScene(null); setRenderError(error); }
  }, [viewer, input]);
  useLayoutEffect(() => {
    // 詳細パネルが並んだあとの寸法で初回だけ全体を収める。その後の移動とズームは利用者が決める。
    if (!ready) return;
    try { resize(viewer, canvas); viewer.fit(); }
    catch (error) { viewer.setScene(null); setRenderError(error); }
  }, [viewer, canvas, input, ready]);

  function choose(next) { setPlace(null); setFile(next); }
  // 同じ図面の用紙を読み直す間も候補とフォーカスを保つ。別のファイルを選べば候補を捨てる。
  const drawing = opened.status === "ready" ? opened.drawing : place?.drawing;
  const failed = opened.status === "error" || renderError;
  const status = failed ? opened.error?.message === "drawing.input.limit" ? "64 MiB 以下のファイルを選んでください。" : "図面を表示できませんでした。"
    : ready ? "表示しました。" : file ? "図面を読んでいます。" : "図面を選んでください。";
  return <main>
    <header><div className="heading"><span className="wordmark">zumen</span><h1>図面ビューア</h1></div>
      <p>選んだ図面をこのブラウザーで開きます。</p></header>
    <div className="file-bar">
      <label htmlFor="drawing-file">図面ファイル</label>
      <input ref={picker} id="drawing-file" type="file" accept=".dxf,.jww,.sfc,.p21,.sfz,.p2z" aria-describedby="file-help"
        onChange={(e) => choose(e.target.files?.[0] ?? null)}/>
      <span id="file-help">DXF・JWW・SXF ／ 64 MiB まで</span>
      {file && <button onClick={() => { choose(null); picker.current.value = ""; picker.current.focus(); }}>閉じる</button>}
    </div>
    <div className="workspace">
      <div className="drawing-pane">
        <div className="toolbar" role="group" aria-label="表示の操作">
          <label>用紙・空間 <select disabled={!drawing} value={place?.id ?? input?.result.from ?? ""} onChange={(e) => setPlace({ id: e.target.value, drawing })}>
            {!drawing ? <option value="">未選択</option> : <>
              {drawing.sheets.map((sheet) => <option key={sheet.id} value={sheet.id}>用紙 {sheet.name ?? sheet.id}</option>)}
              {drawing.spaces.map((space) => <option key={space.id} value={space.id}>空間 {space.id}</option>)}
            </>}
          </select></label>
          <button disabled={!ready} onClick={() => viewer.fit()}>全体を表示</button>
          <button disabled={!ready || state.view.scale >= VIEW_SCALE_LIMITS.max} onClick={() => zoom(viewer, 1.5)}>拡大</button>
          <button disabled={!ready || state.view.scale <= VIEW_SCALE_LIMITS.min} onClick={() => zoom(viewer, 1 / 1.5)}>縮小</button>
        </div>
        <div className={ready ? "paper" : "paper empty"}><canvas ref={setCanvas} tabIndex="0" aria-label="図面の表示" aria-describedby="drawing-help"
          onPointerDown={(e) => e.currentTarget.focus({ preventScroll: true })}/>
          <div className="status" role="status" aria-live="polite" aria-atomic="true">
            {status}{ready && " " + opened.input.document.format.toUpperCase() + " ／ " + state.scene.result.from + " ／ 診断 " + (opened.diagnostics.length + viewer.getReport().issues.length) + " 件"}
          </div>
        </div>
        <p id="drawing-help">図形をクリックするか、要素の一覧から選んで出所を確認します。図面にフォーカスすると矢印キーで移動できます。</p>
      </div>
      {ready && <Inspector key={opened.input.document.hash + "/" + state.scene.result.from} viewer={viewer} state={state} opened={opened}/>}
    </div>
    {failed && !opened.diagnostics?.length && <details><summary>エラーの詳細</summary><p>{(renderError ?? opened.error)?.message}</p></details>}
    {(ready || failed) && <Diagnostics entries={opened.diagnostics ?? []} issues={ready ? viewer.getReport().issues : []}/>}
  </main>;
}
