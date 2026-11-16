//! 面積の境界・単位・丸め・元の寸法を変え、値と診断・停止の再現を照らす。
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    zumen_sdk::testkit::survives_with(data, |data, cx| {
        zumen_fuzz::building_measure::probe(data, cx).map_err(|error| match error {
            zumen_geom3d::Failure::Stopped(stop) => zumen_sdk::Error::Stopped(stop),
            zumen_geom3d::Failure::Unsupported(_) => zumen_sdk::Error::Unsupported,
            _ => zumen_sdk::Error::Invalid,
        })
    })
    .unwrap();
    zumen_fuzz::building_measure::exercise(data);
});
