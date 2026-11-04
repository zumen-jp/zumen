//! 電子納品の成果品（ZIP のアーカイブと管理ファイル）の fuzz の target（#501）。アーカイブを読んで管理ファイルと写真を読み、
//! 照合するまでが、小さい予算の中で止まり、2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_sdk::Error;

fuzz_target!(|data: &[u8]| {
    let checked = zumen_sdk::testkit::survives_with(data, |src, cx| {
        let package = zumen_delivery::read_zip(src, cx)?;
        zumen_delivery::check(&package, &[], cx).map_err(Error::Stopped)
    });
    if let Err(message) = checked {
        panic!("{message}");
    }
});
