//! セッションの点群と出来形の操作。

use super::*;

impl Session {
    /// 点群と TIN の標高の差の、規則の区分ごとの集計（決定 0234）
    #[cfg(feature = "pointcloud")]
    pub(super) fn deviation(
        &self,
        request: &messages::DeviationRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::DeviationResult, Failed> {
        let cloud = self.cloud(&request.doc, "/doc", cx)?;
        let model = self.civil_model(&request.surface.doc, "/surface/doc", "deviation", cx)?;
        let mut profiles = Vec::with_capacity(request.profiles.len());
        for (index, name) in request.profiles.iter().enumerate() {
            let bytes = self.input(name, &format!("/profiles/{index}"), cx)?;
            profiles.push((name.as_str(), bytes));
        }
        asbuilt::deviation(request, cloud, &model, &profiles, cx)
    }

    /// 開いた点群の文書。`pointer` は名前を書いたフィールド
    #[cfg(feature = "pointcloud")]
    pub(super) fn cloud(
        &self,
        doc: &str,
        pointer: &str,
        cx: &mut Cx<'_>,
    ) -> Result<&zumen_pointcloud::PointCloud, Failed> {
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value(pointer)));
        }
        match self.document(doc, cx)? {
            Document::Cloud(cloud) => Ok(cloud),
            _ => Err(fail(cx, unsupported(doc, "pointcloud"))),
        }
    }

    /// 開いた点群の要約と、条件に合う点（決定 0232）
    #[cfg(feature = "pointcloud")]
    pub(super) fn pointcloud(
        &self,
        request: &messages::PointcloudRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::PointcloudResult, Failed> {
        let cloud = self.cloud(&request.doc, "/doc", cx)?;
        pointcloud::pointcloud(request, cloud, cx)
    }
}
