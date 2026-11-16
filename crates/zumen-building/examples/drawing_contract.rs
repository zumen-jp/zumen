//! 合成した切断・隠れ線・出所・予算停止を、ネイティブと WASI で照合する。
#[path = "../tests/support/drawing.rs"]
mod drawing_fixtures;
#[path = "../tests/support/drawing_record.rs"]
mod drawing_record;
#[path = "../tests/support/evaluation.rs"]
mod fixtures;
use std::io::Write;
fn main() -> std::io::Result<()> {
    std::io::stdout().write_all(&drawing_record::record())
}
