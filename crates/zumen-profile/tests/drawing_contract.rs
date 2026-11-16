//! 作図プロファイルの同じバイト列を全環境で確かめる。
#![cfg(feature = "building-drawing")]
#[path = "support/drawing.rs"]
mod input;
#[path = "support/drawing_record.rs"]
mod record;
#[test]
fn drawing_bytes_match_on_every_platform() {
    assert_eq!(
        blake3::hash(&record::record()).to_hex().as_str(),
        include_str!("drawing.blake3").trim()
    );
}
