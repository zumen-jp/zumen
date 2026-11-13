//! SIMA のカンマ区切り形式（`.sim`、Shift_JIS / UTF-8）の L1 走査と中間モデルへの読み取り。

use core::f64::consts::PI;
use std::collections::BTreeMap;

use zumen_base::{ByteRange, Diagnostic, Kind, Ref, codes};
use zumen_crs::{Crs, Realization, Zone};
use zumen_geom2d::Ramp;
use zumen_sdk::{Cx, Error};

use crate::model::{
    AttributeVal, MAX_ALLOC, MapLocation, Parcel, ParcelKind, PointKind, Route, RouteSegment,
    RouteSegmentKind, SimaData, SurveyPoint, bearing_and_distance, chord_arc_params,
    clothoid_curvatures, norm_zero, parse_map_point,
};
use crate::{Record, stopped};

pub(crate) fn read_csv_records(src: &[u8], cx: &mut Cx) -> Result<(bool, Vec<Record>), Error> {
    cx.check(Kind::InputBytes, src.len() as u64)
        .map_err(|s| stopped(cx, s, None))?;
    if src.len() as u64 > MAX_ALLOC {
        cx.report(
            Diagnostic::new(codes::SIMA_FILE_INVALID)
                .with("offset", 0u64)
                .at(Ref::at_bytes(0, 0)),
        );
        return Err(Error::Invalid);
    }
    cx.check(Kind::DecodedBytes, src.len() as u64)
        .map_err(|s| stopped(cx, s, None))?;

    let body_offset = if src.starts_with(b"\xEF\xBB\xBF") {
        3usize
    } else {
        0usize
    };
    // UTF-8 判定の前に走査作業量を計上する
    let chunks = (src.len() as u64 / 64).max(1);
    cx.check(Kind::Work, chunks)
        .map_err(|s| stopped(cx, s, None))?;
    let rest = src.get(body_offset..).unwrap_or_default();
    let utf8 = body_offset == 3 || core::str::from_utf8(rest).is_ok();

    let mut records = Vec::new();
    let mut pos = body_offset;
    while pos < src.len() {
        let line_start = pos;
        while let Some(&b) = src.get(pos) {
            if b == b'\r' || b == b'\n' {
                break;
            }
            pos += 1;
            if (pos - line_start).is_multiple_of(64) {
                cx.check(Kind::Work, 1).map_err(|s| {
                    stopped(cx, s, Some(Ref::at_bytes(line_start as u64, pos as u64)))
                })?;
            }
        }
        let line_end = pos;
        if src.get(pos) == Some(&b'\r') {
            pos += 1;
        }
        if src.get(pos) == Some(&b'\n') {
            pos += 1;
        }
        cx.check(Kind::Work, 1).map_err(|s| {
            stopped(
                cx,
                s,
                Some(Ref::at_bytes(line_start as u64, line_end as u64)),
            )
        })?;

        let raw_line = src.get(line_start..line_end).unwrap_or_default();
        let trimmed = trim_ascii(raw_line);
        if trimmed.is_empty() {
            continue;
        }
        let tag_raw = trimmed.split(|&b| b == b',').next().unwrap_or_default();
        let tag = trim_ascii(tag_raw);
        let range = ByteRange {
            start: line_start as u64,
            end: line_end as u64,
        };
        let at = Ref::at_bytes(range.start, range.end);
        if records.is_empty() && !(trimmed.contains(&b',') && tag.eq_ignore_ascii_case(b"G00")) {
            cx.report(
                Diagnostic::new(codes::SIMA_FILE_INVALID)
                    .with("offset", range.start)
                    .at(at),
            );
            return Err(Error::Invalid);
        }
        cx.check(Kind::Records, 1)
            .map_err(|s| stopped(cx, s, Some(at.clone())))?;
        cx.check(Kind::DecodedBytes, (tag.len() + 24) as u64)
            .map_err(|s| stopped(cx, s, Some(at)))?;
        let mut upper = tag.to_vec();
        upper.make_ascii_uppercase();
        records.push(Record {
            kind: upper,
            bytes: range,
        });
    }

    if records.is_empty() {
        cx.report(
            Diagnostic::new(codes::SIMA_FILE_INVALID)
                .with("offset", 0u64)
                .at(Ref::at_bytes(0, src.len() as u64)),
        );
        return Err(Error::Invalid);
    }

    Ok((utf8, records))
}

fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !matches!(b, b' ' | b'\t'))
        .unwrap_or(bytes.len());
    let tail = bytes.get(start..).unwrap_or_default();
    let end = tail
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\t'))
        .map_or(0, |i| i + 1);
    tail.get(..end).unwrap_or_default()
}

fn decode_line(bytes: &[u8], utf8: bool) -> String {
    if utf8 {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        encoding_rs::SHIFT_JIS
            .decode_without_bom_handling(bytes)
            .0
            .into_owned()
    }
}

#[derive(Default)]
struct PointIndex {
    exact: BTreeMap<(String, String), usize>,
    by_number: BTreeMap<String, usize>,
    by_name: BTreeMap<String, usize>,
}

impl PointIndex {
    fn exact(&self, number: &str, name: &str) -> Option<usize> {
        self.exact
            .get(&(number.to_string(), name.to_string()))
            .copied()
    }

    fn insert(&mut self, number: &str, name: &str, idx: usize) {
        self.exact
            .insert((number.to_string(), name.to_string()), idx);
        if !number.is_empty() {
            self.by_number.insert(number.to_string(), idx);
        }
        if !name.is_empty() {
            self.by_name.insert(name.to_string(), idx);
        }
    }

    fn find(&self, number: &str, name: &str) -> Option<usize> {
        if let Some(&idx) = self.exact.get(&(number.to_string(), name.to_string())) {
            return Some(idx);
        }
        if !number.is_empty() {
            return self.by_number.get(number).copied();
        }
        if !name.is_empty() {
            return self.by_name.get(name).copied();
        }
        None
    }
}

struct ParcelDraft {
    number: String,
    name: Option<String>,
    closed: bool,
    points: Vec<MapLocation>,
    source: Ref,
}

#[derive(Clone, Copy)]
struct ClothoidDraft {
    a_signed: f64,
    explicit_len: Option<f64>,
    start_r: Option<f64>,
    end_r: Option<f64>,
}

#[derive(Default)]
struct IntervalDraft {
    points: Vec<MapLocation>,
    arc_radius: Option<f64>,
    clothoid: Option<ClothoidDraft>,
    source: Option<Ref>,
}

struct RouteDraft {
    number: String,
    name: Option<String>,
    start_station: f64,
    attributes: Vec<(String, AttributeVal)>,
    segments: Vec<RouteSegment>,
    direct_points: Vec<(MapLocation, Ref)>,
    source: Ref,
}

pub(crate) fn parse_csv_data(
    src: &[u8],
    utf8: bool,
    records: &[Record],
    cx: &mut Cx,
) -> Result<SimaData, Error> {
    let mut data = SimaData::default();
    let mut pt_index = PointIndex::default();
    let mut in_coord_block = false;
    let mut parcel: Option<ParcelDraft> = None;
    let mut route: Option<RouteDraft> = None;
    let mut interval: Option<IntervalDraft> = None;
    let mut active_depths = 0usize;

    let res = (|| -> Result<(), Error> {
        for (rec_idx, rec) in records.iter().enumerate() {
            let at = Ref::at_record(rec_idx as u64).with_bytes(rec.bytes.start, rec.bytes.end);
            let start = usize::try_from(rec.bytes.start).unwrap_or(0);
            let end = usize::try_from(rec.bytes.end).unwrap_or(0);
            let raw = src.get(start..end).unwrap_or_default();
            cx.check(Kind::Work, (raw.len() as u64 / 32).max(1))
                .map_err(|s| stopped(cx, s, Some(at.clone())))?;
            let line = decode_line(raw, utf8);
            cx.check(Kind::DecodedBytes, line.len() as u64)
                .map_err(|s| stopped(cx, s, Some(at.clone())))?;

            let cols: Vec<&str> = line.split(',').map(str::trim).collect();
            let code_upper = cols.first().copied().unwrap_or("").to_ascii_uppercase();

            match code_upper.as_str() {
                "G00" => {
                    if let Some(&ver) = cols.get(1)
                        && !ver.is_empty()
                    {
                        data.version = Some(ver.to_string());
                    }
                    if let Some(&site) = cols.get(2)
                        && !site.is_empty()
                    {
                        data.site_name = Some(site.to_string());
                    }
                    if let Some(&crs_col) = cols.get(3)
                        && !crs_col.is_empty()
                    {
                        parse_header_crs(crs_col, &at, &mut data, cx);
                    }
                }
                "Z00" => {}
                "A00" => {
                    if !in_coord_block {
                        cx.enter().map_err(|s| stopped(cx, s, Some(at.clone())))?;
                        active_depths += 1;
                        in_coord_block = true;
                    }
                }
                "A99" => {
                    if in_coord_block {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        in_coord_block = false;
                    }
                }
                "A01" => {
                    let number = cols.get(1).copied().unwrap_or("").to_string();
                    let name = cols.get(2).copied().unwrap_or("").to_string();
                    let x_opt = cols.get(3).and_then(|s| parse_finite_f64(s));
                    let y_opt = cols.get(4).and_then(|s| parse_finite_f64(s));
                    let Some((x_north, y_east)) = x_opt.zip(y_opt) else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                .with("kind", "A01")
                                .with("field", "coordinate")
                                .at(at),
                        );
                        continue;
                    };
                    let Some(position) = parse_map_point(x_north, y_east) else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                .with("kind", "A01")
                                .with("field", "coordinate")
                                .at(at),
                        );
                        continue;
                    };
                    let altitude = match cols.get(5).copied().unwrap_or("") {
                        "" => None,
                        z_str => match parse_finite_f64(z_str) {
                            Some(z) => Some(norm_zero(z)),
                            None => {
                                cx.report(
                                    Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                        .with("kind", "A01")
                                        .with("field", "altitude")
                                        .at(at.clone()),
                                );
                                None
                            }
                        },
                    };
                    if let Some(existing_idx) = pt_index.exact(&number, &name)
                        && let Some(existing) = data.points.get_mut(existing_idx)
                    {
                        existing.position = position;
                        existing.altitude = altitude;
                        existing.source = at;
                    } else {
                        cx.check(Kind::Elements, 1)
                            .map_err(|s| stopped(cx, s, Some(at.clone())))?;
                        cx.check(Kind::DecodedBytes, (number.len() + name.len() + 64) as u64)
                            .map_err(|s| stopped(cx, s, Some(at.clone())))?;
                        let idx = data.points.len();
                        pt_index.insert(&number, &name, idx);
                        data.points.push(SurveyPoint {
                            id_hint: None,
                            number,
                            name,
                            position,
                            altitude,
                            kind: PointKind::Sokuten,
                            attributes: Vec::new(),
                            source: at,
                        });
                    }
                }
                "D00" => {
                    if let Some(prev) = parcel.take() {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        finish_parcel(prev, &mut data, cx)?;
                    }
                    cx.enter().map_err(|s| stopped(cx, s, Some(at.clone())))?;
                    active_depths += 1;
                    let number = cols.get(1).copied().unwrap_or("").to_string();
                    let name = cols
                        .get(2)
                        .copied()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    let flag = cols.get(3).copied().unwrap_or("1");
                    let closed = flag != "2";
                    parcel = Some(ParcelDraft {
                        number,
                        name,
                        closed,
                        points: Vec::new(),
                        source: at,
                    });
                }
                "D99" => {
                    if let Some(prev) = parcel.take() {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        finish_parcel(prev, &mut data, cx)?;
                    }
                }
                "F00" => {
                    if let Some(iv) = interval.take() {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        flush_interval(iv, &mut route, &mut data, cx)?;
                    }
                    if let Some(prev_route) = route.take() {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        finish_route(prev_route, &mut data, cx)?;
                    }
                    cx.enter().map_err(|s| stopped(cx, s, Some(at.clone())))?;
                    active_depths += 1;
                    let col1 = cols.get(1).copied().unwrap_or("");
                    let col2 = cols.get(2).copied().unwrap_or("");
                    let (number, name) = if col2.is_empty() {
                        (
                            (data.routes.len() + 1).to_string(),
                            (!col1.is_empty()).then(|| col1.to_string()),
                        )
                    } else {
                        (col1.to_string(), Some(col2.to_string()))
                    };
                    let start_station = match cols.get(3).copied().unwrap_or("") {
                        "" => 0.0,
                        raw_sta => match parse_finite_f64(raw_sta) {
                            Some(v) => norm_zero(v),
                            None => {
                                cx.report(
                                    Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                        .with("kind", "F00")
                                        .with("field", "start_station")
                                        .at(at.clone()),
                                );
                                0.0
                            }
                        },
                    };
                    route = Some(RouteDraft {
                        number,
                        name,
                        start_station,
                        attributes: Vec::new(),
                        segments: Vec::new(),
                        direct_points: Vec::new(),
                        source: at,
                    });
                }
                "F01" | "F02" | "F03" => {
                    if let Some(r) = route.as_mut() {
                        let val = cols
                            .get(1..)
                            .map_or_else(String::new, |rest| rest.join(","));
                        let trimmed = val.trim_end_matches(',');
                        if !trimmed.is_empty() {
                            r.attributes.push((
                                code_upper.clone(),
                                AttributeVal::Text(trimmed.to_string()),
                            ));
                        }
                    }
                }
                "F99" => {
                    if let Some(iv) = interval.take() {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        flush_interval(iv, &mut route, &mut data, cx)?;
                    }
                    if let Some(prev_route) = route.take() {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        finish_route(prev_route, &mut data, cx)?;
                    }
                }
                "C00" => {
                    if let Some(iv) = interval.take() {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        flush_interval(iv, &mut route, &mut data, cx)?;
                    }
                    cx.enter().map_err(|s| stopped(cx, s, Some(at.clone())))?;
                    active_depths += 1;
                    interval = Some(IntervalDraft {
                        points: Vec::new(),
                        arc_radius: None,
                        clothoid: None,
                        source: Some(at),
                    });
                }
                "C01" => {
                    let Some(r) = cols.get(1).and_then(|s| parse_finite_f64(s)) else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                .with("kind", "C01")
                                .with("field", "radius")
                                .at(at),
                        );
                        continue;
                    };
                    if r == 0.0 {
                        cx.report(
                            Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                .with("kind", "C01")
                                .with("field", "radius")
                                .at(at),
                        );
                        continue;
                    }
                    if interval.is_none() {
                        cx.enter().map_err(|s| stopped(cx, s, Some(at.clone())))?;
                        active_depths += 1;
                        interval = Some(IntervalDraft {
                            source: Some(at),
                            ..IntervalDraft::default()
                        });
                    }
                    if let Some(iv) = interval.as_mut() {
                        iv.arc_radius = Some(norm_zero(r));
                    }
                }
                "C02" => {
                    let Some(a_param) = cols.get(1).and_then(|s| parse_finite_f64(s)) else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                .with("kind", "C02")
                                .with("field", "clothoid")
                                .at(at),
                        );
                        continue;
                    };
                    if a_param == 0.0 {
                        cx.report(
                            Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                .with("kind", "C02")
                                .with("field", "clothoid")
                                .at(at),
                        );
                        continue;
                    }
                    let explicit_len = match cols.get(2).copied().unwrap_or("") {
                        "" => None,
                        raw_len => match parse_finite_f64(raw_len).filter(|&v| v > 0.0) {
                            Some(v) => Some(v),
                            None => {
                                cx.report(
                                    Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                        .with("kind", "C02")
                                        .with("field", "length")
                                        .at(at.clone()),
                                );
                                None
                            }
                        },
                    };
                    let start_r = match cols.get(3).copied().unwrap_or("") {
                        "" => None,
                        raw_r => match parse_finite_f64(raw_r).filter(|&v| v != 0.0) {
                            Some(v) => Some(v),
                            None => {
                                cx.report(
                                    Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                        .with("kind", "C02")
                                        .with("field", "start_radius")
                                        .at(at.clone()),
                                );
                                None
                            }
                        },
                    };
                    let end_r = match cols.get(4).copied().unwrap_or("") {
                        "" => None,
                        raw_r => match parse_finite_f64(raw_r).filter(|&v| v != 0.0) {
                            Some(v) => Some(v),
                            None => {
                                cx.report(
                                    Diagnostic::new(codes::SIMA_VALUE_INVALID)
                                        .with("kind", "C02")
                                        .with("field", "end_radius")
                                        .at(at.clone()),
                                );
                                None
                            }
                        },
                    };
                    if interval.is_none() {
                        cx.enter().map_err(|s| stopped(cx, s, Some(at.clone())))?;
                        active_depths += 1;
                        interval = Some(IntervalDraft {
                            source: Some(at),
                            ..IntervalDraft::default()
                        });
                    }
                    if let Some(iv) = interval.as_mut() {
                        iv.clothoid = Some(ClothoidDraft {
                            a_signed: norm_zero(a_param),
                            explicit_len,
                            start_r,
                            end_r,
                        });
                    }
                }
                "C03" => {
                    cx.report(
                        Diagnostic::new(codes::SIMA_RECORD_UNSUPPORTED)
                            .with("kind", "C03")
                            .at(at),
                    );
                }
                "C99" => {
                    if let Some(iv) = interval.take() {
                        cx.leave();
                        active_depths = active_depths.saturating_sub(1);
                        flush_interval(iv, &mut route, &mut data, cx)?;
                    }
                }
                "B01" | "B03" => {
                    let number = cols.get(1).copied().unwrap_or("");
                    let name = cols.get(2).copied().unwrap_or("");
                    let Some(pt_idx) = pt_index.find(number, name) else {
                        let label = if !number.is_empty() && !name.is_empty() {
                            format!("{number}:{name}")
                        } else if !number.is_empty() {
                            number.to_string()
                        } else {
                            name.to_string()
                        };
                        cx.report(
                            Diagnostic::new(codes::SIMA_POINT_MISSING)
                                .with("target", label)
                                .at(at),
                        );
                        continue;
                    };
                    let Some(pt) = data.points.get(pt_idx) else {
                        continue;
                    };
                    let loc = MapLocation {
                        point: pt.position,
                        altitude: pt.altitude,
                    };
                    if let Some(p) = parcel.as_mut() {
                        cx.check(Kind::DecodedBytes, 32)
                            .map_err(|s| stopped(cx, s, Some(at)))?;
                        p.points.push(loc);
                    } else if let Some(iv) = interval.as_mut() {
                        cx.check(Kind::DecodedBytes, 32)
                            .map_err(|s| stopped(cx, s, Some(at)))?;
                        iv.points.push(loc);
                    } else if let Some(r) = route.as_mut() {
                        cx.check(Kind::DecodedBytes, 48)
                            .map_err(|s| stopped(cx, s, Some(at.clone())))?;
                        r.direct_points.push((loc, at));
                    } else {
                        cx.report(
                            Diagnostic::new(codes::SIMA_RECORD_UNSUPPORTED)
                                .with("kind", code_upper)
                                .at(at),
                        );
                    }
                }
                _ => {
                    cx.report(
                        Diagnostic::new(codes::SIMA_RECORD_UNSUPPORTED)
                            .with("kind", code_upper)
                            .at(at),
                    );
                }
            }
        }

        if let Some(p) = parcel.take() {
            cx.leave();
            active_depths = active_depths.saturating_sub(1);
            finish_parcel(p, &mut data, cx)?;
        }
        if let Some(iv) = interval.take() {
            cx.leave();
            active_depths = active_depths.saturating_sub(1);
            flush_interval(iv, &mut route, &mut data, cx)?;
        }
        if let Some(r) = route.take() {
            cx.leave();
            active_depths = active_depths.saturating_sub(1);
            finish_route(r, &mut data, cx)?;
        }
        Ok(())
    })();

    while active_depths > 0 {
        cx.leave();
        active_depths -= 1;
    }

    res?;
    Ok(data)
}

fn parse_header_crs(crs_col: &str, at: &Ref, data: &mut SimaData, cx: &mut Cx) {
    if let Some(crs) = Crs::from_name(crs_col).filter(|crs| crs.zone().is_some()) {
        data.crs = Some(crs);
        if matches!(crs, Crs::Plane(_)) {
            data.realization = Some(Realization::Survey2011);
        }
        return;
    }
    if let Ok(zone_num) = crs_col.parse::<u8>()
        && let Some(zone) = Zone::new(zone_num)
    {
        data.crs = Some(Crs::Plane(zone));
        data.realization = Some(Realization::Survey2011);
        return;
    }
    data.crs_reported = true;
    cx.report(
        Diagnostic::new(codes::SIMA_CRS_UNSUPPORTED)
            .with("system", crs_col)
            .at(at.clone()),
    );
}

fn parse_finite_f64(raw: &str) -> Option<f64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let val = trimmed.parse::<f64>().ok()?;
    val.is_finite().then_some(val)
}

fn finish_parcel(draft: ParcelDraft, data: &mut SimaData, cx: &mut Cx) -> Result<(), Error> {
    let mut pts = draft.points;
    // 末尾が始点と同じであれば重複を除く
    if draft.closed && pts.len() >= 2 && pts.first().map(|p| p.point) == pts.last().map(|p| p.point)
    {
        pts.pop();
    }
    let min_pts = if draft.closed { 3 } else { 2 };
    if pts.len() < min_pts {
        cx.report(
            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                .with("kind", if draft.closed { "parcel" } else { "polyline" })
                .at(draft.source),
        );
        return Ok(());
    }
    cx.check(Kind::Elements, 1)
        .map_err(|s| stopped(cx, s, Some(draft.source.clone())))?;
    cx.check(Kind::DecodedBytes, (pts.len() * 32 + 64) as u64)
        .map_err(|s| stopped(cx, s, Some(draft.source.clone())))?;
    data.parcels.push(Parcel {
        id_hint: None,
        number: draft.number,
        name: draft.name,
        closed: draft.closed,
        outer: pts,
        holes: Vec::new(),
        curve_segments: Vec::new(),
        ring_curve_approximated: false,
        kind: if draft.closed {
            ParcelKind::Kakuchi
        } else {
            ParcelKind::KaihouKakuchi
        },
        attributes: Vec::new(),
        source: draft.source,
    });
    Ok(())
}

fn flush_interval(
    iv: IntervalDraft,
    route: &mut Option<RouteDraft>,
    data: &mut SimaData,
    cx: &mut Cx,
) -> Result<(), Error> {
    let src_ref = iv.source.unwrap_or_default();
    if let Some(r) = route.as_mut() {
        flush_direct_points(r, cx)?;
    }
    if iv.points.len() < 2 {
        cx.report(
            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                .with("kind", "route_segment")
                .at(src_ref),
        );
        return Ok(());
    }
    let Some(&first) = iv.points.first() else {
        return Ok(());
    };
    let Some(&last) = iv.points.last() else {
        return Ok(());
    };

    let mut built = Vec::new();
    if let Some(radius) = iv.arc_radius {
        let Some((direction, length)) = chord_arc_params(first.point, last.point, radius) else {
            cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "C01")
                    .with("field", "radius")
                    .at(src_ref),
            );
            return Ok(());
        };
        built.push(RouteSegment {
            start: first.point,
            end: Some(last.point),
            direction,
            length,
            kind: RouteSegmentKind::Arc { radius },
            source: src_ref.clone(),
        });
    } else if let Some(ClothoidDraft {
        a_signed,
        explicit_len,
        start_r,
        end_r,
    }) = iv.clothoid
    {
        let Some((chord_dir, chord_len)) = bearing_and_distance(first.point, last.point) else {
            cx.report(
                Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                    .with("kind", "clothoid")
                    .at(src_ref),
            );
            return Ok(());
        };
        let a_abs = libm::fabs(a_signed);
        let sign = if a_signed >= 0.0 { 1.0 } else { -1.0 };
        let inferred_start = start_r.map(norm_zero);
        let (seg_len, inferred_end) = if inferred_start.is_none() && end_r.is_none() {
            let l = match explicit_len {
                Some(v) => v,
                None => {
                    let mut est = chord_len;
                    for _ in 0..4 {
                        let k_end = sign * est / (a_abs * a_abs);
                        if !k_end.is_finite() || !est.is_finite() {
                            break;
                        }
                        let work =
                            zumen_geom2d::along_work([0.0, k_end], est, Some(Ramp::Clothoid), est);
                        cx.check(Kind::Work, work)
                            .map_err(|s| stopped(cx, s, Some(src_ref.clone())))?;
                        let ([dn, de], _) = zumen_geom2d::along(
                            [0.0, 0.0],
                            0.0,
                            [0.0, k_end],
                            est,
                            Some(Ramp::Clothoid),
                            est,
                        );
                        let c = libm::hypot(dn, de);
                        if c.is_finite() && c > 1e-9 {
                            let next_est = chord_len * (est / c);
                            if next_est.is_finite() {
                                est = next_est;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    est
                }
            };
            let r_end = (l > 1e-9).then(|| norm_zero(sign * (a_abs * a_abs) / l));
            (l, r_end)
        } else {
            let r_end = end_r.map(norm_zero);
            let k1_raw = inferred_start.map_or(0.0, |r| 1.0 / r);
            let k2_raw = r_end.map_or(0.0, |r| 1.0 / r);
            let l = explicit_len.unwrap_or_else(|| {
                let dk = libm::fabs(k2_raw - k1_raw);
                if dk.is_finite() && dk > 1e-12 {
                    a_abs * a_abs * dk
                } else {
                    chord_len
                }
            });
            (l, r_end)
        };

        let [k1, k2] = clothoid_curvatures(a_abs, seg_len, inferred_start, inferred_end);
        let dk = k2 - k1;
        let valid_r = |r: Option<f64>| r.is_none_or(|v| v.is_finite() && libm::fabs(v) > 1e-9);
        if !seg_len.is_finite()
            || seg_len <= 1e-9
            || !valid_r(inferred_start)
            || !valid_r(inferred_end)
            || !k1.is_finite()
            || !k2.is_finite()
            || !dk.is_finite()
            || libm::fabs(dk) <= 1e-12
        {
            cx.report(
                Diagnostic::new(codes::SIMA_VALUE_INVALID)
                    .with("kind", "C02")
                    .with("field", "clothoid")
                    .at(src_ref),
            );
            return Ok(());
        }
        let work = zumen_geom2d::along_work([k1, k2], seg_len, Some(Ramp::Clothoid), seg_len);
        cx.check(Kind::Work, work)
            .map_err(|s| stopped(cx, s, Some(src_ref.clone())))?;
        let ([dn0, de0], _) = zumen_geom2d::along(
            [0.0, 0.0],
            0.0,
            [k1, k2],
            seg_len,
            Some(Ramp::Clothoid),
            seg_len,
        );
        let defl = if dn0.is_finite() && de0.is_finite() && (dn0 != 0.0 || de0 != 0.0) {
            libm::atan2(de0, dn0)
        } else {
            0.0
        };
        let start_dir = norm_zero((chord_dir - defl).rem_euclid(2.0 * PI));
        built.push(RouteSegment {
            start: first.point,
            end: Some(last.point),
            direction: start_dir,
            length: norm_zero(seg_len),
            kind: RouteSegmentKind::Clothoid {
                parameter: norm_zero(a_abs),
                start_radius: inferred_start,
                end_radius: inferred_end,
            },
            source: src_ref.clone(),
        });
    } else {
        for pair in iv.points.windows(2) {
            let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                continue;
            };
            if let Some((direction, length)) = bearing_and_distance(a.point, b.point) {
                built.push(RouteSegment {
                    start: a.point,
                    end: Some(b.point),
                    direction,
                    length,
                    kind: RouteSegmentKind::Line,
                    source: src_ref.clone(),
                });
            }
        }
    }

    if built.is_empty() {
        cx.report(
            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                .with("kind", "route_segment")
                .at(src_ref),
        );
        return Ok(());
    }
    cx.check(Kind::Elements, built.len() as u64)
        .map_err(|s| stopped(cx, s, Some(src_ref.clone())))?;
    cx.check(Kind::DecodedBytes, (built.len() * 64) as u64)
        .map_err(|s| stopped(cx, s, Some(src_ref.clone())))?;

    if let Some(r) = route.as_mut() {
        r.segments.extend(built);
    } else {
        let number = (data.routes.len() + 1).to_string();
        data.routes.push(Route {
            number,
            name: None,
            start_station: 0.0,
            segments: built,
            attributes: Vec::new(),
            source: src_ref,
        });
    }
    Ok(())
}

fn flush_direct_points(r: &mut RouteDraft, cx: &mut Cx) -> Result<(), Error> {
    if r.direct_points.len() >= 2 {
        for pair in r.direct_points.windows(2) {
            let (Some((a, at_a)), Some((b, _))) = (pair.first(), pair.get(1)) else {
                continue;
            };
            if let Some((direction, length)) = bearing_and_distance(a.point, b.point) {
                cx.check(Kind::Elements, 1)
                    .map_err(|s| stopped(cx, s, Some(at_a.clone())))?;
                cx.check(Kind::DecodedBytes, 64)
                    .map_err(|s| stopped(cx, s, Some(at_a.clone())))?;
                r.segments.push(RouteSegment {
                    start: a.point,
                    end: Some(b.point),
                    direction,
                    length,
                    kind: RouteSegmentKind::Line,
                    source: at_a.clone(),
                });
            }
        }
    }
    r.direct_points.clear();
    Ok(())
}

fn finish_route(mut r: RouteDraft, data: &mut SimaData, cx: &mut Cx) -> Result<(), Error> {
    flush_direct_points(&mut r, cx)?;
    if r.segments.is_empty() {
        cx.report(
            Diagnostic::new(codes::SIMA_GEOMETRY_DROPPED)
                .with("kind", "route")
                .at(r.source),
        );
        return Ok(());
    }
    cx.check(Kind::DecodedBytes, 64)
        .map_err(|s| stopped(cx, s, Some(r.source.clone())))?;
    data.routes.push(Route {
        number: r.number,
        name: r.name,
        start_station: r.start_station,
        segments: r.segments,
        attributes: r.attributes,
        source: r.source,
    });
    Ok(())
}
