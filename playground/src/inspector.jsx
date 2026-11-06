import React, { useEffect, useState } from "react";

const pageSize = 25;
const pathLabel = (path) => path.map(({ id, column, row }) => id + " [" + column + ", " + row + "]").join(" → ");

// 大きい図面でも、一覧の DOM は現在のページだけ作る。
function Pages({ count, label, className = "", children }) {
  const [page, setPage] = useState(0);
  const last = Math.max(0, Math.ceil(count / pageSize) - 1);
  const current = Math.min(page, last), start = current * pageSize;
  return <>
    <ul className={"rows " + className}>{Array.from({ length: Math.min(pageSize, count - start) }, (_, i) =>
      <li key={start + i}>{children(start + i)}</li>)}</ul>
    {last > 0 && <div className="pager">
      <button disabled={!current} onClick={() => setPage(current - 1)}>前の{label}</button>
      <span>{current + 1} / {last + 1}</span>
      <button disabled={current === last} onClick={() => setPage(current + 1)}>次の{label}</button>
    </div>}
  </>;
}

function Source({ opened, element }) {
  const [reply, setReply] = useState(null);
  useEffect(() => {
    if (!element || opened.status !== "ready") return;
    let current = true;
    opened.getSource(element).then(
      (value) => { if (current) setReply({ opened, element, value }); },
      () => { if (current) setReply({ opened, element, failed: true }); },
    );
    return () => { current = false; };
  }, [opened, element]);
  const answer = reply?.opened === opened && reply.element === element ? reply : null;
  const source = answer?.value?.source;
  return <section aria-labelledby="source-title">
    <h2 id="source-title">出所</h2>
    <div role="status" aria-live="polite" aria-atomic="true">
      {!element ? <p>要素を選ぶと出所を表示します。</p> : <>
        <div className="element-id">{element.id}</div>
        {element.path.length > 0 && <p>配置 {pathLabel(element.path)}</p>}
        {!answer ? <p>出所を調べています。</p> : answer.failed ? <p>出所を取得できませんでした。</p> : !source || !Object.keys(source).length ? <p>出所の記録はありません。</p> :
          <dl className="properties">
            {source.record != null && <><dt>レコード</dt><dd>{source.record}</dd></>}
            {source.bytes && <><dt>バイト範囲（終端を含まない）</dt><dd>{source.bytes.start} 〜 {source.bytes.end}</dd></>}
            {source.handle && <><dt>ハンドル</dt><dd>{source.handle}</dd></>}
            {source.instance && <><dt>インスタンス</dt><dd>#{source.instance}</dd></>}
          </dl>}
      </>}
    </div>
  </section>;
}

export function Inspector({ viewer, state, opened }) {
  const elements = viewer.getElements();
  const selection = state.selection;
  return <aside aria-label="図面の詳細">
    <section aria-labelledby="layers-title">
      <h2 id="layers-title">レイヤ <small>{state.layers.length}</small></h2>
      <Pages count={state.layers.length} label="レイヤ">{(i) => {
        const layer = state.layers[i];
        return <label className="layer"><input type="checkbox" checked={!state.hiddenLayers.includes(layer)}
          onChange={(e) => viewer.setLayersVisible([layer], e.target.checked)}/>{layer}</label>;
      }}</Pages>
      {!state.layers.length && <p>表示するレイヤはありません。</p>}
    </section>
    <section aria-labelledby="elements-title">
      <h2 id="elements-title">要素 <small>{elements.length}</small></h2>
      <Pages count={elements.length} label="要素" className="elements">{(i) => {
        const ref = elements[i];
        return <button type="button" aria-pressed={selection.includes(ref)} onClick={() => viewer.select([ref])}>
          {ref.id}{ref.path.length > 0 && <span>{pathLabel(ref.path)}</span>}
        </button>;
      }}</Pages>
      {selection.length > 0 && <button className="quiet" onClick={() => viewer.select([])}>選択を解除</button>}
    </section>
    <Source opened={opened} element={selection[0]}/>
  </aside>;
}

const stages = { open: "読み込み", drawing: "図面モデル", scene: "表示" };
const severity = { error: "エラー", warning: "警告", info: "情報" };
export function Diagnostics({ entries, issues }) {
  const total = entries.length + issues.length;
  return <section aria-labelledby="diagnostics-title" className="diagnostics">
    <h2 id="diagnostics-title">診断 <small>{total}</small></h2>
    {!total && <p>診断はありません。</p>}
    <Pages count={total} label="診断">{(i) => {
      if (i >= entries.length) {
        const issue = issues[i - entries.length];
        return <><strong>{issue.disposition === "fallback" ? "代替表示" : "未描画"}</strong> {issue.element.id} <span className="detail">{issue.reason}</span></>;
      }
      const { stage, diagnostic, text } = entries[i];
      return <>
      <strong>{severity[diagnostic.severity] ?? diagnostic.severity}・{stages[stage]}</strong> {text}
      <div className="detail">{diagnostic.code} · {diagnostic.count} 件
        {diagnostic.at.map((at, j) => <span key={j}> {at.id} {at.record != null && "レコード " + at.record}{at.bytes && " バイト " + at.bytes.start + "〜" + at.bytes.end}</span>)}</div>
      </>;
    }}</Pages>
  </section>;
}
