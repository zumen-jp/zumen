//! 数値・三角形・辺・ID・診断・停止位置を、各環境で同じ指紋へ照合する。

#[path = "support/evaluation_record.rs"]
mod evaluation_record;
#[path = "support/evaluation.rs"]
mod fixtures;

#[test]
fn evaluation_bytes_match_on_every_platform() {
    let bytes = evaluation_record::record();
    let hash = blake3::hash(&bytes).to_hex();
    assert_eq!(
        hash.as_str(),
        include_str!("evaluation.blake3").trim(),
        "bytes={}",
        bytes.len()
    );
}
