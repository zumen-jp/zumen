import React, { useLayoutEffect, useRef, useState } from "react";
import { useDrawing, useDrawingViewer, useViewerState } from "@zumen-jp/react";
import { VIEW_SCALE_LIMITS } from "@zumen-jp/view2d";
import { samples } from "virtual:samples";
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
// 保存する名前。DXF を開いたときは、元のファイルと分かるよう名前を変える
function dxfName(name) {
  const base = (name || "drawing").replace(/\.[^.]*$/, "") || "drawing";
  return /\.dxf$/i.test(name ?? "") ? base + "-zumen.dxf" : base + ".dxf";
}
function download(bytes, name) {
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/dxf" }));
  const link = Object.assign(document.createElement("a"), { href: url, download: name });
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
// 開いた図面を DXF に変換して、ブラウザーのダウンロードで保存する。図面は外へ送らない（決定 0206、#690）。
function Export({ opened, name }) {
  const [state, setState] = useState({ status: "idle" });
  async function save() {
    setState({ status: "busy" });
    try {
      const { bytes, result, diagnostics } = await opened.convert();
      download(bytes, dxfName(name));
      setState({ status: "done", result, diagnostics });
    } catch (error) {
      setState({ status: "error", diagnostics: error.diagnostics ?? [], message: error.message });
    }
  }
  return <section aria-labelledby="export-title" className="export">
    <h2 id="export-title">DXF で保存</h2>
    <p>開いた図面を DXF に変換して、このブラウザーのダウンロードで保存します。書けない値は、変換の診断に種類・件数・場所を出します。</p>
    <button disabled={state.status === "busy"} onClick={save}>DXF で保存</button>
    <div role="status" aria-live="polite" aria-atomic="true">
      {state.status === "busy" ? "変換しています。" : state.status === "done" ? dxfName(name) + " を保存しました（要素 " + state.result.elements + " 件・変換の診断 " + state.diagnostics.length + " 件）。"
        : state.status === "error" ? "変換できませんでした。" : ""}
    </div>
    {state.status === "error" && !state.diagnostics.some(({ diagnostic }) => diagnostic.severity === "error") && <details><summary>エラーの詳細</summary><p>{state.message}</p></details>}
    {(state.status === "done" || state.status === "error") && <Diagnostics entries={state.diagnostics} issues={[]} title="変換の診断" id="export-diagnostics-title"/>}
  </section>;
}
export function Playground() {
  const [canvas, setCanvas] = useState(null);
  const [file, setFile] = useState(null);
  const [place, setPlace] = useState(null);
  const [sample, setSample] = useState(null);
  const [fetchError, setFetchError] = useState(null);
  const picker = useRef(null);
  const fetching = useRef(0);
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

  function choose(next, from = null) {
    fetching.current++;
    setPlace(null); setFile(next); setSample(from); setFetchError(null);
  }
  // 公開サンプルは、配布物の中のファイルをローカルの GET で取り、選んだファイルと同じに開く（#689）。
  async function openSample(path) {
    const chosen = samples.find((item) => item.path === path);
    if (!chosen) return;
    picker.current.value = "";
    choose(null, chosen);
    const token = fetching.current;
    try {
      const response = await fetch(chosen.file, { redirect: "error" });
      if (!response.ok) throw new Error("sample.load");
      const bytes = await response.arrayBuffer();
      if (token !== fetching.current) return;
      setFile(new File([bytes], chosen.path.split("/").pop()));
    } catch (error) {
      // 同じサンプルを選び直せるよう、選択は空に戻す
      if (token === fetching.current) { setSample(null); setFetchError(error); }
    }
  }
  // 同じ図面の用紙を読み直す間も候補とフォーカスを保つ。別のファイルを選べば候補を捨てる。
  const drawing = opened.status === "ready" ? opened.drawing : place?.drawing;
  const failed = opened.status === "error" || renderError || fetchError;
  const status = fetchError ? "サンプルを取れませんでした。" : failed ? opened.error?.message === "drawing.input.limit" ? "64 MiB 以下のファイルを選んでください。" : "図面を表示できませんでした。"
    : ready ? "表示しました。" : file || sample ? "図面を読んでいます。" : "図面を選んでください。";
  return <main>
    <header><div className="heading"><span className="wordmark">zumen</span><h1>図面ビューア</h1></div>
      <p>選んだ図面をこのブラウザーで開きます。</p></header>
    <div className="file-bar">
      <label htmlFor="drawing-file">図面ファイル</label>
      <input ref={picker} id="drawing-file" type="file" accept=".dxf,.dwg,.jww,.sfc,.p21,.sfz,.p2z" aria-describedby="file-help"
        onChange={(e) => choose(e.target.files?.[0] ?? null)}/>
      <span id="file-help">DXF・DWG・JWW・SXF ／ 64 MiB まで</span>
      <label htmlFor="drawing-sample">公開サンプル</label>
      <select id="drawing-sample" value={sample?.path ?? ""} onChange={(e) => openSample(e.target.value)}>
        <option value="">選ぶ</option>
        {samples.map((item) => <option key={item.path} value={item.path}>{item.title}</option>)}
      </select>
      {(file || sample) && <button onClick={() => { choose(null); picker.current.value = ""; picker.current.focus(); }}>閉じる</button>}
    </div>
    {sample && <section className="sample-source" aria-label="サンプルの提供元とライセンス">
      <h2>{sample.title}</h2>
      <dl className="properties">
        <dt>公開した者</dt><dd>{sample.publisher}</dd>
        <dt>ライセンス</dt><dd><a href={sample.licenseUrl} target="_blank" rel="noreferrer noopener">{sample.license}</a>{sample.text && <>（<a href={sample.text} target="_blank" rel="noopener">本文</a>）</>}</dd>
        <dt>出所</dt><dd><a href={sample.source} target="_blank" rel="noreferrer noopener">{sample.source}</a></dd>
        <dt>表示</dt><dd>{sample.attribution}</dd>
        {sample.changes && <><dt>変えたこと</dt><dd>{sample.changes}</dd></>}
      </dl>
    </section>}
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
    {failed && !opened.diagnostics?.length && <details><summary>エラーの詳細</summary><p>{(fetchError ?? renderError ?? opened.error)?.message}</p></details>}
    {(ready || failed) && <Diagnostics entries={opened.diagnostics ?? []} issues={ready ? viewer.getReport().issues : []}/>}
    {ready && <Export key={opened.input.document.hash} opened={opened} name={file?.name}/>}
  </main>;
}
