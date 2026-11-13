//! SIMA の fuzz の target（#374）。読んで図面・土木・地理の 3 つの領域モデルに移し、
//! さらに `.sim` に書き出すまでが、小さい予算の中で止まり、2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_civil::CivilModel;
use zumen_drawing::Drawing2D;
use zumen_geo::GeoModel;
use zumen_sdk::{Codec, Lift};
use zumen_sima::{
    Sima, lower_civil_to_sima, lower_drawing_to_sima, lower_geo_to_sima, lower_sima_to_sima,
};

fuzz_target!(|data: &[u8]| {
    let result = zumen_sdk::testkit::survives_with(data, |src, cx| {
        let file = Sima::read(src, cx)?;
        let out_self = match lower_sima_to_sima(&file, cx) {
            Ok(v) => Some(v),
            Err(e @ zumen_sdk::Error::Stopped(_)) => return Err(e),
            Err(_) => None,
        };
        let drawing: Drawing2D = file.lift(cx)?;
        let civil: CivilModel = file.lift(cx)?;
        let geo: GeoModel = file.lift(cx)?;
        let out_drawing = match lower_drawing_to_sima(&drawing, cx) {
            Ok(v) => Some(v),
            Err(e @ zumen_sdk::Error::Stopped(_)) => return Err(e),
            Err(_) => None,
        };
        let out_civil = match lower_civil_to_sima(&civil, cx) {
            Ok(v) => Some(v),
            Err(e @ zumen_sdk::Error::Stopped(_)) => return Err(e),
            Err(_) => None,
        };
        let out_geo = match lower_geo_to_sima(&geo, cx) {
            Ok(v) => Some(v),
            Err(e @ zumen_sdk::Error::Stopped(_)) => return Err(e),
            Err(_) => None,
        };
        Ok((out_self, out_drawing, out_civil, out_geo))
    });
    if let Err(message) = result {
        panic!("{message}");
    }
});
