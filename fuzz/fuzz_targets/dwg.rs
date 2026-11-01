//! DWG の読み取りの fuzz の target（#398）。小さい予算の中で止まり、2 回読んで同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Err(message) = zumen_sdk::testkit::survives::<zumen_acad::Dwg>(data) {
        panic!("{message}");
    }
});
