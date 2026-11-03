//! LAS・LAZ の fuzz の target（#501）。読んで点群の領域モデルに移すまでが、小さい予算の中で止まり、
//! 2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_pointcloud::PointCloud;

fuzz_target!(|data: &[u8]| {
    // LAZ は、展開を任せる laz 0.13.0 の中のパニックを直すまで飛ばす（#510）。LAZ かは、LAS 1.4 R15 の見出し
    // （Public Header Block）の表の 104 バイト目、点のレコードの形式（Point Data Record Format）の上の 2 ビットで
    // 決める。上の 2 ビットを LAZ の印にするのは LASzip の決まりで、読み取りと同じ（docs/formats/las.md の
    // 「見出しと VLR」）
    if data.get(104).is_some_and(|id| id & 0xc0 != 0) {
        return;
    }
    if let Err(message) = zumen_sdk::testkit::survives_lift::<zumen_las::Las, PointCloud>(data) {
        panic!("{message}");
    }
});
