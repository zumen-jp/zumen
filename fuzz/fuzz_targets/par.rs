//! TKY2JGD・PatchJGD の形の格子（座標の補正量）の fuzz の target（#501・#679）。読み取りが、小さい予算の中で止まり、
//! 2 回で同じ結果になることを確かめる。読めた格子では、決まった点の補間（`ShiftGrid::correction`）と、日本測地系と
//! JGD2000 の平面直角座標から JGD2011 への変換（`to_jgd2011`）まで通し、落ちないことと、2 回で同じ結果になることを
//! 確かめる。点には、3 次メッシュのコードで表せない所（北緯 66.66 度より北・東経 100 度より西・東経 180 度・南緯）と、
//! 投影の範囲の外を含む。2 回の補間と変換の値は、`survives_with` が比べないので、ここでビットで比べる（NaN も同じ値と
//! して比べる）

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_crs::{Crs, GeoPoint, MapPoint, ShiftGrid, Shifts, Zone, to_jgd2011};

/// 補間する経緯度（度）
const GEOGRAPHIC: [(f64, f64); 8] = [
    (35.0, 139.0),
    (35.658_4, 139.745_4),
    (24.0, 123.0),
    (45.5, 148.9),
    (66.7, 139.0),
    (35.0, 99.9),
    (35.0, 180.0),
    (-1.0, 139.0),
];

/// IX 系で変換する平面直角座標（m）
const PLANE: [(f64, f64); 4] = [
    (0.0, 0.0),
    (-35_000.0, -6_000.0),
    (300_000.0, 200_000.0),
    (1.0e7, 1.0e7),
];

type Bits = Option<(u64, u64)>;

fn bits(pair: Option<(f64, f64)>) -> Bits {
    pair.map(|(a, b)| (a.to_bits(), b.to_bits()))
}

fn run(src: &[u8], cx: &mut zumen_base::Cx) -> Result<(ShiftGrid, Vec<Bits>), zumen_sdk::Error> {
    let grid = ShiftGrid::read(src, cx)?;
    let mut out = Vec::new();
    for (latitude, longitude) in GEOGRAPHIC {
        out.push(bits(grid.correction(GeoPoint {
            latitude,
            longitude,
        })));
    }
    let Some(zone) = Zone::new(9) else {
        return Ok((grid, out));
    };
    let shifts = Shifts {
        tky2jgd: Some(&grid),
        patch: &[&grid],
    };
    for crs in [Crs::Tokyo(zone), Crs::Jgd2000(zone)] {
        for (northing, easting) in PLANE {
            let point = MapPoint { northing, easting };
            let found = to_jgd2011(crs, point, &shifts, cx);
            out.push(bits(found.map(|(_, p)| (p.northing, p.easting))));
        }
    }
    Ok((grid, out))
}

fuzz_target!(|data: &[u8]| {
    let previous = std::cell::RefCell::new(None);
    let checked = zumen_sdk::testkit::survives_with(data, |src, cx| {
        let result = run(src, cx);
        if let Ok((_, values)) = &result {
            if let Some(first) = previous.borrow().as_ref() {
                assert_eq!(first, values, "同じ格子で、補間と変換の値が 2 回で違う");
            }
            *previous.borrow_mut() = Some(values.clone());
        }
        result
    });
    if let Err(message) = checked {
        panic!("{message}");
    }
});
