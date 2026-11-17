//! 対応点から、図面（プロジェクトの空間）から地図への変換を求める（#685）。
//!
//! 図面の点と地図の点の組から、尺度・回転・平行移動の相似の変換を最小二乗で求める。変換は [`MapConversion`]（IFC の
//! `IfcMapConversion`）と同じ形で返す。鏡映は含めない。高さは求めず、`orthogonal_height` を 0 にする。
//!
//! 相似の変換の未知数は 4 つ（尺度と回転を表す 2 つと、平行移動の 2 つ）なので、図面の上で離れた 2 点があれば決まる。
//! 一直線に並ぶ 3 点以上でも決まる。図面の点がすべて重なるときは、尺度と回転が決まらない。

use zumen_base::{Cx, Diagnostic, Kind, codes};

use crate::{MapConversion, MapPoint, ProjectPoint};

/// 対応点の組。図面の点は、プロジェクトの空間の X・Y だけを使い、Z は見ない
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlPoint {
    /// 図面（プロジェクトの空間）の点
    pub drawing: ProjectPoint,
    /// 地図の点
    pub map: MapPoint,
}

/// 対応点から求めた変換と、点ごとの残差
#[derive(Clone, Debug, PartialEq)]
pub struct Fitted {
    /// 図面から地図への変換
    pub conversion: MapConversion,
    /// 点ごとの残差（地図の点から、変換で写した図面の点を引いたもの。m）。渡した組の順
    pub residuals: Vec<MapPoint>,
}

/// 対応点の組 `points` から、図面から地図への相似の変換を最小二乗で求める。
///
/// 値が有限でない組があれば `crs.control-point.invalid`（`index` は 0 から数えた組の番号）を、組が 2 つより
/// 少ないか、図面の点がすべて重なるか、求めた変換が有限でなければ `crs.control-points.failed` を診断して `None` を
/// 返す。組の数だけ予算の作業量（`work`）を数え、止まれば理由を診断して `None` を返す
pub fn fit_map_conversion(points: &[ControlPoint], cx: &mut Cx) -> Option<Fitted> {
    if let Err(stop) = cx.check(Kind::Work, points.len() as u64) {
        cx.report(stop.to_diagnostic());
        return None;
    }
    let failed = |cx: &mut Cx| {
        cx.report(
            Diagnostic::new(codes::CRS_CONTROL_POINTS_FAILED).with("points", points.len() as u64),
        );
    };
    if let Some(index) = points.iter().position(|p| {
        ![p.drawing.x, p.drawing.y, p.map.easting, p.map.northing]
            .iter()
            .all(|v| v.is_finite())
    }) {
        cx.report(Diagnostic::new(codes::CRS_CONTROL_POINT_INVALID).with("index", index as u64));
        return None;
    }
    if points.len() < 2 {
        failed(cx);
        return None;
    }
    // 重心を引いてから和を取り、地図の座標の大きな値で桁を落とさない
    let n = points.len() as f64;
    let mean = |f: fn(&ControlPoint) -> f64| points.iter().map(f).sum::<f64>() / n;
    let (x0, y0) = (mean(|p| p.drawing.x), mean(|p| p.drawing.y));
    let (e0, n0) = (mean(|p| p.map.easting), mean(|p| p.map.northing));
    let (mut norm, mut cos, mut sin) = (0.0, 0.0, 0.0);
    for p in points {
        let (x, y) = (p.drawing.x - x0, p.drawing.y - y0);
        let (e, n) = (p.map.easting - e0, p.map.northing - n0);
        norm += x * x + y * y;
        cos += x * e + y * n;
        sin += x * n - y * e;
    }
    // easting = a x − b y + E、northing = b x + a y + N。a・b は尺度と回転の余弦・正弦の積
    let (a, b) = (cos / norm, sin / norm);
    let scale = libm::sqrt(a * a + b * b);
    if !(norm > 0.0 && scale.is_finite() && scale > 0.0) {
        failed(cx);
        return None;
    }
    let conversion = MapConversion {
        eastings: e0 - (a * x0 - b * y0),
        northings: n0 - (b * x0 + a * y0),
        orthogonal_height: 0.0,
        x_axis_abscissa: a / scale,
        x_axis_ordinate: b / scale,
        scale,
    };
    let mut residuals = Vec::with_capacity(points.len());
    for p in points {
        let Some(mapped) = conversion.to_map(ProjectPoint {
            z: 0.0,
            ..p.drawing
        }) else {
            failed(cx);
            return None;
        };
        residuals.push(MapPoint {
            northing: p.map.northing - mapped.point.northing,
            easting: p.map.easting - mapped.point.easting,
        });
    }
    Some(Fitted {
        conversion,
        residuals,
    })
}
