//! 合成のシーンを 4 つの形式で符号化し、ファイルに書いて、大きさと時間を出す。
//!
//! 使い方: `encode <出力のディレクトリ> [折れ線の数]`

use std::time::Instant;
use zumen_bench_transfer as bench;

/// 形式の名前と符号化の関数。
type Format = (&'static str, fn(&bench::Scene) -> Vec<u8>);

// 測定の道具なので、核では禁じている時計を使う。
#[allow(clippy::disallowed_methods)]
fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| "out".to_owned());
    let polylines = args.next().and_then(|v| v.parse().ok()).unwrap_or(227_000);
    std::fs::create_dir_all(&dir)?;
    let scene = bench::synthetic(polylines, 1);
    println!(
        "chunks\t{}\npolylines\t{}\nvertices\t{}",
        scene.chunks(),
        scene.polylines(),
        scene.vertices()
    );
    let formats: [Format; 4] = [
        ("json", bench::json),
        ("columnar", bench::columnar),
        ("flatbuffers", bench::flatbuffers),
        ("arrow", bench::arrow),
    ];
    println!("format\tbytes\tencode_ms");
    for (name, encode) in formats {
        let mut times = Vec::new();
        let mut bytes = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            bytes = encode(&scene);
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(f64::total_cmp);
        println!("{name}\t{}\t{:.1}", bytes.len(), times[2]);
        std::fs::write(format!("{dir}/{name}.bin"), &bytes)?;
    }
    Ok(())
}
