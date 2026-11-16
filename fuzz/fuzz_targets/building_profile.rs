//! 建物プロファイルの読み取りと、変異した属性・配置・面積の診断と予算を照らす。
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    zumen_sdk::testkit::survives_with(data, zumen_fuzz::building_profile::probe).unwrap();
    zumen_fuzz::building_profile::exercise(data);
});
