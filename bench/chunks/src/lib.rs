//! #219 の測定。公開入力と生成した DXF だけを使い、コアの設定は変更しない。

use std::collections::BTreeMap;
use std::sync::Mutex;

use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::transform::Point;
use zumen_drawing::{Drawing2D, Edge, Item, Origin, Shape, SpaceId};
use zumen_scene::layout as l;
use zumen_scene::{Chunking, Options, Palette, Scene};
use zumen_sdk::{Lift, testkit};

pub const CASES: [&str; 5] = [
    "layout-dwg",
    "arcdim-dxf",
    "text-dxf",
    "synthetic-10k",
    "synthetic-100k",
];
pub const POLICIES: [&str; 5] = [
    "provisional",
    "items-1024",
    "items-16384",
    "span-16384",
    "span-262144",
];
const INPUTS: [&[u8]; 3] = [
    include_bytes!("../../../samples/dwg/zumen-layout-r2000.dwg"),
    include_bytes!("../../../samples/dwg/zumen-arcdim-r2004_ascii.dxf"),
    include_bytes!("../../../samples/dxf/text_alignments.dxf"),
];

struct Case {
    drawing: Drawing2D,
    input_bytes: usize,
    // 実線の線分だけ。曲線の近似や文字の整形の誤差とは混ぜない。
    lines: BTreeMap<Vec<u8>, [[f64; 2]; 2]>,
    ids: std::collections::BTreeSet<String>,
    walked: usize,
}

struct State {
    case: Case,
    scene: Option<Scene>,
    diagnostics: String,
    report: Vec<u8>,
    canonical: Option<Vec<u8>>,
}
static STATE: Mutex<Option<State>> = Mutex::new(None);

fn context<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> T {
    f(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
}

fn synthetic(n: usize) -> Vec<u8> {
    // 1 万本は件数の上限、10 万本は範囲の上限が効く配置を測る。
    let scale = if n == 10_000 { 0.001 } else { 1.0 };
    let mut out = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
    );
    for i in 0..n {
        // 地図の大きな座標の近くに、小数の細かい差を持つ線分を置く。
        let x = 100_000.0 + (i % 1000) as f64 * 1.001_234_567 * scale;
        let y = 50_000.0 + (i / 1000) as f64 * 9.876_543_21 * scale;
        out.push_str(&format!(
            "0\nLINE\n5\n{:X}\n8\n0\n10\n{x:.12}\n20\n{y:.12}\n11\n{:.12}\n21\n{:.12}\n",
            i + 1,
            x + 0.123_456_789 * scale,
            y + 0.234_567_891 * scale
        ));
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    out.into_bytes()
}

fn source_key<'a>(id: &str, steps: impl Iterator<Item = (&'a str, [u32; 2])>) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(&(id.len() as u64).to_le_bytes());
    key.extend_from_slice(id.as_bytes());
    for (id, cell) in steps {
        key.extend_from_slice(&(id.len() as u64).to_le_bytes());
        key.extend_from_slice(id.as_bytes());
        key.extend_from_slice(&cell[0].to_le_bytes());
        key.extend_from_slice(&cell[1].to_le_bytes());
    }
    key
}

fn line_ends<P: Point>(shape: &Shape<P>) -> Option<[[f64; 2]; 2]> {
    match shape {
        Shape::Line { start, end } | Shape::Curve(Edge::Line { start, end }) => {
            Some([start.xy(), end.xy()])
        }
        _ => None,
    }
}

fn make_case(index: usize) -> Case {
    let generated;
    let input = if index < INPUTS.len() {
        INPUTS[index]
    } else {
        generated = synthetic(if index == 3 { 10_000 } else { 100_000 });
        &generated
    };
    let drawing = if index == 0 {
        let file = testkit::read::<Dwg>(input).0.unwrap();
        context(|cx| file.lift(cx).unwrap())
    } else {
        let file = testkit::read::<Dxf>(input).0.unwrap();
        context(|cx| file.lift(cx).unwrap())
    };
    let visits = context(|cx| drawing.walk_parts(Origin::Space(SpaceId(0)), cx).unwrap());
    let mut lines = BTreeMap::new();
    for visit in &visits {
        if !visit.visible {
            continue;
        }
        // 線種を展開する線は対象外として件数を明示する。
        if visit.line_type.is_some_and(|id| {
            drawing
                .line_types
                .iter()
                .any(|t| t.id == id && t.definition.as_ref().is_some_and(|d| !d.pattern.is_empty()))
        }) {
            continue;
        }
        let ends = match visit.item {
            Item::Model(element) => line_ends(&element.shape),
            Item::Paper(element) => line_ends(&element.shape),
            _ => None,
        };
        if let Some(ends) = ends {
            let [a, b, c, d, e, f] = visit.transform.m;
            lines.insert(
                source_key(
                    &visit.id,
                    visit.path.iter().map(|s| (s.id.as_str(), s.cell)),
                ),
                ends.map(|[x, y]| [a * x + b * y + c, d * x + e * y + f]),
            );
        }
    }
    let walked = visits.len();
    let ids = visits.iter().map(|visit| visit.id.clone()).collect();
    drop(visits);
    Case {
        drawing,
        input_bytes: input.len(),
        lines,
        ids,
        walked,
    }
}

pub fn prepare(index: usize) {
    assert!(index < CASES.len());
    *STATE.lock().unwrap() = Some(State {
        case: make_case(index),
        scene: None,
        diagnostics: String::new(),
        report: Vec::new(),
        canonical: None,
    });
}

fn policy(index: usize) -> Chunking {
    let mut value = Chunking::PROVISIONAL;
    match index {
        0 => (),
        1 => value.items = 1024,
        2 => value.items = 16384,
        3 => value.span = 16384.0,
        4 => value.span = 262144.0,
        _ => panic!("unknown policy"),
    }
    value
}

pub fn render(index: usize) {
    let mut guard = STATE.lock().unwrap();
    let state = guard.as_mut().unwrap();
    let options = Options {
        origin: Origin::Space(SpaceId(0)),
        tolerance: Some(0.01),
        print: false,
        region: None,
        palette: Palette::default(),
        chunking: policy(index),
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    state.scene = Some(
        zumen_scene::build(
            &state.case.drawing,
            &options,
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        )
        .unwrap(),
    );
    // 場所や元図面の文字列は出力しない。診断の code と件数だけ。
    state.diagnostics = diag
        .iter()
        .map(|d| format!("{}:{}", d.code().name(), d.count()))
        .collect::<Vec<_>>()
        .join(",");
}

struct Read<'a>(&'a [u8]);
impl<'a> Read<'a> {
    fn column(&self, section: usize) -> &'a [u8] {
        let start = u32_at(self.0, 16 + section * 8) as usize;
        let len = u32_at(self.0, 20 + section * 8) as usize;
        &self.0[start..start + len]
    }
    fn uint(&self, section: usize, row: usize) -> usize {
        u32_at(self.column(section), row * 4) as usize
    }
    fn float(&self, section: usize, row: usize) -> f64 {
        f32::from_le_bytes(
            self.column(section)[row * 4..row * 4 + 4]
                .try_into()
                .unwrap(),
        ) as f64
    }
    fn string(&self, section: usize, row: usize) -> &'a str {
        let start = self.uint(section, row * 2);
        let len = self.uint(section, row * 2 + 1);
        std::str::from_utf8(&self.column(l::STRINGS)[start..start + len]).unwrap()
    }
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

pub fn audit() -> Vec<u8> {
    let mut guard = STATE.lock().unwrap();
    let state = guard.as_mut().unwrap();
    let scene = state.scene.as_ref().unwrap();
    let r = Read(&scene.bytes);
    let entities = r.column(l::ENTITY_KINDS).len();
    for entity in 0..entities {
        assert!(
            state.case.ids.contains(r.string(l::ENTITY_IDS, entity)),
            "scene ID is absent from the input walk"
        );
    }
    let mut canonical = Vec::new();
    // ID・種類・配置の道筋と描く単位の表が設定によらず同じことを実際の列で確認。
    for section in [
        l::ENTITY_IDS,
        l::ENTITY_KINDS,
        l::ENTITY_PATHS,
        l::STEP_IDS,
        l::STEP_CELLS,
        l::UNIT_ENTITIES,
        l::UNIT_COLORS,
        l::UNIT_WEIGHTS,
        l::UNIT_CLIPS,
        l::UNIT_LAYERS,
        l::STRINGS,
    ] {
        canonical.extend_from_slice(&(r.column(section).len() as u64).to_le_bytes());
        canonical.extend_from_slice(r.column(section));
    }
    // 表示部品と同じく塊の順に進み、塊内は描く単位の順に種類をまとめる。
    let groups = [
        (l::CHUNK_FACES, l::FACE_UNITS),
        (l::CHUNK_LINES, l::LINE_UNITS),
        (l::CHUNK_TEXTS, l::TEXT_UNITS),
        (l::CHUNK_POINTS, l::POINT_UNITS),
        (l::CHUNK_IMAGES, l::IMAGE_UNITS),
    ];
    for chunk in 0..scene.counts.chunks as usize {
        let mut order = Vec::new();
        for (kind, (starts, units)) in groups.iter().enumerate() {
            let start = r.uint(*starts, chunk);
            let end = if chunk + 1 == scene.counts.chunks as usize {
                r.column(*units).len() / 4
            } else {
                r.uint(*starts, chunk + 1)
            };
            for row in start..end {
                order.push((r.uint(*units, row), kind, row));
            }
        }
        order.sort_unstable();
        for (unit, kind, row) in order {
            for value in [unit, kind, row] {
                canonical.extend_from_slice(&(value as u64).to_le_bytes());
            }
        }
    }
    // 種類ごとの順と、塊をまたいだ描く単位の順を比較する。
    for section in [
        l::LINE_UNITS,
        l::FACE_UNITS,
        l::TEXT_UNITS,
        l::POINT_UNITS,
        l::IMAGE_UNITS,
    ] {
        canonical.extend_from_slice(&(r.column(section).len() as u64).to_le_bytes());
        canonical.extend_from_slice(r.column(section));
    }
    if let Some(first) = &state.canonical {
        assert_eq!(first, &canonical, "IDs or per-kind drawing order changed");
    } else {
        state.canonical = Some(canonical.clone());
    }
    let mut max_error = 0.0_f64;
    let mut checked_lines = 0;
    let mut checked_ids = std::collections::BTreeSet::new();
    let origins = r.column(l::CHUNK_ORIGINS);
    let mut max_span = 0.0_f64;
    for chunk in 0..scene.counts.chunks as usize {
        let origin = [0, 1].map(|axis| {
            f64::from_le_bytes(
                origins[chunk * 16 + axis * 8..chunk * 16 + axis * 8 + 8]
                    .try_into()
                    .unwrap(),
            )
        });
        max_span = max_span
            .max(r.float(l::CHUNK_BOUNDS, chunk * 4 + 2))
            .max(r.float(l::CHUNK_BOUNDS, chunk * 4 + 3));
        let start = r.uint(l::CHUNK_LINES, chunk);
        let end = if chunk + 1 == scene.counts.chunks as usize {
            scene.counts.lines as usize
        } else {
            r.uint(l::CHUNK_LINES, chunk + 1)
        };
        for line in start..end {
            let unit = r.uint(l::LINE_UNITS, line);
            let entity = r.uint(l::UNIT_ENTITIES, unit);
            let id = r.string(l::ENTITY_IDS, entity);
            let first_step = r.uint(l::ENTITY_PATHS, entity);
            let last_step = if entity + 1 == entities {
                r.column(l::STEP_IDS).len() / 8
            } else {
                r.uint(l::ENTITY_PATHS, entity + 1)
            };
            let key = source_key(
                id,
                (first_step..last_step).map(|step| {
                    (
                        r.string(l::STEP_IDS, step),
                        [
                            r.uint(l::STEP_CELLS, step * 2) as u32,
                            r.uint(l::STEP_CELLS, step * 2 + 1) as u32,
                        ],
                    )
                }),
            );
            let Some(expected) = state.case.lines.get(&key) else {
                continue;
            };
            let first = r.uint(l::LINE_VERTICES, line);
            let last = if line + 1 == scene.counts.lines as usize {
                r.column(l::LINE_XY).len() / 8
            } else {
                r.uint(l::LINE_VERTICES, line + 1)
            };
            assert_eq!(last - first, 2);
            for (offset, point) in expected.iter().enumerate() {
                for axis in 0..2 {
                    let restored = origin[axis] + r.float(l::LINE_XY, (first + offset) * 2 + axis);
                    assert!(restored.is_finite(), "non-finite restored coordinate");
                    max_error = max_error.max((restored - point[axis]).abs());
                }
            }
            checked_lines += 1;
            assert!(checked_ids.insert(key), "an expected line appeared twice");
        }
    }
    assert_eq!(
        checked_ids.len(),
        state.case.lines.len(),
        "an expected line disappeared"
    );
    assert!(
        max_error <= scene.tolerance,
        "coordinate error exceeds tolerance"
    );
    let c = scene.counts;
    let error_text = if checked_lines == 0 {
        "null".into()
    } else {
        max_error.to_string()
    };
    let ratio_text = if checked_lines == 0 {
        "null".into()
    } else {
        (max_error / scene.tolerance).to_string()
    };
    let report = format!(
        "{{\"input_bytes\":{},\"walked\":{},\"entities\":{entities},\"chunks\":{},\"lines\":{},\"vertices\":{},\"faces\":{},\"texts\":{},\"points\":{},\"images\":{},\"scene_bytes\":{},\"tolerance\":{},\"max_chunk_span\":{max_span},\"checked_lines\":{checked_lines},\"max_coordinate_error\":{error_text},\"error_over_tolerance\":{ratio_text},\"identity_order_fnv1a\":\"{:016x}\",\"diagnostics\":\"{}\"}}",
        state.case.input_bytes,
        state.case.walked,
        c.chunks,
        c.lines,
        c.vertices,
        c.faces,
        c.texts,
        c.points,
        c.images,
        scene.bytes.len(),
        scene.tolerance,
        hash(&canonical),
        state.diagnostics
    );
    state.report = report.into_bytes();
    state.report.clone()
}

pub fn bytes() -> Vec<u8> {
    STATE
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .scene
        .as_ref()
        .unwrap()
        .bytes
        .clone()
}

#[unsafe(no_mangle)]
pub extern "C" fn bench_prepare(case: u32) {
    prepare(case as usize);
}
#[unsafe(no_mangle)]
pub extern "C" fn bench_render(policy: u32) {
    render(policy as usize);
}
#[unsafe(no_mangle)]
pub extern "C" fn bench_audit() {
    audit();
}
#[unsafe(no_mangle)]
pub extern "C" fn scene_ptr() -> *const u8 {
    STATE
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .scene
        .as_ref()
        .unwrap()
        .bytes
        .as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn scene_len() -> usize {
    STATE
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .scene
        .as_ref()
        .unwrap()
        .bytes
        .len()
}
#[unsafe(no_mangle)]
pub extern "C" fn report_ptr() -> *const u8 {
    STATE.lock().unwrap().as_ref().unwrap().report.as_ptr()
}
#[unsafe(no_mangle)]
pub extern "C" fn report_len() -> usize {
    STATE.lock().unwrap().as_ref().unwrap().report.len()
}
