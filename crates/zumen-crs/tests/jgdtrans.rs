//! 座標の補正量の格子（TKY2JGD・PatchJGD の形）の読みと補間を、外部の実装の jgdtrans 0.3.1（MIT OR Apache-2.0）と
//! 照らす（#670、決定 0286）。格子はテストの中で作り、国土地理院のパラメーターのファイルは使わない（決定 0019）。
//!
//! jgdtrans は、データの行を桁の位置で読む。コードは 1〜8 桁目、緯度・経度の補正量は 10〜18 桁目・20〜28 桁目、
//! 高さの補正量は 30〜38 桁目。ヘッダーの行数は形で決まり、TKY2JGD は 2 行、PatchJGD と SemiDynaEXE は 16 行。
//! 補間の間隔は形で決まり、TKY2JGD と PatchJGD は 3 次メッシュ、SemiDynaEXE は 5 倍地域メッシュ。テストの格子は、
//! この桁とヘッダーの行数で書く。

use jgdtrans::{Format, ParData, Point, Transformer};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_crs::{
    Crs, Ellipsoid, GeoPoint, MapPoint, ShiftGrid, Shifts, Zone, to_geographic_on, to_jgd2011,
    to_plane,
};

/// テストの格子の形
#[cfg(test)]
struct Layout {
    /// jgdtrans の形
    format: Format,
    /// ヘッダーの行数。最後の行は列の見出し
    header: usize,
    /// 格子点の間隔（3 次メッシュの数）
    step: u32,
    /// 高さの補正量の列を書くか
    altitude: bool,
}

const TKY2JGD: Layout = Layout {
    format: Format::TKY2JGD,
    header: 2,
    step: 1,
    altitude: false,
};

const PATCHJGD: Layout = Layout {
    format: Format::PatchJGD,
    header: 16,
    step: 1,
    altitude: false,
};

/// 5 倍地域メッシュの格子。jgdtrans では、その間隔で補間する形は SemiDynaEXE などに限られる
const FIVE_TIMES: Layout = Layout {
    format: Format::SemiDynaEXE,
    header: 16,
    step: 5,
    altitude: true,
};

/// 格子点の番号（緯度の 30 秒ごと、経度 100 度から 45 秒ごと）の 3 次メッシュのコード
#[cfg(test)]
fn code(i: u32, j: u32) -> u32 {
    (i / 80 * 100 + j / 80) * 10_000
        + (i % 80) / 10 * 1_000
        + (j % 80) / 10 * 100
        + i % 10 * 10
        + j % 10
}

/// 格子点ごとに決まる、-10 秒から 10 秒の補正量（小数 5 桁）。一次の式にせず、補間の値が、使った格子点と重みで
/// 変わるようにする
#[cfg(test)]
fn value(i: u32, j: u32, salt: u32) -> f64 {
    let mixed =
        (i.wrapping_mul(7_919) ^ j.wrapping_mul(104_729) ^ salt.wrapping_mul(65_537)) % 2_000_001;
    f64::from(mixed) / 100_000.0 - 10.0
}

/// 格子点の番号 `(i0, j0)` を中心に、`step` の間隔で南北・東西に `-half..=half` 個ずつ並べた格子点
#[cfg(test)]
fn block(i0: u32, j0: u32, half: u32, step: u32) -> Vec<(u32, u32)> {
    let mut nodes = Vec::new();
    for a in 0..=2 * half {
        for b in 0..=2 * half {
            nodes.push((i0 - half * step + a * step, j0 - half * step + b * step));
        }
    }
    nodes
}

/// 格子点 `nodes` を、形 `layout` の桁とヘッダーの行数で書く。データの行は、コード（8 桁）と補正量（9 桁ずつ）を
/// 半角の空白で区切る（TKY2JGD の論文の表 2 と同じ並び）
#[cfg(test)]
fn par(layout: &Layout, nodes: &[(u32, u32)], salt: u32) -> String {
    let mut text = String::new();
    for line in 1..layout.header {
        text.push_str(&format!("header {line}\n"));
    }
    text.push_str(if layout.altitude {
        "MeshCode dB(sec)  dL(sec) dH(m)\n"
    } else {
        "MeshCode   dB(sec)   dL(sec)\n"
    });
    for &(i, j) in nodes {
        text.push_str(&format!(
            "{:08} {:9.5} {:9.5}",
            code(i, j),
            value(i, j, salt),
            value(j, i, salt + 1)
        ));
        if layout.altitude {
            text.push_str(&format!(" {:9.5}", value(i, j, salt + 2)));
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
fn read(text: &str) -> ShiftGrid {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let grid = ShiftGrid::read(text.as_bytes(), &mut cx).unwrap();
    assert!(diag.is_empty());
    grid
}

/// 格子点の番号の範囲 `i`・`j`（3 次メッシュの数）に散らした点。格子の線に乗らないよう、端数をずらす
#[cfg(test)]
fn points(i: (f64, f64), j: (f64, f64)) -> Vec<GeoPoint> {
    let mut points = Vec::new();
    for a in 0..37 {
        for b in 0..41 {
            let y = i.0 + (i.1 - i.0) * (f64::from(a) + 0.373) / 37.0;
            let x = j.0 + (j.1 - j.0) * (f64::from(b) + 0.619) / 41.0;
            points.push(GeoPoint {
                latitude: y / 120.0,
                longitude: 100.0 + x / 80.0,
            });
        }
    }
    points
}

/// 格子 `text` を両方で読み、格子点と、点 `points` の補間の値を照らす。補間の値（秒）の差の最大と、両方が補間した
/// 点の数を返す
#[cfg(test)]
fn compare(layout: &Layout, text: &str, points: &[GeoPoint]) -> (f64, usize) {
    let grid = read(text);
    assert_eq!(grid.spacing(), (30 * layout.step, 45 * layout.step));
    let data = ParData::from_str(text, layout.format).unwrap();
    // 読み: 格子点の数と、格子点ごとの補正量が同じ
    assert_eq!(grid.nodes().count(), data.parameter.len());
    for (code, b, l) in grid.nodes() {
        let parameter = data.parameter.get(&code).unwrap();
        assert_eq!((parameter.latitude, parameter.longitude), (b, l), "{code}");
    }
    // 補間: 四隅の格子点がそろう点は同じ値にし、そろわない点はどちらも補間しない
    let transformer = Transformer::new(data);
    let mut max = 0.0_f64;
    let mut compared = 0;
    for point in points {
        let ours = grid.correction(*point);
        let theirs =
            transformer.forward_corr(&Point::new_unchecked(point.latitude, point.longitude, 0.0));
        match (ours, theirs) {
            (Some((b, l)), Ok(correction)) => {
                max = max
                    .max((b - correction.latitude * 3600.0).abs())
                    .max((l - correction.longitude * 3600.0).abs());
                compared += 1;
            }
            (None, Err(_)) => {}
            (ours, theirs) => panic!("{point:?}: {ours:?} {theirs:?}"),
        }
    }
    (max, compared)
}

/// TKY2JGD の形（ヘッダー 2 行、3 次メッシュ）の格子
#[test]
fn tky2jgd_grids_are_read_and_interpolated_like_jgdtrans() {
    // 東京の辺りの 9 × 9 の格子点
    let (i0, j0) = (4272, 3176);
    let text = par(&TKY2JGD, &block(i0, j0, 4, 1), 1);
    let (i, j) = (f64::from(i0), f64::from(j0));
    let (max, compared) = compare(
        &TKY2JGD,
        &text,
        &points((i - 5.0, i + 5.0), (j - 5.0, j + 5.0)),
    );
    assert!(compared > 500, "{compared}");
    assert!(max < 1e-9, "{max}");
}

/// PatchJGD の形（ヘッダー 16 行、3 次メッシュ）の、ところどころ格子点の欠けた格子。欠けた格子点を囲む区画の点は、
/// どちらも補間しない
#[test]
fn patchjgd_grids_with_holes_are_read_and_interpolated_like_jgdtrans() {
    // 仙台の辺りの 13 × 13 の格子点から、11 個に 1 個ほどを除く
    let (i0, j0) = (4595, 3268);
    let nodes: Vec<_> = block(i0, j0, 6, 1)
        .into_iter()
        .filter(|(i, j)| (i * 3 + j * 5) % 11 != 0)
        .collect();
    let text = par(&PATCHJGD, &nodes, 2);
    let (i, j) = (f64::from(i0), f64::from(j0));
    let (max, compared) = compare(
        &PATCHJGD,
        &text,
        &points((i - 7.0, i + 7.0), (j - 7.0, j + 7.0)),
    );
    assert!(compared > 300, "{compared}");
    assert!(max < 1e-9, "{max}");
}

/// 5 倍地域メッシュ（緯度 150 秒・経度 225 秒）の格子。高さの補正量の列があっても、緯度・経度の補正量は同じに読む
#[test]
fn grids_at_the_five_times_mesh_are_read_and_interpolated_like_jgdtrans() {
    let (i0, j0) = (4595, 3270);
    let text = par(&FIVE_TIMES, &block(i0, j0, 4, 5), 3);
    let (i, j) = (f64::from(i0), f64::from(j0));
    let (max, compared) = compare(
        &FIVE_TIMES,
        &text,
        &points((i - 25.0, i + 25.0), (j - 25.0, j + 25.0)),
    );
    assert!(compared > 500, "{compared}");
    assert!(max < 1e-9, "{max}");
}

/// 平面直角座標を JGD2011 に直した点は、jgdtrans で経緯度を順に直してから平面直角座標にした点と同じになる。PatchJGD
/// のファイルは、渡した順に当てる
#[test]
fn conversions_match_jgdtrans_applied_in_order() {
    let zone = Zone::new(9).unwrap();
    // 東京の辺りの、日本測地系の点
    let tokyo = MapPoint {
        northing: -35_000.0,
        easting: -6_000.0,
    };
    let bessel = to_geographic_on(Ellipsoid::Bessel, zone, tokyo)
        .unwrap()
        .point;
    let (i0, j0) = (
        (bessel.latitude * 120.0) as u32,
        ((bessel.longitude - 100.0) * 80.0) as u32,
    );
    let tky2jgd = par(&TKY2JGD, &block(i0, j0, 6, 1), 4);
    let earlier = par(&PATCHJGD, &block(i0, j0, 6, 1), 5);
    let later = par(&FIVE_TIMES, &block(i0 / 5 * 5, j0 / 5 * 5, 3, 5), 6);
    let grids = [read(&tky2jgd), read(&earlier), read(&later)];
    let transformers = [
        (tky2jgd, TKY2JGD.format),
        (earlier, PATCHJGD.format),
        (later, FIVE_TIMES.format),
    ]
    .map(|(text, format)| Transformer::from_str(&text, format).unwrap());
    let mut compared = 0;
    for (dn, de) in [
        (0.0, 0.0),
        (700.0, -900.0),
        (-1_300.0, 400.0),
        (950.0, 1_250.0),
    ] {
        let point = MapPoint {
            northing: tokyo.northing + dn,
            easting: tokyo.easting + de,
        };
        // 日本測地系: TKY2JGD と、2 つの PatchJGD を順に当てる
        let start = to_geographic_on(Ellipsoid::Bessel, zone, point)
            .unwrap()
            .point;
        let mut theirs = Point::new_unchecked(start.latitude, start.longitude, 0.0);
        for transformer in &transformers {
            theirs = transformer.forward(&theirs).unwrap();
        }
        let shifts = Shifts {
            tky2jgd: Some(&grids[0]),
            patch: &[&grids[1], &grids[2]],
        };
        let expected = plane(zone, &theirs);
        let ours = convert(Crs::Tokyo(zone), point, &shifts);
        assert!(distance(ours, expected) < 1e-6, "{ours:?} {expected:?}");
        // JGD2000: PatchJGD だけを順に当てる
        let start = to_geographic_on(Ellipsoid::Grs80, zone, point)
            .unwrap()
            .point;
        let mut theirs = Point::new_unchecked(start.latitude, start.longitude, 0.0);
        for transformer in &transformers[1..] {
            theirs = transformer.forward(&theirs).unwrap();
        }
        let shifts = Shifts {
            tky2jgd: None,
            patch: &[&grids[1], &grids[2]],
        };
        let expected = plane(zone, &theirs);
        let ours = convert(Crs::Jgd2000(zone), point, &shifts);
        assert!(distance(ours, expected) < 1e-6, "{ours:?} {expected:?}");
        compared += 1;
    }
    assert_eq!(compared, 4);
}

#[cfg(test)]
fn plane(zone: Zone, point: &Point) -> MapPoint {
    to_plane(
        zone,
        GeoPoint {
            latitude: point.latitude,
            longitude: point.longitude,
        },
    )
    .unwrap()
    .point
}

#[cfg(test)]
fn convert(crs: Crs, point: MapPoint, shifts: &Shifts<'_>) -> MapPoint {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let (crs, moved) = to_jgd2011(crs, point, shifts, &mut cx).unwrap();
    assert!(diag.is_empty());
    assert!(matches!(crs, Crs::Plane(_)));
    moved
}

#[cfg(test)]
fn distance(a: MapPoint, b: MapPoint) -> f64 {
    libm::hypot(a.northing - b.northing, a.easting - b.easting)
}
