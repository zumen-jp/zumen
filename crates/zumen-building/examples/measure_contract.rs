//! 数量と十進の丸め・判定の根拠・予算停止を、ネイティブと WASI で照合する。
#[path = "../tests/support/evaluation.rs"]
mod fixtures;
#[path = "../tests/support/measure.rs"]
#[allow(dead_code, reason = "数量の試験と共有する固定入力")]
mod input;
#[path = "../tests/support/measure_record.rs"]
mod measure_record;
use std::io::Write;
fn main() -> std::io::Result<()> {
    std::io::stdout().write_all(&measure_record::record())
}
