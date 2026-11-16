//! 点群を、八分木の節ごとの点の区画を持つ ZMS3 1.1 で返す（#475、決定 0232）。出来形の差の色分け（#476）も同じ道で作る。
use super::*;

/// 点群のシーンの点の選び方。要求の `box` は東・北・高さ
pub(in crate::session) struct CloudSelection {
    /// 選ぶ箱
    pub region: Option<Bounds>,
    /// 点の間隔
    pub tolerance: Option<f64>,
    /// 点の数の上限
    pub limit: Option<u64>,
}

impl CloudSelection {
    /// 要求の値を確かめる
    pub(in crate::session) fn read(
        r#box: Option<&Scene3dBox>,
        tolerance: Option<f64>,
        limit: Option<u64>,
        pointer: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Self, Failed> {
        if tolerance.is_some_and(|v| !v.is_finite() || v <= 0.0) {
            return Err(fail(cx, invalid_value(format!("{pointer}/tolerance"))));
        }
        let region = r#box
            .map(|b| box_at(b, &format!("{pointer}/box"), cx))
            .transpose()?;
        Ok(Self {
            region,
            tolerance,
            limit,
        })
    }
}

/// 点群のシーンの結果と blob のバイト列。結果の `blob` は 0 のままで、blob を足す側が書く
pub(in crate::session) fn cloud_scene(
    doc: &str,
    cloud: &zumen_pointcloud::PointCloud,
    selection: &CloudSelection,
    colors: Option<&[[u16; 4]]>,
    cx: &mut Cx<'_>,
) -> Result<(Scene3dResult, Vec<u8>), Failed> {
    // 要求の箱は東・北・高さ、点群の Query は北・東・高さ
    let query = zumen_pointcloud::Query {
        bounds: selection
            .region
            .map(|[[e0, n0, h0], [e1, n1, h1]]| [[n0, e0, h0], [n1, e1, h1]]),
        spacing: selection.tolerance,
        limit: selection.limit.unwrap_or(u64::MAX),
    };
    let scene = zumen_pointcloud::scene3d::build_colored(cloud, &query, colors, cx)
        .map_err(|e| scene_error(cx, e))?;
    let tolerance = selection
        .tolerance
        .unwrap_or_else(|| cloud.octree.spacing(scene.level));
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
        doc: doc.to_string(),
        unit: BuildingUnit {
            kind: "LENGTHUNIT".into(),
            name: "METRE".into(),
            prefix: None,
            scale: None,
        },
        map: Some(Scene3dMap {
            crs_name: cloud.crs.map(|crs| format!("EPSG:{}", crs.epsg())),
            eastings: 0.0,
            northings: 0.0,
            height: 0.0,
            x_axis_abscissa: Some(1.0),
            x_axis_ordinate: Some(0.0),
            scale: Some(1.0),
        }),
        height_crs: cloud.vertical.map(|v| u64::from(v.epsg())),
        lod: None,
        tolerance,
        truncated: Some(scene.truncated),
        r#box: selection.region.map(output_box),
        extent: scene.extent.map(output_box),
        counts,
        blob: 0,
    };
    Ok((result, bytes))
}

impl Session {
    /// 開いた点群の文書か。GeoTIFF の DEM は地物の入口に任せる
    pub(super) fn is_cloud(&self, doc: &str) -> bool {
        matches!(self.docs.get(doc), Some(Document::Cloud(_)))
    }

    /// 3D のシーンの blob を足し、番号を返す
    pub(in crate::session) fn push_scene3d(&mut self, bytes: Vec<u8>) -> u64 {
        let id = self.blobs.len() as u64;
        self.blobs.push(Some(Blob {
            kind: BlobKind::Scene3d,
            bytes,
        }));
        id
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
        let selection = CloudSelection::read(
            request.r#box.as_ref(),
            request.tolerance,
            request.limit,
            "",
            cx,
        )?;
        cx.check(Kind::DecodedBytes, 1024 + request.doc.len() as u64)
            .map_err(|s| stopped(cx, s))?;
        let cloud = self.cloud(&request.doc, "/doc", cx)?;
        let (mut result, bytes) = cloud_scene(&request.doc, &cloud, &selection, None, cx)?;
        drop(cloud);
        result.blob = self.push_scene3d(bytes);
        Ok(result)
    }
}
