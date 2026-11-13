//! 地理の領域モデル（[`GeoModel`]）から GeoJSON（RFC 7946）への書き出し。

use std::collections::BTreeSet;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_crs::{Crs, GeoPoint, MapPoint, Zone, to_geographic};
use zumen_geo::{Attribute, Feature, Form, GeoModel, Location, Position, Value};
use zumen_geom3d::{Shape, Transform, Xyz};
use zumen_json::Writer;
use zumen_sdk::{Cx, Error};

use crate::json::MAX_ALLOC;
use crate::stopped;

/// 書き出した GeoJSON のバイト列と地物の数
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredGeoJson {
    /// UTF-8 の JSON バイト列
    pub bytes: Vec<u8>,
    /// 書き出した `Feature` の数
    pub features: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pos {
    lon: f64,
    lat: f64,
    height: Option<f64>,
}

enum OutGeom {
    Point(Pos),
    Line(Vec<Pos>),
    AreaPolygon {
        outer: Vec<Pos>,
        holes: Vec<Vec<Pos>>,
    },
    SolidPolygon {
        outer: Vec<Pos>,
        holes: Vec<Vec<Pos>>,
    },
}

enum RingOutcome {
    Ring(Vec<Pos>),
    Degenerate2D,
    InvalidCoord,
}

#[derive(Clone, Copy)]
enum Mode {
    Wgs84,
    Geographic,
    Plane(Zone),
    NoGeometry,
}

/// 出力の増分ごとに `Kind::DecodedBytes` と `MAX_ALLOC` を検査する `Writer` のラッパー
struct CheckedWriter {
    inner: Writer,
    charged: usize,
}

impl CheckedWriter {
    fn new() -> Self {
        Self {
            inner: Writer::new(),
            charged: 0,
        }
    }

    fn sync(&mut self, cx: &mut Cx<'_>, source: Option<&Ref>) -> Result<(), Error> {
        let len = self.inner.len();
        if (len as u64) > MAX_ALLOC {
            return Err(Error::Invalid);
        }
        if len > self.charged {
            let delta = (len - self.charged) as u64;
            cx.check(Kind::DecodedBytes, delta)
                .map_err(|s| stopped(cx, s, source.cloned()))?;
            self.charged = len;
        }
        Ok(())
    }

    fn into_bytes(mut self, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
        self.sync(cx, None)?;
        Ok(self.inner.into_bytes())
    }
}

/// 地理の領域モデルを RFC 7946 の `FeatureCollection` に書き出す
pub fn lower_to_geojson(model: &GeoModel, cx: &mut Cx<'_>) -> Result<LoweredGeoJson, Error> {
    let has_geometries = model.features.iter().any(|f| !f.geometries.is_empty());
    let mode = match model.crs {
        Some(Crs::Wgs84) => Mode::Wgs84,
        Some(Crs::Geographic) => {
            if has_geometries {
                cx.report(
                    Diagnostic::new(codes::GEOJSON_DATUM_APPROXIMATED)
                        .with("epsg", u64::from(Crs::Geographic.epsg())),
                );
            }
            Mode::Geographic
        }
        Some(crs @ Crs::Plane(zone)) => {
            if has_geometries {
                cx.report(
                    Diagnostic::new(codes::GEOJSON_DATUM_APPROXIMATED)
                        .with("epsg", u64::from(crs.epsg())),
                );
            }
            Mode::Plane(zone)
        }
        Some(crs @ (Crs::Jgd2000(_) | Crs::Tokyo(_))) => {
            cx.report(
                Diagnostic::new(codes::GEOJSON_CRS_UNSUPPORTED).with("epsg", u64::from(crs.epsg())),
            );
            return Err(Error::Invalid);
        }
        None => {
            if has_geometries {
                cx.report(Diagnostic::new(codes::GEOJSON_CRS_MISSING));
                return Err(Error::Invalid);
            }
            Mode::NoGeometry
        }
    };

    let mut out = CheckedWriter::new();
    out.inner.begin_object();
    out.inner.key("type");
    out.inner.string("FeatureCollection");
    out.inner.key("features");
    out.inner.begin_array();
    out.sync(cx, None)?;

    for feature in &model.features {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(feature.source.clone())))?;
        cx.check(Kind::Elements, 1)
            .map_err(|s| stopped(cx, s, Some(feature.source.clone())))?;
        write_feature(&mut out, feature, mode, cx)?;
    }

    out.inner.end_array();
    out.inner.end_object();

    let bytes = out.into_bytes(cx)?;
    Ok(LoweredGeoJson {
        bytes,
        features: model.features.len() as u64,
    })
}

fn write_feature(
    out: &mut CheckedWriter,
    feature: &Feature,
    mode: Mode,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let mut geoms = Vec::new();
    for geom in &feature.geometries {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(feature.source.clone())))?;
        convert_form(&geom.form, mode, &feature.source, cx, &mut geoms)?;
    }

    out.inner.begin_object();
    out.inner.key("type");
    out.inner.string("Feature");
    if !feature.id.is_empty() {
        out.inner.key("id");
        out.inner.string(&feature.id);
    }
    out.sync(cx, Some(&feature.source))?;

    out.inner.key("geometry");
    write_geometries(out, &geoms, &feature.source, cx)?;

    out.inner.key("properties");
    write_properties(out, feature, cx)?;
    out.inner.end_object();
    out.sync(cx, Some(&feature.source))?;
    Ok(())
}

fn convert_form(
    form: &Form,
    mode: Mode,
    source: &Ref,
    cx: &mut Cx<'_>,
    out: &mut Vec<OutGeom>,
) -> Result<(), Error> {
    match form {
        Form::Point(loc) => match convert_location(*loc, mode, source, cx) {
            Some(pos) => out.push(OutGeom::Point(pos)),
            None => report_dropped(cx, "point", source),
        },
        Form::Line(pts) => {
            if pts.len() < 2 {
                report_dropped(cx, "line", source);
                return Ok(());
            }
            let mut conv = Vec::with_capacity(pts.len());
            for loc in pts {
                cx.check(Kind::Work, 1)
                    .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                let Some(pos) = convert_location(*loc, mode, source, cx) else {
                    report_dropped(cx, "line", source);
                    return Ok(());
                };
                conv.push(pos);
            }
            out.push(OutGeom::Line(conv));
        }
        Form::Area { outer, holes } => {
            let RingOutcome::Ring(outer_ring) =
                convert_ring_locations(outer, true, mode, source, cx)?
            else {
                report_dropped(cx, "area", source);
                return Ok(());
            };
            let mut hole_rings = Vec::with_capacity(holes.len());
            for hole in holes {
                let RingOutcome::Ring(ring) =
                    convert_ring_locations(hole, false, mode, source, cx)?
                else {
                    report_dropped(cx, "area", source);
                    return Ok(());
                };
                hole_rings.push(ring);
            }
            out.push(OutGeom::AreaPolygon {
                outer: outer_ring,
                holes: hole_rings,
            });
        }
        Form::Solid(shape) => {
            let before = out.len();
            let mut had_invalid_coord = false;
            convert_solid_shape(
                shape,
                Transform::IDENTITY,
                mode,
                source,
                cx,
                out,
                &mut had_invalid_coord,
            )?;
            if out.len() > before {
                cx.report(Diagnostic::new(codes::GEOJSON_SOLID_APPROXIMATED).at(source.clone()));
                if had_invalid_coord {
                    report_dropped(cx, "solid", source);
                }
            } else {
                report_dropped(cx, "solid", source);
            }
        }
    }
    Ok(())
}

fn convert_solid_shape(
    shape: &Shape,
    transform: Transform,
    mode: Mode,
    source: &Ref,
    cx: &mut Cx<'_>,
    out: &mut Vec<OutGeom>,
    had_invalid_coord: &mut bool,
) -> Result<(), Error> {
    cx.enter()
        .map_err(|s| stopped(cx, s, Some(source.clone())))?;
    let res = convert_solid_shape_inner(shape, transform, mode, source, cx, out, had_invalid_coord);
    cx.leave();
    res
}

fn convert_solid_shape_inner(
    shape: &Shape,
    transform: Transform,
    mode: Mode,
    source: &Ref,
    cx: &mut Cx<'_>,
    out: &mut Vec<OutGeom>,
    had_invalid_coord: &mut bool,
) -> Result<(), Error> {
    cx.check(Kind::Work, 1)
        .map_err(|s| stopped(cx, s, Some(source.clone())))?;
    match shape {
        Shape::FaceSet { faces, .. } => {
            for face in faces {
                cx.check(Kind::Work, 1)
                    .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                let outer = match convert_ring_xyz(&face.outer, transform, true, mode, source, cx)?
                {
                    RingOutcome::Ring(r) => r,
                    RingOutcome::Degenerate2D => continue,
                    RingOutcome::InvalidCoord => {
                        *had_invalid_coord = true;
                        continue;
                    }
                };
                let mut holes = Vec::with_capacity(face.holes.len());
                let mut holes_ok = true;
                for hole in &face.holes {
                    match convert_ring_xyz(hole, transform, false, mode, source, cx)? {
                        RingOutcome::Ring(r) => holes.push(r),
                        RingOutcome::Degenerate2D => {
                            holes_ok = false;
                            break;
                        }
                        RingOutcome::InvalidCoord => {
                            *had_invalid_coord = true;
                            holes_ok = false;
                            break;
                        }
                    }
                }
                if holes_ok {
                    out.push(OutGeom::SolidPolygon { outer, holes });
                }
            }
        }
        Shape::Triangles {
            positions,
            triangles,
            ..
        } => {
            for &[i0, i1, i2] in triangles {
                cx.check(Kind::Work, 1)
                    .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                let (Some(&p0), Some(&p1), Some(&p2)) = (
                    positions.get(i0 as usize),
                    positions.get(i1 as usize),
                    positions.get(i2 as usize),
                ) else {
                    *had_invalid_coord = true;
                    continue;
                };
                match convert_ring_xyz(&[p0, p1, p2], transform, true, mode, source, cx)? {
                    RingOutcome::Ring(outer) => out.push(OutGeom::SolidPolygon {
                        outer,
                        holes: Vec::new(),
                    }),
                    RingOutcome::Degenerate2D => {}
                    RingOutcome::InvalidCoord => {
                        *had_invalid_coord = true;
                    }
                }
            }
        }
        Shape::Mapped {
            transform: inner,
            shape: child,
        } => {
            let combined = inner.then(&transform);
            convert_solid_shape(child, combined, mode, source, cx, out, had_invalid_coord)?;
        }
        Shape::Group(children) => {
            for child in children {
                convert_solid_shape(child, transform, mode, source, cx, out, had_invalid_coord)?;
            }
        }
        Shape::Extrusion { .. }
        | Shape::Revolution { .. }
        | Shape::SweptDisk { .. }
        | Shape::Boolean { .. }
        | Shape::HalfSpace { .. } => {}
    }
    Ok(())
}

fn normalize_zero(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}

fn convert_location(loc: Location, mode: Mode, source: &Ref, cx: &mut Cx<'_>) -> Option<Pos> {
    if let Some(h) = loc.height
        && !h.is_finite()
    {
        return None;
    }
    let height = loc.height.map(normalize_zero);
    match (loc.position, mode) {
        (Position::Geo(geo), Mode::Wgs84 | Mode::Geographic) => {
            valid_geo(geo).map(|(lon, lat)| Pos { lon, lat, height })
        }
        (Position::Map(map), Mode::Plane(zone)) => {
            unproject_map(zone, map, source, cx).map(|(lon, lat)| Pos { lon, lat, height })
        }
        (Position::Geo(geo), Mode::Plane(_)) => {
            valid_geo(geo).map(|(lon, lat)| Pos { lon, lat, height })
        }
        _ => None,
    }
}

fn convert_xyz(
    pt: Xyz,
    transform: Transform,
    mode: Mode,
    source: &Ref,
    cx: &mut Cx<'_>,
) -> Option<Pos> {
    let [a, b, z] = transform.point(pt);
    if !z.is_finite() {
        return None;
    }
    let height = Some(normalize_zero(z));
    match mode {
        Mode::Wgs84 | Mode::Geographic => {
            // GeoModel::Form::Solid は地理の空間では [緯度, 経度, 標高] の順
            valid_geo(GeoPoint {
                latitude: a,
                longitude: b,
            })
            .map(|(lon, lat)| Pos { lon, lat, height })
        }
        Mode::Plane(zone) => {
            // GeoModel::Form::Solid は地図の空間では [easting, northing, 標高] の順
            unproject_map(
                zone,
                MapPoint {
                    easting: a,
                    northing: b,
                },
                source,
                cx,
            )
            .map(|(lon, lat)| Pos { lon, lat, height })
        }
        Mode::NoGeometry => None,
    }
}

fn valid_geo(geo: GeoPoint) -> Option<(f64, f64)> {
    (geo.longitude.is_finite()
        && geo.latitude.is_finite()
        && (-90.0..=90.0).contains(&geo.latitude))
    .then_some((normalize_zero(geo.longitude), normalize_zero(geo.latitude)))
}

fn unproject_map(zone: Zone, map: MapPoint, source: &Ref, cx: &mut Cx<'_>) -> Option<(f64, f64)> {
    let Some(unprojected) = to_geographic(zone, map) else {
        cx.report(
            Diagnostic::new(codes::CRS_PROJECTION_UNSUPPORTED)
                .with("crs", u64::from(Crs::Plane(zone).epsg()))
                .at(source.clone()),
        );
        return None;
    };
    let Some(pair) = valid_geo(unprojected.point) else {
        cx.report(
            Diagnostic::new(codes::CRS_PROJECTION_UNSUPPORTED)
                .with("crs", u64::from(Crs::Plane(zone).epsg()))
                .at(source.clone()),
        );
        return None;
    };
    Some(pair)
}

fn convert_ring_locations(
    pts: &[Location],
    want_ccw: bool,
    mode: Mode,
    source: &Ref,
    cx: &mut Cx<'_>,
) -> Result<RingOutcome, Error> {
    let mut ring = Vec::with_capacity(pts.len().saturating_add(1));
    for loc in pts {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(source.clone())))?;
        let Some(pos) = convert_location(*loc, mode, source, cx) else {
            return Ok(RingOutcome::InvalidCoord);
        };
        ring.push(pos);
    }
    Ok(match finalize_ring(ring, want_ccw) {
        Some(r) => RingOutcome::Ring(r),
        None => RingOutcome::Degenerate2D,
    })
}

fn convert_ring_xyz(
    pts: &[Xyz],
    transform: Transform,
    want_ccw: bool,
    mode: Mode,
    source: &Ref,
    cx: &mut Cx<'_>,
) -> Result<RingOutcome, Error> {
    let mut ring = Vec::with_capacity(pts.len().saturating_add(1));
    for &pt in pts {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(source.clone())))?;
        let Some(pos) = convert_xyz(pt, transform, mode, source, cx) else {
            return Ok(RingOutcome::InvalidCoord);
        };
        ring.push(pos);
    }
    Ok(match finalize_ring(ring, want_ccw) {
        Some(r) => RingOutcome::Ring(r),
        None => RingOutcome::Degenerate2D,
    })
}

fn same_2d(a: Pos, b: Pos) -> bool {
    a.lon == b.lon && a.lat == b.lat
}

/// 水平 `(lon, lat)` 上の連続重複を除き、非ゼロの面積を持つ輪だけを RFC 7946 3.1.6 の右手の法則に整えて閉合する
fn finalize_ring(raw: Vec<Pos>, want_ccw: bool) -> Option<Vec<Pos>> {
    let mut open: Vec<Pos> = Vec::with_capacity(raw.len().saturating_add(1));
    for pos in raw {
        if !open.last().is_some_and(|&prev| same_2d(prev, pos)) {
            open.push(pos);
        }
    }
    if open.len() > 1
        && let (Some(&first), Some(&last)) = (open.first(), open.last())
        && same_2d(first, last)
    {
        open.pop();
    }
    if open.len() < 3 {
        return None;
    }
    let area2 = signed_area2(&open);
    if !area2.is_finite() || area2 == 0.0 {
        return None;
    }
    if (want_ccw && area2 < 0.0) || (!want_ccw && area2 > 0.0) {
        open.reverse();
    }
    let first = *open.first()?;
    open.push(first);
    Some(open)
}

/// 最初の頂点を基準にした 2 倍符号つき面積（反時計回りが正、時計回りが負）
fn signed_area2(open: &[Pos]) -> f64 {
    let Some(&origin) = open.first() else {
        return 0.0;
    };
    let mut sum = 0.0;
    for (idx, cur) in open.iter().enumerate() {
        let next = open.get(idx + 1).or_else(|| open.first());
        if let Some(next) = next {
            let x0 = cur.lon - origin.lon;
            let y0 = cur.lat - origin.lat;
            let x1 = next.lon - origin.lon;
            let y1 = next.lat - origin.lat;
            sum += x0 * y1 - x1 * y0;
        }
    }
    sum
}

fn report_dropped(cx: &mut Cx<'_>, form: &'static str, source: &Ref) {
    cx.report(
        Diagnostic::new(codes::GEOJSON_GEOMETRY_DROPPED)
            .with("form", zumen_base::Value::Key(form))
            .at(source.clone()),
    );
}

fn write_geometries(
    out: &mut CheckedWriter,
    geoms: &[OutGeom],
    source: &Ref,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    match geoms {
        [] => out.inner.null(),
        [single] => write_single_geom(out, single, source, cx)?,
        many => {
            if many.iter().all(|g| matches!(g, OutGeom::Point(_))) {
                out.inner.begin_object();
                out.inner.key("type");
                out.inner.string("MultiPoint");
                out.inner.key("coordinates");
                out.inner.begin_array();
                for g in many {
                    cx.check(Kind::Work, 1)
                        .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                    if let OutGeom::Point(pos) = g {
                        write_pos(&mut out.inner, *pos);
                    }
                    out.sync(cx, Some(source))?;
                }
                out.inner.end_array();
                out.inner.end_object();
            } else if many.iter().all(|g| matches!(g, OutGeom::Line(_))) {
                out.inner.begin_object();
                out.inner.key("type");
                out.inner.string("MultiLineString");
                out.inner.key("coordinates");
                out.inner.begin_array();
                for g in many {
                    cx.check(Kind::Work, 1)
                        .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                    if let OutGeom::Line(pts) = g {
                        write_pos_array(out, pts, source, cx)?;
                    }
                }
                out.inner.end_array();
                out.inner.end_object();
            } else if many
                .iter()
                .all(|g| matches!(g, OutGeom::AreaPolygon { .. }))
            {
                out.inner.begin_object();
                out.inner.key("type");
                out.inner.string("MultiPolygon");
                out.inner.key("coordinates");
                out.inner.begin_array();
                for g in many {
                    cx.check(Kind::Work, 1)
                        .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                    if let OutGeom::AreaPolygon { outer, holes } = g {
                        write_polygon_rings(out, outer, holes, source, cx)?;
                    }
                }
                out.inner.end_array();
                out.inner.end_object();
            } else {
                out.inner.begin_object();
                out.inner.key("type");
                out.inner.string("GeometryCollection");
                out.inner.key("geometries");
                out.inner.begin_array();
                for g in many {
                    cx.check(Kind::Work, 1)
                        .map_err(|s| stopped(cx, s, Some(source.clone())))?;
                    write_single_geom(out, g, source, cx)?;
                }
                out.inner.end_array();
                out.inner.end_object();
            }
        }
    }
    out.sync(cx, Some(source))
}

fn write_single_geom(
    out: &mut CheckedWriter,
    geom: &OutGeom,
    source: &Ref,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    out.inner.begin_object();
    match geom {
        OutGeom::Point(pos) => {
            out.inner.key("type");
            out.inner.string("Point");
            out.inner.key("coordinates");
            write_pos(&mut out.inner, *pos);
        }
        OutGeom::Line(pts) => {
            out.inner.key("type");
            out.inner.string("LineString");
            out.inner.key("coordinates");
            write_pos_array(out, pts, source, cx)?;
        }
        OutGeom::AreaPolygon { outer, holes } | OutGeom::SolidPolygon { outer, holes } => {
            out.inner.key("type");
            out.inner.string("Polygon");
            out.inner.key("coordinates");
            write_polygon_rings(out, outer, holes, source, cx)?;
        }
    }
    out.inner.end_object();
    out.sync(cx, Some(source))
}

fn write_polygon_rings(
    out: &mut CheckedWriter,
    outer: &[Pos],
    holes: &[Vec<Pos>],
    source: &Ref,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    out.inner.begin_array();
    write_pos_array(out, outer, source, cx)?;
    for hole in holes {
        write_pos_array(out, hole, source, cx)?;
    }
    out.inner.end_array();
    out.sync(cx, Some(source))
}

fn write_pos_array(
    out: &mut CheckedWriter,
    pts: &[Pos],
    source: &Ref,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    out.inner.begin_array();
    for (idx, &pos) in pts.iter().enumerate() {
        if idx.is_multiple_of(32) {
            cx.check(Kind::Work, 1)
                .map_err(|s| stopped(cx, s, Some(source.clone())))?;
        }
        write_pos(&mut out.inner, pos);
        if idx.is_multiple_of(32) {
            out.sync(cx, Some(source))?;
        }
    }
    out.inner.end_array();
    out.sync(cx, Some(source))
}

fn write_pos(out: &mut Writer, pos: Pos) {
    out.begin_array();
    out.float(pos.lon);
    out.float(pos.lat);
    if let Some(h) = pos.height {
        out.float(h);
    }
    out.end_array();
}

fn write_properties(
    out: &mut CheckedWriter,
    feature: &Feature,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    let mut rev_attrs: Vec<&Attribute> = Vec::new();
    for attr in feature.attributes.iter().rev() {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(feature.source.clone())))?;
        if seen.insert(attr.name.as_str()) {
            rev_attrs.push(attr);
        }
    }
    rev_attrs.reverse();

    out.inner.begin_object();
    if !feature.kind.is_empty() && feature.kind != "Feature" && !seen.contains("kind") {
        out.inner.key("kind");
        out.inner.string(&feature.kind);
    }
    if let Some(code) = &feature.code
        && !seen.contains("code")
    {
        out.inner.key("code");
        out.inner.string(code);
    }
    if let Some(name) = &feature.name
        && !seen.contains("name")
    {
        out.inner.key("name");
        out.inner.string(name);
    }
    out.sync(cx, Some(&feature.source))?;

    for attr in rev_attrs {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(feature.source.clone())))?;
        match &attr.value {
            Value::Text(s) | Value::Date(s) => {
                out.inner.key(&attr.name);
                out.inner.string(s);
            }
            Value::Code { value, .. } => {
                out.inner.key(&attr.name);
                out.inner.string(value);
            }
            Value::Bool(b) => {
                out.inner.key(&attr.name);
                out.inner.bool(*b);
            }
            Value::Integer(n) => {
                out.inner.key(&attr.name);
                out.inner.int(*n);
            }
            Value::Real(v) | Value::Measure { value: v, .. } => {
                if v.is_finite() {
                    out.inner.key(&attr.name);
                    out.inner.float(normalize_zero(*v));
                } else {
                    cx.report(
                        Diagnostic::new(codes::GEOJSON_ATTRIBUTE_DROPPED)
                            .with("name", attr.name.as_str())
                            .at(feature.source.clone()),
                    );
                }
            }
        }
        out.sync(cx, Some(&feature.source))?;
    }
    out.inner.end_object();
    out.sync(cx, Some(&feature.source))
}
