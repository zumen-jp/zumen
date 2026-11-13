//! セッションの土木と地理の操作。

use super::*;
use std::borrow::Cow;
use zumen_acad::Dxf;

impl Session {
    /// 線形の上の所を求める。doc を渡せば、開いた土木の領域モデルの線形を使う（決定 0230）
    #[cfg(any(
        feature = "drawing",
        feature = "bim",
        feature = "civil",
        feature = "pointcloud"
    ))]
    pub(super) fn civil(
        &self,
        request: &messages::CivilRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::CivilResult, Failed> {
        let model = request
            .doc
            .as_deref()
            .map(|doc| self.civil_model(doc, "/doc", "civil", cx))
            .transpose()?;
        civil::civil(request, model.as_deref(), cx)
    }

    pub(super) fn civil_model(
        &self,
        doc: &str,
        pointer: &str,
        op: &str,
        cx: &mut Cx<'_>,
    ) -> Result<Cow<'_, zumen_civil::CivilModel>, Failed> {
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value(pointer)));
        }
        match self.document(doc, cx)? {
            Document::Civil(model)
            | Document::Building {
                civil: Some(model), ..
            } => Ok(Cow::Borrowed(model.as_ref())),
            #[cfg(feature = "drawing")]
            Document::Drawing { file, .. } if matches!(file.as_ref(), DrawingFile::Sima(_)) => {
                let DrawingFile::Sima(sima) = file.as_ref() else {
                    return Err(fail(cx, unsupported(doc, op)));
                };
                zumen_sdk::Lift::lift(sima, cx)
                    .map(Cow::Owned)
                    .map_err(|_| Failed)
            }
            #[cfg(all(feature = "civil", not(feature = "drawing")))]
            Document::Sima(sima) => zumen_sdk::Lift::lift(sima.as_ref(), cx)
                .map(Cow::Owned)
                .map_err(|_| Failed),
            _ => Err(fail(cx, unsupported(doc, op))),
        }
    }

    /// 開いた CityGML・基盤地図情報・拡張 DM・GeoJSON・SIMA の地理の領域モデルの要約と、地物の中身（決定 0233）
    #[cfg(feature = "civil")]
    pub(super) fn geo(
        &self,
        request: &messages::GeoRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::GeoResult, Failed> {
        let doc = &request.doc;
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        match self.document(doc, cx)? {
            Document::Geo(model) => geo::geo(request, model, cx),
            #[cfg(feature = "drawing")]
            Document::Drawing { file, .. } if matches!(file.as_ref(), DrawingFile::Sima(_)) => {
                let DrawingFile::Sima(sima) = file.as_ref() else {
                    return Err(fail(cx, unsupported(doc, "geo")));
                };
                let model: zumen_geo::GeoModel =
                    zumen_sdk::Lift::lift(sima, cx).map_err(|_| Failed)?;
                geo::geo(request, &model, cx)
            }
            #[cfg(all(feature = "civil", not(feature = "drawing")))]
            Document::Sima(sima) => {
                let model: zumen_geo::GeoModel =
                    zumen_sdk::Lift::lift(sima.as_ref(), cx).map_err(|_| Failed)?;
                geo::geo(request, &model, cx)
            }
            _ => Err(fail(cx, unsupported(doc, "geo"))),
        }
    }

    /// 土木の文書の線形から図を作り、新しい DXF の文書として開く（決定 0239）
    #[cfg(feature = "drawing")]
    pub(super) fn civil_drawing(
        &mut self,
        request: &messages::CivilDrawingRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::CivilDrawingResult, Failed> {
        let doc = &request.doc;
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        if !name::is_safe(&request.to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        let to = &request.to;
        self.check_new_document(to, cx)?;
        let model = self.civil_model(doc, "/doc", "civil_drawing", cx)?;
        let names = request.profiles.as_deref().unwrap_or_default();
        let mut profiles = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let bytes = self.input(name, &format!("/profiles/{index}"), cx)?;
            profiles.push((name.as_str(), bytes));
        }
        let (converted, mut result) = in_input_order(cx, |cx| {
            let (drawing, result) = civil::civil_drawing(request, &model, &profiles, cx)?;
            Ok((convert::to_dxf(&drawing, cx)?, result))
        })?;
        let hash =
            Hash::document_checked(&converted.bytes, cx).map_err(|stop| stopped(cx, stop))?;
        self.docs.insert(
            to.clone(),
            Document::Drawing {
                file: Box::new(DrawingFile::Dxf(converted.file)),
                hash,
            },
        );
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes: converted.bytes,
        }));
        result.format = Dxf::FORMAT.to_string();
        result.hash = hash.to_string();
        result.elements = converted.elements;
        Ok(result)
    }
}
