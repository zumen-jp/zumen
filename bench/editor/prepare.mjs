// エディターの設計（#693・決定 0288）のための測りの準備。公開サンプルと適合テストの合成の図面を土台にし、apply の add で
// 要素を足した図面を作り、測る場合の一覧（plan.txt）を書く。使うのはこのリポジトリの wasm と公開の API だけ。
//
//   node prepare.mjs <図面用の wasm> <出力のフォルダー>
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { loadSync } from "../../bindings/js/src/index.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const [wasmPath, out] = process.argv.slice(2);
if (!wasmPath || !out) {
  throw new Error("使い方: node prepare.mjs <図面用の wasm> <出力のフォルダー>");
}
mkdirSync(out, { recursive: true });
const zumen = loadSync(readFileSync(wasmPath));

// 適合テストの組から、合成の入力を取り出す
function conformanceInput(caseName, inputName) {
  const steps = JSON.parse(readFileSync(join(root, "bindings/conformance/cases", caseName + ".json"), "utf8")).steps;
  const push = steps.find((step) => step.push?.name === inputName)?.push;
  if (!push) {
    throw new Error(`${caseName} に ${inputName} がない`);
  }
  return Buffer.from(push.hex ?? push.text, push.hex ? "hex" : "utf8");
}

const bases = [
  { name: "dwg", ext: "dwg", color: { aci: 1 }, bytes: readFileSync(join(root, "samples/dwg/zumen-layout-r2018.dwg")) },
  { name: "dxf", ext: "dxf", color: { aci: 1 }, bytes: readFileSync(join(root, "samples/dwg/zumen-layout-r2018_ascii.dxf")) },
  { name: "jww", ext: "jww", color: { jww: 1 }, bytes: conformanceInput("apply", "d.jww") },
  { name: "sfc", ext: "sfc", color: { sxf: 1 }, bytes: conformanceInput("apply", "b.sfc") },
  { name: "p21", ext: "p21", color: { sxf: 1 }, bytes: conformanceInput("apply", "c.p21") },
];
// 大きな図面は 10 万件にする。DWG は、足して $HANDSEED のバイト数が増える（0xFFFF を超える）と書き戻せない（#701）
// ので 6 万件、P21 は、apply の時間が図面の大きさより速く増え（#709）、足して作るのにも測るのにも時間がかかるので
// 3 万件にする
const largeOf = { dwg: { label: "60k", count: 60_000 }, p21: { label: "30k", count: 30_000 } };
const sizesOf = (base) => [
  { label: "base", count: 0 },
  { label: "1k", count: 1_000 },
  { label: "10k", count: 10_000 },
  largeOf[base.name] ?? { label: "100k", count: 100_000 },
];
// 1 回の apply で足す数。P21 は、図面が大きくなるほど 1 回の apply が重くなるので、1 回で足す
const chunkOf = (base) => (base.name === "p21" ? 1_000_000 : 5_000);

function opened(bytes, input) {
  const session = zumen.open();
  session.push(input, bytes);
  const doc = session.call({ v: 1, op: "open", input }).result.doc;
  return { session, doc };
}

// 文書の要素をすべて読む。fields は id のほかに返すもの
function entities(session, doc, fields) {
  const items = [];
  let offset = 0;
  for (;;) {
    const result = session.call({ v: 1, op: "entities", doc, fields, offset, limit: 10_000 }).result;
    items.push(...result.items);
    if (result.next_offset === undefined) {
      return { items, total: result.total };
    }
    offset = result.next_offset;
  }
}

// 最初の空間か用紙の ID と、文書のハッシュ
function summaryOf(session, doc) {
  const summary = session.call({ v: 1, op: "drawing", doc }).result;
  const place = summary.spaces?.[0]?.id ?? summary.sheets?.[0]?.id;
  if (!place) {
    throw new Error(`${doc} に空間も用紙もない`);
  }
  return { place, hash: summary.hash };
}

function extentOf(items) {
  let box = null;
  for (const { bbox } of items) {
    if (!bbox) continue;
    const next = [bbox.min_x, bbox.min_y, bbox.max_x, bbox.max_y];
    box = box ? [Math.min(box[0], next[0]), Math.min(box[1], next[1]), Math.max(box[2], next[2]), Math.max(box[3], next[3])] : next;
  }
  return box ?? [0, 0, 1000, 1000];
}

// i 番目に足す要素。線・折れ線・円・円弧・文字を、元の範囲に格子で並べる。JWW は、API の add で線・円・円弧を
// 足せない（#702）ので、点を足す
function addition(i, base, place, layer, lineType, box) {
  const columns = 400;
  const width = Math.max(box[2] - box[0], 1);
  const step = width / columns;
  const x = box[0] + (i % columns) * step;
  const y = box[1] + Math.floor(i / columns) * step;
  const size = step * 0.4;
  const shapes = {
    line: { shape: "line", start: [x, y], end: [x + size, y + size] },
    point: { shape: "point", position: [x, y], rotation: 0 },
    polyline: { shape: "polyline", points: [[x, y], [x + size, y], [x + size, y + size]], closed: false },
    circle: { shape: "circle", center: [x, y], radius: size / 2 },
    arc: { shape: "arc", center: [x, y], radius: size / 2, start_angle: 0, end_angle: 90 },
    text: { shape: "text", content: `T${i}`, position: [x, y], height: size / 2, rotation: 0 },
  };
  // SXF の add は文字を持たず、線種と線幅をレイヤに従わせられないので、明示する
  const sxf = base.name === "sfc" || base.name === "p21";
  const names = base.name === "jww" ? ["point"] : sxf ? ["line", "polyline", "circle", "arc"] : ["line", "polyline", "circle", "arc", "text"];
  const edit = { kind: "add", place, color: base.color, geometry: shapes[names[i % names.length]] };
  if (layer !== undefined) edit.layer = layer;
  if (sxf) {
    edit.line_type = { name: lineType };
    edit.line_weight = { mm: 0.25 };
  }
  return edit;
}

function grow(base, count) {
  if (count === 0) {
    return base.bytes;
  }
  const input = `${base.name}.${base.ext}`;
  const { session, doc } = opened(base.bytes, input);
  try {
    const { place } = summaryOf(session, doc);
    const { items } = entities(session, doc, ["layer", "bbox"]);
    const layer = items.find((item) => item.id.startsWith(place + "/") && item.layer !== undefined)?.layer;
    // DXF・DWG は線種をレイヤに従わせる。SXF は図面にある最初の線種を名前で付ける。JWW の点は線種を持たない
    const lineType = base.name === "dwg" || base.name === "dxf" || base.name === "jww"
      ? undefined
      : session.call({ v: 1, op: "tables", doc }).result.line_types?.[0]?.name;
    const box = extentOf(items);
    let file = null;
    for (let start = 0; start < count; start += chunkOf(base)) {
      const edits = [];
      for (let i = start; i < Math.min(count, start + chunkOf(base)); i++) {
        edits.push(addition(i, base, place, layer, lineType, box));
      }
      let response;
      try {
        response = session.call({ v: 1, op: "apply", doc, edits });
      } catch (error) {
        const text = error.bytes ? new TextDecoder().decode(error.bytes) : String(error);
        throw new Error(`${base.name} の ${start} 件目からの apply が失敗: ${text}`);
      }
      file = response.blobs.find((blob) => blob.kind === "file");
      file = session.take(file.id);
    }
    return file;
  } finally {
    session.close();
  }
}

// 1 つの図面について、測る場合を作る
function casesFor(label, file, base) {
  const input = file.split("/").pop();
  const path = join(out, file);
  const bytes = readFileSync(path);
  const { session, doc } = opened(bytes, input);
  try {
    const { place, hash } = summaryOf(session, doc);
    const { items, total } = entities(session, doc, ["kind", "layer", "hash", "bbox"]);
    const own = items.filter((item) => item.id.startsWith(place + "/"));
    // 動かす要素。JWW は足したものが点なので、点も含める
    const movable = own.filter((item) => item.kind === "line" || item.kind === "polyline" || item.kind === "point");
    if (movable.length === 0) {
      throw new Error(`${label} に動かせる線がない`);
    }
    const pick = (n) => movable.slice(0, n);
    const target = movable[movable.length - 1];
    const transform = (list) => list.map(({ id, hash }) => ({ kind: "transform", id, hash, matrix: [1, 0, 5, 0, 1, 7] }));
    const restyle = { kind: "restyle", id: target.id, hash: target.hash, color: Object.fromEntries(Object.entries(base.color).map(([k]) => [k, 3])) };
    const lineType = session.call({ v: 1, op: "tables", doc }).result.line_types?.[0]?.name;
    const addOne = base.name === "jww"
      ? { kind: "add", place, layer: own[0]?.layer, color: base.color, geometry: { shape: "point", position: [0, 0], rotation: 0 } }
      : base.name === "sfc" || base.name === "p21"
        ? { kind: "add", place, layer: own[0]?.layer, color: base.color, line_type: { name: lineType }, line_weight: { mm: 0.25 }, geometry: { shape: "line", start: [0, 0], end: [10, 10] } }
        : { kind: "add", place, color: base.color, geometry: { shape: "line", start: [0, 0], end: [10, 10] } };
    const removeOne = { kind: "remove", id: target.id, hash: target.hash };
    const opener = [`push\t${input}\t${file}`, "call\t" + JSON.stringify({ v: 1, op: "open", input, doc })];
    const timed = (name, request, setup = opener) => ({ name: `${label}/${name}`, setup, request });
    // エディターと同じく、apply には文書のハッシュを付ける（決定 0288）
    const apply = (edits, docHash = hash) => ({ v: 1, op: "apply", doc, hash: docHash, edits });
    const scene = { v: 1, op: "scene", doc, from: place };
    const call = (request) => "call\t" + JSON.stringify(request);
    // エディターが開いたときと同じく、open のあとに scene を流しておく
    const shown = [...opener, call(scene)];
    // 最初の操作（1 件を動かす）。続けての操作は、同じ要素をもう一度動かす。前提条件は、最初の操作の結果から作る
    const first = apply(transform(pick(1)));
    const firstResult = session.call(first).result;
    const moved = firstResult.applied[0];
    if (moved?.id === undefined || moved.hash === undefined) {
      throw new Error(`${label} の最初の操作が、動かした要素の ID とハッシュを返さない`);
    }
    const next = apply([{ kind: "transform", id: moved.id, hash: moved.hash, matrix: [1, 0, 5, 0, 1, 7] }], firstResult.hash);
    // 選んだ要素を読む。選んだ範囲は、最初に動かす要素の範囲にする
    const box = pick(1)[0].bbox;
    const select = {
      v: 1,
      op: "entities",
      doc,
      place,
      region: [box.min_x, box.min_y, box.max_x, box.max_y],
      fields: ["kind", "layer", "hash", "geometry"],
    };
    // 取り消し。最初の操作のあと、文書を閉じ、元のバイト列（版 0）を同じ Worker で開き直して、シーンを作る
    const original = `v0-${input}`;
    const undo = [{ v: 1, op: "close", docs: [doc] }, { v: 1, op: "open", input: original, doc }, scene];
    return {
      size: bytes.length,
      elements: total,
      cases: [
        timed("open", { v: 1, op: "open", input, doc }, [`push\t${input}\t${file}`]),
        timed("drawing", { v: 1, op: "drawing", doc }),
        timed("scene", scene),
        timed("entities_page", { v: 1, op: "entities", doc, fields: ["hash"], limit: 1000 }),
        // entities は 1 回に 10,000 件まで返す。全件は、ページを続けて読む
        timed(
          "entities_all",
          Array.from({ length: Math.max(Math.ceil(total / 10_000), 1) }, (_, page) => (
            { v: 1, op: "entities", doc, fields: ["kind", "layer", "hash"], offset: page * 10_000, limit: 10_000 }
          )),
        ),
        timed("apply_restyle_1", apply([restyle])),
        timed("apply_transform_1", apply(transform(pick(1)))),
        timed("apply_transform_100", apply(transform(pick(100)))),
        timed("apply_add_1", apply([addOne])),
        timed("apply_remove_1", apply([removeOne])),
        timed("scene_after_apply", scene, [...opener, call(apply(transform(pick(1))))]),
        // ここからは、エディターの流れ（決定 0288）。開いてシーンを作ったあとの、操作・選択・取り消し
        timed("op_first", [first, scene], shown),
        timed("op_next", [next, scene], [...shown, call(first), call(scene)]),
        timed("select", select, shown),
        timed("undo", undo, [`push\t${original}\t${file}`, ...shown, call(first), call(scene)]),
      ],
    };
  } finally {
    session.close();
  }
}

const plan = [];
const table = ["図面\tバイト数\t要素の数"];
for (const base of bases) {
  for (const size of sizesOf(base)) {
    const label = `${base.name}-${size.label}`;
    const file = `${label}.${base.ext}`;
    // 前に作った図面があれば使う（作り直すときは出力のフォルダーを消す）
    if (!existsSync(join(out, file))) {
      writeFileSync(join(out, file), grow(base, size.count));
    }
    const { size: bytes, elements, cases } = casesFor(label, file, base);
    table.push(`${label}\t${bytes}\t${elements}`);
    for (const { name, setup, request } of cases) {
      const big = size.count >= 50_000;
      const search = name.endsWith("/open") || name.endsWith("/scene") || name.endsWith("/apply_transform_1");
      const kinds = !search || size.label === "base" ? "-" : big ? "work" : "work,records,elements,segments";
      const requests = Array.isArray(request) ? request : [request];
      // エディターの流れの場合は、決定に使うので回数を増やす
      const flow = /\/(op_first|op_next|select|undo)$/.test(name);
      const repeat = flow ? (big ? 5 : 7) : big ? 3 : 5;
      plan.push(`case\t${name}\t${repeat}\t${kinds}`, ...setup, ...requests.map((r) => "time\t" + JSON.stringify(r)), "end");
    }
    console.log(`${label}: ${bytes} バイト・${elements} 要素`);
  }
}
writeFileSync(join(out, "plan.txt"), plan.join("\n") + "\n");
writeFileSync(join(out, "inputs.tsv"), table.join("\n") + "\n");
