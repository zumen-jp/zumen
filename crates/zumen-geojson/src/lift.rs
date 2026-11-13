//! GeoJSON（RFC 7946）から地理の領域モデル（[`GeoModel`]）への変換。

use std::collections::BTreeSet;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_crs::{Crs, GeoPoint};
use zumen_geo::{Attribute, Feature, Form, GeoModel, Geometry, Location, Position, Value};
use zumen_sdk::{Cx, Error};

use crate::json::{JsonKind, SpannedValue};
use crate::stopped;

const GEOMETRY_TYPES: &[&str] = &[
    "Point",
    "MultiPoint",
    "LineString",
    "MultiLineString",
    "Polygon",
    "MultiPolygon",
    "GeometryCollection",
];

pub(crate) fn is_geojson_root_type(kind: &str) -> bool {
    kind == "FeatureCollection" || kind == "Feature" || GEOMETRY_TYPES.contains(&kind)
}

pub(crate) fn lift(root: &SpannedValue, cx: &mut Cx<'_>) -> Result<GeoModel, Error> {
    let Some(root_type) = root.get("type").and_then(SpannedValue::as_str) else {
        return Err(invalid_file(root, cx));
    };
    if !is_geojson_root_type(root_type) {
        return Err(invalid_file(root, cx));
    }

    let mut reader = Reader {
        cx,
        ids: BTreeSet::new(),
        next_auto_id: 1,
        features: Vec::new(),
    };

    match root_type {
        "FeatureCollection" => {
            reader.check_crs_member(root);
            let Some(items) = root.get("features").and_then(SpannedValue::as_array) else {
                return Err(invalid_file(root, reader.cx));
            };
            for item in items {
                reader.check(Kind::Work, 1, item.at())?;
                reader.feature_item(item)?;
            }
        }
        "Feature" => {
            reader.feature_object(root)?;
        }
        geom_type => {
            reader.check(Kind::Elements, 1, root.at())?;
            let mut geometries = Vec::new();
            reader.geometry(root, &mut geometries)?;
            let id = reader.unique_id(None, geom_type);
            reader.features.push(Feature {
                id,
                kind: geom_type.to_string(),
                code: None,
                name: None,
                attributes: Vec::new(),
                geometries,
                parent: None,
                source: root.at(),
            });
        }
    }

    Ok(GeoModel {
        features: reader.features,
        crs: Some(Crs::Wgs84),
        height: None,
    })
}

fn invalid_file(val: &SpannedValue, cx: &mut Cx<'_>) -> Error {
    cx.report(
        Diagnostic::new(codes::GEOJSON_FILE_INVALID)
            .with("offset", val.span.start)
            .at(val.at()),
    );
    Error::Invalid
}

struct Reader<'c, 'b> {
    cx: &'c mut Cx<'b>,
    ids: BTreeSet<String>,
    next_auto_id: usize,
    features: Vec<Feature>,
}

impl Reader<'_, '_> {
    fn check(&mut self, kind: Kind, amount: u64, at: Ref) -> Result<(), Error> {
        self.cx
            .check(kind, amount)
            .map_err(|s| stopped(self.cx, s, Some(at)))
    }

    fn enter(&mut self, at: Ref) -> Result<(), Error> {
        self.cx.enter().map_err(|s| stopped(self.cx, s, Some(at)))
    }

    fn check_crs_member(&mut self, obj: &SpannedValue) {
        if let Some(crs) = obj.get("crs") {
            self.cx
                .report(Diagnostic::new(codes::GEOJSON_CRS_DROPPED).at(crs.at()));
        }
    }

    fn unique_id(&mut self, candidate: Option<String>, prefix: &str) -> String {
        if let Some(id) = candidate
            && !id.is_empty()
            && self.ids.insert(id.clone())
        {
            return id;
        }
        loop {
            let generated = format!("{prefix}/{}", self.next_auto_id);
            self.next_auto_id = self.next_auto_id.saturating_add(1);
            if self.ids.insert(generated.clone()) {
                return generated;
            }
        }
    }

    fn feature_item(&mut self, item: &SpannedValue) -> Result<(), Error> {
        let Some(kind) = item.get("type").and_then(SpannedValue::as_str) else {
            self.cx.report(
                Diagnostic::new(codes::GEOJSON_TYPE_UNSUPPORTED)
                    .with("kind", "")
                    .at(item.at()),
            );
            return Ok(());
        };
        if kind != "Feature" {
            self.cx.report(
                Diagnostic::new(codes::GEOJSON_TYPE_UNSUPPORTED)
                    .with("kind", kind)
                    .at(item.at()),
            );
            return Ok(());
        }
        self.feature_object(item)
    }

    fn feature_object(&mut self, obj: &SpannedValue) -> Result<(), Error> {
        self.check(Kind::Work, 1, obj.at())?;
        self.check(Kind::Elements, 1, obj.at())?;
        self.check_crs_member(obj);

        let raw_id = match obj.get("id").map(|v| &v.kind) {
            Some(JsonKind::String(s)) => Some(s.clone()),
            Some(JsonKind::Number {
                integer: Some(n), ..
            }) => Some(n.to_string()),
            Some(JsonKind::Number { raw, .. }) => Some(raw.clone()),
            _ => None,
        };
        let id = self.unique_id(raw_id, "Feature");

        let mut attributes = Vec::new();
        let mut name = None;
        match obj.get("properties") {
            None => {}
            Some(props) => match &props.kind {
                JsonKind::Null => {}
                JsonKind::Object(entries) => {
                    self.properties_object(entries, "", props.at(), &mut name, &mut attributes)?;
                }
                _ => {
                    self.cx.report(
                        Diagnostic::new(codes::GEOJSON_PROPERTY_DROPPED)
                            .with("name", "properties")
                            .at(props.at()),
                    );
                }
            },
        }

        let mut geometries = Vec::new();
        match obj.get("geometry") {
            None => {}
            Some(geom) => match &geom.kind {
                JsonKind::Null => {}
                JsonKind::Object(_) => {
                    self.geometry(geom, &mut geometries)?;
                }
                _ => {
                    self.invalid_geometry("Feature", "geometry", geom.at());
                }
            },
        }

        self.features.push(Feature {
            id,
            kind: "Feature".to_string(),
            code: None,
            name,
            attributes,
            geometries,
            parent: None,
            source: obj.at(),
        });
        Ok(())
    }

    fn properties_object(
        &mut self,
        entries: &[(String, SpannedValue)],
        prefix: &str,
        at: Ref,
        feature_name: &mut Option<String>,
        out: &mut Vec<Attribute>,
    ) -> Result<(), Error> {
        self.enter(at)?;
        let res = self.properties_object_inner(entries, prefix, feature_name, out);
        self.cx.leave();
        res
    }

    fn properties_object_inner(
        &mut self,
        entries: &[(String, SpannedValue)],
        prefix: &str,
        feature_name: &mut Option<String>,
        out: &mut Vec<Attribute>,
    ) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        let mut selected = Vec::new();
        for (key, val) in entries.iter().rev() {
            self.check(Kind::Work, 1, val.at())?;
            if seen.insert(key.as_str()) {
                selected.push((key, val));
            }
        }
        selected.reverse();

        for (key, val) in selected {
            let full_name = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}/{key}")
            };
            self.check(Kind::DecodedBytes, full_name.len() as u64, val.at())?;
            match &val.kind {
                JsonKind::String(s) => {
                    if prefix.is_empty() && key == "name" && feature_name.is_none() {
                        *feature_name = Some(s.clone());
                    }
                    out.push(Attribute {
                        name: full_name,
                        value: Value::Text(s.clone()),
                    });
                }
                JsonKind::Bool(b) => {
                    out.push(Attribute {
                        name: full_name,
                        value: Value::Bool(*b),
                    });
                }
                JsonKind::Number { value, integer, .. } => {
                    let attr_val = match integer {
                        Some(n) => Value::Integer(*n),
                        None => Value::Real(*value),
                    };
                    out.push(Attribute {
                        name: full_name,
                        value: attr_val,
                    });
                }
                JsonKind::Object(nested) => {
                    self.properties_object(nested, &full_name, val.at(), feature_name, out)?;
                }
                JsonKind::Null | JsonKind::Array(_) => {
                    self.cx.report(
                        Diagnostic::new(codes::GEOJSON_PROPERTY_DROPPED)
                            .with("name", full_name)
                            .at(val.at()),
                    );
                }
            }
        }
        Ok(())
    }

    fn geometry(&mut self, obj: &SpannedValue, out: &mut Vec<Geometry>) -> Result<(), Error> {
        self.enter(obj.at())?;
        let res = self.geometry_inner(obj, out);
        self.cx.leave();
        res
    }

    fn geometry_inner(&mut self, obj: &SpannedValue, out: &mut Vec<Geometry>) -> Result<(), Error> {
        self.check(Kind::Work, 1, obj.at())?;
        self.check(Kind::Elements, 1, obj.at())?;
        self.check_crs_member(obj);

        let Some(kind) = obj.get("type").and_then(SpannedValue::as_str) else {
            self.cx.report(
                Diagnostic::new(codes::GEOJSON_TYPE_UNSUPPORTED)
                    .with("kind", "")
                    .at(obj.at()),
            );
            return Ok(());
        };

        match kind {
            "Point" => {
                let Some(coords) = obj.get("coordinates") else {
                    self.invalid_geometry(kind, "coordinates", obj.at());
                    return Ok(());
                };
                if let Some(loc) = self.position(kind, coords)? {
                    out.push(Geometry {
                        lod: None,
                        form: Form::Point(loc),
                    });
                }
            }
            "MultiPoint" => {
                let Some(items) = obj.get("coordinates").and_then(SpannedValue::as_array) else {
                    self.invalid_geometry(kind, "coordinates", obj.at());
                    return Ok(());
                };
                for item in items {
                    if let Some(loc) = self.position(kind, item)? {
                        self.check(Kind::Elements, 1, item.at())?;
                        out.push(Geometry {
                            lod: None,
                            form: Form::Point(loc),
                        });
                    }
                }
            }
            "LineString" => {
                let Some(coords) = obj.get("coordinates") else {
                    self.invalid_geometry(kind, "coordinates", obj.at());
                    return Ok(());
                };
                if let Some(line) = self.line_positions(kind, coords)? {
                    out.push(Geometry {
                        lod: None,
                        form: Form::Line(line),
                    });
                }
            }
            "MultiLineString" => {
                let Some(items) = obj.get("coordinates").and_then(SpannedValue::as_array) else {
                    self.invalid_geometry(kind, "coordinates", obj.at());
                    return Ok(());
                };
                for item in items {
                    if let Some(line) = self.line_positions(kind, item)? {
                        self.check(Kind::Elements, 1, item.at())?;
                        out.push(Geometry {
                            lod: None,
                            form: Form::Line(line),
                        });
                    }
                }
            }
            "Polygon" => {
                let Some(coords) = obj.get("coordinates") else {
                    self.invalid_geometry(kind, "coordinates", obj.at());
                    return Ok(());
                };
                if let Some(form) = self.polygon_rings(kind, coords)? {
                    out.push(Geometry { lod: None, form });
                }
            }
            "MultiPolygon" => {
                let Some(items) = obj.get("coordinates").and_then(SpannedValue::as_array) else {
                    self.invalid_geometry(kind, "coordinates", obj.at());
                    return Ok(());
                };
                for item in items {
                    if let Some(form) = self.polygon_rings(kind, item)? {
                        self.check(Kind::Elements, 1, item.at())?;
                        out.push(Geometry { lod: None, form });
                    }
                }
            }
            "GeometryCollection" => {
                let Some(items) = obj.get("geometries").and_then(SpannedValue::as_array) else {
                    self.invalid_geometry(kind, "geometries", obj.at());
                    return Ok(());
                };
                for item in items {
                    self.geometry(item, out)?;
                }
            }
            other => {
                self.cx.report(
                    Diagnostic::new(codes::GEOJSON_TYPE_UNSUPPORTED)
                        .with("kind", other)
                        .at(obj.at()),
                );
            }
        }
        Ok(())
    }

    fn position(&mut self, kind: &str, val: &SpannedValue) -> Result<Option<Location>, Error> {
        self.check(Kind::Work, 1, val.at())?;
        let Some(nums) = val.as_array() else {
            self.invalid_geometry(kind, "position", val.at());
            return Ok(None);
        };
        let (Some(lon_val), Some(lat_val)) = (nums.first(), nums.get(1)) else {
            self.invalid_geometry(kind, "position", val.at());
            return Ok(None);
        };
        let (Some(lon), Some(lat)) = (lon_val.as_f64(), lat_val.as_f64()) else {
            self.invalid_geometry(kind, "position", val.at());
            return Ok(None);
        };
        if !lon.is_finite() || !lat.is_finite() || !(-90.0..=90.0).contains(&lat) {
            self.invalid_geometry(kind, "position", val.at());
            return Ok(None);
        }
        let height = match nums.get(2) {
            None => None,
            Some(z_val) => {
                let Some(z) = z_val.as_f64() else {
                    self.invalid_geometry(kind, "position", val.at());
                    return Ok(None);
                };
                if !z.is_finite() {
                    self.invalid_geometry(kind, "position", val.at());
                    return Ok(None);
                }
                Some(z)
            }
        };
        for extra in nums.iter().skip(3) {
            if extra.as_f64().is_none_or(|v| !v.is_finite()) {
                self.invalid_geometry(kind, "position", val.at());
                return Ok(None);
            }
        }
        Ok(Some(Location {
            position: Position::Geo(GeoPoint {
                latitude: lat,
                longitude: lon,
            }),
            height,
        }))
    }

    fn line_positions(
        &mut self,
        kind: &str,
        val: &SpannedValue,
    ) -> Result<Option<Vec<Location>>, Error> {
        let Some(items) = val.as_array() else {
            self.invalid_geometry(kind, "line_positions", val.at());
            return Ok(None);
        };
        if items.len() < 2 {
            self.invalid_geometry(kind, "line_positions", val.at());
            return Ok(None);
        }
        self.check(
            Kind::DecodedBytes,
            (items.len() as u64).saturating_mul(24),
            val.at(),
        )?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let Some(loc) = self.position(kind, item)? else {
                return Ok(None);
            };
            out.push(loc);
        }
        Ok(Some(out))
    }

    fn linear_ring(
        &mut self,
        kind: &str,
        val: &SpannedValue,
    ) -> Result<Option<Vec<Location>>, Error> {
        let Some(items) = val.as_array() else {
            self.invalid_geometry(kind, "ring_positions", val.at());
            return Ok(None);
        };
        if items.len() < 4 {
            self.invalid_geometry(kind, "ring_positions", val.at());
            return Ok(None);
        }
        self.check(
            Kind::DecodedBytes,
            (items.len() as u64).saturating_mul(24),
            val.at(),
        )?;
        let mut pts = Vec::with_capacity(items.len());
        for item in items {
            let Some(loc) = self.position(kind, item)? else {
                return Ok(None);
            };
            pts.push(loc);
        }
        if pts.first() != pts.last() {
            self.invalid_geometry(kind, "ring_unclosed", val.at());
            return Ok(None);
        }
        pts.pop();
        Ok(Some(pts))
    }

    fn polygon_rings(&mut self, kind: &str, val: &SpannedValue) -> Result<Option<Form>, Error> {
        let Some(rings) = val.as_array() else {
            self.invalid_geometry(kind, "ring_positions", val.at());
            return Ok(None);
        };
        let Some(outer_val) = rings.first() else {
            self.invalid_geometry(kind, "ring_positions", val.at());
            return Ok(None);
        };
        let Some(outer) = self.linear_ring(kind, outer_val)? else {
            return Ok(None);
        };
        let mut holes = Vec::with_capacity(rings.len().saturating_sub(1));
        for hole_val in rings.iter().skip(1) {
            let Some(hole) = self.linear_ring(kind, hole_val)? else {
                return Ok(None);
            };
            holes.push(hole);
        }
        Ok(Some(Form::Area { outer, holes }))
    }

    fn invalid_geometry(&mut self, kind: &str, reason: &'static str, at: Ref) {
        self.cx.report(
            Diagnostic::new(codes::GEOJSON_GEOMETRY_INVALID)
                .with("kind", kind)
                .with("reason", zumen_base::Value::Key(reason))
                .at(at),
        );
    }
}
