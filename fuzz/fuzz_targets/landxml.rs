//! LandXML（J-LandXML）の fuzz の target（#501）。読んで土木の領域モデルに移すまでが、小さい予算の中で止まり、
//! 2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_civil::CivilModel;

fuzz_target!(|data: &[u8]| {
    if let Err(message) =
        zumen_sdk::testkit::survives_lift::<zumen_landxml::LandXml, CivilModel>(data)
    {
        panic!("{message}");
    }
});
