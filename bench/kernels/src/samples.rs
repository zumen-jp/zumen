//! 公開サンプルの IFC の形を、カーネルの候補ごとに評価し、要素ごとに 1 行ずつ書く（#5）。
//!
//! 使い方: `samples [候補] <IFC>...`。候補は own・manifold-rust・csgrs（省けばすべて）。`lift` なら評価せず、
//! 読みと建物モデルへの写しと形の写しだけを測る。
//!
//! 要素ごとに Body の形状表現を写し（[`zumen_bench_kernels::ifc`]）、開口（`IfcRelVoidsElement`）の Body を、
//! なければ Reference を、要素の局所の座標に移して、順に引く。要素が部品の組なら、部品ごとに引く。開口の要素は、
//! それだけでは評価しない。行は TSV。終わりに、`#` で始まる集計の行を書く。メモリは、確保したバイト数の山を数える。

// 速さを測る道具なので、時計を読む（コアの規則の外。clippy.toml）
#![allow(clippy::disallowed_methods)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Limits};
use zumen_bench_kernels::ifc::{Miss, Model, inverse, kinds};
use zumen_bench_kernels::{candidates, evaluate_with, fingerprint};
use zumen_building::{Building, ElementRef, Opening};
use zumen_geom3d::{
    BooleanOp, Failure, Kernel, Mesh, MeshSection, Plane, Section, Shape, Tolerance, Transform,
};
use zumen_ifc::Ifc;
use zumen_sdk::{Codec, Lift};

/// 確保しているバイト数と、その山を数える
struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(n: usize) {
    let now = CURRENT.fetch_add(n, Ordering::Relaxed) + n;
    PEAK.fetch_max(now, Ordering::Relaxed);
}

fn shrink(n: usize) {
    CURRENT.fetch_sub(n, Ordering::Relaxed);
}

// SAFETY: 確保と解放は System に任せ、数を数えるだけ
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            grow(layout.size());
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            grow(layout.size());
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        shrink(layout.size());
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            if new_size >= layout.size() {
                grow(new_size - layout.size());
            } else {
                shrink(layout.size() - new_size);
            }
        }
        p
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// 山を今の量に戻し、今の量を返す
fn reset_peak() -> usize {
    let now = CURRENT.load(Ordering::Relaxed);
    PEAK.store(now, Ordering::Relaxed);
    now
}

fn peak_since(base: usize) -> usize {
    PEAK.load(Ordering::Relaxed).saturating_sub(base)
}

/// 1 m を、ファイルの長さの単位で表した値
fn metre(building: &Building) -> f64 {
    let Some(unit) = building
        .document
        .units
        .iter()
        .find(|u| u.kind == "LENGTHUNIT")
    else {
        return 1.0;
    };
    let prefix = match unit.prefix.as_deref() {
        Some("MILLI") => 1e-3,
        Some("CENTI") => 1e-2,
        Some("DECI") => 1e-1,
        Some("KILO") => 1e3,
        _ => 1.0,
    };
    1.0 / (prefix * unit.scale.unwrap_or(1.0))
}

fn transform(t: zumen_building::Transform) -> Transform {
    Transform(t.0)
}

/// 組の形（写像の中の組を含む）なら、その項目
fn parts(shape: &Shape) -> Option<Vec<Shape>> {
    match shape {
        Shape::Group(shapes) => Some(shapes.clone()),
        Shape::Mapped { transform, shape } => parts(shape).map(|shapes| {
            shapes
                .into_iter()
                .map(|s| Shape::Mapped {
                    transform: *transform,
                    shape: Box::new(s),
                })
                .collect()
        }),
        _ => None,
    }
}

/// 評価の結果の名前。閉じたメッシュは `ok`。閉じていなくても、組の項目がどれも閉じていれば `parts`（扉の枠と戸の
/// ように、閉じた部品を並べた形）。ほかは `open`
fn status(
    result: &Result<Result<Mesh, Failure>, ()>,
    shape: &Shape,
    kernel: &dyn Kernel,
    tolerance: Tolerance,
) -> String {
    match result {
        Err(()) => "panic".into(),
        Ok(Ok(mesh)) => {
            let welded = mesh.welded();
            // 両面で閉じた厚さのない面（中野区の面の集合など）は、閉じていても体積がない
            let solid = welded.bounds().is_some_and(|[lo, hi]| {
                let size = zumen_geom3d::length(zumen_geom3d::sub(hi, lo));
                welded.volume().abs() > 1e-9 * size * size * size
            });
            if welded.is_closed() {
                if solid { "ok" } else { "flat" }.into()
            } else if parts(shape).is_some_and(|shapes| {
                shapes.iter().all(|s| {
                    catch_unwind(AssertUnwindSafe(|| evaluate_with(kernel, s, tolerance)))
                        .is_ok_and(|r| r.is_ok_and(|m| m.welded().is_closed()))
                })
            }) {
                "parts".into()
            } else if watertight(&welded) {
                if solid { "touching" } else { "flat" }.into()
            } else {
                "open".into()
            }
        }
        Ok(Err(Failure::Unsupported(kind))) => format!("unsupported:{kind}"),
        Ok(Err(Failure::Invalid(what))) => format!("invalid:{what}"),
        Ok(Err(Failure::Failed(why))) => format!("failed:{why}"),
        Ok(Err(Failure::Stopped(_))) => "stopped".into(),
    }
}

/// 向きつきの辺のどれも、逆向きの辺と同じ回数だけ使われる（水を漏らさない）。閉じた部分どうしが辺で接すると、
/// 点を 1 つにしたメッシュは [`Mesh::is_closed`] にならないが、これにはなる
fn watertight(mesh: &Mesh) -> bool {
    let mut count: BTreeMap<(u32, u32), i64> = BTreeMap::new();
    for t in &mesh.triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *count.entry((a.min(b), a.max(b))).or_default() += if a < b { 1 } else { -1 };
        }
    }
    !count.is_empty() && count.values().all(|n| *n == 0)
}

fn timed(f: impl FnOnce() -> Result<Mesh, Failure>) -> (Result<Result<Mesh, Failure>, ()>, u128) {
    let start = Instant::now();
    let result = catch_unwind(AssertUnwindSafe(f)).map_err(|_| ());
    (result, start.elapsed().as_micros())
}

/// 箱の高さの中ほどを、局所の水平の平面で切る。輪の数と、開いた鎖の数
fn section(mesh: &Mesh) -> String {
    let Some([lo, hi]) = mesh.bounds() else {
        return String::new();
    };
    let plane = Plane {
        origin: [0.0, 0.0, (lo[2] + hi[2]) / 2.0],
        normal: [0.0, 0.0, 1.0],
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    match MeshSection.section(mesh, &plane, &mut cx) {
        Ok(rings) => {
            let open = rings.iter().filter(|r| r.len() < 3).count();
            format!("{}/{open}", rings.len())
        }
        Err(error) => format!("{error:?}"),
    }
}

fn fnv(h: &mut u64, v: u64) {
    for b in v.to_le_bytes() {
        *h ^= u64::from(b);
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

/// 試料ごと・候補ごとの集計
struct Tally {
    elements: u64,
    miss: BTreeMap<String, u64>,
    body: BTreeMap<String, u64>,
    hosts: u64,
    full: BTreeMap<String, u64>,
    micros_body: u128,
    micros_full: u128,
    peak: usize,
    fingerprint: u64,
}

impl Tally {
    fn new() -> Self {
        Self {
            elements: 0,
            miss: BTreeMap::new(),
            body: BTreeMap::new(),
            hosts: 0,
            full: BTreeMap::new(),
            micros_body: 0,
            micros_full: 0,
            peak: 0,
            fingerprint: 0xcbf2_9ce4_8422_2325,
        }
    }
}

fn counts(map: &BTreeMap<String, u64>) -> String {
    map.iter()
        .map(|(k, n)| format!("{k}={n}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// 開口を引く前の形と、引いた形
struct Target {
    instance: u64,
    class: String,
    items: String,
    openings: usize,
    body: Shape,
    full: Option<Shape>,
}

fn measure(path: &str, candidates: &[(&'static str, Box<dyn Kernel>)]) -> Vec<String> {
    let base = reset_peak();
    let start = Instant::now();
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Ifc::read(&bytes, &mut cx).unwrap_or_else(|e| panic!("{path}: {e:?}"));
    let read_micros = start.elapsed().as_micros();
    // 読んだバイト列と L1 の頂点。lift の頂点は、これに建物モデルを足したもの
    let read_peak = peak_since(base);
    let building: Building =
        Lift::<Building>::lift(&file, &mut cx).unwrap_or_else(|e| panic!("{path}: {e:?}"));
    let lift_micros = start.elapsed().as_micros() - read_micros;
    let lift_peak = peak_since(base);

    let metre = metre(&building);
    let model = Model::new(&file, metre);
    let tolerance = Tolerance {
        chord: 0.001 * metre,
        length: 1e-9 * metre,
    };
    let c = &building.components;
    let count = building.elements().len();
    let element = |at: usize| ElementRef(at as u32);
    let mut voids: BTreeMap<u32, Vec<ElementRef>> = BTreeMap::new();
    for at in 0..count {
        if let Some(Opening::Voids(host)) = c.opening.get(element(at)) {
            voids.entry(host.0).or_default().push(element(at));
        }
    }

    // 写す
    let before_convert = CURRENT.load(Ordering::Relaxed);
    let convert_start = Instant::now();
    let mut targets = Vec::new();
    let mut miss: BTreeMap<String, u64> = BTreeMap::new();
    let mut rows = Vec::new();
    for at in 0..count {
        let e = element(at);
        let Some(instance) = building.elements()[at].source().instance else {
            continue;
        };
        let class = c
            .class
            .get(e)
            .map(|k| k.original.clone())
            .unwrap_or_default();
        if class.to_ascii_uppercase().starts_with("IFCOPENING") {
            continue;
        }
        let body = match model.body(instance) {
            Ok(Some(shape)) => shape,
            Ok(None) => continue,
            Err(why) => {
                let key = match &why {
                    Miss::Item(kind) => kind.clone(),
                    Miss::Invalid(what) => format!("invalid:{what}"),
                };
                *miss.entry(key.clone()).or_default() += 1;
                rows.push(format!("{path}\t{instance}\t{class}\t\t\t\tmiss:{key}"));
                continue;
            }
        };
        let mut items = Vec::new();
        kinds(&body, &mut items);
        let openings = voids.get(&e.0).cloned().unwrap_or_default();
        let full = if openings.is_empty() {
            None
        } else {
            let host = building
                .world_placement(e, &mut cx)
                .ok()
                .flatten()
                .map(transform);
            let mut cuts = Vec::new();
            let mut lost = None;
            for o in &openings {
                let opening = building.elements()[o.0 as usize].source().instance;
                let placed = building
                    .world_placement(*o, &mut cx)
                    .ok()
                    .flatten()
                    .map(transform);
                let relative = match (host.as_ref().and_then(inverse), placed) {
                    (Some(inv), Some(p)) => p.then(&inv),
                    _ => {
                        lost = Some("placement".to_owned());
                        continue;
                    }
                };
                match opening.map(|i| model.opening(i)) {
                    Some(Ok(Some(cut))) => cuts.push(Shape::Mapped {
                        transform: relative,
                        shape: Box::new(cut),
                    }),
                    Some(Err(Miss::Item(kind))) => lost = Some(kind),
                    _ => lost = Some("opening".to_owned()),
                }
            }
            if let Some(kind) = lost {
                *miss.entry(format!("opening:{kind}")).or_default() += 1;
            }
            // 組（壁の層ごとの部品など）は、部品ごとに引く。接した部品を 1 つのメッシュにすると、閉じた形にならない
            let mut pieces: Vec<Shape> = parts(&body)
                .unwrap_or_else(|| vec![body.clone()])
                .into_iter()
                .map(|piece| {
                    cuts.iter().fold(piece, |shape, cut| Shape::Boolean {
                        op: BooleanOp::Difference,
                        first: Box::new(shape),
                        second: Box::new(cut.clone()),
                    })
                })
                .collect();
            Some(if pieces.len() == 1 {
                pieces.remove(0)
            } else {
                Shape::Group(pieces)
            })
        };
        targets.push(Target {
            instance,
            class,
            items: items.join(","),
            openings: openings.len(),
            body,
            full,
        });
    }
    let convert_micros = convert_start.elapsed().as_micros();
    // 形を写して増えたメモリ（写した形と、行の文字列）
    let convert_bytes = CURRENT
        .load(Ordering::Relaxed)
        .saturating_sub(before_convert);

    let mut summary = vec![format!(
        "{path}\tbytes={}\tread_micros={read_micros}\tlift_micros={lift_micros}\tread_peak={read_peak}\tlift_peak={lift_peak}\tconvert_micros={convert_micros}\tconvert_bytes={convert_bytes}\ttargets={}\tmiss={}",
        bytes.len(),
        targets.len(),
        counts(&miss)
    )];
    for (key, kernel) in candidates {
        let mut tally = Tally::new();
        tally.elements = targets.len() as u64;
        tally.miss = miss.clone();
        let base = reset_peak();
        for target in &targets {
            let (body, micros_body) =
                timed(|| evaluate_with(kernel.as_ref(), &target.body, tolerance));
            let body_status = status(&body, &target.body, kernel.as_ref(), tolerance);
            *tally.body.entry(body_status.clone()).or_default() += 1;
            tally.micros_body += micros_body;
            let (full_status, micros_full, last) = match &target.full {
                Some(shape) => {
                    let (full, micros) = timed(|| evaluate_with(kernel.as_ref(), shape, tolerance));
                    let s = status(&full, shape, kernel.as_ref(), tolerance);
                    tally.hosts += 1;
                    *tally.full.entry(s.clone()).or_default() += 1;
                    tally.micros_full += micros;
                    (s, micros, full)
                }
                None => ("-".to_owned(), 0, body),
            };
            let (closed, volume, triangles, print, cut) = match &last {
                Ok(Ok(mesh)) => {
                    let welded = mesh.welded();
                    let closed = welded.is_closed();
                    let print = fingerprint(mesh);
                    fnv(&mut tally.fingerprint, print);
                    (
                        closed.to_string(),
                        format!("{:.6e}", welded.volume()),
                        mesh.triangles.len().to_string(),
                        format!("{print:016x}"),
                        if closed { section(mesh) } else { String::new() },
                    )
                }
                _ => Default::default(),
            };
            rows.push(format!(
                "{path}\t{}\t{}\t{}\t{}\t{key}\t{body_status}\t{full_status}\t{closed}\t{volume}\t{triangles}\t{micros_body}\t{micros_full}\t{print}\t{cut}",
                target.instance, target.class, target.items, target.openings
            ));
        }
        tally.peak = peak_since(base);
        summary.push(format!(
            "{path}\t{key}\telements={}\tbody={}\thosts={}\tfull={}\tmicros_body={}\tmicros_full={}\tpeak={}\tfingerprint={:016x}",
            tally.elements,
            counts(&tally.body),
            tally.hosts,
            counts(&tally.full),
            tally.micros_body,
            tally.micros_full,
            tally.peak,
            tally.fingerprint
        ));
    }
    for row in rows {
        println!("{row}");
    }
    summary
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let all = candidates();
    let want = match args.first() {
        Some(first) if all.iter().any(|(key, _)| key == first) => Some(args.remove(0)),
        _ => None,
    };
    let lift_only = want.is_none() && args.first().is_some_and(|a| a == "lift");
    if lift_only {
        args.remove(0);
    }
    let chosen: Vec<_> = all
        .into_iter()
        .filter(|(key, _)| !lift_only && want.as_deref().is_none_or(|w| w == *key))
        .collect();
    // パニックの文は、行の `panic` で足りるので書かない
    std::panic::set_hook(Box::new(|_| {}));
    println!(
        "file\tinstance\tclass\titems\topenings\tcandidate\tbody\tfull\tclosed\tvolume\ttriangles\tmicros_body\tmicros_full\tfingerprint\tsection"
    );
    let mut summary = Vec::new();
    for path in &args {
        summary.extend(measure(path, &chosen));
    }
    for line in summary {
        println!("# {line}");
    }
}
