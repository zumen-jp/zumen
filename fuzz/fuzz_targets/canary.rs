//! 一時の target（#647 の確かめだけに使い、マージの前に消す）。`ZC` で始まる入力で、識別用の文字列を含めて
//! パニックする。公開のログ・注釈・summary・artifact・キャッシュに、入力と誤りの文が出ないことを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() >= 2 && data[0] == b'Z' && data[1] == b'C' {
        panic!("ZUMEN-CANARY-647 {}", data.len());
    }
    let read = zumen_sdk::testkit::survives_with(data, |_, _| Ok(()));
    if let Err(message) = read {
        panic!("{message}");
    }
});
