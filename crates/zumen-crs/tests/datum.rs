//! 日本測地系と JGD2000 の座標を、合成の TKY2JGD・PatchJGD の格子で JGD2011 に直す（決定 0233、#471）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_crs::{
    Crs, Ellipsoid, GeoPoint, MapPoint, ShiftGrid, Shifts, Zone, to_geographic_on, to_jgd2011,
    to_plane, to_plane_on,
};
use zumen_sdk::Error;

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
        patch: &[&patch],
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
        patch: &[&patch],
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
        patch: &[&patch],
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

/// PatchJGD のファイルのように、ヘッダーが長く、UTF-8 でない行（Shift_JIS の説明）を含んでも、初めのデータの行の
/// 前は読まない。行の終わりは CR+LF
#[test]
fn long_headers_in_other_encodings_are_skipped() {
    let around = GeoPoint {
        latitude: 35.6,
        longitude: 139.7,
    };
    let short = grid(around, tky);
    let mut text = b"for PatchJGD Ver.1.0.0 001\r\n".to_vec();
    for _ in 0..12 {
        // Shift_JIS の「補正」。UTF-8 としては読めない
        text.extend_from_slice(b"\x95\xe2\x90\xb3 \x95\xe2\x90\xb3\r\n");
    }
    text.extend_from_slice(b"\r\nMeshCode   dB(sec)   dL(sec)\r\n");
    for line in short.lines().skip(2) {
        text.extend_from_slice(line.as_bytes());
        text.extend_from_slice(b"\r\n");
    }
    let (long, diag) = with_cx(|cx| ShiftGrid::read(&text, cx));
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(long.ok(), Some(read(&short)));
}

/// 初めのデータの行の後は、空白だけの行を読み飛ばし、UTF-8 として読めない行は読めない行にする（決定 0237）
#[test]
fn unreadable_lines_after_the_data_are_reported() {
    let text = grid(
        GeoPoint {
            latitude: 35.6,
            longitude: 139.7,
        },
        tky,
    );
    // 空白だけの行は、データの行の後でも読み飛ばす
    let mut blank = text.clone().into_bytes();
    blank.extend_from_slice(b" \t\r\n\r\n");
    let (found, diag) = with_cx(|cx| ShiftGrid::read(&blank, cx));
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(found.ok(), Some(read(&text)));
    // ヘッダー 2 行と格子点 25 行の後の 28 行目に、Shift_JIS の「補正」（UTF-8 としては読めない）を置く
    let mut broken = text.into_bytes();
    broken.extend_from_slice(b"\x95\xe2\x90\xb3\n");
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let found = ShiftGrid::read(&broken, &mut cx);
    assert!(matches!(found, Err(Error::Invalid)));
    let reported: Vec<_> = diag.iter().collect();
    assert_eq!(reported.len(), 1);
    assert_eq!(reported[0].code(), codes::CRS_DATUM_GRID_INVALID);
    assert_eq!(reported[0].param("line"), Some(&28_u64.into()));
}

/// 格子点を 1 つずつ、予算の作業量で数える。合成の格子の格子点は 25 個なので、作業量の上限が 25 なら読め、
/// 24 なら止まる
#[test]
fn nodes_are_counted_in_the_budget() {
    let text = grid(
        GeoPoint {
            latitude: 35.6,
            longitude: 139.7,
        },
        tky,
    );
    let read_within = |work: u64| {
        let mut diag = Diagnostics::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(work)));
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let found = ShiftGrid::read(text.as_bytes(), &mut cx);
        (found, diag.iter().map(|d| d.code()).collect::<Vec<_>>())
    };
    let (found, diag) = read_within(25);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(found.ok(), Some(read(&text)));
    let (found, diag) = read_within(24);
    assert!(matches!(found, Err(Error::Stopped(_))));
    assert_eq!(diag, [codes::BUDGET_WORK_EXCEEDED]);
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

/// 点 `around` を含む区画の南西の隅から、`step` の間隔（3 次メッシュの数）の格子点を南北・東西に `-half..=half` 区画
/// 並べ、緯度と経度の式 `shift` の補正量（秒）を書いた格子。補正量は小数 6 桁に丸める
#[cfg(test)]
fn grid_every(
    around: GeoPoint,
    step: u32,
    half: u32,
    shift: impl Fn(f64, f64) -> (f64, f64),
) -> String {
    let (i0, j0) = (
        (around.latitude * 120.0) as u32 / step * step,
        ((around.longitude - 100.0) * 80.0) as u32 / step * step,
    );
    let mut text = String::from("for PatchJGD Ver.1.0.0\nMeshCode   dB(sec)   dL(sec)\n");
    for a in 0..=2 * half {
        for b in 0..=2 * half {
            let (i, j) = (i0 - half * step + a * step, j0 - half * step + b * step);
            let (db, dl) = shift(f64::from(i) / 120.0, 100.0 + f64::from(j) / 80.0);
            text.push_str(&format!("{} {db:.6} {dl:.6}\n", code(i, j)));
        }
    }
    text
}

/// 格子 `text` から、格子点の番号 `(i, j)` の行を除く
#[cfg(test)]
fn without(text: &str, i: u32, j: u32) -> String {
    let removed = code(i, j);
    let kept: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with(&removed))
        .collect();
    assert_eq!(kept.len() + 1, text.lines().count(), "{removed}");
    kept.join("\n") + "\n"
}

/// [`with_cx`] と同じく文脈を作り、診断を値ごと返す
#[cfg(test)]
fn with_diagnostics<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<zumen_base::Diagnostic>) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let out = f(&mut cx);
    (out, diag.into_vec())
}

#[cfg(test)]
fn distance(a: MapPoint, b: MapPoint) -> f64 {
    libm::hypot(a.northing - b.northing, a.easting - b.easting)
}

/// 経緯度 `point` に、緯度・経度の補正量（秒）を足す
#[cfg(test)]
fn shifted(point: GeoPoint, (b, l): (f64, f64)) -> GeoPoint {
    GeoPoint {
        latitude: point.latitude + b / 3600.0,
        longitude: point.longitude + l / 3600.0,
    }
}

/// 公共測量成果改定マニュアル（令和 7 年 4 月）の第 3 条の解説のとおり、格子点が 3 次メッシュの 5 倍の間隔の所（緯度
/// 150 秒・経度 225 秒の倍数）だけにある格子は、その間隔で補間する。3 次メッシュの四隅に格子点のない点も、補正量を
/// 0 にしない（#670、決定 0286）
#[test]
fn grids_at_five_times_the_mesh_are_interpolated_at_their_spacing() {
    let zone = Zone::new(10).unwrap();
    let point = MapPoint {
        northing: -190_000.0,
        easting: 30_000.0,
    };
    let geographic = to_geographic_on(Ellipsoid::Grs80, zone, point)
        .unwrap()
        .point;
    // 一次の式なら、双一次の補間は式と同じ値になる
    let linear = |latitude: f64, longitude: f64| {
        (
            5.0 + 2.0 * (latitude - 38.0),
            -7.0 + 3.0 * (longitude - 141.0),
        )
    };
    let patch = read(&grid_every(geographic, 5, 2, linear));
    assert_eq!(patch.spacing(), (150, 225));
    assert_eq!(read(&grid(geographic, tky)).spacing(), (30, 45));
    // 3 次メッシュの区画の四隅に、5 倍の間隔の格子点が 1 つもない点
    let (i0, j0) = (
        (geographic.latitude * 120.0) as u32 / 5 * 5,
        ((geographic.longitude - 100.0) * 80.0) as u32 / 5 * 5,
    );
    let inner = GeoPoint {
        latitude: (f64::from(i0) + 2.4) / 120.0,
        longitude: 100.0 + (f64::from(j0) + 2.6) / 80.0,
    };
    let (b, l) = patch.correction(inner).unwrap();
    let (eb, el) = linear(inner.latitude, inner.longitude);
    assert!((b - eb).abs() < 1e-5 && (l - el).abs() < 1e-5, "{b} {l}");
    // JGD2000 の平面直角座標を直すときも、同じ補正量を当てる
    let shifts = Shifts {
        tky2jgd: None,
        patch: &[&patch],
    };
    let (found, diag) = with_cx(|cx| to_jgd2011(Crs::Jgd2000(zone), point, &shifts, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let expected = to_plane(
        zone,
        shifted(
            geographic,
            linear(geographic.latitude, geographic.longitude),
        ),
    )
    .unwrap()
    .point;
    let (crs, moved) = found.unwrap();
    assert_eq!(crs, Crs::Plane(zone));
    assert!(distance(moved, expected) < 1e-4, "{moved:?} {expected:?}");
    assert!(distance(moved, point) > 100.0, "{moved:?}");
}

/// 格子点が、すべて 3 次メッシュの 5 倍の間隔の所にあるのでも、3 次メッシュの間隔で隣り合うのでもなければ、補間の
/// 間隔が決まらないので読まない（決定 0286）
#[test]
fn grids_that_fit_neither_spacing_are_not_read() {
    let around = GeoPoint {
        latitude: 35.6,
        longitude: 139.7,
    };
    // 3 次メッシュの 1 つおきの格子点
    let every_other = grid_every(around, 2, 3, tky);
    let (found, diag) = with_cx(|cx| ShiftGrid::read(every_other.as_bytes(), cx));
    assert!(matches!(found, Err(Error::Unsupported)), "{found:?}");
    assert_eq!(diag, [codes::CRS_DATUM_GRID_UNSUPPORTED]);
    // 格子点が 1 つだけなら、5 倍の間隔の所にあればその間隔とし、なければ読まない
    let single = |i: u32, j: u32| format!("MeshCode dB(sec) dL(sec)\n{} 1.0 2.0\n", code(i, j));
    let (found, diag) = with_cx(|cx| ShiftGrid::read(single(4281, 3181).as_bytes(), cx));
    assert!(found.is_err());
    assert_eq!(diag, [codes::CRS_DATUM_GRID_UNSUPPORTED]);
    assert_eq!(read(&single(4280, 3180)).spacing(), (150, 225));
    // 読んだ格子点は、3 次メッシュのコードで返す
    let nodes: Vec<_> = read(&single(4280, 3180)).nodes().collect();
    assert_eq!(nodes, [(53_394_600, 1.0, 2.0)]);
    let text = grid(around, tky);
    let mut written: Vec<u32> = text
        .lines()
        .skip(2)
        .map(|line| line.split_whitespace().next().unwrap().parse().unwrap())
        .collect();
    let mut found: Vec<u32> = read(&text).nodes().map(|(code, _, _)| code).collect();
    written.sort_unstable();
    found.sort_unstable();
    assert_eq!(found.len(), 25);
    assert_eq!(found, written);
}

/// 同じ地区の地震ごとの PatchJGD のファイルは、渡した順（古い順）に、前のファイルで直した経緯度に当てる（公共測量
/// 成果改定マニュアルの第 3 条の解説、決定 0286）
#[test]
fn patchjgd_files_are_applied_oldest_first() {
    let zone = Zone::new(10).unwrap();
    let point = MapPoint {
        northing: -190_000.0,
        easting: 30_000.0,
    };
    let geographic = to_geographic_on(Ellipsoid::Grs80, zone, point)
        .unwrap()
        .point;
    // 前の地震は一様な補正量。後の地震の補正量は場所で変わるので、前のファイルで直した点で決まる
    let first = |_: f64, _: f64| (20.0, -30.0);
    let second =
        |latitude: f64, longitude: f64| (60.0 * (latitude - 38.0), -45.0 * (longitude - 141.0));
    let older = read(&grid_every(geographic, 1, 3, first));
    let newer = read(&grid_every(geographic, 5, 3, second));
    let once = shifted(geographic, first(geographic.latitude, geographic.longitude));
    let twice = shifted(once, second(once.latitude, once.longitude));
    let expected = to_plane(zone, twice).unwrap().point;
    let shifts = Shifts {
        tky2jgd: None,
        patch: &[&older, &newer],
    };
    let (found, diag) = with_cx(|cx| to_jgd2011(Crs::Jgd2000(zone), point, &shifts, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let (_, moved) = found.unwrap();
    assert!(distance(moved, expected) < 1e-4, "{moved:?} {expected:?}");
    // 逆の順に当てると、後の地震の補正量を直す前の点で決めるので、別の点になる
    let reversed = Shifts {
        tky2jgd: None,
        patch: &[&newer, &older],
    };
    let (found, _) = with_cx(|cx| to_jgd2011(Crs::Jgd2000(zone), point, &reversed, cx));
    let (_, other) = found.unwrap();
    assert!(distance(other, expected) > 1.0, "{other:?} {expected:?}");
    // 前のファイルの範囲の外の点（3 次メッシュの 4.5 区画北）は、前のファイルの補正量を 0 とし、後のファイルだけを当てる
    let far = GeoPoint {
        latitude: geographic.latitude + 135.0 / 3600.0,
        longitude: geographic.longitude,
    };
    let far_point = to_plane(zone, far).unwrap().point;
    let (found, diag) = with_cx(|cx| to_jgd2011(Crs::Jgd2000(zone), far_point, &shifts, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let (_, moved) = found.unwrap();
    let expected = to_plane(zone, shifted(far, second(far.latitude, far.longitude)))
        .unwrap()
        .point;
    assert!(distance(moved, expected) < 1e-4, "{moved:?} {expected:?}");
    // 後のファイルで、直した点を囲む格子点が欠けていれば、何番目のファイルかを診断に書いて直さない
    let cell = (
        (once.latitude * 120.0 / 5.0) as u32 * 5,
        ((once.longitude - 100.0) * 80.0 / 5.0) as u32 * 5,
    );
    let holed = read(&without(
        &grid_every(geographic, 5, 3, second),
        cell.0 + 5,
        cell.1 + 5,
    ));
    let shifts = Shifts {
        tky2jgd: None,
        patch: &[&older, &holed],
    };
    let (found, diag) = with_diagnostics(|cx| to_jgd2011(Crs::Jgd2000(zone), point, &shifts, cx));
    assert_eq!(found, None);
    assert_eq!(diag.len(), 1, "{diag:?}");
    assert_eq!(diag[0].code(), codes::CRS_DATUM_SHIFT_MISSING);
    assert_eq!(diag[0].param("grid"), Some(&"PatchJGD 2".into()));
    // ファイルが 1 つなら、番号を付けない
    let shifts = Shifts {
        tky2jgd: None,
        patch: &[&holed],
    };
    let once_point = to_plane(zone, once).unwrap().point;
    let (found, diag) =
        with_diagnostics(|cx| to_jgd2011(Crs::Jgd2000(zone), once_point, &shifts, cx));
    assert_eq!(found, None);
    assert_eq!(diag.len(), 1, "{diag:?}");
    assert_eq!(diag[0].param("grid"), Some(&"PatchJGD".into()));
}

/// 格子の補正量が大きすぎて、直した点が 3 次メッシュのコードで表せない所に出ても、パニックせずに直さない（受け取った
/// ファイルを信用しない）
#[test]
fn shifts_beyond_mesh_codes_do_not_panic() {
    let zone = Zone::new(IX).unwrap();
    let tokyo = to_geographic_on(Ellipsoid::Bessel, zone, POINT)
        .unwrap()
        .point;
    let huge = read(&grid(tokyo, |_, _| (1e300, 1e300)));
    let patch = read(&grid(tokyo, |_, _| (0.0, 0.0)));
    let shifts = Shifts {
        tky2jgd: Some(&huge),
        patch: &[&patch],
    };
    let (found, _) = with_cx(|cx| to_jgd2011(Crs::Tokyo(zone), POINT, &shifts, cx));
    assert_eq!(found, None);
    for (latitude, longitude) in [(1e300, 139.0), (35.0, 1e300), (35.0, 180.0), (66.7, 139.0)] {
        assert_eq!(
            patch.correction(GeoPoint {
                latitude,
                longitude
            }),
            None
        );
    }
}
