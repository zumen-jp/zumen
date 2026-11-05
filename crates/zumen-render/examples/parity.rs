//! ネイティブと wasm の出力・診断・予算・停止を照合するテスト専用のモジュール。

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;

use std::sync::OnceLock;

static TRANSCRIPT: OnceLock<Vec<u8>> = OnceLock::new();

/// 固定した合成ケースの結果の先頭。変更しないメモリを Node.js が読む
#[allow(unsafe_code)] // テストだけの名前を export する属性。ポインターの参照は行わない。
#[unsafe(no_mangle)]
pub extern "C" fn transcript_ptr() -> *const u8 {
    TRANSCRIPT.get_or_init(fixtures::transcript).as_ptr()
}

/// 結果の長さ
#[allow(unsafe_code)] // 上と同じ、テストだけの export。
#[unsafe(no_mangle)]
pub extern "C" fn transcript_len() -> usize {
    TRANSCRIPT.get_or_init(fixtures::transcript).len()
}
