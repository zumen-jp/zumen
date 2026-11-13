//! GeoJSON の fuzz の target（#876）。読んで地理の領域モデルに移し、さらに GeoJSON に書き出すまでが、
//! 小さい予算の中で止まり、2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_geo::GeoModel;
use zumen_geojson::{GeoJson, lower_to_geojson};
use zumen_sdk::{Codec, Lift};

fuzz_target!(|data: &[u8]| {
    let result = zumen_sdk::testkit::survives_with(data, |src, cx| {
        let file = GeoJson::read(src, cx)?;
        let model: GeoModel = file.lift(cx)?;
        lower_to_geojson(&model, cx)
    });
    if let Err(message) = result {
        panic!("{message}");
    }
});
