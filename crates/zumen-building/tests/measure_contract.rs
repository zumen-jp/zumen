//! 計測値・丸め・根拠・診断・予算停止の全バイトを各環境で照合する。
#![cfg(feature = "measure")]
#[path = "support/evaluation.rs"]
mod fixtures;
#[path = "support/measure.rs"]
#[allow(dead_code, reason = "数量の試験と共有する固定入力")]
mod input;
#[path = "support/measure_record.rs"]
mod measure_record;

#[test]
fn measurement_bytes_match_on_every_platform() {
    let bytes = measure_record::record();
    assert_eq!(
        blake3::hash(&bytes).to_hex().as_str(),
        include_str!("measure.blake3").trim(),
        "bytes={}",
        bytes.len()
    );
}
