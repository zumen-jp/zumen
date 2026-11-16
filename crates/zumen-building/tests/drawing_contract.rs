//! 図面への切り出し・出所・診断・停止の全バイトを各環境で照合する。
#![cfg(feature = "drawing")]
#[path = "support/drawing.rs"]
mod drawing_fixtures;
#[path = "support/drawing_record.rs"]
mod drawing_record;
#[path = "support/evaluation.rs"]
mod fixtures;

#[test]
fn drawing_bytes_match_on_every_platform() {
    let bytes = drawing_record::record();
    assert_eq!(
        blake3::hash(&bytes).to_hex().as_str(),
        include_str!("drawing.blake3").trim(),
        "bytes={}",
        bytes.len()
    );
}
