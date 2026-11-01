//! 測地系の間の変換（決定 0019・0233）。日本測地系と JGD2000 の平面直角座標を、JGD2011 の平面直角座標に直す。
//!
//! 変換のパラメーターは、国土地理院の TKY2JGD（日本測地系から JGD2000）と PatchJGD（JGD2000 から JGD2011）の
//! 形の格子で、3 次メッシュのコードの格子点ごとに、緯度と経度の補正量（秒）を持つ。求める点を囲む四隅の格子点の
//! 値で双一次に補間し、経緯度に足す。格子は利用者が入手して渡し、zumen は同梱しない。
//!
//! - 日本測地系: ベッセル楕円体で経緯度に直し、TKY2JGD で JGD2000 にし、PatchJGD で JGD2011 にする。
//! - JGD2000: GRS80 で経緯度に直し、PatchJGD で JGD2011 にする。
//!
//! PatchJGD の格子は、地殻変動のあった地域だけを持つ。点を囲む格子点が 1 つもなければ、補正量を 0 とする。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Kind, Stop, codes};
use zumen_sdk::Error;

use crate::projection::{Ellipsoid, to_geographic_on, to_plane};
use crate::{Crs, GeoPoint, MapPoint};

/// 格子点の数の上限。3 次メッシュで日本の全域を覆っても、これより少ない
const MAX_NODES: usize = 1 << 22;

/// 変換のパラメーターの格子（TKY2JGD・PatchJGD の形）
#[derive(Clone, Debug, PartialEq)]
pub struct ShiftGrid {
    /// 格子点（緯度の 30 秒ごとの番号、経度 100 度から 45 秒ごとの番号）と、緯度・経度の補正量（秒）
    nodes: BTreeMap<(u32, u32), (f64, f64)>,
}

/// 変換に使う格子
#[derive(Clone, Copy, Debug, Default)]
pub struct Shifts<'a> {
    /// 日本測地系から JGD2000（TKY2JGD）
    pub tky2jgd: Option<&'a ShiftGrid>,
    /// JGD2000 から JGD2011（PatchJGD）
    pub patch: Option<&'a ShiftGrid>,
}

fn stopped(cx: &mut Cx, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

/// 3 次メッシュのコード（8 桁）の南西の隅の、格子点の番号
fn node(code: &str) -> Option<(u32, u32)> {
    let digits: Vec<u32> = code
        .chars()
        .map(|c| c.to_digit(10))
        .collect::<Option<_>>()?;
    let [p, q, r, s, t, u, v, w] = digits.as_slice() else {
        return None;
    };
    if *t > 7 || *u > 7 {
        return None;
    }
    Some((
        (p * 10 + q) * 80 + t * 10 + v,
        (r * 10 + s) * 80 + u * 10 + w,
    ))
}

impl ShiftGrid {
    /// TKY2JGD・PatchJGD の形の格子を読む。初めの数字の行の前は見出しとして飛ばす。数字の行は、3 次メッシュの
    /// コードと、緯度・経度の補正量（秒）。高さの補正量の列があっても読まない
    pub fn read(src: &[u8], cx: &mut Cx) -> Result<ShiftGrid, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let mut nodes = BTreeMap::new();
        let mut started = false;
        for (index, line) in src.split(|b| *b == b'\n').enumerate() {
            let invalid = |cx: &mut Cx| {
                cx.report(
                    Diagnostic::new(codes::CRS_DATUM_GRID_INVALID).with("line", index as u64 + 1),
                );
                Error::Invalid
            };
            let text = core::str::from_utf8(line).ok();
            let fields: Vec<&str> = text
                .map(|t| t.split_whitespace().collect())
                .unwrap_or_default();
            let parsed = match fields.as_slice() {
                [code, b, l] | [code, b, l, _] => node(code).zip(
                    b.parse::<f64>()
                        .ok()
                        .zip(l.parse::<f64>().ok())
                        .filter(|(b, l)| b.is_finite() && l.is_finite()),
                ),
                _ => None,
            };
            match parsed {
                Some((at, shift)) => {
                    started = true;
                    cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                    if nodes.len() >= MAX_NODES || nodes.insert(at, shift).is_some() {
                        return Err(invalid(cx));
                    }
                }
                None if !started || fields.is_empty() => {}
                None => return Err(invalid(cx)),
            }
        }
        if nodes.is_empty() {
            cx.report(Diagnostic::new(codes::CRS_DATUM_GRID_INVALID).with("line", 1_u64));
            return Err(Error::Invalid);
        }
        Ok(ShiftGrid { nodes })
    }

    /// 点 `at` の緯度・経度の補正量（秒）を、囲む四隅の格子点から双一次に補間する。重みが 0 でない格子点の
    /// どれかがなければ `None`。囲む格子点が 1 つもなければ `Some(None)` にせず、呼ぶ側が決められるよう
    /// 見つかった数を返す
    fn interpolate(&self, at: GeoPoint) -> (usize, Option<(f64, f64)>) {
        let y = at.latitude * 120.0;
        let x = (at.longitude - 100.0) * 80.0;
        if !(y.is_finite() && x.is_finite() && y >= 0.0 && x >= 0.0) {
            return (0, None);
        }
        let (i, j) = (libm::floor(y), libm::floor(x));
        let (t, u) = (y - i, x - j);
        let (i, j) = (i as u32, j as u32);
        let corners = [
            (i, j, (1.0 - t) * (1.0 - u)),
            (i, j + 1, (1.0 - t) * u),
            (i + 1, j, t * (1.0 - u)),
            (i + 1, j + 1, t * u),
        ];
        let found = corners
            .iter()
            .filter(|(i, j, _)| self.nodes.contains_key(&(*i, *j)))
            .count();
        let mut shift = (0.0, 0.0);
        for (i, j, weight) in corners {
            if weight == 0.0 {
                continue;
            }
            let Some((b, l)) = self.nodes.get(&(i, j)) else {
                return (found, None);
            };
            shift.0 += weight * b;
            shift.1 += weight * l;
        }
        (found, Some(shift))
    }
}

/// 格子で経緯度を直す。`partial` が真なら、囲む格子点が 1 つもない点の補正量を 0 とする
fn shift(
    grid: &ShiftGrid,
    name: &str,
    point: GeoPoint,
    partial: bool,
    cx: &mut Cx,
) -> Option<GeoPoint> {
    let (found, value) = grid.interpolate(point);
    let (b, l) = match value {
        Some(value) => value,
        None if partial && found == 0 => (0.0, 0.0),
        None => {
            cx.report(
                Diagnostic::new(codes::CRS_DATUM_SHIFT_MISSING)
                    .with("grid", name)
                    .with("latitude", point.latitude)
                    .with("longitude", point.longitude),
            );
            return None;
        }
    };
    Some(GeoPoint {
        latitude: point.latitude + b / 3600.0,
        longitude: point.longitude + l / 3600.0,
    })
}

/// 座標系 `crs` の点を、同じ系の JGD2011 の平面直角座標に直す。JGD2011 の平面直角座標ならそのまま返す。
/// 要る格子がないか、点が格子の範囲の外なら、診断して `None` を返す。呼ぶ側は、座標系の違う点を比べない
pub fn to_jgd2011(
    crs: Crs,
    point: MapPoint,
    shifts: &Shifts<'_>,
    cx: &mut Cx,
) -> Option<(Crs, MapPoint)> {
    let missing = |grid: &str, cx: &mut Cx| {
        cx.report(
            Diagnostic::new(codes::CRS_DATUM_GRID_MISSING)
                .with("crs", u64::from(crs.epsg()))
                .with("grid", grid),
        );
    };
    let (zone, geographic) = match crs {
        Crs::Geographic => return None,
        Crs::Plane(_) => return Some((crs, point)),
        Crs::Jgd2000(zone) => (zone, to_geographic_on(Ellipsoid::Grs80, zone, point)?.point),
        Crs::Tokyo(zone) => {
            let Some(grid) = shifts.tky2jgd else {
                missing("TKY2JGD", cx);
                return None;
            };
            let tokyo = to_geographic_on(Ellipsoid::Bessel, zone, point)?.point;
            (zone, shift(grid, "TKY2JGD", tokyo, false, cx)?)
        }
    };
    let Some(patch) = shifts.patch else {
        missing("PatchJGD", cx);
        return None;
    };
    let jgd2011 = shift(patch, "PatchJGD", geographic, true, cx)?;
    Some((Crs::Plane(zone), to_plane(zone, jgd2011)?.point))
}
