//! 地物の投影と ZMS3 を、建物と同じ scene3d の入口から返す。
use super::*;

impl Session {
    pub(super) fn geo_scene3d(
        &mut self,
        request: &Scene3dRequest,
        cx: &mut Cx<'_>,
    ) -> Result<Scene3dResult, Failed> {
        if request.storeys.is_some() {
            return Err(fail(cx, invalid_value("/storeys")));
        }
        let zone = request
            .zone
            .map(|number| {
                u8::try_from(number)
                    .ok()
                    .and_then(zumen_crs::Zone::new)
                    .ok_or_else(|| fail(cx, invalid_value("/zone")))
            })
            .transpose()?;
        let lod = request
            .lod
            .map(|number| {
                u8::try_from(number)
                    .ok()
                    .filter(|n| *n <= 4)
                    .ok_or_else(|| fail(cx, invalid_value("/lod")))
            })
            .transpose()?;
        let region = request
            .r#box
            .as_ref()
            .map(|b| box_value(b, cx))
            .transpose()?;
        cx.check(Kind::DecodedBytes, 1024 + request.doc.len() as u64)
            .map_err(|s| stopped(cx, s))?;
        let model = self.geo_model(&request.doc, "scene3d", cx)?;
        let output = zumen_geo::scene3d::build(
            &model,
            zumen_geo::scene3d::Options {
                zone,
                selection: zumen_geo::Selection {
                    ids: request.ids.as_deref(),
                    kinds: request.classes.as_deref(),
                    lod,
                },
                tolerance: request.tolerance,
                region,
            },
            cx,
        )
        .map_err(|e| scene_error(cx, e.into()))?;
        let scene = &output.scene;
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
                kind: "LENGTHUNIT".into(),
                name: "METRE".into(),
                prefix: None,
                scale: None,
            },
            map: Some(Scene3dMap {
                crs_name: Some(format!("EPSG:{}", output.zone.epsg())),
                eastings: 0.0,
                northings: 0.0,
                height: 0.0,
                x_axis_abscissa: Some(1.0),
                x_axis_ordinate: Some(0.0),
                scale: Some(1.0),
            }),
            height_crs: output.height.map(|h| u64::from(h.epsg())),
            lod: output.lod.map(u64::from),
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
