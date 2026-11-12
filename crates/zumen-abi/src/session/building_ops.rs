//! セッションの建物の編集と IDS による照合。

use super::*;

impl Session {
    /// 変更の一覧を IFC に当てて書き戻し、書き戻したファイルを blob で返す（決定 0244）
    #[cfg(feature = "bim")]
    pub(super) fn building_apply(
        &mut self,
        request: &messages::BuildingApplyRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::BuildingApplyResult, Failed> {
        let model = self.model(&request.doc, "building_apply", cx)?;
        // 書き出しが予算や中断で止まっても、元の文書は変更しない。
        let (updated, result, bytes) = building::apply(model, request, cx)?;
        // 線形の読み取りも終わってから、建物と土木の見方を一緒に差し替える。
        self.docs
            .insert(request.doc.clone(), Document::from_building(updated, cx)?);
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// 名前の建物モデル。図面の文書なら、`op` を使えない
    pub(super) fn model(
        &self,
        doc: &str,
        op: &str,
        cx: &mut Cx<'_>,
    ) -> Result<&building::Model, Failed> {
        if !name::is_safe(doc) {
            return Err(fail(cx, invalid_value("/doc")));
        }
        match self.document(doc, cx)? {
            Document::Building { model, .. } => Ok(model),
            _ => Err(fail(cx, unsupported(doc, op))),
        }
    }

    /// 建物モデルを IDS で照合する（決定 0219）
    pub(super) fn ids_check(
        &self,
        request: &IdsCheckRequest,
        cx: &mut Cx<'_>,
    ) -> Result<IdsCheckResult, Failed> {
        let model = self.model(&request.doc, "ids_check", cx)?;
        let ids = self.input(&request.ids, "/ids", cx)?;
        let express = match &request.express {
            Some(name) => Some(self.input(name, "/express", cx)?),
            None => None,
        };
        let (ids, report) = building::check(model, ids, express, cx)?;
        Ok(building::ids_check(
            &ids,
            &report,
            request.offset,
            request.limit,
        ))
    }
}
