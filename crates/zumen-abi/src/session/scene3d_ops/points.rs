//! 点群を、八分木の節ごとの点の区画を持つ ZMS3 1.1 で返す（#475、決定 0232）。
use super::*;

impl Session {
    /// 開いた点群の文書か。GeoTIFF の DEM は地物の入口に任せる
    pub(super) fn is_cloud(&self, doc: &str) -> bool {
        matches!(self.docs.get(doc), Some(Document::Cloud(_)))
    }

    pub(super) fn cloud_scene3d(
        &mut self,
        request: &Scene3dRequest,
        cx: &mut Cx<'_>,
    ) -> Result<Scene3dResult, Failed> {
        for (present, pointer) in [
            (request.storeys.is_some(), "/storeys"),
            (request.classes.is_some(), "/classes"),
            (request.ids.is_some(), "/ids"),
            (request.zone.is_some(), "/zone"),
            (request.lod.is_some(), "/lod"),
        ] {
            if present {
                return Err(fail(cx, invalid_value(pointer)));
            }
        }
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
        cx.check(Kind::DecodedBytes, 1024 + request.doc.len() as u64)
            .map_err(|s| stopped(cx, s))?;
        let cloud = self.cloud(&request.doc, "/doc", cx)?;
        // 要求の箱は東・北・高さ、点群の Query は北・東・高さ
        let query = zumen_pointcloud::Query {
            bounds: region.map(|[[e0, n0, h0], [e1, n1, h1]]| [[n0, e0, h0], [n1, e1, h1]]),
            spacing: request.tolerance,
            limit: request.limit.unwrap_or(u64::MAX),
        };
        let scene =
            zumen_pointcloud::scene3d::build(&cloud, &query, cx).map_err(|e| scene_error(cx, e))?;
        let tolerance = request
            .tolerance
            .unwrap_or_else(|| cloud.octree.spacing(scene.level));
        let crs = cloud.crs.map(|crs| format!("EPSG:{}", crs.epsg()));
        let height_crs = cloud.vertical.map(|v| u64::from(v.epsg()));
        drop(cloud);
        let data = &scene.data;
        let counts = Scene3dCounts {
            chunks: data.chunks.len() as u64,
            meshes: 0,
            instances: 0,
            elements: 0,
            colors: 0,
            vertices: 0,
            triangles: 0,
            edges: 0,
            points: Some(scene.points.positions.len() as u64),
        };
        let bytes = zumen_scene3d::encode_with_points(data, &scene.points, cx)
            .map_err(|e| scene_error(cx, e))?;
        let result = Scene3dResult {
            doc: request.doc.clone(),
            unit: BuildingUnit {
                kind: "LENGTHUNIT".into(),
                name: "METRE".into(),
                prefix: None,
                scale: None,
            },
            map: Some(Scene3dMap {
                crs_name: crs,
                eastings: 0.0,
                northings: 0.0,
                height: 0.0,
                x_axis_abscissa: Some(1.0),
                x_axis_ordinate: Some(0.0),
                scale: Some(1.0),
            }),
            height_crs,
            lod: None,
            tolerance,
            truncated: Some(scene.truncated),
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
