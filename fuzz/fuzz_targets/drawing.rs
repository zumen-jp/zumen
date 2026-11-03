//! 図面モデルのバイナリ（決定 0247）の fuzz の target（#501）。読み取りが、小さい予算の中で止まり、2 回で同じ結果に
//! なることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_drawing::binary;
use zumen_sdk::Error;

fuzz_target!(|data: &[u8]| {
    let read = zumen_sdk::testkit::survives_with(data, |src, cx| {
        binary::read(src, cx).map_err(|error| match error {
            binary::Error::Stopped(stop) => Error::Stopped(stop),
            binary::Error::Invalid => Error::Invalid,
            binary::Error::Unsupported => Error::Unsupported,
        })
    });
    if let Err(message) = read {
        panic!("{message}");
    }
});
