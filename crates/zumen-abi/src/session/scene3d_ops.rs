//! 建物と地物の 3D シーン。列をすべて作れた場合だけ blob を登録する。

use super::*;
use messages::{
    BuildingUnit, Scene3dBox, Scene3dCounts, Scene3dMap, Scene3dRequest, Scene3dResult,
};
#[cfg(feature = "bim")]
use zumen_scene3d::Options;
use zumen_scene3d::{Bounds, Error};

#[cfg(feature = "civil")]
mod geo;

fn scene_error(cx: &mut Cx, error: Error) -> Failed {
    match error {
        Error::Stopped(stop) => stopped(cx, stop),
        Error::Invalid(reason) => fail(
            cx,
            Diagnostic::new(codes::SCENE3D_VALUE_INVALID).with("reason", reason),
        ),
    }
}

#[cfg(feature = "bim")]
fn ids(
    values: Option<&[String]>,
    path: &str,
    cx: &mut Cx,
) -> Result<Option<Vec<zumen_building::Id>>, Failed> {
    values
        .map(|values| {
            cx.check(Kind::DecodedBytes, values.len() as u64 * 32)
                .map_err(|s| stopped(cx, s))?;
            let mut ids = Vec::new();
            ids.try_reserve_exact(values.len()).map_err(|_| {
                fail(
                    cx,
                    Diagnostic::new(codes::SCENE3D_VALUE_INVALID).with("reason", "allocation"),
                )
            })?;
            for (index, value) in values.iter().enumerate() {
                cx.check(Kind::Work, value.len() as u64 + 1)
                    .map_err(|s| stopped(cx, s))?;
                ids.push(
                    value
                        .parse()
                        .map_err(|_| fail(cx, invalid_value(format!("{path}/{index}"))))?,
                );
            }
            Ok(ids)
        })
        .transpose()
}

fn box_value(value: &Scene3dBox, cx: &mut Cx) -> Result<Bounds, Failed> {
    let ([x, y, z], [u, v, w]) = (value.min.as_slice(), value.max.as_slice()) else {
        return Err(fail(cx, invalid_value("/box")));
    };
    let (min, max) = ([*x, *y, *z], [*u, *v, *w]);
    if min
        .into_iter()
        .zip(max)
        .any(|(a, b)| !a.is_finite() || !b.is_finite() || a > b)
    {
        return Err(fail(cx, invalid_value("/box")));
    }
    Ok([min, max])
}

fn output_box([min, max]: Bounds) -> Scene3dBox {
    Scene3dBox {
        min: min.to_vec(),
        max: max.to_vec(),
    }
}

impl Session {
    pub(super) fn scene3d(
        &mut self,
        request: &Scene3dRequest,
        cx: &mut Cx<'_>,
    ) -> Result<Scene3dResult, Failed> {
        if !name::is_safe(&request.doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        #[cfg(feature = "civil")]
        if !matches!(self.document(&request.doc, cx)?, Document::Building { .. }) {
            return self.geo_scene3d(request, cx);
        }
        if request.zone.is_some() {
            return Err(fail(cx, invalid_value("/zone")));
        }
        if request.lod.is_some() {
            return Err(fail(cx, invalid_value("/lod")));
        }
        #[cfg(feature = "bim")]
        {
            self.building_scene3d(request, cx)
        }
        #[cfg(not(feature = "bim"))]
        {
            Err(fail(cx, unsupported(&request.doc, "scene3d")))
        }
    }

    #[cfg(feature = "bim")]
    fn building_scene3d(
        &mut self,
        request: &Scene3dRequest,
        cx: &mut Cx<'_>,
    ) -> Result<Scene3dResult, Failed> {
        if request
            .tolerance
            .is_some_and(|v| !v.is_finite() || v <= 0.0)
        {
            return Err(fail(cx, invalid_value("/tolerance")));
        }
        let region = request
            .r#box
            .as_ref()
            .map(|b| box_value(b, cx))
            .transpose()?;
        let element_ids = ids(request.ids.as_deref(), "/ids", cx)?;
        let storeys = ids(request.storeys.as_deref(), "/storeys", cx)?;
        let model = self.model(&request.doc, "scene3d", cx)?;
        // 通常の open は属性だけを lift する。形はこの呼び出しの予算で作り、
        // セッションへ残さないため、再呼び出しの予算停止位置も同じにする。
        let geometry;
        let building = if let Some(file) = &model.file {
            geometry = file.lift_geometry(cx).map_err(|_| Failed)?;
            &geometry
        } else {
            &model.building
        };
        let unit = building
            .document
            .units
            .iter()
            .find(|u| u.kind == "LENGTHUNIT")
            .ok_or_else(|| scene_error(cx, Error::Invalid("unit_missing")))?;
        cx.check(
            Kind::DecodedBytes,
            512 + request.doc.len() as u64
                + unit.kind.len() as u64
                + unit.name.len() as u64
                + unit.prefix.as_ref().map_or(0, |s| s.len()) as u64
                + building
                    .document
                    .map
                    .as_ref()
                    .and_then(|m| m.crs_name.as_ref())
                    .map_or(0, |s| s.len()) as u64,
        )
        .map_err(|s| stopped(cx, s))?;
        let map = building.document.map.as_ref().map(|map| Scene3dMap {
            crs_name: map.crs_name.clone(),
            eastings: map.eastings,
            northings: map.northings,
            height: map.height,
            x_axis_abscissa: map.x_axis_abscissa,
            x_axis_ordinate: map.x_axis_ordinate,
            scale: map.scale,
        });
        if map.as_ref().is_some_and(|m| {
            [
                Some(m.eastings),
                Some(m.northings),
                Some(m.height),
                m.x_axis_abscissa,
                m.x_axis_ordinate,
                m.scale,
            ]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite())
        }) {
            return Err(scene_error(cx, Error::Invalid("map")));
        }
        let options = Options {
            storeys: storeys.as_deref(),
            classes: request.classes.as_deref(),
            ids: element_ids.as_deref(),
            region,
            tolerance: request.tolerance,
            ..Options::default()
        };
        let scene = zumen_scene3d::build(building, options, cx).map_err(|e| scene_error(cx, e))?;
        let data = &scene.data;
        let counts = Scene3dCounts {
            chunks: data.chunks.len() as u64,
            meshes: data.meshes.len() as u64,
            instances: data.instances.len() as u64,
            elements: data.elements.len() as u64,
            colors: data.colors.len() as u64,
            vertices: data.meshes.iter().map(|m| m.vertices.len() as u64).sum(),
            triangles: data.meshes.iter().map(|m| m.triangles.len() as u64).sum(),
            edges: data.meshes.iter().map(|m| m.edges.len() as u64).sum(),
        };
        let bytes = zumen_scene3d::encode(data, cx).map_err(|e| scene_error(cx, e))?;
        let result = Scene3dResult {
            doc: request.doc.clone(),
            unit: BuildingUnit {
                kind: unit.kind.clone(),
                prefix: unit.prefix.clone(),
                name: unit.name.clone(),
                scale: unit.scale,
            },
            map,
            height_crs: None,
            lod: None,
            tolerance: scene.tolerance,
            r#box: region.map(output_box),
            extent: scene.extent.map(output_box),
            counts,
            blob: self.blobs.len() as u64,
        };
        self.blobs.push(Some(Blob {
            kind: BlobKind::Scene3d,
            bytes,
        }));
        Ok(result)
    }
}
