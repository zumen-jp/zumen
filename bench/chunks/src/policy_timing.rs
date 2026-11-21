use std::time::Instant;
use zumen_bench_chunks::{audit, bytes, render_with_tolerance};

pub fn measure(index: usize, tolerance: Option<f64>) -> Result<(f64, Vec<u8>, String), String> {
    render_with_tolerance(index, tolerance);
    let expected = bytes();
    let metrics = audit();
    let mut times = [0.0; 3];
    for time in &mut times {
        #[allow(clippy::disallowed_methods, reason = "シーンを作る時間を計測する")]
        let start = Instant::now();
        render_with_tolerance(index, tolerance);
        *time = start.elapsed().as_secs_f64() * 1000.0;
        if bytes() != expected || audit() != metrics {
            return Err("同じ設定でシーンまたは監査結果が変わった".into());
        }
    }
    times.sort_by(f64::total_cmp);
    Ok((
        times[1],
        expected,
        String::from_utf8(metrics).map_err(|e| e.to_string())?,
    ))
}
