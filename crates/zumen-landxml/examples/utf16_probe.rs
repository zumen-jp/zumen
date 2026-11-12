//! UTF-16 の原本と公開値を、ネイティブ・wasm・独立した XML 実装で比べる（#468）。

#[path = "../tests/utf16_contract.rs"]
mod contract;
#[path = "../../../xtask/verify/xml/legacy_probe.rs"]
mod legacy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let output = match std::env::args().nth(1).as_deref() {
        Some("--external") => contract::external_cases(),
        Some("--legacy") => legacy::snapshot(),
        _ => contract::verify_contracts(),
    }
    .map_err(std::io::Error::other)?;
    std::io::stdout().lock().write_all(output.as_bytes())?;
    Ok(())
}
