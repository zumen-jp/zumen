//! 作図プロファイルの結果と停止を、各環境で照らす。
#[path = "../tests/support/drawing.rs"]
mod input;
#[path = "../tests/support/drawing_record.rs"]
mod record;
use std::io::Write;
fn main() -> std::io::Result<()> {
    let bytes = record::record();
    if std::env::args().any(|v| v == "--hash") {
        writeln!(std::io::stdout(), "{}", blake3::hash(&bytes).to_hex())
    } else {
        std::io::stdout().write_all(&bytes)
    }
}
