//! 平断面・断面・立面と格子の値を変え、出所と予算停止の再現を照らす。
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    zumen_sdk::testkit::survives_with(data, |data, cx| {
        zumen_fuzz::building_drawing::probe(data, cx).map_err(|error| match error {
            zumen_geom3d::Failure::Stopped(stop) => zumen_sdk::Error::Stopped(stop),
            zumen_geom3d::Failure::Unsupported(_) => zumen_sdk::Error::Unsupported,
            _ => zumen_sdk::Error::Invalid,
        })
    })
    .unwrap();
    zumen_fuzz::building_drawing::exercise(data);
});
