//! DXF の fuzz の target（#398・#501）。読んで図面モデルに移すまでが、小さい予算の中で止まり、
//! 2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_drawing::Drawing2D;

fuzz_target!(|data: &[u8]| {
    if let Err(message) = zumen_sdk::testkit::survives_lift::<zumen_acad::Dxf, Drawing2D>(data) {
        panic!("{message}");
    }
});
