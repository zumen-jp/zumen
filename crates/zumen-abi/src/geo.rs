//! 地理の領域モデル（決定 0233）の境界の処理。CityGML を読み、`geo` の op で要約と地物の中身を返す。

use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, codes};
use zumen_citygml::CityGml;
use zumen_geo::{Feature, Form, GeoModel, Value};
use zumen_geom3d::Shape;
use zumen_sdk::{Codec, Lift};

use crate::messages::{
    GeoAttribute, GeoFeature, GeoForm, GeoGeometry, GeoKind, GeoRequest, GeoResult, GeoValueKind,
};
use crate::session::{Failed, fail};

/// CityGML なら読み、地理の領域モデルに移す。ほかの形式なら `None`
pub(crate) fn read(bytes: &[u8], cx: &mut Cx<'_>) -> Option<Result<GeoModel, Failed>> {
    if !CityGml::SIGNATURES.iter().any(|rule| rule.matches(bytes)) {
        return None;
    }
    // 失敗の理由は、コーデックと移す処理が診断として足してある
    Some(
        CityGml::read(bytes, cx)
            .and_then(|file| file.lift(cx))
            .map_err(|_| Failed),
    )
}

/// 地理の領域モデルの要約と、`id` で指した地物の中身
pub(crate) fn geo(
    request: &GeoRequest,
    model: &GeoModel,
    cx: &mut Cx<'_>,
) -> Result<GeoResult, Failed> {
    let mut kinds = BTreeMap::<&str, u64>::new();
    for feature in &model.features {
        *kinds.entry(feature.kind.as_str()).or_default() += 1;
    }
    let feature = match &request.id {
        None => None,
        Some(id) => {
            let Some(found) = model.features.iter().position(|f| &f.id == id) else {
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CALL_VALUE_INVALID).with("pointer", "/id"),
                ));
            };
            feature(model, found)
        }
    };
    Ok(GeoResult {
        crs: model.crs.map(|crs| u64::from(crs.epsg())),
        height: model.height.map(|height| u64::from(height.epsg())),
        features: model.features.len() as u64,
        kinds: kinds
            .into_iter()
            .map(|(kind, count)| GeoKind {
                kind: kind.to_string(),
                count,
            })
            .collect(),
        feature,
    })
}

fn feature(model: &GeoModel, index: usize) -> Option<GeoFeature> {
    let found: &Feature = model.features.get(index)?;
    Some(GeoFeature {
        id: found.id.clone(),
        kind: found.kind.clone(),
        code: found.code.clone(),
        name: found.name.clone(),
        parent: found
            .parent
            .and_then(|parent| model.features.get(parent))
            .map(|parent| parent.id.clone()),
        children: model
            .features
            .iter()
            .filter(|child| child.parent == Some(index))
            .count() as u64,
        attributes: found
            .attributes
            .iter()
            .map(|attribute| {
                let (kind, value, unit, space) = match &attribute.value {
                    Value::Text(text) => (GeoValueKind::Text, text.clone(), None, None),
                    Value::Integer(value) => (GeoValueKind::Integer, value.to_string(), None, None),
                    Value::Real(value) => (GeoValueKind::Real, format!("{value:?}"), None, None),
                    Value::Bool(value) => (GeoValueKind::Bool, value.to_string(), None, None),
                    Value::Date(text) => (GeoValueKind::Date, text.clone(), None, None),
                    Value::Code { space, value } => {
                        (GeoValueKind::Code, value.clone(), None, space.clone())
                    }
                    Value::Measure { value, unit } => (
                        GeoValueKind::Measure,
                        format!("{value:?}"),
                        unit.clone(),
                        None,
                    ),
                };
                GeoAttribute {
                    name: attribute.name.clone(),
                    kind,
                    value,
                    unit,
                    space,
                }
            })
            .collect(),
        geometries: found
            .geometries
            .iter()
            .map(|geometry| {
                let (form, points, faces) = match &geometry.form {
                    Form::Point(_) => (GeoForm::Point, 1, None),
                    Form::Line(points) => (GeoForm::Line, points.len(), None),
                    Form::Area { outer, holes } => (
                        GeoForm::Area,
                        outer.len() + holes.iter().map(Vec::len).sum::<usize>(),
                        None,
                    ),
                    Form::Solid(shape) => {
                        let (points, faces) = count(shape);
                        (GeoForm::Solid, points, Some(faces as u64))
                    }
                };
                GeoGeometry {
                    lod: geometry.lod.map(u64::from),
                    form,
                    points: points as u64,
                    faces,
                }
            })
            .collect(),
    })
}

/// 面の集まりの点と面の数
fn count(shape: &Shape) -> (usize, usize) {
    match shape {
        Shape::FaceSet { faces, .. } => (
            faces
                .iter()
                .map(|face| face.outer.len() + face.holes.iter().map(Vec::len).sum::<usize>())
                .sum(),
            faces.len(),
        ),
        Shape::Mapped { shape, .. } => count(shape),
        _ => (0, 0),
    }
}
