import { SCENE_NONE, SCENE_VALUES as V } from "@zumen-jp/zumen";
import { View2D } from "../src/index.js";
import { bundle, fixture } from "./fixtures.mjs";

const square = (minX, minY, maxX, maxY) => [minX, minY, maxX, minY, maxX, maxY, minX, maxY];

function minimal() {
  const strings = [...new TextEncoder().encode("space/0/h1")];
  return {
    chunk_origins: [0, 0], chunk_bounds: [-50, -50, 50, 50], chunk_lines: [0], chunk_faces: [0], chunk_texts: [0], chunk_points: [0], chunk_images: [0],
    unit_entities: [0], unit_colors: [0], unit_weights: [2], unit_clips: [SCENE_NONE], unit_layers: [SCENE_NONE],
    entity_ids: [0, strings.length], entity_kinds: [V.entity_kind.line], entity_paths: [0],
    color_systems: [0], color_codes: [0], color_files: [0, 0, 0, 0], color_rgba: [0, 0, 0, 255], color_flags: [0], strings,
  };
}

function line(xy = [-20, 0, 20, 0]) { return { ...minimal(), line_vertices: [0], line_units: [0], line_xy: xy }; }
function face(rings = [square(-30, -30, 30, 30)]) {
  let count = 0;
  return { ...minimal(), face_rings: [0], face_units: [0], ring_vertices: rings.map((ring) => { const start = count; count += ring.length / 2; return start; }), face_xy: rings.flat() };
}
function point(mark, frame = 0, sizeKind = V.point_size.length, value = 10) {
  return { ...minimal(), point_xy: [0, 0], point_units: [0], point_rotations: [0], point_markers: [0, 0, 0, 0], point_scales: [1], point_style: [mark, frame, sizeKind, value] };
}
function glyph(rings) {
  let count = 0;
  return { ...minimal(), text_strings: [0, 0], text_xy: [0, 0], text_second: [NaN, NaN], text_heights: [8], text_widths: [1],
    text_rotations: [0], text_obliques: [0], text_boxes: [0], text_aligns: [0, 0], text_flags: [0], text_lines: [0], text_fonts: [SCENE_NONE], text_units: [0],
    text_glyphs: [0], glyph_shapes: [0], glyph_axes: [1, 0, 0, 1], glyph_xy: [0, 0], shape_rings: [0],
    outline_vertices: rings.map((ring) => { const start = count; count += ring.length / 2; return start; }), outline_xy: rings.flat(),
  };
}
function clipped(source, polygons) {
  let count = 0;
  return { ...source, unit_clips: [0], clip_polygons: [0], polygon_flags: polygons.map((p) => p.outside ? 1 : 0),
    polygon_vertices: polygons.map(({ xy }) => { const start = count; count += xy.length / 2; return start; }), clip_xy: polygons.flatMap(({ xy }) => xy) };
}

/** 合成のシーンで、描画の形と可視領域に対する問い合わせを確かめる。 */
export function testHit({ assert }) {
  const canvas = document.createElement("canvas");
  const viewer = new View2D(canvas, { width: 200, height: 200, pixelRatio: 2, pixelsPerMillimeter: 2 });
  const view = { center: { x: 0, y: 0 }, scale: 2, rotation: 0 };
  viewer.jumpTo(view);
  const set = (source) => viewer.setScene(bundle(source));
  const hit = (x, y, radius = 0) => viewer.hitTest({ x, y }, radius).length > 0;
  const at = (x, y, radius = 0) => viewer.hitTest(viewer.toScreen({ x, y }), radius).length > 0;
  const rectangle = (min_x, min_y, max_x, max_y) => viewer.hitTestRect({ min_x, min_y, max_x, max_y }).length > 0;
  const region = (x1, y1, x2, y2) => {
    const a = viewer.toScreen({ x: x1, y: y1 }), b = viewer.toScreen({ x: x2, y: y2 });
    return rectangle(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.max(a.x, b.x), Math.max(a.y, b.y));
  };
  try {
    set(line());
    assert(hit(100, 101.5) && !hit(100, 102.5), "線の中心だけでなく実際の線幅を選ぶ");
    assert(!hit(59.5, 100) && hit(59.5, 100, 1), "butt の端と CSS px の許容半径");
    assert(!hit(59.2, 102.8, 1), "許容範囲は外接矩形ではなく円");
    assert(rectangle(80, 101, 120, 101.5) && !rectangle(80, 102.2, 120, 103), "中心線が入らない範囲でも線幅との交差を調べる");
    assert(rectangle(100, 100, 100, 100), "点に縮んだ矩形の問い合わせ");
    set(line([-10, 0, 0, 0, 0, 10]));
    assert(hit(101.2, 101.2) && !hit(101.8, 101.8), "折れ線の round join の扇形を使う");
    set(line([0, 0, 0, 0, 0, 0]));
    assert(!hit(100, 100, 2), "長さのない butt の線を拾わない");

    viewer.jumpTo({ ...view, scale: 1 });
    set(line([-0.5, 0, 0, 0, 0.5, 0]));
    assert(!hit(101.5, 100) && !hit(98.5, 100), "短い共線の join が butt の端からはみ出さない");
    assert(!hit(101.5, 100, 0.9) && hit(101.5, 100, 1), "短い線への許容円は実際の butt の端から測る");
    assert(!rectangle(101, 99, 101.5, 101) && rectangle(100.5, 99, 101, 101), "範囲も短い線の butt の端との接触で判定する");
    set(clipped(line([-0.5, 0, 0, 0, 0.5, 0]), [{ xy: square(1, -3, 3, 3) }]));
    assert(!hit(101.5, 100, 5) && !rectangle(90, 90, 110, 110), "join の全円だけが切り抜きに入っても選ばない");
    set(clipped(line([-0.5, 0, 0, 0, 0, 0.5]), [{ xy: square(-2, -2, -0.9, -0.9) }]));
    assert(!hit(98.5, 101.5) && !rectangle(90, 90, 110, 110), "短い曲がりの扇形の外を切り抜いても描くものはない");
    set(clipped(line([-0.5, 0, 0, 0, 0, 0.5]), [{ xy: square(0, -3, 3, 0) }]));
    assert(hit(101, 101) && hit(100, 100) && rectangle(101, 101, 101.2, 101.2), "扇形の内側と半径方向の clip 境界への接触を拾う");
    set(clipped(line([-0.5, 0, 0, 0, 0, 0.5]), [{ xy: square(-3, -3, 3, -2) }]));
    assert(!hit(100, 102) && !rectangle(90, 90, 110, 110), "join の円弧に切り抜きが接するだけなら拾わない");

    // 当たり判定の幾何をまねず、Canvas が構築した stroke の包含と照らす。
    const reference = document.createElement("canvas").getContext("2d");
    reference.lineWidth = 4; reference.lineCap = "butt"; reference.lineJoin = "round";
    const compareStroke = (path, name) => {
      let mismatch;
      let inside = 0, outside = 0;
      for (let y = 0; y < 25; y++) for (let x = 0; x < 25; x++) {
        const px = 97.073 + x * 0.25, py = 97.073 + y * 0.25;
        const expected = reference.isPointInStroke(path, px, py);
        // Canvas の円弧の近似に依存しないよう 0.002 CSS px 以内の境界は外す。
        // ちょうど接する問い合わせは、上の明示した端・clip のケースで確かめる。
        if ([[0.002, 0], [-0.002, 0], [0, 0.002], [0, -0.002]].some(([dx, dy]) => reference.isPointInStroke(path, px + dx, py + dy) !== expected)) continue;
        if (expected) inside++; else outside++;
        if (hit(px, py) !== expected) mismatch ??= { x: px, y: py };
      }
      assert(inside > 0 && outside > 0 && !mismatch, `${name} の round join を Canvas と照合: ${JSON.stringify(mismatch)}`);
    };
    const cases = [
      ["短い共線", [-0.5, 0, 0, 0, 0.5, 0]],
      ["逆向きの共線", [0.5, 0, 0, 0, -0.5, 0]],
      ["左への曲がり", [-0.5, 0, 0, 0, 0, 0.5]],
      ["右への曲がり", [-0.5, 0, 0, 0, 0, -0.5]],
      ["小さい角度の曲がり", [-0.5, 0, 0, 0, 0.5, 0.25]],
      ["大きい角度の曲がり", [-0.5, 0, 0, 0, -0.375, 0.25]],
      ["折返し", [-0.5, 0, 0, 0, -0.5, 0]],
      ["短い折返し", [-0.5, 0, 0, 0, -0.25, 0]],
      ["重複した頂点", [-0.5, 0, 0, 0, 0, 0, 0, 0.5]],
      ["近接した曲がり", [-0.5, 0, 0, 0, 0, 0.5, 0.5, 0.5]],
      ["始点へ戻る開いた線", [-0.5, 0, 0, 0, 0, 0.5, -0.5, 0]],
    ];
    for (const [name, values] of cases) {
      const xy = new Float32Array(values), path = new Path2D();
      set(line(values));
      for (let i = 0; i < xy.length; i += 2) {
        const p = viewer.toScreen({ x: xy[i], y: xy[i + 1] });
        if (i === 0) path.moveTo(p.x, p.y); else path.lineTo(p.x, p.y);
      }
      compareStroke(path, name);
    }
    viewer.jumpTo({ ...view, scale: 0.25, rotation: Math.PI / 5 });
    set(line([-2, 0, 0, 0, 0, 2]));
    const rotated = new Path2D();
    for (const [i, p] of [[-2, 0], [0, 0], [0, 2]].map(([x, y]) => viewer.toScreen({ x, y })).entries()) {
      if (i === 0) rotated.moveTo(p.x, p.y); else rotated.lineTo(p.x, p.y);
    }
    compareStroke(rotated, "縮小して回転した曲がり");
    viewer.jumpTo({ ...view, scale: 1 });
    set(point(V.point_mark.none, V.point_frame.square, V.point_size.length, 2));
    const closed = new Path2D(); closed.rect(99, 99, 2, 2); compareStroke(closed, "短い辺の閉じた四角");
    viewer.jumpTo(view);

    set(face([square(-30, -30, 30, 30), square(-10, -10, 10, 10)]));
    assert(!at(0, 0) && at(15, 0), "面の偶奇規則の穴");
    assert(!region(-5, -5, 5, 5) && region(8, -2, 12, 2), "矩形が面の穴の中か、穴の縁を横切るかを区別する");
    assert(region(15, 15, 16, 16), "面の内部に入る矩形");
    set(face([square(-40, -1, 40, 1)]));
    assert(region(-2, -10, 2, 10), "四隅を含まない細い面と矩形の交差");
    set(face([[-10, -10, -10, -10, 10, -10, 0, 10, -10, -10]]));
    assert(at(0, 0), "連続する同じ頂点と繰り返した始点があっても面を拾う");
    set(face()); assert(at(30, 0) && at(30, 30), "塗りのある面の辺と頂点への接触を拾う");
    set(face([square(-30, -30, 30, 30), square(-30, -30, 30, 30)]));
    assert(!at(30, 0) && !at(30, 30) && !region(-40, -40, 40, 40), "偶奇規則で相殺された面は、輪の辺と頂点も拾わない");

    set(clipped(face(), [{ xy: square(-5, -5, 5, 5) }]));
    assert(at(0, 0) && !at(10, 0), "内側を残す切り抜き");
    assert(region(-15, -15, 15, 15), "矩形内にある切り抜きの小さな可視領域を拾う");
    set(clipped(face(), [{ xy: square(-5, -5, 5, 5), outside: true }]));
    assert(!at(0, 0) && at(10, 0) && !region(-2, -2, 2, 2), "外側を残す切り抜き");
    set(clipped(face(), [{ xy: square(-20, -20, 5, 20) }, { xy: square(10, -20, 20, 20) }]));
    assert(!region(-25, -25, 25, 25), "別々に交差する切り抜きの共通部分が空なら拾わない");
    set(clipped(face(), [{ xy: square(-20, -20, 0, 20) }, { xy: square(0, -20, 20, 20) }]));
    assert(!at(0, 0) && !at(0, 20) && !region(-25, -25, 25, 25), "切り抜きの共通部分が辺だけなら描かれる領域はない");
    set(clipped(face([square(-30, -30, 30, 30), square(-10, -10, 10, 10)]), [{ xy: square(-5, -5, 5, 5) }]));
    assert(!region(-25, -25, 25, 25), "切り抜きの全体が面の穴に入る場合を拾わない");
    set(clipped(face(), [{ xy: square(-30, -30, 30, 30) }, { xy: square(-30, -30, 8, 30), outside: true }, { xy: square(10, -30, 30, 30), outside: true }]));
    assert(region(0, -2, 20, 2) && !region(-2, -2, 2, 2), "複数の反転した切り抜きが残す細い領域");
    set(clipped(line(), [{ xy: square(-30, 2, 30, 20) }]));
    assert(!at(0, 2, 10) && !region(-25, -25, 25, 25), "許容半径が大きくても完全に切り抜かれた元の線を拾わない");
    set(clipped(line(), [{ xy: square(-30, 1, 30, 20) }]));
    assert(!at(0, 1) && !region(-25, -25, 25, 25), "線幅の縁と切り抜きが接するだけの場合も拾わない");
    set(clipped(line(), [{ xy: square(0, -20, 30, 20) }]));
    assert(at(-2, 0, 5) && !at(-2, 0, 3), "切り抜き外からの許容円が可視形へ届く場合だけ拾う");
    assert(rectangle(99, 99, 101, 101) && !rectangle(60, 99, 99, 101), "切り抜いた stroke と矩形を照らす");

    const outer = square(0, 0, 8, 8), hole = [2, 2, 2, 6, 6, 6, 6, 2];
    set(glyph([outer, hole]));
    assert(at(1, 1) && !at(4, 4) && !region(3, 3, 5, 5), "字形の非零規則の穴を点と範囲に適用する");
    const mirrored = glyph([outer, hole]); mirrored.glyph_axes = [-1, 0, 0, 1]; set(mirrored);
    assert(at(-1, 1) && !at(-4, 4), "反転した字形でも穴を保つ");
    set(glyph([outer, square(2, 2, 6, 6)]));
    assert(at(4, 4), "同じ向きの字形の輪は偶奇規則の穴にしない");
    set(glyph([outer, [0, 8, 8, 8, 8, 0, 0, 0]]));
    assert(!at(0, 4) && !at(0, 0) && !region(-1, -1, 9, 9), "非零規則で相殺された字形は輪郭の上も拾わない");
    const overlapping = glyph([outer]); overlapping.glyph_shapes = [0, 0]; overlapping.glyph_axes = [1, 0, 0, 1, -1, 0, 0, 1]; overlapping.glyph_xy = [0, 0, 8, 0]; set(overlapping);
    assert(at(4, 4), "反対向きの別の字形を重ねても輪を打ち消さない");
    set(glyph([[]])); assert(!at(0, 0, 3), "輪郭のない字形を基点で拾わない");

    set(point(V.point_mark.none, V.point_frame.circle));
    assert(!hit(100, 100) && hit(110, 100) && !hit(113, 100), "点の円の囲みは塗りつぶさず線幅を使う");
    assert(hit(111, 111, 4) && !hit(111, 111, 3), "円の囲みと許容円の部分的な交差");
    assert(!rectangle(97, 97, 103, 103) && rectangle(107.9, 107.9, 108.1, 108.1), "円の囲みの内部の空白と斜めの縁");
    set(clipped(point(V.point_mark.none, V.point_frame.circle), [{ xy: square(6, -20, 20, 20) }]));
    assert(!hit(112, 100) && !rectangle(80, 80, 140, 120), "円の囲みの外側と切り抜きが接するだけなら拾わない");
    set(clipped(point(V.point_mark.none, V.point_frame.circle), [{ xy: square(-20, -20, 4, 20) }]));
    assert(hit(108, 100), "円の穴と切り抜きの接点に狭い可視領域が残る場合は拾う");
    set(point(V.point_mark.none, V.point_frame.square));
    assert(!hit(100, 100) && hit(110, 100), "点の四角の囲みも線だけを拾う");
    set(point(V.point_mark.plus, 0, V.point_size.screen, 10));
    assert(hit(109, 100) && !hit(111, 100) && !hit(105, 105), "十字は実際の二本の線を拾う");
    viewer.jumpTo({ ...view, scale: 5 });
    assert(hit(109, 100) && !hit(111, 100), "画面の高さに対する点の大きさはズームに連動しない");
    viewer.jumpTo(view);
    const tick = point(V.point_mark.tick); tick.point_rotations = [90]; set(tick);
    assert(hit(91, 100) && !hit(100, 91), "点の回転を線に反映する");
    set(point(V.point_mark.dot));
    assert(hit(100.4, 100) && !hit(100.6, 100), "画素比 2 の小さい点の半径");
    viewer.resize(200, 200, 1);
    assert(hit(100.8, 100), "画素比 1 の小さい点の半径");
    viewer.resize(200, 200, 1.5);
    assert(hit(100.6, 100) && !hit(100.8, 100), "小数の画素比も CSS px で問い合わせる");

    set(line()); viewer.jumpTo({ ...view, rotation: Math.PI / 3 });
    assert(at(10, 0) && !at(10, 5), "回転した表示の線を点で問い合わせる");
    const mid = viewer.toScreen({ x: 10, y: 0 });
    assert(rectangle(mid.x - 1, mid.y - 1, mid.x + 1, mid.y + 1), "回転した表示も画面の矩形で問い合わせる");
    viewer.jumpTo(view);
    const huge = line([0.25, 0, 0.25, 10]); huge.chunk_origins = [1e16, 1e16]; set(huge);
    viewer.jumpTo({ center: { x: 1e16, y: 1e16 }, scale: 16, rotation: 0 });
    assert(hit(104, 50) && !hit(100, 50), "大きい原点でも相対座標の下位桁を失わない");
    viewer.jumpTo(view);

    const layered = line(); layered.layer_ids = [...layered.entity_ids]; layered.unit_layers = [0]; set(layered);
    viewer.setHiddenLayers(["space/0/h1"]);
    assert(!hit(100, 100) && !rectangle(50, 50, 150, 150), "非表示のレイヤを点と範囲から除く");
    viewer.setHiddenLayers([]); assert(hit(100, 100), "レイヤを表示し直すと選べる");

    const transparent = line(); transparent.color_rgba[3] = 0; set(transparent);
    assert(!hit(100, 100) && !rectangle(50, 50, 150, 150), "完全に透明なシーンの色を点と範囲から除く");
    viewer.select([{ id: "space/0/h1", path: [] }]);
    assert(hit(100, 100), "不透明な選択色で描くと透明だった要素も拾える");
    set(line());
    for (const color of ["transparent", "#1230", "rgba(0, 0, 0, 0)", "rgb(0 0 0 / 0%)"]) {
      viewer.setHighlight("invisible", [{ id: "space/0/h1", path: [] }], color);
      assert(!hit(100, 100), "正規化された透明な強調色を当たり判定にも反映する");
    }
    viewer.setHighlight("invisible", [{ id: "space/0/h1", path: [] }], "#000000");
    assert(hit(100, 100), "黒の RGB を透明な alpha と読み違えない");

    const repeated = line();
    repeated.line_vertices = [0, 2, 4]; repeated.line_units = [0, 1, 2]; repeated.line_xy = Array(3).fill(repeated.line_xy).flat();
    repeated.unit_entities = [0, 1, 2]; repeated.unit_colors = [0, 0, 0]; repeated.unit_weights = [2, 2, 2]; repeated.unit_clips = Array(3).fill(SCENE_NONE); repeated.unit_layers = Array(3).fill(SCENE_NONE);
    repeated.entity_ids = Array(3).fill(repeated.entity_ids).flat(); repeated.entity_kinds = Array(3).fill(V.entity_kind.line); repeated.entity_paths = [0, 1, 2];
    repeated.step_ids = [...repeated.entity_ids]; repeated.step_cells = [0, 0, 1, 0, 0, 0]; set(repeated);
    const refs = viewer.hitTest({ x: 100, y: 100 }, 0);
    assert(refs.length === 2 && refs[0].path[0].column === 0 && refs[1].path[0].column === 1, "逆描画順で ID と道筋の重複を除き、格子の列を区別する");
    assert(Object.isFrozen(refs) && Object.isFrozen(refs[0]) && Object.isFrozen(refs[0].path), "当たり判定は凍結した参照を返す");
    set(fixture());
    assert(!viewer.hitTestRect({ min_x: 0, min_y: 0, max_x: 200, max_y: 200 }).some(({ id }) => id === "space/0/h5"), "未描画の画像を範囲から除く");
    const unknown = clipped(line(), [{ xy: square(-20, -20, 20, 20) }]); unknown.polygon_flags = [2]; set(unknown);
    assert(!hit(100, 100), "未対応の切り抜きで描かない要素は拾わない");
    assert(!hit(-100, -100, 1), "Canvas の外には描かれていない");
    viewer.setScene(null); assert(!hit(100, 100) && !rectangle(0, 0, 200, 200), "空のシーンの問い合わせ");
  } finally { viewer.destroy(); }

  const limitedCanvas = document.createElement("canvas");
  const limited = new View2D(limitedCanvas, { width: 200, height: 200, limits: { work: 1000 } });
  try {
    limited.setScene(bundle(face(Array.from({ length: 60 }, () => square(-30, -30, 30, 30)))));
    const state = limited.getState(), before = limitedCanvas.getContext("2d").getImageData(100, 100, 1, 1).data.join(",");
    let failed = false;
    try { limited.hitTestRect({ min_x: 95, min_y: 95, max_x: 105, max_y: 105 }); } catch (error) { failed = error instanceof RangeError; }
    assert(failed, "輪と境界の反復を問い合わせの作業量に数える");
    assert(limited.getState() === state && limitedCanvas.getContext("2d").getImageData(100, 100, 1, 1).data.join(",") === before, "作業量を超えた問い合わせは画面と状態を変えない");
  } finally { limited.destroy(); }
}
