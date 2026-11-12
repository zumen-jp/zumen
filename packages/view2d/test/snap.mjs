import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { DEFAULT_LIMITS, prepare } from "../src/validate.js";
import { indexElements } from "../src/interaction.js";
import { reportOf } from "../src/render.js";
import { measurementOf, positionAt, snapPoint } from "../src/snap.js";
import { bundle, fixture } from "./fixtures.mjs";

const square = (a, b, c, d) => [a, b, c, b, c, d, a, d];

function base(count = 1) {
  const strings = [], entity_ids = [];
  for (let i = 0; i < count; i++) {
    const value = new TextEncoder().encode(`space/0/h${i}`);
    entity_ids.push(strings.length, value.length); strings.push(...value);
  }
  return {
    chunk_origins: [0, 0], chunk_bounds: [-50, -50, 50, 50], chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: Array.from({ length: count }, (_, i) => i), unit_colors: Array(count).fill(0), unit_weights: Array(count).fill(1),
    unit_clips: Array(count).fill(SCENE_NONE), unit_layers: Array(count).fill(SCENE_NONE),
    entity_ids, entity_kinds: Array(count).fill(V.entity_kind.line), entity_paths: Array(count).fill(0),
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, 255], color_flags: [0], strings,
  };
}

function lines(...values) {
  let count = 0;
  return { ...base(values.length), line_vertices: values.map((xy) => { const first = count; count += xy.length / 2; return first; }),
    line_units: values.map((_, i) => i), line_xy: values.flat() };
}

function face(...rings) {
  let count = 0;
  return { ...base(), face_units: [0], face_rings: [0],
    ring_vertices: rings.map((xy) => { const first = count; count += xy.length / 2; return first; }), face_xy: rings.flat() };
}

function point(mark = V.point_mark.dot, frame = 0) {
  return { ...base(), point_units: [0], point_xy: [0, 0], point_rotations: [0], point_markers: [0, 0, 0, 0], point_scales: [1],
    point_style: [mark, frame, V.point_size.length, 10] };
}

function clipped(source, ...polygons) {
  let count = 0;
  return { ...source, unit_clips: [0], clip_polygons: [0], polygon_flags: polygons.map((p) => p.outside ? 1 : 0),
    polygon_vertices: polygons.map(({ xy }) => { const first = count; count += xy.length / 2; return first; }), clip_xy: polygons.flatMap(({ xy }) => xy) };
}

/** 吸着と計測は表示の列だけを使い、元の曲線や配置の尺度を推測しない。 */
export function testSnap({ assert, near }) {
  const ctx = document.createElement("canvas").getContext("2d");
  const settings = { pixelsPerMillimeter: 2, foreground: "#111111", unresolvedColor: "#777777", fallbackFont: "sans-serif" };
  let view = { center: { x: 0, y: 0 }, scale: 2, rotation: 0 }, size = { width: 200, height: 200, pixelRatio: 2 };
  let prepared, index, display;
  const set = (source, metadata = {}) => {
    prepared = prepare(bundle(source, metadata), DEFAULT_LIMITS);
    display = { hidden: new Set(), colors: new Map() };
    index = indexElements(prepared);
    Object.assign(prepared, reportOf(prepared, DEFAULT_LIMITS, display, index.elements));
  };
  const snap = (x, y, tolerance = 8, limits = DEFAULT_LIMITS) => snapPoint(ctx, prepared, settings, view, size, display, index, limits, { x, y }, tolerance);
  const measure = (start, end, limits = DEFAULT_LIMITS) => measurementOf(prepared, start, end, index, limits);
  const rejects = (fn, message) => {
    let caught;
    try { fn(); } catch (error) { caught = error; }
    assert(caught instanceof RangeError, message);
  };

  set(lines([-20, 0, 0, 0, 20, 0]));
  const middle = snap(101, 99, Math.sqrt(2));
  assert(middle.kind === "vertex" && middle.position.element === index.elements[0], "折れ線の中間の頂点と canonical な参照を返す");
  near(middle.distance, Math.sqrt(2), "吸着距離は CSS px");
  assert(middle.screen.x === 100 && middle.screen.y === 100 && middle.tolerance === Math.sqrt(2), "候補の画面座標と指定した半径を返す");
  assert(Object.isFrozen(middle) && Object.isFrozen(middle.screen) && Object.isFrozen(middle.position) &&
    Object.isFrozen(middle.position.origin) && Object.isFrozen(middle.position.offset), "吸着結果は入れ子も凍結する");
  assert(snap(101, 99, 1.4) === null && snap(100, 100, 0) !== null, "円の許容範囲と半径 0 を扱う");
  set(lines([-20, 0, 20, 0]));
  assert(snap(100, 100, 1) === null, "線の途中に頂点や交点を作らない");
  assert(snap(60, 100, 0)?.position.offset.x === -20 && snap(140, 100, 0)?.position.offset.x === 20, "butt の両端も表示の境界として吸着する");
  set(lines([0, 0, 0, 0]));
  assert(snap(100, 100) === null, "長さのない線の頂点は吸着しない");

  set(face(square(-30, -30, 30, 30), square(-5, -5, 5, 5)));
  assert(snap(40, 160, 0)?.kind === "vertex" && snap(90, 110, 0)?.kind === "vertex", "面の外輪と穴の境界の頂点に吸着する");
  assert(snap(100, 100, 5) === null, "面の穴の中に候補を作らない");
  set(face(square(-10, -10, 10, 10), square(-10, -10, 10, 10)));
  assert(snap(80, 120, 1) === null, "相殺される面の輪の境界に吸着しない");

  set(point());
  assert(snap(100, 100, 0)?.kind === "point", "点の中心に吸着する");
  for (const frame of [V.point_frame.circle, V.point_frame.square]) {
    set(point(V.point_mark.none, frame));
    assert(snap(100, 100, 20) === null, "囲みの空白にある点の中心を吸着しない");
  }
  set(point(V.point_mark.plus, V.point_frame.circle));
  assert(snap(100, 100, 0)?.kind === "point", "中心を描く記号と囲みの組に吸着する");
  const unsupportedPoint = point(); unsupportedPoint.point_style[0] = 999;
  set(unsupportedPoint);
  assert(snap(100, 100) === null, "省略した点の表し方に吸着しない");
  set(fixture());
  assert(snap(40, 50, 0) === null && snap(100, 100, 0) === null, "字形の頂点や未解決の画像を候補にしない");

  const layered = lines([-20, 0, 20, 0]), label = new TextEncoder().encode("layer/0");
  layered.layer_ids = [layered.strings.length, label.length]; layered.strings.push(...label); layered.unit_layers[0] = 0;
  set(layered); display.hidden.add(0);
  assert(snap(60, 100) === null, "非表示レイヤの頂点に吸着しない");
  display.hidden.clear();
  assert(snap(60, 100) !== null, "レイヤを戻すと吸着できる");
  const transparent = lines([-20, 0, 20, 0]); transparent.color_rgba[3] = 0;
  set(transparent);
  assert(snap(60, 100) === null, "透明な線の頂点に吸着しない");
  display.colors.set(0, "#ff8800");
  assert(snap(60, 100) !== null, "不透明な強調があると吸着できる");
  display.colors.set(0, "rgba(1,2,3,0)");
  assert(snap(60, 100) === null, "強調の実効色も当たり判定と共有する");

  set(clipped(lines([-20, 0, 20, 0]), { xy: square(-10, -5, 10, 5) }));
  assert(snap(100, 100, 100) === null, "線の途中が見えても、切り抜かれた元の端点を拾わない");
  set(clipped(lines([-20, 0, 0, 0, 20, 0]), { xy: square(-5, -5, 5, 5), outside: true }));
  assert(snap(100, 100, 1) === null && snap(60, 100, 0) !== null, "反転した切り抜きを候補点へ反映する");
  set(clipped(lines([0, 0, 5, 0]), { xy: square(-10, -10, 0, 10) }, { xy: square(0, -10, 10, 10) }));
  assert(snap(100, 100, 1) === null, "辺で接するだけの複数の切り抜きに吸着しない");
  set(clipped(lines([0, 0, 5, 0]), { xy: square(0, -10, 10, 10) }));
  assert(snap(100, 100, 0) !== null, "切り抜きの境界に可視側が残る端点を拾う");
  set(clipped(point(V.point_mark.plus, V.point_frame.circle), { xy: square(1, -10, 10, 10) }));
  assert(snap(100, 100, 20) === null, "記号の一部が見えても切り抜かれた中心を拾わない");
  const unsupportedClip = clipped(lines([-20, 0, 20, 0]), { xy: square(-30, -5, 30, 5) }); unsupportedClip.polygon_flags[0] = 2;
  set(unsupportedClip);
  assert(snap(60, 100) === null, "未対応の切り抜きで省略した線を拾わない");
  set(lines([-60, 0, -50, 0]));
  assert(snap(-20, 100, 5) === null && snap(0, 100, 0) === null, "画面外と画面に端が接するだけの線を拾わない");
  set(lines([-50, 0, 0, 0]));
  assert(snap(0, 100, 0) !== null, "画面の内側に延びる線の境界を拾う");

  set(lines([-1, 0, -30, 0], [1, 0, 30, 0]));
  assert(snap(100, 100, 2)?.position.element === index.elements[1], "同距離は後に描いた単位を優先する");
  set(lines([-0.5, 0, -30, 0], [1, 0, 30, 0]));
  assert(snap(100, 100, 2)?.position.element === index.elements[0], "描画順より CSS 距離を優先する");
  const duplicate = lines([-0.5, 0, -30, 0], [1, 0, 30, 0]); duplicate.unit_entities[1] = 0;
  set(duplicate);
  assert(snap(100, 100, 2)?.position.offset.x === -0.5, "同じ要素の別の線にも近い頂点を探す");
  const chunks = lines([-1, 0, -30, 0], [1, 0, 30, 0]);
  chunks.chunk_origins.push(0, 0); chunks.chunk_bounds.push(-50, -50, 50, 50); chunks.chunk_lines.push(1);
  for (const kind of ["faces", "texts", "points", "images"]) chunks[`chunk_${kind}`].push(0);
  chunks.line_units = [1, 0];
  set(chunks);
  assert(snap(100, 100, 2)?.position.element === index.elements[0], "塊の描画順も逆から調べる");

  set(lines([10, 20, 20, 20]));
  view = { center: { x: 5, y: 7 }, scale: 2, rotation: Math.PI / 2 };
  const rotated = snap(75, 91, 2);
  near(rotated.screen.x, 74, "回転した候補の x"); near(rotated.screen.y, 90, "回転した候補の y");
  near(rotated.distance, Math.sqrt(2), "回転後の CSS 距離");
  const free = positionAt(view, size, { x: 74, y: 90 });
  near(free.origin.x + free.offset.x, 10, "逆変換の x"); near(free.origin.y + free.offset.y, 20, "逆変換の y");
  assert(free.element === undefined && Object.isFrozen(free) && Object.isFrozen(free.origin) && Object.isFrozen(free.offset), "自由な位置は凍結し要素へ結び付けない");
  size = { ...size, pixelRatio: 3 };
  near(snap(75, 91, 2).distance, rotated.distance, "画素比を変えても吸着距離は CSS px");

  const origin = 1e16, large = lines([0.25, 0, 0.75, 0]); large.chunk_origins = [origin, origin];
  view = { center: { x: origin, y: origin }, scale: 100, rotation: 0 };
  size = { width: 200, height: 200, pixelRatio: 2 };
  set(large, { unit: "meter", tolerance: 0.001 });
  const first = snap(125, 100, 0), second = snap(175, 100, 0);
  assert(first.position.origin.x === origin && first.position.offset.x === 0.25 && second.position.offset.x === 0.75, "大きな原点に相対座標を足さず保つ");
  const largeFree = positionAt(view, size, { x: 125, y: 100 });
  assert(largeFree.origin.x === origin && largeFree.offset.x === 0.25, "自由な位置の逆変換も小さい相対座標を保つ");
  const distance = measure(first.position, second.position);
  assert(distance.distance === 0.5 && distance.delta.x === 0.5 && distance.delta.y === 0, "大きな原点で小さい差を失わず測る");
  assert(distance.from === "space/0" && distance.unit === "meter" && distance.tolerance === 0.001 && distance.real.status === "known" &&
    distance.real.distance === 0.5 && distance.real.unit === "meter", "実寸の空間・単位・許容差をそのまま返す");
  assert(Object.isFrozen(distance) && Object.isFrozen(distance.delta) && Object.isFrozen(distance.real) && Object.isFrozen(distance.start), "計測結果も入れ子を凍結する");
  const shifted = { origin: { x: origin + 2, y: origin }, offset: { x: -1.25, y: 0 }, element: index.elements[0] };
  assert(measure(first.position, shifted).distance === 0.5, "別の原点を持つ位置どうしでも差を先に取る");
  const split = (x, dx) => ({ origin: { x, y: 0 }, offset: { x: dx, y: 0 } });
  assert(measure(split(0, 1), split(1e16, -1e16 + 2)).delta.x === 1, "異なる原点と相対座標の正確な和から差を一度だけ丸める");
  assert(measure(split(Number.MAX_VALUE, -Number.MAX_VALUE), split(-Number.MAX_VALUE, Number.MAX_VALUE)).distance === 0, "有限成分の相殺を途中の桁あふれで失わない");
  assert(measure(split(0, 0), split(1, 2 ** -53)).delta.x === 1, "紙上の差も最も近い偶数へ丸める");
  assert(measure(split(Number.MIN_VALUE, -Number.MIN_VALUE), split(0, Number.MIN_VALUE)).distance === Number.MIN_VALUE, "最小の非正規数の差を保つ");
  rejects(() => measure(split(0, 0), split(Number.MAX_VALUE, Number.MAX_VALUE)), "最終的な紙距離の桁あふれは拒否する");

  view = { center: { x: 0, y: 0 }, scale: 2, rotation: 0 };
  const start = { origin: { x: 0, y: 0 }, offset: { x: 0, y: 0 }, element: { id: "space/0/h0", path: [] } };
  const end = { origin: { x: 0, y: 0 }, offset: { x: 3, y: 4 }, element: { id: "space/0/h0", path: [] } };
  for (const unit of ["millimeter", "centimeter", "meter", "inch", "foot"]) {
    set(lines([0, 0, 3, 4]), { unit });
    const value = measure(start, end);
    assert(value.distance === 5 && value.real.status === "known" && value.real.unit === unit && value.start.element === index.elements[0], `${unit} を変換せず現在の参照に戻す`);
  }
  const cloned = measure(start, end);
  start.offset.x = 20; end.origin.y = 20;
  assert(cloned.start.offset.x === 0 && cloned.end.origin.y === 0 && cloned.distance === 5, "利用側の位置の書き換えを保管した計測へ持ち込まない");
  start.offset.x = 0; end.origin.y = 0;
  const unknown = (metadata, reason) => {
    set(lines([0, 0, 3, 4]), metadata);
    const value = measure(start, end);
    assert(value.distance === 5 && value.real.status === "unknown" && value.real.reason === reason, `${reason} から実寸を推測しない`);
  };
  unknown({ from: "sheet/0", unit: "millimeter" }, "paper");
  unknown({ from: "space/0", view: 0, unit: "millimeter" }, "paper");
  assert(measure(start, end).view === 0, "ビューの起点を計測結果に保つ");
  unknown({ from: "definition/0", unit: "millimeter" }, "definition");
  unknown({ unit: "unknown" }, "unit-unknown");
  unknown({ unit: "future-unit" }, "unit-unknown");
  set(lines([0, 0, 3, 4]), { unit: "meter" });
  assert(measure(positionAt(view, size, { x: 100, y: 100 }), end).real.reason === "unbound", "要素に結び付かない位置から実寸を確定しない");

  // 同じ ID を、シーン上の長さ 15 と 30 の配置として描く。
  const placed = lines([-1, 0, -16, 0], [1, 0, 31, 0]);
  placed.entity_ids[2] = placed.entity_ids[0]; placed.entity_ids[3] = placed.entity_ids[1];
  const step = new TextEncoder().encode("space/0/hinsert"), stepAt = placed.strings.length; placed.strings.push(...step);
  placed.entity_paths = [0, 1]; placed.step_ids = [stepAt, step.length, stepAt, step.length]; placed.step_cells = [1, 2, 3, 4];
  set(placed, { unit: "meter" });
  const left = snap(98, 100, 0), right = snap(102, 100, 0);
  assert(left.position.element.id === right.position.element.id && left.position.element !== right.position.element &&
    right.position.element.path[0].column === 3 && right.position.element.path[0].row === 4, "同じ ID の格子の配置を道筋全体で区別する");
  assert(measure(left.position, right.position).real.reason === "placement-scale", "異なる尺度を持ちうる配置から実寸を推測しない");
  const short = measure(left.position, snap(68, 100, 0).position), long = measure(right.position, snap(162, 100, 0).position);
  assert(short.distance === 15 && long.distance === 30 && short.real.reason === "placement-scale" && long.real.reason === "placement-scale", "同じ要素を異なる尺度で描いても実寸へ戻さない");
  const bad = { ...right.position, element: { id: right.position.element.id, path: [{ ...right.position.element.path[0], row: 5 }] } };
  rejects(() => measure(left.position, bad), "現在のシーンにない配置を計測に使わない");
  rejects(() => measure({ origin: { x: NaN, y: 0 }, offset: { x: 0, y: 0 } }, right.position), "位置の非有限値を拒む");
  rejects(() => measure(left.position, { ...right.position, element: null }), "不正な要素参照を拒む");
  rejects(() => measure(left.position, right.position, { ...DEFAULT_LIMITS, work: 50 }), "両端の要素参照を合わせた作業量を制限する");
  rejects(() => snap(NaN, 100), "吸着の非有限な問い合わせを拒む");
  rejects(() => snap(100, 100, -1), "負の吸着半径を拒む");
  rejects(() => positionAt(view, size, { x: Infinity, y: 0 }), "逆変換の非有限な問い合わせを拒む");

  const many = Array.from({ length: 1000 }, (_, i) => [20 + i / 100, 20]).flat();
  set(lines(many, [0, 0, 1, 0]));
  const before = ctx.getTransform();
  rejects(() => snap(100, 100, 0, { ...DEFAULT_LIMITS, work: 500 }), "候補を見つけた後の上限超過も途中の結果を返さない");
  const after = ctx.getTransform();
  assert(before.a === after.a && before.e === after.e && before.f === after.f, "失敗した吸着でも Canvas の状態を戻す");
  prepared = null;
  assert(snap(100, 100) === null, "シーンがなければ吸着しない");
  rejects(() => measure(start, end), "シーンのない計測を拒む");
}
