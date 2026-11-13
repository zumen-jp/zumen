//! TIFF・GeoTIFF の fuzz の target（#877）。読んで図面・地理・点群の領域モデルに移し、
//! 先頭ページを最大辺 64 の PNG に復号するまでが、小さい予算の中で止まり、2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_drawing::Drawing2D;
use zumen_geo::GeoModel;
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Lift};
use zumen_tiff::Tiff;

fuzz_target!(|data: &[u8]| {
    let result = zumen_sdk::testkit::survives_with(data, |src, cx| {
        let file = Tiff::read(src, cx)?;
        let drawing: Drawing2D = file.lift(cx)?;
        let geo: GeoModel = file.lift(cx)?;
        let cloud = match Lift::<PointCloud>::lift(&file, cx) {
            Ok(c) => Some(c.points.len()),
            Err(e @ zumen_sdk::Error::Stopped(_)) => return Err(e),
            Err(_) => None,
        };
        let png = match file.encode_raster_png(1, Some(64), cx) {
            Ok(p) => Some(p.bytes),
            Err(e @ zumen_sdk::Error::Stopped(_)) => return Err(e),
            Err(_) => None,
        };
        Ok((drawing.spaces.len(), geo.features.len(), cloud, png))
    });
    if let Err(message) = result {
        panic!("{message}");
    }
});
