//! TKY2JGD・PatchJGD の形の格子（座標の補正量）の fuzz の target（#501）。読み取りが、小さい予算の中で止まり、2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Err(message) = zumen_sdk::testkit::survives_with(data, zumen_crs::ShiftGrid::read) {
        panic!("{message}");
    }
});
