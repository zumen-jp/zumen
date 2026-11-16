//! 図面とシーンで共有する選択・投影・予算。座標は元のモデルへ書き戻さない。
use std::collections::BTreeSet;

use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
#[cfg(feature = "scene3d")]
use zumen_crs::MapPoint;
use zumen_crs::{Crs, GeoPoint, Zone, to_geographic, to_plane};
use zumen_geom3d::{Failure, Shape, Transform};

use crate::{Feature, Form, GeoModel, Geometry, Location, Position};

/// 条件の中は和、条件どうしは積。順序は元の地物と形の順を保つ。
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection<'a> {
    /// 元の地物 ID。空の列なら選ばない。
    pub ids: Option<&'a [String]>,
    /// 元の型の名前。
    pub kinds: Option<&'a [String]>,
    /// 明記された LOD の選択。LOD のない 2D の形は常に含める。
    pub lod: Option<u8>,
}

impl Selection<'_> {
    pub(crate) fn includes(&self, feature: &Feature, cx: &mut Cx) -> Result<bool, Failure> {
        let mut matches = true;
        for (choices, value) in [(self.ids, &feature.id), (self.kinds, &feature.kind)] {
            if let Some(choices) = choices {
                let mut found = false;
                for choice in choices {
                    cx.check(Kind::Work, choice.len().min(value.len()) as u64 + 1)?;
                    found |= choice == value;
                }
                matches &= found;
            }
        }
        Ok(matches)
    }

    pub(crate) fn geometry(&self, geometry: &Geometry) -> bool {
        self.lod.is_none() || geometry.lod.is_none() || self.lod == geometry.lod
    }
}

pub(crate) fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    slot: u64,
    cx: &mut Cx,
) -> Result<(), Failure> {
    let size = values
        .len()
        .checked_add(count)
        .ok_or(Failure::Invalid("size"))?;
    if size as u64 > (512 * 1024 * 1024) / slot.max(1) {
        return Err(Failure::Invalid("size"));
    }
    if size > values.capacity() {
        let capacity = size
            .max(values.capacity().saturating_mul(2))
            .min(((512 * 1024 * 1024) / slot.max(1)) as usize);
        cx.check(
            Kind::DecodedBytes,
            (capacity - values.capacity()) as u64 * slot,
        )?;
        values
            .try_reserve_exact(capacity - values.len())
            .map_err(|_| Failure::Invalid("allocation"))?;
    }
    Ok(())
}

pub(crate) fn text(value: &str, cx: &mut Cx) -> Result<String, Failure> {
    cx.check(Kind::Work, value.len() as u64 + 1)?;
    cx.check(Kind::DecodedBytes, value.len() as u64)?;
    Ok(value.to_owned())
}

pub(crate) fn validate(
    model: &GeoModel,
    selection: Selection<'_>,
    cx: &mut Cx,
) -> Result<(), Failure> {
    if selection.lod.is_some_and(|lod| lod > 4) {
        return Err(Failure::Invalid("lod"));
    }
    let mut ids = BTreeSet::new();
    for feature in &model.features {
        cx.check(Kind::Elements, 1)?;
        cx.check(Kind::DecodedBytes, 128)?;
        cx.check(Kind::Work, (feature.id.len() as u64 + 1).saturating_mul(64))?;
        if feature.id.is_empty() || !ids.insert(feature.id.as_str()) {
            return Err(Failure::Invalid("feature_id"));
        }
    }
    Ok(())
}

pub(crate) fn omitted(
    feature: &Feature,
    form: &str,
    reason: &str,
    cx: &mut Cx,
) -> Result<(), Failure> {
    cx.check(
        Kind::DecodedBytes,
        feature.id.len() as u64 + form.len() as u64 + reason.len() as u64 + 128,
    )?;
    let place = source(feature, cx)?;
    cx.report(
        Diagnostic::new(codes::GEO_GEOMETRY_UNSUPPORTED)
            .with("form", form)
            .with("reason", reason)
            .at(place),
    );
    cx.check(Kind::Work, 0)?;
    Ok(())
}

pub(crate) fn source(feature: &Feature, cx: &mut Cx) -> Result<Ref, Failure> {
    let mut source = Ref::default().with_id(text(&feature.id, cx)?);
    if let Some(bytes) = feature.source.bytes() {
        source = source.with_bytes(bytes.start, bytes.end);
    }
    if let Some(record) = feature.source.record() {
        source = source.with_record(record);
    }
    if let Some(key) = feature.source.key() {
        source = source.with_key(key);
    }
    Ok(source)
}

pub(crate) fn form_name(form: &Form) -> &'static str {
    match form {
        Form::Point(_) => "point",
        Form::Line(_) => "line",
        Form::Area { .. } => "area",
        Form::Solid(shape) => shape.kind(),
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Projection {
    source: Crs,
    pub zone: Zone,
}

impl Projection {
    pub fn new(model: &GeoModel, zone: Option<Zone>, cx: &mut Cx) -> Result<Self, Failure> {
        cx.check(Kind::Work, 1)?;
        let source = model.crs.ok_or(Failure::Invalid("crs_missing"))?;
        if !matches!(source, Crs::Geographic | Crs::Plane(_)) {
            return Err(Failure::Unsupported("datum_transform"));
        }
        let zone = if let Some(zone) = zone.or(source.zone()) {
            zone
        } else {
            let mut bounds: Option<[[f64; 2]; 2]> = None;
            for feature in &model.features {
                for geometry in &feature.geometries {
                    let mut part: Option<[[f64; 2]; 2]> = None;
                    let result = visit(&geometry.form, cx, &mut |p, cx| {
                        cx.check(Kind::Work, 1)?;
                        if !p[0].is_finite()
                            || !p[1].is_finite()
                            || p[0].abs() >= 90.0
                            || p[1].abs() > 180.0
                        {
                            return Err(Failure::Invalid("coordinate"));
                        }
                        if let Some([lo, hi]) = &mut part {
                            for ((lo, hi), value) in lo.iter_mut().zip(hi).zip(p) {
                                *lo = lo.min(value);
                                *hi = hi.max(value);
                            }
                        } else {
                            part = Some([[p[0], p[1]]; 2]);
                        }
                        Ok(())
                    });
                    match result {
                        Err(Failure::Stopped(stop)) => return Err(stop.into()),
                        Err(_) => continue,
                        Ok(()) => {}
                    }
                    if let Some([a, b]) = part {
                        if let Some([lo, hi]) = &mut bounds {
                            for ((lo, hi), (a, b)) in
                                lo.iter_mut().zip(hi).zip(a.into_iter().zip(b))
                            {
                                *lo = lo.min(a);
                                *hi = hi.max(b);
                            }
                        } else {
                            bounds = Some([a, b]);
                        }
                    }
                }
            }
            let [lo, hi] = bounds.ok_or(Failure::Invalid("zone_missing"))?;
            let center = GeoPoint {
                latitude: lo[0] / 2.0 + hi[0] / 2.0,
                longitude: lo[1] / 2.0 + hi[1] / 2.0,
            };
            // 適用地域の行政界ではなく、中心を投影した原点からの距離が最小の系を計算用に仮定する。
            let mut best: Option<(f64, Zone)> = None;
            for number in 1..=19 {
                cx.check(Kind::Work, 256)?;
                let Some(zone) = Zone::new(number) else {
                    continue;
                };
                if let Some(p) = to_plane(zone, center) {
                    let distance =
                        p.point.easting * p.point.easting + p.point.northing * p.point.northing;
                    if distance.is_finite() && best.is_none_or(|old| distance < old.0) {
                        best = Some((distance, zone));
                    }
                }
            }
            let zone = best.ok_or(Failure::Invalid("projection"))?.1;
            cx.report(
                Diagnostic::new(codes::GEO_ZONE_ASSUMED).with("zone", u64::from(zone.number())),
            );
            cx.check(Kind::Work, 0)?;
            zone
        };
        Ok(Self { source, zone })
    }

    pub fn report(&self, feature: &Feature, form: &Form, cx: &mut Cx) -> Result<(), Failure> {
        if self.source != Crs::Plane(self.zone) && !matches!(form, Form::Point(_)) {
            let place = source(feature, cx)?;
            cx.report(
                Diagnostic::new(codes::GEO_PROJECTION_APPROXIMATED)
                    .with("source", u64::from(self.source.epsg()))
                    .with("target", u64::from(self.zone.epsg()))
                    .at(place),
            );
            cx.check(Kind::Work, 0)?;
        }
        Ok(())
    }

    pub fn xy(&self, position: Position, cx: &mut Cx) -> Result<[f64; 2], Failure> {
        cx.check(Kind::Work, 512)?;
        let point = match (self.source, position) {
            (Crs::Plane(zone), Position::Map(p)) if zone == self.zone => p,
            (Crs::Plane(zone), Position::Map(p)) => {
                let geo = to_geographic(zone, p)
                    .ok_or(Failure::Invalid("projection"))?
                    .point;
                to_plane(self.zone, geo)
                    .ok_or(Failure::Invalid("projection"))?
                    .point
            }
            (Crs::Geographic, Position::Geo(p))
                if p.latitude.abs() < 90.0 && p.longitude.abs() <= 180.0 =>
            {
                to_plane(self.zone, p)
                    .ok_or(Failure::Invalid("projection"))?
                    .point
            }
            _ => return Err(Failure::Invalid("coordinate_space")),
        };
        if !point.easting.is_finite() || !point.northing.is_finite() {
            return Err(Failure::Invalid("coordinate"));
        }
        Ok([point.easting, point.northing])
    }

    #[cfg(feature = "scene3d")]
    pub fn xyz(&self, p: [f64; 3], cx: &mut Cx) -> Result<[f64; 3], Failure> {
        let position = if self.source == Crs::Geographic {
            Position::Geo(GeoPoint {
                latitude: p[0],
                longitude: p[1],
            })
        } else {
            Position::Map(MapPoint {
                easting: p[0],
                northing: p[1],
            })
        };
        let [x, y] = self.xy(position, cx)?;
        if !p[2].is_finite() {
            return Err(Failure::Invalid("height"));
        }
        Ok([x, y, p[2]])
    }
}

fn raw(location: &Location) -> [f64; 3] {
    let [x, y] = match location.position {
        Position::Geo(p) => [p.latitude, p.longitude],
        Position::Map(p) => [p.easting, p.northing],
    };
    [x, y, location.height.unwrap_or(0.0)]
}

pub(crate) fn visit(
    form: &Form,
    cx: &mut Cx,
    f: &mut impl FnMut([f64; 3], &mut Cx) -> Result<(), Failure>,
) -> Result<(), Failure> {
    cx.check(Kind::Work, 1)?;
    match form {
        Form::Point(p) => f(raw(p), cx),
        Form::Line(points) => {
            for p in points {
                f(raw(p), cx)?;
            }
            Ok(())
        }
        Form::Area { outer, holes } => {
            for ring in std::iter::once(outer).chain(holes) {
                cx.check(Kind::Work, 1)?;
                for p in ring {
                    f(raw(p), cx)?;
                }
            }
            Ok(())
        }
        Form::Solid(shape) => visit_shape(shape, Transform::IDENTITY, 0, cx, f),
    }
}

fn visit_shape(
    shape: &Shape,
    transform: Transform,
    depth: u32,
    cx: &mut Cx,
    f: &mut impl FnMut([f64; 3], &mut Cx) -> Result<(), Failure>,
) -> Result<(), Failure> {
    cx.check(Kind::Work, 1)?;
    if depth >= 64 {
        return Err(Failure::Invalid("depth"));
    }
    cx.enter()?;
    let result = (|| match shape {
        Shape::FaceSet { faces, .. } => {
            for face in faces {
                cx.check(Kind::Work, 1)?;
                for ring in std::iter::once(&face.outer).chain(&face.holes) {
                    cx.check(Kind::Work, 1)?;
                    for &p in ring {
                        f(transform.point(p), cx)?;
                    }
                }
            }
            Ok(())
        }
        Shape::Triangles { positions, .. } => {
            for &p in positions {
                f(transform.point(p), cx)?;
            }
            Ok(())
        }
        Shape::Group(shapes) => {
            for shape in shapes {
                visit_shape(shape, transform, depth + 1, cx, f)?;
            }
            Ok(())
        }
        Shape::Mapped {
            transform: local,
            shape,
        } if local.is_finite() => visit_shape(shape, local.then(&transform), depth + 1, cx, f),
        _ => Err(Failure::Unsupported(shape.kind())),
    })();
    cx.leave();
    result
}
