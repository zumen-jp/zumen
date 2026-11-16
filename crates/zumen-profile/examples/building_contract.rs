//! 建物プロファイルの全結果と予算を、ネイティブと wasm で照らす。
#[path = "../tests/support/building.rs"]
mod input;
#[path = "../tests/support/building_record.rs"]
mod record;
use std::io::Write;
fn main() -> std::io::Result<()> {
    std::io::stdout().write_all(&record::record())
}
