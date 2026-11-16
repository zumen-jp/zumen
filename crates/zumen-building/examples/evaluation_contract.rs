//! 合成形状・建物・予算停止の全バイトを、ネイティブと WASI で照合する。

#[path = "../tests/support/evaluation_record.rs"]
mod evaluation_record;
#[path = "../tests/support/evaluation.rs"]
mod fixtures;

use std::io::Write;

fn main() -> std::io::Result<()> {
    std::io::stdout().write_all(&evaluation_record::record())
}
