//! LAS・LAZ の fuzz の target（#501）。読んで点群の領域モデルに移すまでが、小さい予算の中で止まり、
//! 2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_pointcloud::PointCloud;

fuzz_target!(|data: &[u8]| {
    // LAZ は、展開を任せる laz 0.13.0 の中のパニックを直すまで飛ばす（#510）。LAZ かは、LAS 1.4 R15 のヘッダー
    // （Public Header Block）の表の 104 バイト目、点のレコードの形式（Point Data Record Format）で決める。LAZ 1.4
    // （OGC 24-070r1）の 6.2・6.3 は、LAZ ではこの値に 128 を足す（ビット 7 を立てる）とする。ビット 6 も見るのは、
    // 読み取り（zumen-las のヘッダー）と同じ条件にするため
    if data.get(104).is_some_and(|id| id & 0xc0 != 0) {
        return;
    }
    if let Err(message) = zumen_sdk::testkit::survives_lift::<zumen_las::Las, PointCloud>(data) {
        panic!("{message}");
    }
});
