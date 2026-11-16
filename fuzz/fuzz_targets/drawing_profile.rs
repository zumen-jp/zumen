//! 作図プロファイルと、変異した属性・形・切る面を同じ予算で照らす。
#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    zumen_sdk::testkit::survives_with(data, zumen_fuzz::drawing_profile::probe).unwrap();
    zumen_fuzz::drawing_profile::exercise(data);
});
