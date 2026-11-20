//! 松浦市の SFC の実寸の空間で 5 設定を比較する。図面は同梱しない。
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;
use zumen_bench_chunks::{POLICIES, audit, bytes, prepare_matsuura, render_with_tolerance};

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [input, output] = args.as_slice() else {
        return Err("使い方: sfc <図郭 1 または 2 の SFC> <照合用の出力先>".into());
    };
    if !matches!(
        fs::metadata(input).map_err(|e| e.to_string())?.len(),
        17_975_570 | 57_227_256
    ) {
        return Err("公開図郭のサイズと違う".into());
    }
    let sheet = prepare_matsuura(&fs::read(input).map_err(|e| e.to_string())?)?;
    let output = Path::new(output);
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    for (index, policy) in POLICIES.iter().enumerate() {
        render_with_tolerance(index, None);
        let expected = bytes();
        let metrics = audit();
        let mut times = [0.0; 3];
        for time in &mut times {
            #[allow(clippy::disallowed_methods, reason = "比較するシーンの生成時間を測る")]
            let start = Instant::now();
            render_with_tolerance(index, None);
            *time = start.elapsed().as_secs_f64() * 1000.0;
            if bytes() != expected || audit() != metrics {
                return Err("同じ設定でシーンまたは監査結果が変わった".into());
            }
        }
        times.sort_by(f64::total_cmp);
        fs::write(output.join(format!("sfc-{sheet}-{policy}.bin")), expected)
            .map_err(|e| e.to_string())?;
        let metrics = std::str::from_utf8(&metrics).map_err(|e| e.to_string())?;
        println!(
            "{{\"sheet\":{sheet},\"policy\":\"{policy}\",\"median_ms\":{},\"metrics\":{metrics}}}",
            times[1]
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
