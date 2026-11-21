//! 公開 DWG のモデル空間と JWW の用紙で、塊の 5 設定を照らす。
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use zumen_acad::Dwg;
use zumen_bench_chunks::{POLICIES, prepare_cad};
use zumen_drawing::{Origin, SheetId, SpaceId};
use zumen_jww::Jww;
mod policy_timing;

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [input, output] = args.as_slice() else {
        return Err("使い方: cad <入力 DWG または JWW> <シーンの出力先>".into());
    };
    let input = fs::read(input).map_err(|error| error.to_string())?;
    let diagnostics = if input.starts_with(b"JwwData.") {
        prepare_cad::<Jww>(&input, Origin::Sheet(SheetId(0)))?
    } else if input.starts_with(b"AC10") {
        prepare_cad::<Dwg>(&input, Origin::Space(SpaceId(0)))?
    } else {
        return Err("DWG または JWW を指定する".into());
    };
    drop(input);
    let output = Path::new(output);
    fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let diagnostics = diagnostics
        .iter()
        .map(|d| format!("{}:{}", d.code().name(), d.count()))
        .collect::<Vec<_>>()
        .join(",");
    for (index, policy) in POLICIES.iter().enumerate() {
        let (median, expected, metrics) = policy_timing::measure(index, Some(0.01))?;
        fs::write(output.join(format!("{policy}.bin")), expected).map_err(|e| e.to_string())?;
        println!(
            "{{\"policy\":\"{policy}\",\"median_ms\":{median},\"input_diagnostics\":\"{diagnostics}\",\"metrics\":{metrics}}}"
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
