//! 日本測地系と JGD2000 の座標を、合成の TKY2JGD・PatchJGD の格子で JGD2011 に直す（決定 0233、#471）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_crs::{
    Crs, Ellipsoid, GeoPoint, MapPoint, ShiftGrid, Shifts, Zone, to_geographic_on, to_jgd2011,
    to_plane, to_plane_on,
};

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Code>) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let out = f(&mut cx);
    (out, diag.iter().map(|d| d.code()).collect())
}

/// 格子点（緯度の 30 秒ごと、経度 100 度から 45 秒ごとの番号）の 3 次メッシュのコード
#[cfg(test)]
fn code(i: u32, j: u32) -> String {
    format!(
        "{:02}{:02}{}{}{}{}",
        i / 80,
        j / 80,
        (i % 80) / 10,
        (j % 80) / 10,
        i % 10,
        j % 10
    )
}

/// 点 `around` の近くの格子点に、緯度と経度の一次の式で補正量を書いた格子。一次の式なら、双一次の補間は式と
/// 同じ値になる。補正量は小数 6 桁（0.03 mm ほど）に丸めて書く
#[cfg(test)]
fn grid(around: GeoPoint, shift: impl Fn(f64, f64) -> (f64, f64)) -> String {
    let (i0, j0) = (
        (around.latitude * 120.0) as u32,
        ((around.longitude - 100.0) * 80.0) as u32,
    );
    let mut text = String::from("JGD2000-TokyoDatum Ver.2.1.2\nMeshCode   dB(sec)   dL(sec)\n");
    for i in i0 - 2..=i0 + 2 {
        for j in j0 - 2..=j0 + 2 {
            let (b, l) = shift(f64::from(i) / 120.0, 100.0 + f64::from(j) / 80.0);
            text.push_str(&format!("{} {b:.6} {l:.6}\n", code(i, j)));
        }
    }
    text
}

#[cfg(test)]
fn read(text: &str) -> ShiftGrid {
    with_cx(|cx| ShiftGrid::read(text.as_bytes(), cx))
        .0
        .unwrap()
}

/// 緯度・経度に一次の補正量（秒）
#[cfg(test)]
fn tky(latitude: f64, longitude: f64) -> (f64, f64) {
    (
        11.6 + 0.2 * (latitude - 35.0),
        -11.8 + 0.3 * (longitude - 139.0),
    )
}

const IX: u8 = 9;
const POINT: MapPoint = MapPoint {
    northing: -35_000.0,
    easting: -6_000.0,
};

#[test]
fn tokyo_coordinates_are_shifted_by_the_interpolated_parameters() {
    let zone = Zone::new(IX).unwrap();
    let tokyo = to_geographic_on(Ellipsoid::Bessel, zone, POINT)
        .unwrap()
        .point;
    let tky2jgd = read(&grid(tokyo, tky));
    // PatchJGD は別の地域だけを持つので、この点の補正量は 0
    let far = GeoPoint {
        latitude: 38.3,
        longitude: 141.0,
    };
    let patch = read(&grid(far, |_, _| (-0.3, 0.9)));
    let shifts = Shifts {
        tky2jgd: Some(&tky2jgd),
        patch: Some(&patch),
    };
    let (found, diag) = with_cx(|cx| to_jgd2011(Crs::Tokyo(zone), POINT, &shifts, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let (b, l) = tky(tokyo.latitude, tokyo.longitude);
    let expected = to_plane(
        zone,
        GeoPoint {
            latitude: tokyo.latitude + b / 3600.0,
            longitude: tokyo.longitude + l / 3600.0,
        },
    )
    .unwrap()
    .point;
    let (crs, point) = found.unwrap();
    assert_eq!(crs, Crs::Plane(zone));
    assert!(
        (point.northing - expected.northing).abs() < 1e-4,
        "{point:?} {expected:?}"
    );
    assert!((point.easting - expected.easting).abs() < 1e-4, "{point:?}");
    // 日本測地系と JGD2011 の差は、東京の辺りで数百 m
    let moved = libm::hypot(
        point.northing - POINT.northing,
        point.easting - POINT.easting,
    );
    assert!((300.0..600.0).contains(&moved), "{moved}");
}

#[test]
fn jgd2000_coordinates_are_shifted_by_patchjgd() {
    let zone = Zone::new(10).unwrap();
    let point = MapPoint {
        northing: -190_000.0,
        easting: 30_000.0,
    };
    let geographic = to_geographic_on(Ellipsoid::Grs80, zone, point)
        .unwrap()
        .point;
    let patch = read(&grid(geographic, |_, _| (-0.3, 0.9)));
    let shifts = Shifts {
        tky2jgd: None,
        patch: Some(&patch),
    };
    let (found, diag) = with_cx(|cx| to_jgd2011(Crs::Jgd2000(zone), point, &shifts, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let expected = to_plane(
        zone,
        GeoPoint {
            latitude: geographic.latitude - 0.3 / 3600.0,
            longitude: geographic.longitude + 0.9 / 3600.0,
        },
    )
    .unwrap()
    .point;
    let (_, moved) = found.unwrap();
    assert!((moved.northing - expected.northing).abs() < 1e-6);
    assert!((moved.easting - expected.easting).abs() < 1e-6);
    // JGD2011 の座標は、そのまま返す
    let (same, diag) = with_cx(|cx| to_jgd2011(Crs::Plane(zone), point, &Shifts::default(), cx));
    assert_eq!(same, Some((Crs::Plane(zone), point)));
    assert!(diag.is_empty());
}

#[test]
fn without_parameters_nothing_is_converted() {
    let zone = Zone::new(IX).unwrap();
    let (found, diag) = with_cx(|cx| to_jgd2011(Crs::Tokyo(zone), POINT, &Shifts::default(), cx));
    assert_eq!(found, None);
    assert_eq!(diag, [codes::CRS_DATUM_GRID_MISSING]);
    let (found, diag) = with_cx(|cx| to_jgd2011(Crs::Jgd2000(zone), POINT, &Shifts::default(), cx));
    assert_eq!(found, None);
    assert_eq!(diag, [codes::CRS_DATUM_GRID_MISSING]);
    // TKY2JGD の範囲の外の点も、直さない
    let far = GeoPoint {
        latitude: 43.0,
        longitude: 141.3,
    };
    let tky2jgd = read(&grid(far, tky));
    let patch = read(&grid(far, |_, _| (0.0, 0.0)));
    let shifts = Shifts {
        tky2jgd: Some(&tky2jgd),
        patch: Some(&patch),
    };
    let (found, diag) = with_cx(|cx| to_jgd2011(Crs::Tokyo(zone), POINT, &shifts, cx));
    assert_eq!(found, None);
    assert_eq!(diag, [codes::CRS_DATUM_SHIFT_MISSING]);
}

#[test]
fn broken_lines_are_reported() {
    let text = format!(
        "{}5339a 1 2\n",
        grid(
            GeoPoint {
                latitude: 35.6,
                longitude: 139.7
            },
            tky
        )
    );
    let (grid, diag) = with_cx(|cx| ShiftGrid::read(text.as_bytes(), cx));
    assert!(grid.is_err());
    assert_eq!(diag, [codes::CRS_DATUM_GRID_INVALID]);
    let (grid, diag) = with_cx(|cx| ShiftGrid::read(b"header only\n", cx));
    assert!(grid.is_err());
    assert_eq!(diag, [codes::CRS_DATUM_GRID_INVALID]);
}

#[test]
fn old_plane_systems_are_named() {
    let zone = Zone::new(IX).unwrap();
    assert_eq!(Crs::from_epsg(2451), Some(Crs::Jgd2000(zone)));
    assert_eq!(Crs::from_epsg(30169), Some(Crs::Tokyo(zone)));
    assert_eq!(Crs::Jgd2000(zone).epsg(), 2451);
    assert_eq!(Crs::Tokyo(zone).epsg(), 30169);
    assert_eq!(
        Crs::from_name("JGD2000 / Japan Plane Rectangular CS IX"),
        Some(Crs::Jgd2000(zone))
    );
    assert_eq!(
        Crs::from_name("Tokyo / Japan Plane Rectangular CS IX"),
        Some(Crs::Tokyo(zone))
    );
    assert_eq!(Crs::from_name("EPSG:30169"), Some(Crs::Tokyo(zone)));
    assert_eq!(Crs::Tokyo(zone).zone(), Some(zone));
    assert_eq!(Crs::Geographic.zone(), None);
    // ベッセル楕円体の換算は、往きと帰りで同じ点に戻る
    let geo = to_geographic_on(Ellipsoid::Bessel, zone, POINT)
        .unwrap()
        .point;
    let back = to_plane_on(Ellipsoid::Bessel, zone, geo).unwrap().point;
    assert!((back.northing - POINT.northing).abs() < 1e-6);
    assert!((back.easting - POINT.easting).abs() < 1e-6);
}
