//! 3D カーネルのラッパーへ形・参照・有限でない値を渡し、結果と停止を照らす。
#![no_main]
use libfuzzer_sys::fuzz_target;
use zumen_geom3d::{Kernel, ManifoldKernel, Tolerance};
fuzz_target!(|data: &[u8]| {
    // 成功・失敗と診断の再現は共通キット、メッシュと低い予算は専用の照合で確かめる。
    zumen_sdk::testkit::survives_with(data, |data, cx| {
        ManifoldKernel
            .evaluate(
                &zumen_fuzz::geom3d::shape(data),
                Tolerance {
                    chord: 0.001,
                    length: 1e-9,
                },
                cx,
            )
            .map_err(|error| match error {
                zumen_geom3d::Failure::Stopped(stop) => zumen_sdk::Error::Stopped(stop),
                zumen_geom3d::Failure::Unsupported(_) => zumen_sdk::Error::Unsupported,
                _ => zumen_sdk::Error::Invalid,
            })
    })
    .unwrap();
    zumen_fuzz::geom3d::exercise(data);
});
