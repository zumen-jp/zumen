//! 図面（`Drawing2D`）・土木（`CivilModel`）・地理（`GeoModel`）からカンマ区切り `.sim` への決定的な書き出し。

use core::f64::consts::PI;
use std::collections::BTreeMap;
use std::fmt::Write as _;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_civil::{CivilModel, Horizontal, HorizontalKind, TransitionCurve};
use zumen_crs::{Axes, Crs, MapPoint};
use zumen_drawing::{Drawing2D, Edge, FieldValue, Length, ModelPoint, Shape, Target};
use zumen_geo::{Form, GeoModel, Location, Position as GeoPos};
use zumen_geom2d::Ramp;
use zumen_sdk::{Cx, Error};

use crate::model::{
    CURVE_TOLERANCE_M, MAX_ALLOC, Parcel, RouteSegment, RouteSegmentKind, SurveyPoint,
    arc_to_drawing, norm_zero,
};
use crate::{SimaFile, stopped};

/// SIMA へ書き出した結果
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredSima {
    /// UTF-8（CRLF 改行）の `.sim` バイト列
    pub bytes: Vec<u8>,
    /// 書き出した測点（`A01`）の数
    pub points: usize,
    /// 書き出した画地（`D00`）の数
    pub parcels: usize,
    /// 書き出した路線（`F00`）の数
    pub routes: usize,
}

struct CheckedWriter<'a, 'b> {
    buf: String,
    cx: &'a mut Cx<'b>,
}

impl<'a, 'b> CheckedWriter<'a, 'b> {
    fn new(cx: &'a mut Cx<'b>) -> Self {
        Self {
            buf: String::new(),
            cx,
        }
    }

    fn push_line(&mut self, line: &str, at: Option<Ref>) -> Result<(), Error> {
        let add = (line.len() + 2) as u64;
        self.cx
            .check(Kind::Work, 1)
            .map_err(|s| stopped(self.cx, s, at.clone()))?;
        self.cx
            .check(Kind::DecodedBytes, add)
            .map_err(|s| stopped(self.cx, s, at.clone()))?;
        if self.buf.len() as u64 + add > MAX_ALLOC {
            let mut d =
                Diagnostic::new(codes::SIMA_FILE_INVALID).with("offset", self.buf.len() as u64);
            if let Some(r) = at {
                d = d.at(r);
            }
            self.cx.report(d);
            return Err(Error::Invalid);
        }
        self.buf.push_str(line);
        self.buf.push_str("\r\n");
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.buf.into_bytes()
    }
}

fn sanitize_csv_field(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ',' | '\r' | '\n' => ' ',
            other => other,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn fmt_num(v: f64) -> String {
    let n = norm_zero(v);
    let mut s = String::new();
    let _ = write!(&mut s, "{n}");
    s
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct PointKey {
    northing_bits: u64,
    easting_bits: u64,
    alt_bits: Option<u64>,
}

impl PointKey {
    fn new(mp: MapPoint, alt: Option<f64>) -> Self {
        Self {
            northing_bits: norm_zero(mp.northing).to_bits(),
            easting_bits: norm_zero(mp.easting).to_bits(),
            alt_bits: alt.map(|z| norm_zero(z).to_bits()),
        }
    }
}

struct PointEntry {
    number: usize,
    name: String,
    point: MapPoint,
    altitude: Option<f64>,
    source: Option<Ref>,
}

#[derive(Default)]
struct PointTable {
    entries: Vec<PointEntry>,
    by_coord: BTreeMap<PointKey, usize>,
}

impl PointTable {
    fn intern(
        &mut self,
        mp: MapPoint,
        alt: Option<f64>,
        preferred_name: Option<&str>,
        source: Option<Ref>,
        cx: &mut Cx,
    ) -> Result<(usize, String), Error> {
        if !mp.northing.is_finite() || !mp.easting.is_finite() {
            let mut d = Diagnostic::new(codes::SIMA_VALUE_INVALID)
                .with("kind", "A01")
                .with("field", "coordinate");
            if let Some(ref r) = source {
                d = d.at(r.clone());
            }
            cx.report(d);
            return Err(Error::Invalid);
        }
        let clean_mp = MapPoint {
            northing: norm_zero(mp.northing),
            easting: norm_zero(mp.easting),
        };
        let clean_alt = match alt {
            None => None,
            Some(z) if z.is_finite() => Some(norm_zero(z)),
            Some(_) => {
                let mut d = Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "A01")
                    .with("field", "altitude");
                if let Some(ref r) = source {
                    d = d.at(r.clone());
                }
                cx.report(d);
                None
            }
        };
        let key = PointKey::new(clean_mp, clean_alt);
        if let Some(&idx) = self.by_coord.get(&key)
            && let Some(entry) = self.entries.get_mut(idx)
        {
            if entry.name.is_empty()
                && let Some(name) = preferred_name
            {
                let clean = sanitize_csv_field(name);
                if !clean.is_empty() {
                    entry.name = clean;
                }
            }
            return Ok((entry.number, entry.name.clone()));
        }
        let number = self.entries.len() + 1;
        let name = preferred_name
            .map(sanitize_csv_field)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("P{number}"));
        cx.check(Kind::Elements, 1)
            .map_err(|s| stopped(cx, s, source.clone()))?;
        cx.check(Kind::DecodedBytes, (name.len() + 48) as u64)
            .map_err(|s| stopped(cx, s, source.clone()))?;
        let idx = self.entries.len();
        self.entries.push(PointEntry {
            number,
            name: name.clone(),
            point: clean_mp,
            altitude: clean_alt,
            source,
        });
        self.by_coord.insert(key, idx);
        Ok((number, name))
    }

    fn insert_survey_point(&mut self, pt: &SurveyPoint, cx: &mut Cx) -> Result<(), Error> {
        if !pt.position.northing.is_finite() || !pt.position.easting.is_finite() {
            cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "A01")
                    .with("field", "coordinate")
                    .at(pt.source.clone()),
            );
            return Ok(());
        }
        let clean_mp = MapPoint {
            northing: norm_zero(pt.position.northing),
            easting: norm_zero(pt.position.easting),
        };
        let clean_alt = match pt.altitude {
            None => None,
            Some(z) if z.is_finite() => Some(norm_zero(z)),
            Some(_) => {
                cx.report(
                    Diagnostic::new(codes::SIMA_VALUE_INVALID)
                        .with("kind", "A01")
                        .with("field", "altitude")
                        .at(pt.source.clone()),
                );
                None
            }
        };
        let number = self.entries.len() + 1;
        let raw_name = if !pt.name.is_empty() {
            pt.name.as_str()
        } else {
            pt.number.as_str()
        };
        let name = {
            let clean = sanitize_csv_field(raw_name);
            if clean.is_empty() {
                format!("P{number}")
            } else {
                clean
            }
        };
        cx.check(Kind::Elements, 1)
            .map_err(|s| stopped(cx, s, Some(pt.source.clone())))?;
        cx.check(Kind::DecodedBytes, (name.len() + 48) as u64)
            .map_err(|s| stopped(cx, s, Some(pt.source.clone())))?;
        let idx = self.entries.len();
        self.entries.push(PointEntry {
            number,
            name,
            point: clean_mp,
            altitude: clean_alt,
            source: Some(pt.source.clone()),
        });
        self.by_coord
            .entry(PointKey::new(clean_mp, clean_alt))
            .or_insert(idx);
        self.by_coord
            .entry(PointKey::new(clean_mp, None))
            .or_insert(idx);
        Ok(())
    }

    fn write_section(&self, w: &mut CheckedWriter<'_, '_>) -> Result<(), Error> {
        if self.entries.is_empty() {
            return Ok(());
        }
        w.push_line("A00,", None)?;
        for entry in &self.entries {
            // 決定 0019: SIMA の A01 は第 3 列が北（X）、第 4 列が東（Y）
            let [x_north, y_east] = [entry.point.northing, entry.point.easting];
            let z_str = entry.altitude.map_or_else(String::new, fmt_num);
            let line = format!(
                "A01,{},{},{},{},{},",
                entry.number,
                entry.name,
                fmt_num(x_north),
                fmt_num(y_east),
                z_str
            );
            w.push_line(&line, entry.source.clone())?;
        }
        w.push_line("A99,", None)?;
        Ok(())
    }
}

struct OutParcel {
    number: String,
    name: String,
    closed: bool,
    refs: Vec<(usize, String)>,
    source: Option<Ref>,
}

struct OutRouteSeg {
    start_ref: (usize, String),
    end_ref: (usize, String),
    arc_radius: Option<f64>,
    clothoid: Option<(f64, f64, Option<f64>, Option<f64>)>,
}

struct OutRoute {
    number: String,
    name: String,
    start_station: f64,
    segs: Vec<OutRouteSeg>,
    source: Option<Ref>,
}

fn write_header(
    w: &mut CheckedWriter<'_, '_>,
    site_name: &str,
    crs: Option<Crs>,
) -> Result<(), Error> {
    let clean_site = sanitize_csv_field(site_name);
    let site = if clean_site.is_empty() {
        "zumen"
    } else {
        clean_site.as_str()
    };
    let line = match crs {
        Some(c) => format!("G00,01,{site},EPSG:{},", c.epsg()),
        None => format!("G00,01,{site},"),
    };
    w.push_line(&line, None)
}

fn write_parcels_and_routes(
    w: &mut CheckedWriter<'_, '_>,
    parcels: &[OutParcel],
    routes: &[OutRoute],
) -> Result<(), Error> {
    for p in parcels {
        let flag = if p.closed { "1" } else { "2" };
        w.push_line(
            &format!("D00,{},{},{flag},", p.number, p.name),
            p.source.clone(),
        )?;
        for (pt_num, pt_name) in &p.refs {
            w.push_line(&format!("B01,{pt_num},{pt_name},"), p.source.clone())?;
        }
        w.push_line("D99,", p.source.clone())?;
    }
    for r in routes {
        w.push_line(
            &format!("F00,{},{},{},", r.number, r.name, fmt_num(r.start_station)),
            r.source.clone(),
        )?;
        for seg in &r.segs {
            w.push_line("C00,", r.source.clone())?;
            w.push_line(
                &format!("B01,{},{},", seg.start_ref.0, seg.start_ref.1),
                r.source.clone(),
            )?;
            if let Some(radius) = seg.arc_radius {
                w.push_line(&format!("C01,{},", fmt_num(radius)), r.source.clone())?;
            } else if let Some((a_signed, seg_len, start_r, end_r)) = seg.clothoid {
                let r1 = start_r.map_or_else(String::new, fmt_num);
                let r2 = end_r.map_or_else(String::new, fmt_num);
                w.push_line(
                    &format!("C02,{},{},{r1},{r2},", fmt_num(a_signed), fmt_num(seg_len)),
                    r.source.clone(),
                )?;
            }
            w.push_line(
                &format!("B01,{},{},", seg.end_ref.0, seg.end_ref.1),
                r.source.clone(),
            )?;
            w.push_line("C99,", r.source.clone())?;
        }
        w.push_line("F99,", r.source.clone())?;
    }
    Ok(())
}

fn check_planar_crs(crs: Option<Crs>, cx: &mut Cx) -> Result<(), Error> {
    if let Some(c) = crs
        && c.zone().is_none()
    {
        cx.report(
            Diagnostic::new(codes::SIMA_WRITE_CRS_UNSUPPORTED).with("epsg", u64::from(c.epsg())),
        );
        return Err(Error::Invalid);
    }
    Ok(())
}

fn sanitize_station(raw: f64, at: Option<&Ref>, cx: &mut Cx) -> f64 {
    if raw.is_finite() {
        norm_zero(raw)
    } else {
        let mut d = Diagnostic::new(codes::SIMA_VALUE_INVALID)
            .with("kind", "F00")
            .with("field", "start_station");
        if let Some(r) = at {
            d = d.at(r.clone());
        }
        cx.report(d);
        0.0
    }
}

/// 隣接区間の計算終点と次区間の始点を共有測点へスナップする許容差（1 mm）
const SNAP_TOL_M: f64 = 1e-3;

fn snap_to_next(calc_end: MapPoint, next_start: Option<MapPoint>) -> MapPoint {
    if let Some(ns) = next_start
        && libm::hypot(
            ns.northing - calc_end.northing,
            ns.easting - calc_end.easting,
        ) <= SNAP_TOL_M
    {
        ns
    } else {
        calc_end
    }
}

/// 中心角が 135°（`0.75 * PI`）を超える優弧・半円・円を等分して複数の `C01` 区間に分割する
#[allow(clippy::too_many_arguments)]
fn push_split_arc_c01(
    start: MapPoint,
    direction: f64,
    length: f64,
    signed_radius: f64,
    exact_end: Option<MapPoint>,
    source: Option<Ref>,
    points: &mut PointTable,
    segs: &mut Vec<OutRouteSeg>,
    cx: &mut Cx,
) -> Result<bool, Error> {
    let r_abs = libm::fabs(signed_radius);
    if !r_abs.is_finite()
        || r_abs <= 1e-9
        || !direction.is_finite()
        || !length.is_finite()
        || length <= 1e-12
    {
        return Ok(false);
    }
    let sweep = length / r_abs;
    if !sweep.is_finite() || sweep <= 1e-12 {
        return Ok(false);
    }
    let max_sub_sweep = 0.75 * PI;
    let parts =
        (libm::ceil(sweep / max_sub_sweep).min(zumen_geom2d::MAX_STEPS as f64) as usize).max(1);
    cx.check(Kind::Work, parts as u64)
        .map_err(|s| stopped(cx, s, source.clone()))?;

    let sub_len = length / parts as f64;
    let mut cur_start = start;
    let mut emitted_any = false;
    for i in 0..parts {
        let sub_dir =
            norm_zero((direction + (i as f64) * sub_len / signed_radius).rem_euclid(2.0 * PI));
        let Some((_, _, _, _, _, calc_end)) =
            arc_to_drawing(cur_start, sub_dir, sub_len, signed_radius)
        else {
            return Ok(emitted_any);
        };
        let sub_end = if i + 1 == parts {
            snap_to_next(calc_end, exact_end)
        } else {
            calc_end
        };
        if cur_start == sub_end {
            continue;
        }
        let r1 = points.intern(cur_start, None, None, source.clone(), cx)?;
        let r2 = points.intern(sub_end, None, None, source.clone(), cx)?;
        segs.push(OutRouteSeg {
            start_ref: r1,
            end_ref: r2,
            arc_radius: Some(norm_zero(signed_radius)),
            clothoid: None,
        });
        cur_start = sub_end;
        emitted_any = true;
    }
    Ok(emitted_any)
}

/// 読んだ [`SimaFile`]（`.sim` または JPGIS 版 XML）を、測点・画地・路線の全要素を保って `.sim` に書き出す
pub fn lower_sima_to_sima(file: &SimaFile, cx: &mut Cx) -> Result<LoweredSima, Error> {
    let data = file.parse_data(cx)?;
    check_planar_crs(data.crs, cx)?;

    let mut points = PointTable::default();
    let mut parcels = Vec::new();
    let mut routes = Vec::new();

    for pt in &data.points {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(pt.source.clone())))?;
        points.insert_survey_point(pt, cx)?;
    }

    for parcel in &data.parcels {
        cx.check(Kind::Work, (parcel.outer.len() as u64).max(1))
            .map_err(|s| stopped(cx, s, Some(parcel.source.clone())))?;
        if !parcel.closed && !parcel.curve_segments.is_empty() {
            let mut segs = Vec::new();
            for seg in &parcel.curve_segments {
                push_model_route_seg(seg, &mut points, &mut segs, cx)?;
            }
            if !segs.is_empty() {
                let num = sanitize_or_default(&parcel.number, routes.len() + 1, "");
                let name = parcel
                    .name
                    .as_deref()
                    .map(sanitize_csv_field)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("R{num}"));
                routes.push(OutRoute {
                    number: num,
                    name,
                    start_station: 0.0,
                    segs,
                    source: Some(parcel.source.clone()),
                });
            }
            continue;
        }
        push_model_parcel(parcel, &mut points, &mut parcels, cx)?;
    }

    for route in &data.routes {
        cx.check(Kind::Work, (route.segments.len() as u64).max(1))
            .map_err(|s| stopped(cx, s, Some(route.source.clone())))?;
        let mut segs = Vec::new();
        for seg in &route.segments {
            push_model_route_seg(seg, &mut points, &mut segs, cx)?;
        }
        if segs.is_empty() {
            continue;
        }
        let num = sanitize_or_default(&route.number, routes.len() + 1, "");
        let name = route
            .name
            .as_deref()
            .map(sanitize_csv_field)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("R{num}"));
        let sta = sanitize_station(route.start_station, Some(&route.source), cx);
        routes.push(OutRoute {
            number: num,
            name,
            start_station: sta,
            segs,
            source: Some(route.source.clone()),
        });
    }

    let site = data.site_name.as_deref().unwrap_or("zumen");
    let mut w = CheckedWriter::new(cx);
    write_header(&mut w, site, data.crs.filter(|c| c.zone().is_some()))?;
    points.write_section(&mut w)?;
    write_parcels_and_routes(&mut w, &parcels, &routes)?;
    let n_pts = points.entries.len();
    let n_parcels = parcels.len();
    let n_routes = routes.len();
    Ok(LoweredSima {
        bytes: w.finish(),
        points: n_pts,
        parcels: n_parcels,
        routes: n_routes,
    })
}

fn sanitize_or_default(raw: &str, fallback_num: usize, prefix: &str) -> String {
    let clean = sanitize_csv_field(raw);
    if clean.is_empty() {
        format!("{prefix}{fallback_num}")
    } else {
        clean
    }
}

fn push_model_parcel(
    parcel: &Parcel,
    points: &mut PointTable,
    parcels: &mut Vec<OutParcel>,
    cx: &mut Cx,
) -> Result<(), Error> {
    if parcel.ring_curve_approximated {
        cx.report(
            Diagnostic::new(codes::SIMA_CURVE_APPROXIMATED)
                .with("kind", "ring_arc")
                .with("tolerance", CURVE_TOLERANCE_M)
                .at(parcel.source.clone()),
        );
    }
    let mut refs = Vec::new();
    for loc in &parcel.outer {
        let r = points.intern(
            loc.point,
            loc.altitude,
            None,
            Some(parcel.source.clone()),
            cx,
        )?;
        if refs.last() != Some(&r) {
            refs.push(r);
        }
    }
    if parcel.closed && refs.len() >= 2 && refs.first() == refs.last() {
        refs.pop();
    }
    let min_pts = if parcel.closed { 3 } else { 2 };
    if refs.len() < min_pts {
        cx.report(
            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                .with("kind", if parcel.closed { "parcel" } else { "polyline" })
                .at(parcel.source.clone()),
        );
        return Ok(());
    }
    for _ in &parcel.holes {
        cx.report(
            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                .with("kind", "inner_ring")
                .at(parcel.source.clone()),
        );
    }
    let num = sanitize_or_default(&parcel.number, parcels.len() + 1, "");
    let default_prefix = if parcel.closed { "K" } else { "L" };
    let name = parcel
        .name
        .as_deref()
        .map(sanitize_csv_field)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("{default_prefix}{num}"));
    parcels.push(OutParcel {
        number: num,
        name,
        closed: parcel.closed,
        refs,
        source: Some(parcel.source.clone()),
    });
    Ok(())
}

fn push_model_route_seg(
    seg: &RouteSegment,
    points: &mut PointTable,
    segs: &mut Vec<OutRouteSeg>,
    cx: &mut Cx,
) -> Result<(), Error> {
    match seg.kind {
        RouteSegmentKind::Line => {
            let end = seg.end.unwrap_or_else(|| MapPoint {
                northing: norm_zero(seg.start.northing + seg.length * libm::cos(seg.direction)),
                easting: norm_zero(seg.start.easting + seg.length * libm::sin(seg.direction)),
            });
            if seg.start == end || !end.northing.is_finite() || !end.easting.is_finite() {
                cx.report(
                    Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                        .with("kind", "route_segment")
                        .at(seg.source.clone()),
                );
                return Ok(());
            }
            let r1 = points.intern(seg.start, None, None, Some(seg.source.clone()), cx)?;
            let r2 = points.intern(end, None, None, Some(seg.source.clone()), cx)?;
            segs.push(OutRouteSeg {
                start_ref: r1,
                end_ref: r2,
                arc_radius: None,
                clothoid: None,
            });
        }
        RouteSegmentKind::Arc { radius } => {
            let ok = push_split_arc_c01(
                seg.start,
                seg.direction,
                seg.length,
                radius,
                seg.end,
                Some(seg.source.clone()),
                points,
                segs,
                cx,
            )?;
            if !ok {
                cx.report(
                    Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                        .with("kind", "arc")
                        .at(seg.source.clone()),
                );
            }
        }
        RouteSegmentKind::Clothoid {
            parameter,
            start_radius,
            end_radius,
        } => {
            let [k1, k2] =
                crate::model::clothoid_curvatures(parameter, seg.length, start_radius, end_radius);
            let dk = k2 - k1;
            let valid_r = |r: Option<f64>| r.is_none_or(|v| v.is_finite() && libm::fabs(v) > 1e-9);
            if !parameter.is_finite()
                || parameter <= 1e-9
                || !seg.length.is_finite()
                || seg.length <= 1e-9
                || !valid_r(start_radius)
                || !valid_r(end_radius)
                || !k1.is_finite()
                || !k2.is_finite()
                || !dk.is_finite()
                || libm::fabs(dk) <= 1e-12
            {
                cx.report(
                    Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                        .with("kind", "clothoid")
                        .at(seg.source.clone()),
                );
                return Ok(());
            }
            let end_pt = match seg.end {
                Some(e) if e.northing.is_finite() && e.easting.is_finite() => e,
                _ => {
                    let work = zumen_geom2d::along_work(
                        [k1, k2],
                        seg.length,
                        Some(Ramp::Clothoid),
                        seg.length,
                    );
                    cx.check(Kind::Work, work)
                        .map_err(|s| stopped(cx, s, Some(seg.source.clone())))?;
                    let ([n, e], _) = zumen_geom2d::along(
                        [seg.start.northing, seg.start.easting],
                        seg.direction,
                        [k1, k2],
                        seg.length,
                        Some(Ramp::Clothoid),
                        seg.length,
                    );
                    MapPoint {
                        northing: norm_zero(n),
                        easting: norm_zero(e),
                    }
                }
            };
            if !end_pt.northing.is_finite() || !end_pt.easting.is_finite() || seg.start == end_pt {
                cx.report(
                    Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                        .with("kind", "clothoid")
                        .at(seg.source.clone()),
                );
                return Ok(());
            }
            let sign = if dk >= 0.0 { 1.0 } else { -1.0 };
            let r1 = points.intern(seg.start, None, None, Some(seg.source.clone()), cx)?;
            let r2 = points.intern(end_pt, None, None, Some(seg.source.clone()), cx)?;
            segs.push(OutRouteSeg {
                start_ref: r1,
                end_ref: r2,
                arc_radius: None,
                clothoid: Some((
                    norm_zero(sign * parameter),
                    norm_zero(seg.length),
                    start_radius.map(norm_zero),
                    end_radius.map(norm_zero),
                )),
            });
        }
    }
    Ok(())
}

/// 地理の領域モデル（[`GeoModel`]）をカンマ区切り `.sim` に書き出す
pub fn lower_geo_to_sima(model: &GeoModel, cx: &mut Cx) -> Result<LoweredSima, Error> {
    check_planar_crs(model.crs, cx)?;

    let mut points = PointTable::default();
    let mut parcels = Vec::new();
    let routes = Vec::new();

    for feat in &model.features {
        cx.check(Kind::Work, (feat.geometries.len() as u64).max(1))
            .map_err(|s| stopped(cx, s, Some(feat.source.clone())))?;
        let feat_name = feat.name.as_deref();
        for geom in &feat.geometries {
            match &geom.form {
                Form::Point(loc) => {
                    let Some(mp) = resolve_geo_loc(loc, model.crs, &feat.source, cx)? else {
                        continue;
                    };
                    let _ =
                        points.intern(mp, loc.height, feat_name, Some(feat.source.clone()), cx)?;
                }
                Form::Line(locs) => {
                    let mut refs = Vec::new();
                    for loc in locs {
                        if let Some(mp) = resolve_geo_loc(loc, model.crs, &feat.source, cx)? {
                            let r = points.intern(
                                mp,
                                loc.height,
                                None,
                                Some(feat.source.clone()),
                                cx,
                            )?;
                            if refs.last() != Some(&r) {
                                refs.push(r);
                            }
                        }
                    }
                    if refs.len() < 2 {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "line")
                                .at(feat.source.clone()),
                        );
                        continue;
                    }
                    let num = (parcels.len() + 1).to_string();
                    let name = feat_name
                        .map(sanitize_csv_field)
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("L{num}"));
                    parcels.push(OutParcel {
                        number: feat
                            .code
                            .as_deref()
                            .map(sanitize_csv_field)
                            .filter(|s| !s.is_empty())
                            .unwrap_or(num),
                        name,
                        closed: false,
                        refs,
                        source: Some(feat.source.clone()),
                    });
                }
                Form::Area { outer, holes } => {
                    let mut refs = Vec::new();
                    for loc in outer {
                        if let Some(mp) = resolve_geo_loc(loc, model.crs, &feat.source, cx)? {
                            let r = points.intern(
                                mp,
                                loc.height,
                                None,
                                Some(feat.source.clone()),
                                cx,
                            )?;
                            if refs.last() != Some(&r) {
                                refs.push(r);
                            }
                        }
                    }
                    if refs.len() >= 2 && refs.first() == refs.last() {
                        refs.pop();
                    }
                    if refs.len() < 3 {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "area")
                                .at(feat.source.clone()),
                        );
                        continue;
                    }
                    for _ in holes {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "inner_ring")
                                .at(feat.source.clone()),
                        );
                    }
                    let num = (parcels.len() + 1).to_string();
                    let name = feat_name
                        .map(sanitize_csv_field)
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("K{num}"));
                    parcels.push(OutParcel {
                        number: feat
                            .code
                            .as_deref()
                            .map(sanitize_csv_field)
                            .filter(|s| !s.is_empty())
                            .unwrap_or(num),
                        name,
                        closed: true,
                        refs,
                        source: Some(feat.source.clone()),
                    });
                }
                Form::Solid(_) => {
                    cx.report(
                        Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                            .with("kind", "solid")
                            .at(feat.source.clone()),
                    );
                }
            }
        }
    }

    let mut w = CheckedWriter::new(cx);
    write_header(&mut w, "zumen", model.crs.filter(|c| c.zone().is_some()))?;
    points.write_section(&mut w)?;
    write_parcels_and_routes(&mut w, &parcels, &routes)?;
    let n_pts = points.entries.len();
    let n_parcels = parcels.len();
    Ok(LoweredSima {
        bytes: w.finish(),
        points: n_pts,
        parcels: n_parcels,
        routes: 0,
    })
}

fn resolve_geo_loc(
    loc: &Location,
    crs: Option<Crs>,
    at: &Ref,
    cx: &mut Cx,
) -> Result<Option<MapPoint>, Error> {
    match loc.position {
        GeoPos::Map(mp) => {
            if !mp.northing.is_finite() || !mp.easting.is_finite() {
                cx.report(
                    Diagnostic::new(codes::SIMA_VALUE_INVALID)
                        .with("kind", "GeoModel")
                        .with("field", "coordinate")
                        .at(at.clone()),
                );
                return Ok(None);
            }
            Ok(Some(MapPoint {
                northing: norm_zero(mp.northing),
                easting: norm_zero(mp.easting),
            }))
        }
        GeoPos::Geo(gp) => {
            let Some(c) = crs else {
                cx.report(Diagnostic::new(codes::SIMA_CRS_MISSING).at(at.clone()));
                return Err(Error::Invalid);
            };
            let Some(zone) = c.zone() else {
                cx.report(
                    Diagnostic::new(codes::SIMA_WRITE_CRS_UNSUPPORTED)
                        .with("epsg", u64::from(c.epsg()))
                        .at(at.clone()),
                );
                return Err(Error::Invalid);
            };
            let Some(proj) = zumen_crs::to_plane(zone, gp) else {
                cx.report(
                    Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                        .with("kind", "projection")
                        .at(at.clone()),
                );
                return Ok(None);
            };
            Ok(Some(proj.point))
        }
    }
}

/// 土木の領域モデル（[`CivilModel`]）をカンマ区切り `.sim` に書き出す
pub fn lower_civil_to_sima(model: &CivilModel, cx: &mut Cx) -> Result<LoweredSima, Error> {
    check_planar_crs(model.crs, cx)?;

    let mut points = PointTable::default();
    let parcels = Vec::new();
    let mut routes = Vec::new();

    for (idx, align) in model.alignments.iter().enumerate() {
        cx.check(Kind::Work, (align.horizontal.len() as u64).max(1))
            .map_err(|s| stopped(cx, s, Some(align.source.clone())))?;
        if !align.vertical.is_empty() {
            cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", "vertical")
                    .at(align.source.clone()),
            );
        }
        if !align.cant.is_empty() {
            cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", "cant")
                    .at(align.source.clone()),
            );
        }
        if !align.cross_sections.is_empty() {
            cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", "cross_section")
                    .at(align.source.clone()),
            );
        }
        if !align.stations.breaks.is_empty() {
            cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", "station_break")
                    .at(align.source.clone()),
            );
        }
        let mut segs = Vec::new();
        for (h_idx, h) in align.horizontal.iter().enumerate() {
            if !h.start.northing.is_finite()
                || !h.start.easting.is_finite()
                || !h.direction.is_finite()
                || !h.length.is_finite()
                || h.length <= 1e-12
            {
                cx.report(
                    Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                        .with("kind", "horizontal")
                        .at(h.source.clone()),
                );
                continue;
            }
            let next_start = align
                .horizontal
                .get(h_idx + 1)
                .map(|n| n.start)
                .filter(|p| p.northing.is_finite() && p.easting.is_finite());

            match &h.kind {
                HorizontalKind::Line => {
                    let Some(end_mp) = horizontal_line_end(h, next_start) else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "horizontal")
                                .at(h.source.clone()),
                        );
                        continue;
                    };
                    if h.start == end_mp {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "horizontal")
                                .at(h.source.clone()),
                        );
                        continue;
                    }
                    let start_ref =
                        points.intern(h.start, None, None, Some(h.source.clone()), cx)?;
                    let end_ref = points.intern(end_mp, None, None, Some(h.source.clone()), cx)?;
                    segs.push(OutRouteSeg {
                        start_ref,
                        end_ref,
                        arc_radius: None,
                        clothoid: None,
                    });
                }
                HorizontalKind::Arc { radius } => {
                    let ok = push_split_arc_c01(
                        h.start,
                        h.direction,
                        h.length,
                        *radius,
                        next_start,
                        Some(h.source.clone()),
                        &mut points,
                        &mut segs,
                        cx,
                    )?;
                    if !ok {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "horizontal")
                                .at(h.source.clone()),
                        );
                    }
                }
                HorizontalKind::Transition {
                    start_radius,
                    end_radius,
                    curve,
                } => {
                    if *curve != TransitionCurve::Clothoid {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "transition")
                                .at(h.source.clone()),
                        );
                        continue;
                    }
                    if start_radius.is_some_and(|r| !r.is_finite() || libm::fabs(r) <= 1e-9)
                        || end_radius.is_some_and(|r| !r.is_finite() || libm::fabs(r) <= 1e-9)
                    {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "transition")
                                .at(h.source.clone()),
                        );
                        continue;
                    }
                    let k1 = start_radius.map_or(0.0, |r| 1.0 / r);
                    let k2 = end_radius.map_or(0.0, |r| 1.0 / r);
                    let dk = k2 - k1;
                    if !dk.is_finite() || libm::fabs(dk) <= 1e-12 {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "transition")
                                .at(h.source.clone()),
                        );
                        continue;
                    }
                    let a_abs = libm::sqrt(h.length / libm::fabs(dk));
                    if !a_abs.is_finite() || a_abs <= 1e-9 {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "transition")
                                .at(h.source.clone()),
                        );
                        continue;
                    }
                    let work = zumen_geom2d::along_work(
                        [k1, k2],
                        h.length,
                        Some(Ramp::Clothoid),
                        h.length,
                    );
                    cx.check(Kind::Work, work)
                        .map_err(|s| stopped(cx, s, Some(h.source.clone())))?;
                    let ([n, e], _) = zumen_geom2d::along(
                        [h.start.northing, h.start.easting],
                        h.direction,
                        [k1, k2],
                        h.length,
                        Some(Ramp::Clothoid),
                        h.length,
                    );
                    if !n.is_finite() || !e.is_finite() {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "transition")
                                .at(h.source.clone()),
                        );
                        continue;
                    }
                    let calc_end = MapPoint {
                        northing: norm_zero(n),
                        easting: norm_zero(e),
                    };
                    let end_mp = snap_to_next(calc_end, next_start);
                    if h.start == end_mp {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "transition")
                                .at(h.source.clone()),
                        );
                        continue;
                    }
                    let sign = if dk >= 0.0 { 1.0 } else { -1.0 };
                    let start_ref =
                        points.intern(h.start, None, None, Some(h.source.clone()), cx)?;
                    let end_ref = points.intern(end_mp, None, None, Some(h.source.clone()), cx)?;
                    segs.push(OutRouteSeg {
                        start_ref,
                        end_ref,
                        arc_radius: None,
                        clothoid: Some((
                            norm_zero(sign * a_abs),
                            norm_zero(h.length),
                            start_radius.map(norm_zero),
                            end_radius.map(norm_zero),
                        )),
                    });
                }
            }
        }
        if segs.is_empty() {
            continue;
        }
        let number = (idx + 1).to_string();
        let name = align
            .name
            .as_deref()
            .map(sanitize_csv_field)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("R{number}"));
        let sta = sanitize_station(align.stations.start, Some(&align.source), cx);
        routes.push(OutRoute {
            number,
            name,
            start_station: sta,
            segs,
            source: Some(align.source.clone()),
        });
    }
    for surface in &model.surfaces {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(surface.source.clone())))?;
        cx.report(
            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                .with("kind", "surface")
                .at(surface.source.clone()),
        );
    }

    let mut w = CheckedWriter::new(cx);
    write_header(&mut w, "zumen", model.crs.filter(|c| c.zone().is_some()))?;
    points.write_section(&mut w)?;
    write_parcels_and_routes(&mut w, &parcels, &routes)?;
    let n_pts = points.entries.len();
    let n_routes = routes.len();
    Ok(LoweredSima {
        bytes: w.finish(),
        points: n_pts,
        parcels: 0,
        routes: n_routes,
    })
}

fn horizontal_line_end(h: &Horizontal, next_start: Option<MapPoint>) -> Option<MapPoint> {
    let n = h.start.northing + h.length * libm::cos(h.direction);
    let e = h.start.easting + h.length * libm::sin(h.direction);
    if !n.is_finite() || !e.is_finite() {
        return None;
    }
    let calc_end = MapPoint {
        northing: norm_zero(n),
        easting: norm_zero(e),
    };
    Some(snap_to_next(calc_end, next_start))
}

/// 図面の領域モデル（[`Drawing2D`]）をカンマ区切り `.sim` に書き出す
pub fn lower_drawing_to_sima(drawing: &Drawing2D, cx: &mut Cx) -> Result<LoweredSima, Error> {
    let mut points = PointTable::default();
    let mut parcels = Vec::new();
    let mut routes = Vec::new();

    let mut site_name = "zumen".to_string();
    let mut doc_crs: Option<Crs> = None;
    let mut label_ids = std::collections::BTreeSet::new();
    let mut elem_names: BTreeMap<String, (Option<String>, Option<f64>)> = BTreeMap::new();

    for ext in &drawing.extensions {
        cx.check(Kind::Work, (ext.fields.len() as u64).saturating_add(1))
            .map_err(|s| stopped(cx, s, None))?;
        if ext.namespace == "sima.label"
            && let Target::Element(ref id) = ext.target
        {
            label_ids.insert(id.clone());
        } else if ext.namespace == "sima.attribute" {
            match &ext.target {
                Target::Document => {
                    for f in &ext.fields {
                        if f.key == "site_name"
                            && let FieldValue::Text(ref s) = f.value
                        {
                            site_name = s.clone();
                        } else if f.key == "epsg"
                            && let FieldValue::Integer(code) = f.value
                        {
                            let parsed = u32::try_from(code).ok().and_then(Crs::from_epsg);
                            match parsed {
                                Some(crs) if crs.zone().is_some() => doc_crs = Some(crs),
                                _ => {
                                    cx.report(
                                        Diagnostic::new(codes::SIMA_WRITE_CRS_UNSUPPORTED)
                                            .with("epsg", u64::try_from(code).unwrap_or(0)),
                                    );
                                    return Err(Error::Invalid);
                                }
                            }
                        }
                    }
                }
                Target::Element(id) => {
                    let mut nm = None;
                    let mut alt = None;
                    for f in &ext.fields {
                        if f.key == "name"
                            && let FieldValue::Text(ref s) = f.value
                            && !s.is_empty()
                        {
                            nm = Some(s.clone());
                        } else if f.key == "altitude"
                            && let FieldValue::Real(z) = f.value
                        {
                            alt = Some(z);
                        }
                    }
                    elem_names.insert(id.clone(), (nm, alt));
                }
                _ => {}
            }
        }
    }

    if let Some(space) = drawing.spaces.first() {
        let scale = match space.unit.millimeters() {
            Some(mm) => mm / 1000.0,
            None => 1.0,
        };
        let to_mp = |p: ModelPoint| -> Option<MapPoint> {
            if !p.x.is_finite() || !p.y.is_finite() {
                return None;
            }
            let mp = space
                .axes
                .map_point([norm_zero(p.x), norm_zero(p.y)], scale);
            (mp.northing.is_finite() && mp.easting.is_finite()).then_some(mp)
        };
        let space_str = zumen_drawing::id::space(space.id);
        let mut namer = zumen_drawing::id::Namer::new(&space_str);
        let mut route_segs = Vec::new();

        for (idx, elem) in space.elements.iter().enumerate() {
            let elem_id = namer.next(idx, elem);
            cx.check(Kind::Work, 1)
                .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
            if label_ids.contains(&elem_id) {
                continue;
            }
            let meta = elem_names.get(&elem_id);
            let preferred_name = meta.and_then(|m| m.0.as_deref());
            let preferred_alt = meta.and_then(|m| m.1);

            match &elem.shape {
                Shape::Point(pt) => {
                    if let Some(mp) = to_mp(pt.position) {
                        let _ = points.intern(
                            mp,
                            preferred_alt,
                            preferred_name,
                            Some(elem.source.clone()),
                            cx,
                        )?;
                    } else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "point")
                                .at(elem.source.clone()),
                        );
                    }
                }
                Shape::Polyline(poly) => {
                    cx.check(Kind::Work, (poly.vertices.len() as u64).max(1))
                        .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
                    let has_bulge = poly
                        .vertices
                        .iter()
                        .any(|v| v.bulge.is_finite() && v.bulge != 0.0);
                    let mut refs = Vec::new();
                    if has_bulge
                        && poly
                            .vertices
                            .iter()
                            .all(|v| v.point.x.is_finite() && v.point.y.is_finite())
                    {
                        let verts: Vec<([f64; 2], f64)> = poly
                            .vertices
                            .iter()
                            .map(|v| {
                                let b = if v.bulge.is_finite() { v.bulge } else { 0.0 };
                                ([v.point.x, v.point.y], b)
                            })
                            .collect();
                        let tol_local =
                            (CURVE_TOLERANCE_M / scale.max(1e-9)).max(f64::MIN_POSITIVE);
                        let seg_pts = zumen_geom2d::polyline_points(&verts, poly.closed, tol_local);
                        cx.check(Kind::Work, seg_pts.max(1))
                            .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
                        cx.check(Kind::Segments, seg_pts)
                            .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
                        cx.check(Kind::DecodedBytes, seg_pts.saturating_mul(16))
                            .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
                        cx.report(
                            Diagnostic::new(codes::SIMA_CURVE_APPROXIMATED)
                                .with("kind", "polyline_bulge")
                                .with("tolerance", CURVE_TOLERANCE_M)
                                .at(elem.source.clone()),
                        );
                        let flattened = zumen_geom2d::polyline(&verts, poly.closed, tol_local);
                        for [x, y] in flattened {
                            let mp = space.axes.map_point([norm_zero(x), norm_zero(y)], scale);
                            if !mp.northing.is_finite() || !mp.easting.is_finite() {
                                continue;
                            }
                            let r = points.intern(mp, None, None, Some(elem.source.clone()), cx)?;
                            if refs.last() != Some(&r) {
                                refs.push(r);
                            }
                        }
                    } else {
                        for v in &poly.vertices {
                            if let Some(mp) = to_mp(v.point) {
                                let r =
                                    points.intern(mp, None, None, Some(elem.source.clone()), cx)?;
                                if refs.last() != Some(&r) {
                                    refs.push(r);
                                }
                            }
                        }
                    }
                    if poly.closed && refs.len() >= 2 && refs.first() == refs.last() {
                        refs.pop();
                    }
                    let min_pts = if poly.closed { 3 } else { 2 };
                    if refs.len() >= min_pts {
                        let num = (parcels.len() + 1).to_string();
                        let name = preferred_name
                            .map(sanitize_csv_field)
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| format!("K{num}"));
                        parcels.push(OutParcel {
                            number: num,
                            name,
                            closed: poly.closed,
                            refs,
                            source: Some(elem.source.clone()),
                        });
                    } else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "polyline")
                                .at(elem.source.clone()),
                        );
                    }
                }
                Shape::Line { start, end } => {
                    if let (Some(a), Some(b)) = (to_mp(*start), to_mp(*end))
                        && a != b
                    {
                        let r1 = points.intern(a, None, None, Some(elem.source.clone()), cx)?;
                        let r2 = points.intern(b, None, None, Some(elem.source.clone()), cx)?;
                        route_segs.push(OutRouteSeg {
                            start_ref: r1,
                            end_ref: r2,
                            arc_radius: None,
                            clothoid: None,
                        });
                    } else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "line")
                                .at(elem.source.clone()),
                        );
                    }
                }
                Shape::Circle {
                    center,
                    radius: Length::Model(r_len),
                } => {
                    let r_m = r_len.value * scale;
                    let mut emitted = false;
                    if r_m.is_finite() && r_m > 1e-9 && center.x.is_finite() && center.y.is_finite()
                    {
                        let p0 = ModelPoint {
                            space: space.id,
                            x: center.x + r_len.value,
                            y: center.y,
                        };
                        if let Some(a) = to_mp(p0) {
                            let (dir, signed_r) = match space.axes {
                                Axes::EastNorth => (0.0, -r_m),
                                Axes::NorthEast => (0.5 * PI, r_m),
                            };
                            emitted = push_split_arc_c01(
                                a,
                                dir,
                                2.0 * PI * r_m,
                                signed_r,
                                Some(a),
                                Some(elem.source.clone()),
                                &mut points,
                                &mut route_segs,
                                cx,
                            )?;
                        }
                    }
                    if !emitted {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "circle")
                                .at(elem.source.clone()),
                        );
                    }
                }
                Shape::Curve(Edge::Arc {
                    center,
                    radius: Length::Model(r_len),
                    start,
                    end,
                    counterclockwise,
                }) => {
                    let r_m = r_len.value * scale;
                    let mut emitted = false;
                    if r_m.is_finite()
                        && r_m > 1e-9
                        && center.x.is_finite()
                        && center.y.is_finite()
                        && start.is_finite()
                        && end.is_finite()
                    {
                        let sweep_deg = zumen_geom2d::sweep(*start, *end, *counterclockwise);
                        let sweep_rad = libm::fabs(sweep_deg) * (PI / 180.0);
                        let s_rad = *start * (PI / 180.0);
                        let e_rad = *end * (PI / 180.0);
                        let p0 = ModelPoint {
                            space: space.id,
                            x: center.x + r_len.value * libm::cos(s_rad),
                            y: center.y + r_len.value * libm::sin(s_rad),
                        };
                        let p1 = ModelPoint {
                            space: space.id,
                            x: center.x + r_len.value * libm::cos(e_rad),
                            y: center.y + r_len.value * libm::sin(e_rad),
                        };
                        let (tx, ty) = if *counterclockwise {
                            (-libm::sin(s_rad), libm::cos(s_rad))
                        } else {
                            (libm::sin(s_rad), -libm::cos(s_rad))
                        };
                        let ((te, tn), ccw_en) = match space.axes {
                            Axes::EastNorth => ((tx, ty), *counterclockwise),
                            Axes::NorthEast => ((ty, tx), !*counterclockwise),
                        };
                        let bearing = norm_zero(libm::atan2(te, tn).rem_euclid(2.0 * PI));
                        let signed_r = if ccw_en { -r_m } else { r_m };
                        let exact_end = if sweep_rad >= 2.0 * PI - 1e-12 {
                            to_mp(p0)
                        } else {
                            to_mp(p1)
                        };
                        if let Some(a) = to_mp(p0) {
                            emitted = push_split_arc_c01(
                                a,
                                bearing,
                                sweep_rad * r_m,
                                signed_r,
                                exact_end,
                                Some(elem.source.clone()),
                                &mut points,
                                &mut route_segs,
                                cx,
                            )?;
                        }
                    }
                    if !emitted {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "arc")
                                .at(elem.source.clone()),
                        );
                    }
                }
                Shape::Curve(Edge::Clothoid {
                    origin,
                    parameter: Length::Model(p_len),
                    angle,
                    counterclockwise,
                    start: Length::Model(s_len),
                    end: Length::Model(e_len),
                }) => {
                    let a_m = p_len.value * scale;
                    let s0_m = s_len.value * scale;
                    let s1_m = e_len.value * scale;
                    let seg_len = libm::fabs(s1_m - s0_m);
                    let mut emitted = false;
                    if a_m.is_finite()
                        && a_m > 1e-9
                        && seg_len.is_finite()
                        && seg_len > 1e-9
                        && origin.x.is_finite()
                        && origin.y.is_finite()
                        && angle.is_finite()
                        && s_len.value.is_finite()
                        && e_len.value.is_finite()
                    {
                        let tol_local = (1e-3 / scale.max(1e-9)).max(f64::MIN_POSITIVE);
                        let work = zumen_geom2d::clothoid_work(
                            p_len.value,
                            s_len.value,
                            e_len.value,
                            tol_local,
                        );
                        let max_pts = (work / 9).saturating_add(2);
                        cx.check(Kind::Work, work)
                            .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
                        cx.check(Kind::Segments, max_pts)
                            .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
                        cx.check(Kind::DecodedBytes, max_pts.saturating_mul(16))
                            .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
                        let pts = zumen_geom2d::clothoid(
                            [origin.x, origin.y],
                            p_len.value,
                            *angle,
                            *counterclockwise,
                            s_len.value,
                            e_len.value,
                            tol_local,
                        );
                        if let (Some(&[x0, y0]), Some(&[x1, y1])) = (pts.first(), pts.last()) {
                            let p0 = ModelPoint {
                                space: space.id,
                                x: x0,
                                y: y0,
                            };
                            let p1 = ModelPoint {
                                space: space.id,
                                x: x1,
                                y: y1,
                            };
                            if let (Some(a), Some(b)) = (to_mp(p0), to_mp(p1))
                                && a != b
                            {
                                let turn_xy = if *counterclockwise { 1.0 } else { -1.0 };
                                let dir_sign = if s1_m >= s0_m { 1.0 } else { -1.0 };
                                let axes_sign = match space.axes {
                                    Axes::EastNorth => -1.0,
                                    Axes::NorthEast => 1.0,
                                };
                                let k1 = axes_sign * dir_sign * turn_xy * s0_m / (a_m * a_m);
                                let k2 = axes_sign * dir_sign * turn_xy * s1_m / (a_m * a_m);
                                let dk = k2 - k1;
                                let start_r = (libm::fabs(k1) > 1e-9).then(|| norm_zero(1.0 / k1));
                                let end_r = (libm::fabs(k2) > 1e-9).then(|| norm_zero(1.0 / k2));
                                let valid_r = |r: Option<f64>| {
                                    r.is_none_or(|v| v.is_finite() && libm::fabs(v) > 1e-9)
                                };
                                if k1.is_finite()
                                    && k2.is_finite()
                                    && dk.is_finite()
                                    && libm::fabs(dk) > 1e-12
                                    && valid_r(start_r)
                                    && valid_r(end_r)
                                {
                                    let sign = if dk >= 0.0 { 1.0 } else { -1.0 };
                                    let r1 = points.intern(
                                        a,
                                        None,
                                        None,
                                        Some(elem.source.clone()),
                                        cx,
                                    )?;
                                    let r2 = points.intern(
                                        b,
                                        None,
                                        None,
                                        Some(elem.source.clone()),
                                        cx,
                                    )?;
                                    route_segs.push(OutRouteSeg {
                                        start_ref: r1,
                                        end_ref: r2,
                                        arc_radius: None,
                                        clothoid: Some((
                                            norm_zero(sign * a_m),
                                            norm_zero(seg_len),
                                            start_r,
                                            end_r,
                                        )),
                                    });
                                    emitted = true;
                                }
                            }
                        }
                    }
                    if !emitted {
                        cx.report(
                            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                                .with("kind", "clothoid")
                                .at(elem.source.clone()),
                        );
                    }
                }
                other => {
                    cx.report(
                        Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                            .with("kind", shape_kind_key(other))
                            .at(elem.source.clone()),
                    );
                }
            }
        }

        if !route_segs.is_empty() {
            routes.push(OutRoute {
                number: "1".to_string(),
                name: "R1".to_string(),
                start_station: 0.0,
                segs: route_segs,
                source: None,
            });
        }
    }
    for extra_space in drawing.spaces.iter().skip(1) {
        for elem in &extra_space.elements {
            cx.check(Kind::Work, 1)
                .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
            cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", shape_kind_key(&elem.shape))
                    .at(elem.source.clone()),
            );
        }
    }
    for sheet in &drawing.sheets {
        for elem in &sheet.elements {
            cx.check(Kind::Work, 1)
                .map_err(|s| stopped(cx, s, Some(elem.source.clone())))?;
            cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", shape_kind_key(&elem.shape))
                    .at(elem.source.clone()),
            );
        }
    }

    let mut w = CheckedWriter::new(cx);
    write_header(&mut w, &site_name, doc_crs)?;
    points.write_section(&mut w)?;
    write_parcels_and_routes(&mut w, &parcels, &routes)?;
    let n_pts = points.entries.len();
    let n_parcels = parcels.len();
    let n_routes = routes.len();
    Ok(LoweredSima {
        bytes: w.finish(),
        points: n_pts,
        parcels: n_parcels,
        routes: n_routes,
    })
}

fn shape_kind_key<P: Copy>(shape: &Shape<P>) -> &'static str {
    match shape {
        Shape::Line { .. } => "line",
        Shape::Circle { .. } => "circle",
        Shape::Curve(_) => "curve",
        Shape::Polyline(_) => "polyline",
        Shape::Text(_) => "text",
        Shape::Formatted(_) => "formatted_text",
        Shape::Dimension(_) => "dimension",
        Shape::Leader(_) => "leader",
        Shape::Callout(_) => "callout",
        Shape::Table(_) => "table",
        Shape::Underlay(_) => "underlay",
        Shape::Symbol(_) => "symbol",
        Shape::Hatch(_) => "hatch",
        Shape::Point(_) => "point",
    }
}
