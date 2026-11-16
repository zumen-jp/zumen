//! ZMS3 の列・参照・数値・予算停止を任意のバイト列で確かめる。
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    zumen_sdk::testkit::survives_with(data, |data, cx| {
        zumen_scene3d::read(data, cx).map(|_| ()).map_err(|error| match error {
            zumen_scene3d::Error::Stopped(stop) => zumen_sdk::Error::Stopped(stop),
            zumen_scene3d::Error::Invalid(_) => zumen_sdk::Error::Invalid,
        })
    }).unwrap();
});
