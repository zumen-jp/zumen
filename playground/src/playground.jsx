import React, { useLayoutEffect, useRef, useState } from "react";
import { useDrawing, useDrawingViewer, useViewerState } from "@zumen-jp/react";
import { VIEW_SCALE_LIMITS } from "@zumen-jp/view2d";
import { samples } from "virtual:samples";
import { catalog, loadModule } from "./module.js";
import { Diagnostics, Inspector, Pages } from "./inspector.jsx";

// 線は案内のサイトの文字の色（DADS の solid-gray-800）、選択はサイトの主の色の青と見分けられる橙（orange-800）にする。
const viewOptions = Object.freeze({ foreground: "#333333", selectionColor: "#c74700" });
// 案内のサイト（site/src/data/site.ts の nav）と同じ並び。ずれは site/scripts/check-output.mjs で確かめる。
const siteNav = [
  { href: "/viewer/", label: "図面を試す" },
  { href: "/features/", label: "特長" },
  { href: "/formats/", label: "対応形式" },
  { href: "/platforms/", label: "対応環境" },
  { href: "/news/", label: "お知らせ" },
  { href: "/faq/", label: "よくある質問" },
];
// サイトの favicon.svg と同じ形。CSP の img-src に頼らず、ひとつのファイルで動くよう SVG を直に置く。
function LogoMark() {
  return <svg className="logo-mark" viewBox="0 0 64 64" width="40" height="40" aria-hidden="true" focusable="false">
    <rect width="64" height="64" rx="14" fill="#0017c1"/>
    <path d="M18 17H46V24L28.5 40H46V47H18V40L35.5 24H18Z" fill="#fff"/>
    <g fill="#fff"><rect x="31" y="3" width="2" height="7"/><rect x="31" y="54" width="2" height="7"/><rect x="3" y="31" width="7" height="2"/><rect x="54" y="31" width="7" height="2"/></g>
  </svg>;
}
function Masthead() {
  return <header className="masthead">
    <div className="wrap">
      <a className="logo" href="/" aria-label="zumen トップ"><LogoMark/>
        <span className="logo-text"><strong>zumen</strong><span>図面の読み書きの基盤</span></span></a>
      <nav className="gnav" aria-label="サイト内"><ul>
        {siteNav.map((item) => <li key={item.href}><a href={item.href} aria-current={item.href === "/viewer/" ? "page" : undefined}>{item.label}</a></li>)}
      </ul></nav>
    </div>
  </header>;
}
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
const fidelities = { native: "保持", equivalent: "等価", approximated: "近似", drawn: "見た目だけ", dropped: "欠落" };
const reasons = {
  text_height_meaning: "文字高の意味を変更", relocated_shape: "形の値を変更", style_default: "既定のスタイルに変更",
  detached_attribute: "属性を文字に分離", baked_view: "ビューを要素に変換", reference_dropped: "参照を欠落",
  template_definition: "出力先の既定の定義を使用", template_sheet: "用紙の設定を変更", format_extension: "形式固有の情報を欠落",
  semantic_fields: "表題欄の項目を欠落", document_default: "文書の既定値を欠落", unmapped_header: "移せないヘッダーの値を欠落",
  sheet: "対応しない用紙の要素を欠落",
  spline_fit_tolerance: "スプラインのフィット許容差を変更", spline_knot_parameterization: "スプラインのノットの取り方を変更",
  line_type_default: "既定の線種に変更", color_code: "色の符号を変更", layer_state: "レイヤの表示状態を変更",
};
const kinds = {
  line: "線", polyline: "折れ線", curve: "曲線", arc: "円弧", circle: "円", ellipse: "楕円", spline: "スプライン", point: "点",
  text: "文字", formatted: "書式付き文字", formatted_text: "書式付き文字", dimension: "寸法", leader: "引出線", callout: "注記",
  symbol: "記号", external_symbol: "外部参照の記号", hatch: "ハッチング", table: "表", underlay: "下絵", raster: "下絵", image: "画像",
  sheet: "用紙", view: "ビュー", layer: "レイヤ", line_type: "線種", text_style: "文字スタイル", dimension_style: "寸法スタイル",
  attribute: "属性", attribute_definition: "属性定義", symbol_definition: "記号の定義", duplicate_symbol_definition: "重複する記号の定義",
  external_symbol_definition: "外部参照の定義",
  extension: "形式固有の情報", title_block: "表題欄", layer_description: "レイヤの説明", layer_transparency: "レイヤの透過性",
  line_type_scale: "文書の線種尺度", default_line_weight: "既定の線幅", default_color: "既定の色", default_line_type: "既定の線種",
  point_style: "点の表し方", header: "ヘッダー", record: "レコード",
};
function InputDiagnostics({ input, described }) {
  if (!input) return null;
  const counts = described?.counts ?? input.counts.map((diagnostic) => ({ stage: "input", diagnostic, text: diagnostic.code }));
  const details = described?.details ?? input.details.map(({ diagnostic, sources }) => ({ stage: "input", diagnostic, sources, text: diagnostic.code }));
  return <section aria-labelledby="input-diagnostics-title">
    <h4 id="input-diagnostics-title">読み取り段階の診断</h4>
    <p>読み取りと図面モデルへの変換で、診断を {input.total} 件報告しました。件数は診断の発生回数です。1 つの要素に複数の診断が出る場合があります。書き出しの対象と忠実度は、上の集計で確認できます。</p>
    <details><summary>読み取り段階の全件集計</summary>
      <Diagnostics entries={counts} issues={[]} title="全件集計" id="input-diagnostics-counts-title" heading="h5" countUnit=" 行" showParams/>
    </details>
    <details><summary>読み取り段階の場所付き明細</summary>
      <Diagnostics entries={details} issues={[]} title="場所付き明細" id="input-diagnostics-details-title" heading="h5" countUnit=" 行" showParams/>
      {input.truncated && <p>読み取り段階の明細は先頭 {input.details.length} 件です。全件集計は明細の上限によらず、すべての診断を含みます。</p>}
    </details>
  </section>;
}
function Estimate({ estimate, inputDiagnostics }) {
  return <section aria-labelledby="estimate-title">
    <h3 id="estimate-title">保存前の見積もり</h3>
    <p>対象 {estimate.total} 件から、DXF に要素 {estimate.elements} 件を書きます。属性の文字など、1 件から複数の要素を書く場合があります。</p>
    <dl className="properties">{Object.entries(fidelities).map(([fidelity, title]) => <React.Fragment key={fidelity}>
      <dt>{title}</dt><dd>{estimate.counts.filter((row) => row.fidelity === fidelity).reduce((total, row) => total + row.count, 0)} 件</dd>
    </React.Fragment>)}</dl>
    <details><summary>種類ごとの見積もり</summary>
      <Pages count={estimate.counts.length} label="種類">{(i) => {
        const row = estimate.counts[i];
        return <>{kinds[row.kind] ?? row.kind}・{fidelities[row.fidelity] ?? row.fidelity}: {row.count} 件（出力 {row.output_elements} 件）</>;
      }}</Pages>
    </details>
    <details><summary>元の場所と見積もり</summary>
      <Pages count={estimate.details.length} label="明細">{(i) => {
        const row = estimate.details[i], source = row.source;
        return <>{kinds[row.kind] ?? row.kind}・{fidelities[row.fidelity] ?? row.fidelity}（出力 {row.output_elements} 件）
          <span className="detail">{row.id}{source.record != null && " レコード " + source.record}{source.handle && " ハンドル " + source.handle}
            {source.instance != null && " インスタンス " + source.instance}{source.bytes && " バイト " + source.bytes.start + "〜" + source.bytes.end}</span>
          {row.reasons.length > 0 && <span className="detail">{row.reasons.map((reason) => reasons[reason] ?? reason).join("・")}</span>}</>;
      }}</Pages>
      {estimate.truncated && <p>明細は先頭 {estimate.details.length} 件です。件数の集計はすべての対象を含みます。</p>}
    </details>
    <InputDiagnostics input={estimate.input_diagnostics} described={inputDiagnostics}/>
  </section>;
}
// 開いた図面の見積もりを確認してから DXF を保存する。図面は外へ送らない（#645・#690）。
function Export({ opened, name }) {
  const [state, setState] = useState({ status: "idle" });
  const active = useRef(null), pending = useRef(false);
  useLayoutEffect(() => {
    active.current = opened;
    setState({ status: "idle" });
    return () => { active.current = null; };
  }, [opened]);
  const busy = state.status === "estimating" || state.status === "saving";
  async function estimate() {
    if (pending.current) return;
    pending.current = true;
    setState({ status: "estimating" });
    try {
      const preview = await opened.estimate();
      if (active.current === opened) setState({ status: "estimated", preview, diagnostics: preview.diagnostics });
    } catch (error) {
      if (active.current === opened) setState({ status: "error", diagnostics: error.diagnostics ?? [], message: error.message });
    } finally { pending.current = false; }
  }
  async function save() {
    // 変換の間もボタンのフォーカスを保つよう、disabled にせず、続けて押したものを無視する
    if (pending.current || !state.preview) return;
    pending.current = true;
    const preview = state.preview;
    setState({ status: "saving", preview, diagnostics: preview.diagnostics });
    try {
      const { bytes, result, diagnostics } = await opened.convert(preview);
      if (active.current !== opened) return;
      download(bytes, dxfName(name));
      setState({ status: "done", preview, result, diagnostics });
    } catch (error) {
      if (active.current === opened) setState({ status: "error", diagnostics: error.diagnostics ?? [], message: error.message });
    } finally { pending.current = false; }
  }
  return <section aria-labelledby="export-title" className="export">
    <h2 id="export-title">DXF で保存</h2>
    <p>保存前に、保てるものと、近似・欠落するものを確認できます。変換と保存は、このブラウザーの中で行います。</p>
    <button aria-disabled={busy} onClick={estimate}>保存前の見積もりを確認</button>
    {state.preview && <Estimate estimate={state.preview.result.estimate} inputDiagnostics={state.preview.inputDiagnostics}/>}
    {state.status === "estimated" || state.status === "saving" || state.status === "done" ? <button aria-disabled={busy || state.status === "done"} onClick={state.status === "done" ? undefined : save}>DXF で保存</button> : null}
    <div role="status" aria-live="polite" aria-atomic="true">
      {state.status === "estimating" ? "見積もりを作っています。" : state.status === "estimated" ? "見積もりを確認して、DXF で保存してください。" : state.status === "saving" ? "変換しています。" : state.status === "done" ? dxfName(name) + " のダウンロードを始めました（要素 " + state.result.elements + " 件・変換の診断 " + state.diagnostics.length + " 件）。"
        : state.status === "error" ? "変換できませんでした。" : ""}
    </div>
    {state.status === "error" && !state.diagnostics.some(({ diagnostic }) => diagnostic.severity === "error") && <details><summary>エラーの詳細</summary><p>{state.message}</p></details>}
    {state.diagnostics && <Diagnostics entries={state.diagnostics} issues={[]} title={state.status === "done" ? "変換の診断" : "見積もりの診断"} id="export-diagnostics-title"/>}
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
    let frame = null, stopped = false;
    // Resize Observer §3.4.6: https://drafts.csswg.org/resize-observer/#deliver-resize-loop-error
    // 状態通知で詳細パネルの有無も変わる。配送中には書かず、次のフレームで最新の CSS 寸法を読む。
    const observer = new ResizeObserver(() => {
      if (stopped || frame !== null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        if (!stopped) resize(viewer, canvas);
      });
    });
    observer.observe(canvas);
    resize(viewer, canvas);
    return () => {
      stopped = true; observer.disconnect();
      if (frame !== null) cancelAnimationFrame(frame);
      detach();
    };
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
    : ready ? viewer.getElements().length ? "表示しました。" : "表示できる要素がありません。診断を確認してください。" : file || sample ? "図面を読んでいます。" : "図面を選んでください。";
  return <>
  <a className="skip" href="#viewer-main">本文へ移動</a>
  <Masthead/>
  <main id="viewer-main">
    <div className="page-header"><div className="wrap">
      <nav className="breadcrumbs" aria-label="現在の位置"><ol><li><a href="/">トップ</a></li><li aria-current="page">図面ビューアー</li></ol></nav>
      <h1>図面ビューアー</h1>
      <p className="page-lead">選んだ図面をこのブラウザーで開きます。図面のバイト列を外へ送らず、読み取りはこのブラウザーの中で行います。PDF・JWC・JWS・LAZ は対象外です。
        {import.meta.env.VITE_ZUMEN_CORE_VERSION && <>zumen {import.meta.env.VITE_ZUMEN_CORE_VERSION} で読みます。未対応の文字や図形は、診断をご確認ください。</>}</p>
    </div></div>
    <div className="wrap">
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
    {ready && opened.canEstimate && <Export key={opened.input.document.hash} opened={opened} name={file?.name}/>}
    <section aria-labelledby="report-title" className="export">
      <h2 id="report-title">不具合を知らせる</h2>
      <p>操作や表示の不具合は <a href="https://github.com/zumen-jp/zumen/issues">GitHub の issue</a> にお知らせください。受け取った図面・顧客の名前・図面の中身は、公開の issue に貼らないでください。</p>
      <p>読み取りが終わらない・落ちる・メモリを使いすぎる入力は、<a href="https://github.com/zumen-jp/zumen/security/advisories/new">非公開の報告窓口</a>へお知らせください。機密の図面は添付せず、<a href="https://github.com/zumen-jp/zumen/blob/main/SECURITY.md">報告の手順</a>をご確認ください。</p>
    </section>
    </div>
  </main>
  <footer className="footer">
    <p><a href="/">zumen の案内に戻る</a></p>
    <p>zumen は MIT License または Apache License 2.0 で提供します。</p>
  </footer>
  </>;
}
