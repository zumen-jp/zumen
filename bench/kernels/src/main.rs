//! 候補ごとに合成の形を評価し、1 行ずつ書く。
//!
//! 使い方: `compare [候補] [組]`。候補は own・manifold-rust・csgrs（省けばすべて）。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use zumen_bench_kernels::{candidates, evaluate, fingerprint};
use zumen_geom3d::Failure;
use zumen_geom3d::cases::{self, Expect};

fn section(mesh: &zumen_geom3d::Mesh) -> String {
    use zumen_geom3d::{MeshSection, Plane, Section};
    let Some([lo, hi]) = mesh.bounds() else {
        return String::new();
    };
    let plane = Plane {
        origin: [0.0, 0.0, (lo[2] + hi[2]) / 2.0 + 1e-7],
        normal: [0.0, 0.0, 1.0],
    };
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let mut cx = zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    match MeshSection.section(mesh, &plane, &mut cx) {
        Ok(rings) => {
            let area: f64 = rings
                .iter()
                .map(|r| {
                    r.iter()
                        .zip(r.iter().cycle().skip(1))
                        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
                        .sum::<f64>()
                        / 2.0
                })
                .sum();
            let open = rings.iter().filter(|r| r.len() < 3).count();
            format!("{}/{open}/{area:.6}", rings.len())
        }
        Err(error) => format!("{error:?}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let want = |i: usize, name: &str| args.get(i).is_none_or(|a| a == name);
    let candidates = candidates();
    println!(
        "candidate\tcase\tstatus\tvolume_error\tbounds\tclosed\ttriangles\tfingerprint\tmicros\tsection"
    );
    for (key, kernel) in &candidates {
        if !want(1, key) {
            continue;
        }
        for case in cases::all() {
            if !want(2, case.name) {
                continue;
            }
            let start = Instant::now();
            let result = catch_unwind(AssertUnwindSafe(|| evaluate(kernel.as_ref(), &case.shape)));
            let micros = start.elapsed().as_micros();
            // 箱の高さの中ほどを、水平の平面で切る。輪の数（開いた鎖の数）と面積
            let section = match &result {
                Ok(Ok(mesh)) => section(mesh),
                _ => String::new(),
            };
            let row = match (result, &case.expect) {
                (Err(_), _) => "panic\t\t\t\t\t".to_string(),
                (Ok(Err(Failure::Unsupported(kind))), _) => format!("unsupported:{kind}\t\t\t\t\t"),
                (Ok(Err(Failure::Invalid(what))), Expect::Invalid) => {
                    format!("rejected:{what}\t\t\t\t\t")
                }
                (Ok(Err(Failure::Failed(why))), Expect::Rejected) => {
                    format!("rejected:{why}\t\t\t\t\t")
                }
                (Ok(Err(failure)), _) => format!("failed:{failure:?}\t\t\t\t\t"),
                (Ok(Ok(mesh)), Expect::Rejected) => {
                    let welded = mesh.welded();
                    let status = if welded.is_closed() || welded.triangles.is_empty() {
                        "closed"
                    } else {
                        "open"
                    };
                    format!(
                        "accepted_{status}\t\t\t{}\t{}\t{:016x}",
                        welded.is_closed(),
                        mesh.triangles.len(),
                        fingerprint(&mesh)
                    )
                }
                (Ok(Ok(mesh)), Expect::Invalid) => format!(
                    "accepted_invalid\t\t\t\t{}\t{:016x}",
                    mesh.triangles.len(),
                    fingerprint(&mesh)
                ),
                (
                    Ok(Ok(mesh)),
                    Expect::Solid {
                        volume,
                        relative,
                        bounds,
                    },
                ) => {
                    let welded = mesh.welded();
                    let error = (welded.volume() - volume).abs() / volume;
                    let got = welded.bounds();
                    // 曲面は、弦の誤差の分だけ箱が小さくなる
                    let slack = if *relative > 1e-6 {
                        2.0 * cases::CHORD
                    } else {
                        1e-6
                    };
                    let near = got.is_some_and(|g| {
                        (0..2).all(|i| (0..3).all(|k| (g[i][k] - bounds[i][k]).abs() <= slack))
                    });
                    let status = if error <= *relative && near && welded.is_closed() {
                        "ok"
                    } else {
                        "wrong"
                    };
                    format!(
                        "{status}\t{error:.2e}\t{}\t{}\t{}\t{:016x}",
                        if near { "ok" } else { "off" },
                        welded.is_closed(),
                        mesh.triangles.len(),
                        fingerprint(&mesh)
                    )
                }
            };
            println!("{key}\t{}\t{row}\t{micros}\t{section}", case.name);
        }
    }
}
