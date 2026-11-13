//! LAS・LAZ の fuzz の target（#501・#510）。展開を含む読み取りと点群の領域モデルへの移しまでが、小さい予算の中で
//! 止まり、2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_pointcloud::PointCloud;

fuzz_target!(|data: &[u8]| {
    if let Err(message) = zumen_sdk::testkit::survives_lift::<zumen_las::Las, PointCloud>(data) {
        panic!("{message}");
    }
});
