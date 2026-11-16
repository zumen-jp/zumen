//! 地物の図面と 3D シーン・予算・診断の記録を、ネイティブと wasm で比較する。
#[allow(dead_code)]
#[path = "../tests/visual.rs"]
mod contract;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    std::io::stdout().lock().write_all(&contract::snapshot())?;
    Ok(())
}
