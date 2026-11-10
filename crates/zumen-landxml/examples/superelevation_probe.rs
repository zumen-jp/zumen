//! 合成入力の片勾配と予算をビット列で出力し、ネイティブと wasm で比較する（#468）。

#[path = "../tests/superelevation_contract.rs"]
mod contract;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;

    let snapshot = contract::verify_contracts().map_err(std::io::Error::other)?;
    std::io::stdout().lock().write_all(snapshot.as_bytes())?;
    Ok(())
}
