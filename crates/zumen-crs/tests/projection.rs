//! 経緯度と平面直角座標の換算を、告示の値と、式の性質で確かめる（決定 0019）。
//! 国土地理院の測量計算サイトの結果との照合は、`docs/formats/crs.md` の検証の記録にある。

use zumen_crs::{Crs, GeoPoint, MapPoint, Zone, to_geographic, to_plane};

fn zones() -> impl Iterator<Item = Zone> {
    (1..=19).filter_map(Zone::new)
}

/// 告示の別表の原点。経度は度と分
#[test]
fn origins_are_those_of_the_notice() {
    let expected = [
        (33.0, 129.0, 30.0),
        (33.0, 131.0, 0.0),
        (36.0, 132.0, 10.0),
        (33.0, 133.0, 30.0),
        (36.0, 134.0, 20.0),
        (36.0, 136.0, 0.0),
        (36.0, 137.0, 10.0),
        (36.0, 138.0, 30.0),
        (36.0, 139.0, 50.0),
        (40.0, 140.0, 50.0),
        (44.0, 140.0, 15.0),
        (44.0, 142.0, 15.0),
        (44.0, 144.0, 15.0),
        (26.0, 142.0, 0.0),
        (26.0, 127.0, 30.0),
        (26.0, 124.0, 0.0),
        (26.0, 131.0, 0.0),
        (20.0, 136.0, 0.0),
        (26.0, 154.0, 0.0),
    ];
    for (zone, (lat, lon, min)) in zones().zip(expected) {
        let origin = zone.origin();
        assert_eq!(origin.latitude, lat, "{zone:?}");
        assert!(
            (origin.longitude - (lon + min / 60.0)).abs() < 1e-12,
            "{zone:?}"
        );
    }
    assert_eq!(Zone::new(0), None);
    assert_eq!(Zone::new(20), None);
}

/// 原点は (0, 0) に写り、縮尺係数は 0.9999、子午線収差角は 0
#[test]
fn the_origin_maps_to_zero_with_the_scale_of_the_notice() {
    for zone in zones() {
        let projected = to_plane(zone, zone.origin()).unwrap();
        assert!(
            projected.point.northing.abs() < 1e-6,
            "{zone:?} {projected:?}"
        );
        assert!(
            projected.point.easting.abs() < 1e-6,
            "{zone:?} {projected:?}"
        );
        assert!(
            (projected.scale - 0.9999).abs() < 1e-12,
            "{zone:?} {projected:?}"
        );
        assert!(
            projected.convergence.abs() < 1e-12,
            "{zone:?} {projected:?}"
        );
        let back = to_geographic(
            zone,
            MapPoint {
                northing: 0.0,
                easting: 0.0,
            },
        )
        .unwrap();
        assert!(
            (back.point.latitude - zone.origin().latitude).abs() < 1e-11,
            "{zone:?} {back:?}"
        );
        assert!(
            (back.point.longitude - zone.origin().longitude).abs() < 1e-11,
            "{zone:?} {back:?}"
        );
    }
}

/// 中央の子午線の東と西は、Y の符号だけが違う。北の X は、緯度が増えると増える
#[test]
fn the_two_sides_of_the_central_meridian_are_mirrored() {
    let zone = Zone::new(9).unwrap();
    let origin = zone.origin();
    let east = to_plane(
        zone,
        GeoPoint {
            latitude: 35.5,
            longitude: origin.longitude + 0.7,
        },
    )
    .unwrap();
    let west = to_plane(
        zone,
        GeoPoint {
            latitude: 35.5,
            longitude: origin.longitude - 0.7,
        },
    )
    .unwrap();
    assert!(east.point.easting > 0.0);
    assert!((east.point.easting + west.point.easting).abs() < 1e-6);
    assert!((east.point.northing - west.point.northing).abs() < 1e-6);
    assert!((east.convergence + west.convergence).abs() < 1e-12);
    let north = to_plane(
        zone,
        GeoPoint {
            latitude: 36.5,
            longitude: origin.longitude,
        },
    )
    .unwrap();
    assert!(north.point.northing > 0.0 && north.point.easting.abs() < 1e-6);
}

/// 子午線に沿う 1 度は、約 111 km に縮尺係数を掛けた長さになる。中央の子午線の上の X は、子午線の弧の長さに
/// 縮尺係数を掛けたもので、GRS80 の北緯 36 度と 37 度の間の子午線の弧（約 111.0 km）と比べる
#[test]
fn the_meridian_arc_has_the_scale_of_the_notice() {
    let zone = Zone::new(9).unwrap();
    let lon = zone.origin().longitude;
    let x = to_plane(
        zone,
        GeoPoint {
            latitude: 37.0,
            longitude: lon,
        },
    )
    .unwrap()
    .point
    .northing;
    // GRS80 の子午線の弧の長さの級数（緯度 φ の弧 = a(1-e²)∫(1-e² sin²φ)^(-3/2) dφ）を、数値の積分で求める
    let (a, f) = (6_378_137.0f64, 1.0 / 298.257_222_101);
    let e2 = f * (2.0 - f);
    let steps = 100_000;
    let (from, to) = (36f64.to_radians(), 37f64.to_radians());
    let h = (to - from) / steps as f64;
    let mut arc = 0.0;
    for i in 0..steps {
        let phi = from + (i as f64 + 0.5) * h;
        let s = libm::sin(phi);
        arc += a * (1.0 - e2) / libm::pow(1.0 - e2 * s * s, 1.5) * h;
    }
    assert!((x - 0.9999 * arc).abs() < 1e-4, "{x} {}", 0.9999 * arc);
}

/// 平面直角座標から経緯度へ換算して戻すと、元の点に 0.1 mm より近く戻る（系の中の 300 km 四方）
#[test]
fn round_trips_stay_within_a_tenth_of_a_millimetre() {
    for zone in zones() {
        for northing in [-300_000.0, -120_000.0, 0.0, 90_000.0, 300_000.0] {
            for easting in [-200_000.0, -50_000.0, 0.0, 80_000.0, 200_000.0] {
                let point = MapPoint { northing, easting };
                let geo = to_geographic(zone, point).unwrap();
                let back = to_plane(zone, geo.point).unwrap();
                assert!(
                    (back.point.northing - northing).abs() < 1e-4,
                    "{zone:?} {point:?} {back:?}"
                );
                assert!(
                    (back.point.easting - easting).abs() < 1e-4,
                    "{zone:?} {point:?} {back:?}"
                );
                assert!((back.scale - geo.scale).abs() < 1e-9, "{zone:?} {point:?}");
                assert!(
                    (back.convergence - geo.convergence).abs() < 1e-8,
                    "{zone:?} {point:?}"
                );
            }
        }
    }
}

/// 中央の子午線から 90 度の経度は換算できない
#[test]
fn points_far_from_the_zone_are_not_projected() {
    let zone = Zone::new(1).unwrap();
    let far = GeoPoint {
        latitude: 33.0,
        longitude: 129.5 + 90.0,
    };
    assert_eq!(to_plane(zone, far), None);
    assert_eq!(
        to_plane(
            zone,
            GeoPoint {
                latitude: 90.0,
                longitude: 129.5
            }
        ),
        None
    );
    assert_eq!(
        to_plane(
            zone,
            GeoPoint {
                latitude: f64::NAN,
                longitude: 129.5
            }
        ),
        None
    );
}

/// EPSG のコードと名前。JGD2011 の名前は JGD2024 と同じに読む
#[test]
fn coordinate_systems_are_named_by_epsg_codes() {
    let ix = Crs::Plane(Zone::new(9).unwrap());
    assert_eq!(Crs::from_epsg(6677), Some(ix));
    assert_eq!(ix.epsg(), 6677);
    assert_eq!(Crs::from_epsg(6668), Some(Crs::Geographic));
    assert_eq!(Crs::from_epsg(6669).map(Crs::epsg), Some(6669));
    assert_eq!(Crs::from_epsg(6687).map(Crs::epsg), Some(6687));
    // JGD2000 と日本測地系は別の測地系
    assert_eq!(Crs::from_epsg(2451), None);
    assert_eq!(Crs::from_epsg(30169), None);
    for name in [
        "EPSG:6677",
        "urn:ogc:def:crs:EPSG::6677",
        "http://www.opengis.net/def/crs/EPSG/0/6677",
        "JGD2024 / Japan Plane Rectangular CS IX",
        "JGD2011 / Japan Plane Rectangular CS IX",
    ] {
        assert_eq!(Crs::from_name(name), Some(ix), "{name}");
    }
    assert_eq!(Crs::from_name("JGD2011"), Some(Crs::Geographic));
    assert_eq!(
        Crs::from_name("JGD2024 / Japan Plane Rectangular CS XX"),
        None
    );
    assert_eq!(Crs::from_name("EPSG:"), None);
    assert_eq!(Zone::new(19).unwrap().numeral(), "XIX");
}
