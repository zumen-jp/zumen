//! #468 の保持値と予算の記録を、ネイティブと wasm で比較する。
#[path = "../tests/values_contract.rs"]
mod contract;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    std::io::stdout()
        .lock()
        .write_all(contract::verify_contracts().as_bytes())?;
    Ok(())
}
