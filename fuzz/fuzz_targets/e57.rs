//! E57 の fuzz の target（#501）。読んで点群の領域モデルに移すまでが、小さい予算の中で止まり、
//! 2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_pointcloud::PointCloud;

fuzz_target!(|data: &[u8]| {
    if let Err(message) = zumen_sdk::testkit::survives_lift::<zumen_e57::E57, PointCloud>(data) {
        panic!("{message}");
    }
});
