//! Sxf の常駐文書・差分編集・履歴の予算と決定性を確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_fuzz::live::{Format, run};

fuzz_target!(|data: &[u8]| {
    let result = zumen_sdk::testkit::survives_calls(|limits| {
        let single = run(data, Format::Sxf, *limits, 1);
        let parallel = run(data, Format::Sxf, *limits, 4);
        assert!(single == parallel, "スレッド数で常駐文書の結果が変わる");
        single
    });
    assert!(result.is_ok(), "同じ入力と予算で常駐文書の結果が変わる");
});
