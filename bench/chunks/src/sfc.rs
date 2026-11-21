//! 松浦市の SFC の実寸の空間で 5 設定を比較する。図面は同梱しない。
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use zumen_bench_chunks::{POLICIES, prepare_matsuura};
mod policy_timing;

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
        let (median, expected, metrics) = policy_timing::measure(index, None)?;
        fs::write(output.join(format!("sfc-{sheet}-{policy}.bin")), expected)
            .map_err(|e| e.to_string())?;
        println!(
            "{{\"sheet\":{sheet},\"policy\":\"{policy}\",\"median_ms\":{median},\"metrics\":{metrics}}}"
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
