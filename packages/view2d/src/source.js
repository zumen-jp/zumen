/** 選択した版を確かめ、定義の空間も含めて L1 の出所へたどる（決定 0267）。 */
export function findSource(session, document, element, maxPages = 1000) {
  if (!Number.isSafeInteger(maxPages) || maxPages <= 0) throw new RangeError("ページの上限");
  const drawing = session.call({ v: 1, op: "drawing", doc: document.doc }).result;
  if (!drawing || drawing.hash !== document.hash) throw new Error("表示した文書と版が異なる");
  // 空間・用紙と要素は、それぞれ重複を /1・/2 で区別する（決定 0117・0156）。
  // 出所の h・i・r・b・e を境に分け、要素側の重複を place に含めない。
  const match = /^((?:space|sheet)\/(?:[0-9]+|h[0-9A-F]+)(?:\/[0-9]+)?)\/(?:h[0-9A-F]+|[irbe][0-9]+)(?:\/[0-9]+)?$/.exec(element.id);
  if (!match) throw new RangeError("要素 ID");
  const place = match[1];
  let offset = 0;
  for (let page = 0; page < maxPages; page++) {
    const result = session.call({ v: 1, op: "entities", doc: document.doc, place, offset, limit: 256, fields: ["source"] }).result;
    if (!result) throw new Error("要素の一覧を読めない");
    const entity = result.items.find((item) => item.id === element.id);
    if (entity) return { id: entity.id, path: element.path, source: entity.source ?? null };
    if (result.next_offset === undefined) return null;
    if (!Number.isSafeInteger(result.next_offset) || result.next_offset <= offset) throw new Error("要素の続きが進まない");
    offset = result.next_offset;
  }
  throw new RangeError("ページの上限");
}
