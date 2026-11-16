//! 建物プロファイルの結果の同じバイト列を照らす。
#![cfg(feature = "building")]
#[path = "support/building.rs"]
mod input;
#[path = "support/building_record.rs"]
mod record;
#[test]
fn profile_bytes_match_on_every_platform() {
    assert_eq!(
        blake3::hash(&record::record()).to_hex().as_str(),
        include_str!("building.blake3").trim()
    );
}
