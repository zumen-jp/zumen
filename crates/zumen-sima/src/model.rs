//! SIMA の両形式（カンマ区切り `.sim` と JPGIS 版 XML）が共有する中間データと幾何補助計算。

use core::f64::consts::PI;

use zumen_base::{Kind, Ref};
use zumen_crs::{Axes, Crs, MapPoint, Realization};
use zumen_sdk::{Cx, Error};

use crate::stopped;

/// 単一確保の安全上限（`i32::MAX`）
pub(crate) const MAX_ALLOC: u64 = i32::MAX as u64;

/// 曲線を折れ線で近似するときの弦誤差の上限（メートル）
pub(crate) const CURVE_TOLERANCE_M: f64 = 0.01;

/// `-0.0` を `0.0` に正規化する
pub(crate) fn norm_zero(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}

/// 測量の北（X）・東（Y）の組から地図空間の [`MapPoint`] を作る。有限でなければ `None`
pub(crate) fn parse_map_point(x_north: f64, y_east: f64) -> Option<MapPoint> {
    if !x_north.is_finite() || !y_east.is_finite() {
        return None;
    }
    Some(Axes::NorthEast.map_point([norm_zero(x_north), norm_zero(y_east)], 1.0))
}

/// 地図平面上の点と任意の標高
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MapLocation {
    pub point: MapPoint,
    pub altitude: Option<f64>,
}

/// 属性の値
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AttributeVal {
    Text(String),
    Integer(i64),
    Real(f64),
    Date(String),
}

/// 測点の種別
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PointKind {
    Sokuten,
    Kijyunten,
    Suijyunten,
}

impl PointKind {
    pub(crate) fn geo_kind(self) -> &'static str {
        match self {
            PointKind::Sokuten => "sima:Sokuten",
            PointKind::Kijyunten => "sima:Kijyunten",
            PointKind::Suijyunten => "sima:Suijyunten",
        }
    }
}

/// 測点・基準点・水準点
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SurveyPoint {
    pub id_hint: Option<String>,
    pub number: String,
    pub name: String,
    pub position: MapPoint,
    pub altitude: Option<f64>,
    pub kind: PointKind,
    pub attributes: Vec<(String, AttributeVal)>,
    pub source: Ref,
}

/// 区画・折れ線の種別
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParcelKind {
    Kakuchi,
    KaihouKakuchi,
    Chiban,
}

impl ParcelKind {
    pub(crate) fn geo_kind(self) -> &'static str {
        match self {
            ParcelKind::Kakuchi => "sima:Kakuchi",
            ParcelKind::KaihouKakuchi => "sima:KaihouKakuchi",
            ParcelKind::Chiban => "sima:Chiban",
        }
    }
}

/// 画地・開放画地・地番
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Parcel {
    pub id_hint: Option<String>,
    pub number: String,
    pub name: Option<String>,
    pub closed: bool,
    pub outer: Vec<MapLocation>,
    pub holes: Vec<Vec<MapLocation>>,
    /// 開放画地に円弧区間が含まれる場合の区間列
    pub curve_segments: Vec<RouteSegment>,
    /// 境界リングの円弧を折れ線へ離散化したか
    pub ring_curve_approximated: bool,
    pub kind: ParcelKind,
    pub attributes: Vec<(String, AttributeVal)>,
    pub source: Ref,
}

/// 路線の 1 区間の種類
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RouteSegmentKind {
    Line,
    /// 単曲線。`radius` は進行方向に対して右へ曲がるものを正、左へ曲がるものを負とする
    Arc {
        radius: f64,
    },
    /// クロソイド。`parameter` は A（正）、符号付き半径は右カーブ正・直線端 `None`
    Clothoid {
        parameter: f64,
        start_radius: Option<f64>,
        end_radius: Option<f64>,
    },
}

/// 路線の 1 区間
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RouteSegment {
    pub start: MapPoint,
    pub end: Option<MapPoint>,
    /// 北から時計回りの方向角（ラジアン）
    pub direction: f64,
    /// 平面距離（メートル）
    pub length: f64,
    pub kind: RouteSegmentKind,
    pub source: Ref,
}

/// 路線（線形）
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Route {
    pub number: String,
    pub name: Option<String>,
    pub start_station: f64,
    pub segments: Vec<RouteSegment>,
    pub attributes: Vec<(String, AttributeVal)>,
    pub source: Ref,
}

/// 読み取った SIMA の中間表現
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SimaData {
    pub version: Option<String>,
    pub site_name: Option<String>,
    pub site_attributes: Vec<(String, AttributeVal)>,
    pub crs: Option<Crs>,
    pub realization: Option<Realization>,
    /// 既に `sima.crs.unsupported` を報告したか（二重に `sima.crs.missing` を出さないため）
    pub crs_reported: bool,
    pub points: Vec<SurveyPoint>,
    pub parcels: Vec<Parcel>,
    pub routes: Vec<Route>,
}

/// 2 点間の北から時計回りの方向角（ラジアン）と平面距離（メートル）を求める
pub(crate) fn bearing_and_distance(a: MapPoint, b: MapPoint) -> Option<(f64, f64)> {
    let dn = b.northing - a.northing;
    let de = b.easting - a.easting;
    let dist = libm::hypot(dn, de);
    if !dist.is_finite() || dist <= 1e-12 {
        return None;
    }
    let mut dir = libm::atan2(de, dn);
    if dir < 0.0 {
        dir += 2.0 * PI;
    }
    Some((norm_zero(dir), norm_zero(dist)))
}

/// 始点 `a`・終点 `b` と符号付き半径 `signed_radius`（右カーブ正、左カーブ負）から、始点接線の方向角と弧長を求める
pub(crate) fn chord_arc_params(a: MapPoint, b: MapPoint, signed_radius: f64) -> Option<(f64, f64)> {
    let r_abs = libm::fabs(signed_radius);
    let denom = 2.0 * r_abs;
    if !r_abs.is_finite() || r_abs <= 1e-9 || !denom.is_finite() {
        return None;
    }
    let (chord_dir, chord) = bearing_and_distance(a, b)?;
    let ratio = chord / denom;
    if ratio > 1.0 + 1e-7 {
        return None;
    }
    let clamped = ratio.clamp(0.0, 1.0);
    let half_delta = libm::asin(clamped);
    let start_dir = if signed_radius >= 0.0 {
        chord_dir - half_delta
    } else {
        chord_dir + half_delta
    };
    let mut norm_dir = start_dir % (2.0 * PI);
    if norm_dir < 0.0 {
        norm_dir += 2.0 * PI;
    }
    let arc_len = 2.0 * half_delta * r_abs;
    if !arc_len.is_finite() || arc_len <= 1e-12 {
        return None;
    }
    Some((norm_zero(norm_dir), norm_zero(arc_len)))
}

/// 3 点（始点 `p0`・通過点 `p1`・終点 `p2`）を通る円弧のパラメーター（始点接線の方向角・弧長・符号付き半径）を求める
pub(crate) fn three_point_arc(p0: MapPoint, p1: MapPoint, p2: MapPoint) -> Option<(f64, f64, f64)> {
    // 東（x）・北（y）の右手系で外接円の中心を求める
    let (x1, y1) = (p0.easting, p0.northing);
    let (x2, y2) = (p1.easting, p1.northing);
    let (x3, y3) = (p2.easting, p2.northing);
    let d = 2.0 * (x1 * (y2 - y3) + x2 * (y3 - y1) + x3 * (y1 - y2));
    if !d.is_finite() || libm::fabs(d) <= 1e-9 {
        return None;
    }
    let s1 = x1 * x1 + y1 * y1;
    let s2 = x2 * x2 + y2 * y2;
    let s3 = x3 * x3 + y3 * y3;
    let cx = (s1 * (y2 - y3) + s2 * (y3 - y1) + s3 * (y1 - y2)) / d;
    let cy = (s1 * (x3 - x2) + s2 * (x1 - x3) + s3 * (x2 - x1)) / d;
    if !cx.is_finite() || !cy.is_finite() {
        return None;
    }
    let radius = libm::hypot(x1 - cx, y1 - cy);
    if !radius.is_finite() || radius <= 1e-9 {
        return None;
    }
    let a0 = libm::atan2(y1 - cy, x1 - cx);
    let a1 = libm::atan2(y2 - cy, x2 - cx);
    let a2 = libm::atan2(y3 - cy, x3 - cx);

    let ccw_span = |from: f64, to: f64| -> f64 {
        let mut diff = to - from;
        while diff < 0.0 {
            diff += 2.0 * PI;
        }
        while diff >= 2.0 * PI {
            diff -= 2.0 * PI;
        }
        diff
    };
    let span01_ccw = ccw_span(a0, a1);
    let span02_ccw = ccw_span(a0, a2);
    // 東・北の右手系で CCW は進行方向左カーブ（負の半径）、CW は右カーブ（正の半径）
    let (signed_radius, sweep, tangent_en) = if span01_ccw < span02_ccw {
        (-radius, span02_ccw, a0 + PI / 2.0)
    } else {
        let cw_sweep = 2.0 * PI - span02_ccw;
        (radius, cw_sweep, a0 - PI / 2.0)
    };
    let tx = libm::cos(tangent_en);
    let ty = libm::sin(tangent_en);
    // 北から時計回りの方向角に変換: atan2(east, north) = atan2(tx, ty)
    let mut bearing = libm::atan2(tx, ty);
    if bearing < 0.0 {
        bearing += 2.0 * PI;
    }
    let arc_len = radius * sweep;
    if !arc_len.is_finite() || arc_len <= 1e-12 {
        return None;
    }
    Some((
        norm_zero(bearing),
        norm_zero(arc_len),
        norm_zero(signed_radius),
    ))
}

/// 始点 `start`・始点接線の北から時計回り方向角 `direction`・弧長 `length`・符号付き半径 `signed_radius` から、
/// 図面空間（東 X・北 Y）の円弧（中心・半径・始角［度］・終角［度］・反時計回りフラグ）と終点を求める
pub(crate) fn arc_to_drawing(
    start: MapPoint,
    direction: f64,
    length: f64,
    signed_radius: f64,
) -> Option<([f64; 2], f64, f64, f64, bool, MapPoint)> {
    let r = libm::fabs(signed_radius);
    if !r.is_finite() || r <= 1e-9 || !length.is_finite() || length <= 1e-12 {
        return None;
    }
    // 進行方向の単位ベクトル（東, 北）= (sin(direction), cos(direction))
    let dir_e = libm::sin(direction);
    let dir_n = libm::cos(direction);
    // 右法線 = (dir_n, -dir_e)、左法線 = (-dir_n, dir_e)
    let (cx, cy, ccw) = if signed_radius >= 0.0 {
        (start.easting + r * dir_n, start.northing - r * dir_e, false)
    } else {
        (start.easting - r * dir_n, start.northing + r * dir_e, true)
    };
    if !cx.is_finite() || !cy.is_finite() {
        return None;
    }
    let a0 = libm::atan2(start.northing - cy, start.easting - cx);
    let sweep = length / r;
    if !sweep.is_finite() {
        return None;
    }
    let a1 = if ccw { a0 + sweep } else { a0 - sweep };
    let end_n = cy + r * libm::sin(a1);
    let end_e = cx + r * libm::cos(a1);
    if !end_n.is_finite() || !end_e.is_finite() {
        return None;
    }
    let end = MapPoint {
        northing: norm_zero(end_n),
        easting: norm_zero(end_e),
    };
    let to_deg = |rad: f64| -> f64 {
        let mut deg = rad * (180.0 / PI) % 360.0;
        if deg < 0.0 {
            deg += 360.0;
        }
        norm_zero(deg)
    };
    Some((
        [norm_zero(cx), norm_zero(cy)],
        norm_zero(r),
        to_deg(a0),
        to_deg(a1),
        ccw,
        end,
    ))
}

/// 円弧区間を `CURVE_TOLERANCE_M` 以内の折れ線点列に離散化し、予算（`Work`・`Segments`・`DecodedBytes`）を計上する
pub(crate) fn sample_arc_locations(
    start: MapPoint,
    direction: f64,
    length: f64,
    signed_radius: f64,
    altitudes: (Option<f64>, Option<f64>),
    at: &Ref,
    cx: &mut Cx,
) -> Result<Vec<MapLocation>, Error> {
    let (alt_start, alt_end) = altitudes;
    let Some((center_en, r, start_deg, _, ccw, end_pt)) =
        arc_to_drawing(start, direction, length, signed_radius)
    else {
        return Ok(vec![MapLocation {
            point: start,
            altitude: alt_start,
        }]);
    };
    let sweep_rad = (length / r).abs();
    let step = if CURVE_TOLERANCE_M < r {
        (2.0 * libm::acos(1.0 - CURVE_TOLERANCE_M / r)).clamp(f64::MIN_POSITIVE, PI / 2.0)
    } else {
        PI / 2.0
    };
    let steps = (libm::ceil(sweep_rad / step).min(zumen_geom2d::MAX_STEPS as f64) as u64).max(1);
    cx.check(Kind::Work, steps)
        .map_err(|s| stopped(cx, s, Some(at.clone())))?;
    cx.check(Kind::Segments, steps)
        .map_err(|s| stopped(cx, s, Some(at.clone())))?;
    cx.check(Kind::DecodedBytes, (steps + 1) * 32)
        .map_err(|s| stopped(cx, s, Some(at.clone())))?;

    let sweep_deg = if ccw {
        sweep_rad * (180.0 / PI)
    } else {
        -sweep_rad * (180.0 / PI)
    };
    let mut xy = Vec::new();
    zumen_geom2d::arc(
        center_en,
        r,
        start_deg,
        sweep_deg,
        CURVE_TOLERANCE_M,
        &mut xy,
    );
    let n = xy.len();
    let mut out = Vec::with_capacity(n);
    for (idx, &[e, north]) in xy.iter().enumerate() {
        let point = if idx == 0 {
            start
        } else if idx + 1 == n {
            end_pt
        } else {
            MapPoint {
                northing: norm_zero(north),
                easting: norm_zero(e),
            }
        };
        let altitude = match (alt_start, alt_end) {
            (Some(z0), Some(z1)) if n > 1 => {
                let t = idx as f64 / (n - 1) as f64;
                Some(norm_zero(z0 + (z1 - z0) * t))
            }
            (Some(z0), None) if idx == 0 => Some(z0),
            (None, Some(z1)) if idx + 1 == n => Some(z1),
            _ => None,
        };
        out.push(MapLocation { point, altitude });
    }
    Ok(out)
}

/// クロソイド区間から曲率 `[k1, k2]`（右カーブ正、左カーブ負）を求める
pub(crate) fn clothoid_curvatures(
    parameter: f64,
    length: f64,
    start_radius: Option<f64>,
    end_radius: Option<f64>,
) -> [f64; 2] {
    let k1 = match start_radius {
        Some(r) if r.is_finite() && libm::fabs(r) > 1e-9 => 1.0 / r,
        _ => 0.0,
    };
    let k2 = match end_radius {
        Some(r) if r.is_finite() && libm::fabs(r) > 1e-9 => 1.0 / r,
        _ => {
            if start_radius.is_none() && parameter.is_finite() && parameter > 1e-9 && length > 1e-9
            {
                length / (parameter * parameter)
            } else {
                0.0
            }
        }
    };
    [norm_zero(k1), norm_zero(k2)]
}

/// クロソイド区間を `CURVE_TOLERANCE_M` 以内の折れ線点列に離散化し、予算を計上する
pub(crate) fn sample_clothoid_locations(
    seg: &RouteSegment,
    parameter: f64,
    start_radius: Option<f64>,
    end_radius: Option<f64>,
    cx: &mut Cx,
) -> Result<Vec<MapLocation>, Error> {
    let [k1, k2] = clothoid_curvatures(parameter, seg.length, start_radius, end_radius);
    let origin_work = zumen_geom2d::clothoid_origin_work([k1, k2], seg.length);
    cx.check(Kind::Work, origin_work)
        .map_err(|s| stopped(cx, s, Some(seg.source.clone())))?;
    let Some(form) = zumen_geom2d::clothoid_origin(
        [seg.start.northing, seg.start.easting],
        seg.direction,
        [k1, k2],
        seg.length,
    ) else {
        let mut fallback = vec![MapLocation {
            point: seg.start,
            altitude: None,
        }];
        if let Some(end) = seg.end
            && end != seg.start
        {
            fallback.push(MapLocation {
                point: end,
                altitude: None,
            });
        }
        return Ok(fallback);
    };
    let angle_deg = norm_zero((90.0 - form.angle.to_degrees()).rem_euclid(360.0));
    let ccw = !form.positive;
    let work = zumen_geom2d::clothoid_work(form.parameter, form.start, form.end, CURVE_TOLERANCE_M);
    cx.check(Kind::Work, work)
        .map_err(|s| stopped(cx, s, Some(seg.source.clone())))?;
    let [orig_n, orig_e] = form.origin;
    let xy = zumen_geom2d::clothoid(
        [orig_e, orig_n],
        form.parameter,
        angle_deg,
        ccw,
        form.start,
        form.end,
        CURVE_TOLERANCE_M,
    );
    let seg_count = (xy.len().saturating_sub(1) as u64).max(1);
    cx.check(Kind::Segments, seg_count)
        .map_err(|s| stopped(cx, s, Some(seg.source.clone())))?;
    cx.check(Kind::DecodedBytes, (xy.len() as u64) * 24)
        .map_err(|s| stopped(cx, s, Some(seg.source.clone())))?;
    let n = xy.len();
    let mut out = Vec::with_capacity(n);
    for (idx, &[e, north]) in xy.iter().enumerate() {
        let point = if idx == 0 {
            seg.start
        } else if idx + 1 == n
            && let Some(end_pt) = seg.end
        {
            end_pt
        } else {
            MapPoint {
                northing: norm_zero(north),
                easting: norm_zero(e),
            }
        };
        out.push(MapLocation {
            point,
            altitude: None,
        });
    }
    Ok(out)
}
