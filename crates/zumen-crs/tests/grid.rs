//! ISG format 2.0 の格子の読み取りと双一次の補間を、合成の格子で確かめる。国土地理院のジオイド・モデルは
//! 使わない（決定 0019）。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Value, codes};
use zumen_crs::{GeoPoint, Grid};
use zumen_sdk::Error;

/// ISG format 2.0 の例（仕様の Example 1・2）と同じ形の、合成の格子。4 行 6 列で、南の 2 行の東の 2 つは無効
fn isg(units: &str, extra: &str) -> String {
    let (lat_min, lat_max, lon_min, lon_max, step) = if units == "dms" {
        (
            "40°00'00\"",
            "41°00'00\"",
            "120°00'00\"",
            "121°40'00\"",
            "0°20'00\"",
        )
    } else {
        (
            "40.000000",
            "41.000000",
            "120.000000",
            "121.666667",
            "0.333333",
        )
    };
    format!(
        "These data are synthetic.\n\
begin_of_head ================================================\n\
model name     : SYNTHETIC\n\
model year     : 2026\n\
model type     : gravimetric\n\
data type      : geoid\n\
data units     : meters\n\
data format    : grid\n\
data ordering  : N-to-S, W-to-E\n\
ref ellipsoid  : GRS80\n\
coord type     : geodetic\n\
coord units    : {units}\n\
map projection : ---\n\
lat min        = {lat_min}\n\
lat max        = {lat_max}\n\
lon min        = {lon_min}\n\
lon max        = {lon_max}\n\
delta lat      = {step}\n\
delta lon      = {step}\n\
nrows          =            4\n\
ncols          =            6\n\
nodata         = -9999.0000\n\
{extra}\
ISG format     =          2.0\n\
end_of_head ==================================================\n\
   30.0000    31.0000    32.0000    33.0000    34.0000    35.0000\n\
   40.0000    41.0000    42.0000    43.0000    44.0000    45.0000\n\
   50.0000    51.0000    52.0000    53.0000 -9999.0000 -9999.0000\n\
   60.0000    61.0000    62.0000    63.0000 -9999.0000 -9999.0000\n"
    )
}

fn read(src: &str) -> (Result<Grid, Error>, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let grid = Grid::read(src.as_bytes(), &mut cx);
    (grid, diag)
}

fn at(latitude: f64, longitude: f64) -> GeoPoint {
    GeoPoint {
        latitude,
        longitude,
    }
}

#[test]
fn grid_points_and_cells_are_interpolated_bilinearly() {
    for units in ["deg", "dms"] {
        let (grid, diag) = read(&isg(units, ""));
        let grid = grid.unwrap();
        assert!(diag.is_empty(), "{units}");
        assert_eq!(grid.model().name, "SYNTHETIC");
        assert_eq!(grid.model().year.as_deref(), Some("2026"));
        let max_lon = if units == "dms" {
            120.0 + 100.0 / 60.0
        } else {
            121.666667
        };
        let step = (max_lon - 120.0) / 5.0;
        // 北西の隅と南西の隅
        assert_eq!(grid.interpolate(at(41.0, 120.0)).unwrap().value, 30.0);
        assert_eq!(grid.interpolate(at(40.0, 120.0)).unwrap().value, 60.0);
        // 4 つの格子点の真ん中は、平均
        let middle = grid
            .interpolate(at(41.0 - 1.0 / 6.0, 120.0 + step / 2.0))
            .unwrap();
        assert!((middle.value - 35.5).abs() < 1e-9, "{units} {middle:?}");
        // 国土地理院の説明書の式: Z = (1-t)(1-u)Z(i,j) + (1-t)u Z(i,j+1) + t(1-u)Z(i+1,j) + tu Z(i+1,j+1)
        let (t, u) = (0.25, 0.75);
        let z = (1.0 - t) * (1.0 - u) * 41.0
            + (1.0 - t) * u * 42.0
            + t * (1.0 - u) * 51.0
            + t * u * 52.0;
        let point = at(41.0 - (1.0 + t) / 3.0, 120.0 + (1.0 + u) * step);
        let value = grid.interpolate(point).unwrap().value;
        assert!((value - z).abs() < 1e-9, "{units} {value} {z}");
        assert_eq!(grid.interpolate(point).unwrap().model.name, "SYNTHETIC");
    }
}

/// 重みが 0 でない格子点のどれかが無効なら求めない。無効の点を結ぶ線の上でも、重みが 0 の点は見ない
#[test]
fn cells_touching_no_data_are_not_interpolated() {
    let (grid, _) = read(&isg("deg", ""));
    let grid = grid.unwrap();
    let step = (121.666667 - 120.0) / 5.0;
    // 無効の点を隅に持つ格子
    assert_eq!(grid.interpolate(at(40.5, 120.0 + 3.5 * step)), None);
    // 無効の点の西の線の上（重みが 0 の東の点は見ない）
    let edge = grid.interpolate(at(40.5, 120.0 + 3.0 * step)).unwrap();
    assert!((edge.value - 48.0).abs() < 1e-9, "{edge:?}");
    // 範囲の外
    assert_eq!(grid.interpolate(at(41.0001, 120.0)), None);
    assert_eq!(grid.interpolate(at(40.5, 119.9)), None);
    assert_eq!(grid.interpolate(at(f64::NAN, 120.0)), None);
}

#[test]
fn broken_grids_are_reported() {
    let good = isg("deg", "");
    let cases = [
        (good.replace("begin_of_head", "start"), "begin_of_head"),
        (good.replace("end_of_head", "finish"), "header"),
        (
            good.replace(
                "nrows          =            4",
                "nrows          =            5",
            ),
            "data",
        ),
        (good.replace("61.0000", "sixty"), "data"),
        (
            good.replace("delta lat      = 0.333333", "delta lat      = 0.1"),
            "delta lat",
        ),
        (
            good.replace("lat max        = 41.000000", "lat max        = 39.000000"),
            "lat max",
        ),
        (good.replace("coord units    : deg\n", ""), "coord units"),
    ];
    for (src, key) in cases {
        let (grid, diag) = read(&src);
        assert_eq!(grid.err(), Some(Error::Invalid), "{key}");
        let found = diag
            .iter()
            .find(|d| d.code() == codes::CRS_GEOID_INVALID)
            .unwrap();
        assert_eq!(
            found.param("key"),
            Some(&Value::Text(key.to_owned())),
            "{key}"
        );
    }
}

#[test]
fn other_kinds_of_grids_are_unsupported() {
    let good = isg("deg", "");
    let cases = [
        good.replace("data format    : grid", "data format    : sparse"),
        good.replace("coord type     : geodetic", "coord type     : projected"),
        good.replace("data units     : meters", "data units     : feet"),
        good.replace(
            "data ordering  : N-to-S, W-to-E",
            "data ordering  : S-to-N, W-to-E",
        ),
        good.replace(
            "ISG format     =          2.0",
            "ISG format     =          1.01",
        ),
    ];
    for src in cases {
        let (grid, diag) = read(&src);
        assert_eq!(grid.err(), Some(Error::Unsupported));
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::CRS_GEOID_UNSUPPORTED)
        );
    }
    // 並びの書き方は、仕様の例の両方を受け付ける
    let (grid, _) = read(&good.replace("N-to-S, W-to-E", "N-S, W-E"));
    assert!(grid.is_ok());
}

/// 値の数は、予算のレコードで数える
#[test]
fn values_are_counted_in_the_budget() {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::new(Limits::default().with(Kind::Records, Limit::Finite(10)));
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let grid = Grid::read(isg("deg", "").as_bytes(), &mut cx);
    assert!(matches!(grid, Err(Error::Stopped(_))));
}
