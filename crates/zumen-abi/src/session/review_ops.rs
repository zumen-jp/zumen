//! セッションの照合とレビュー・BCF の操作。

use super::*;

impl Session {
    /// 起点から配置をたどって、プロファイルで照合する（決定 0173・0179）
    pub(super) fn check(
        &mut self,
        request: &CheckRequest,
        cx: &mut Cx<'_>,
    ) -> Result<CheckResult, Failed> {
        let palette = self.palette(request.palette.as_deref(), cx)?;
        let mut profiles = Vec::with_capacity(request.profiles.len());
        for (index, name) in request.profiles.iter().enumerate() {
            if !name::is_safe(name) {
                return Err(fail(cx, invalid_value(format!("/profiles/{index}"))));
            }
            let Some(bytes) = self.inputs.get(name) else {
                return Err(fail(cx, input_missing(name)));
            };
            profiles.push((name.as_str(), bytes.as_slice()));
        }
        let (doc, _) = self.drawing_file(&request.doc, cx)?;
        let drawing = doc.lift(cx).map_err(|_| Failed)?;
        check::check(&drawing, request, &profiles, &palette, doc.family(), cx)
    }

    /// レビューの文書の対象を、文書のバージョンへ移し、移した文書を blob で返す（決定 0218）
    pub(super) fn review_carry(
        &mut self,
        request: &ReviewCarryRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ReviewCarryResult, Failed> {
        let plan = match self.docs.get(&request.doc) {
            Some(Document::Building { model, .. }) => review::Plan {
                hash: model.hash.to_string(),
                elements: model.hashes(cx)?,
            },
            #[cfg(feature = "drawing")]
            _ => {
                let (file, hash) = self.drawing_file(&request.doc, cx)?;
                let drawing = file.lift(cx).map_err(|_| Failed)?;
                review::Plan::drawing(&drawing, hash.to_string(), cx)?
            }
            #[cfg(not(feature = "drawing"))]
            _ => {
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CALL_DOCUMENT_MISSING)
                        .with("name", request.doc.as_str()),
                ));
            }
        };
        let bytes = self.input(&request.review, "/review", cx)?;
        let (result, bytes) = review::carry(plan, bytes, request, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// 文書を照合し、選んだ指摘をレビューの文書に取り込み、足した文書を blob で返す（決定 0218）
    pub(super) fn review_adopt(
        &mut self,
        request: &ReviewAdoptRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ReviewAdoptResult, Failed> {
        let check = CheckRequest {
            request_common: request.request_common.clone(),
            doc: request.doc.clone(),
            profiles: request.profiles.clone(),
            profile: request.profile.clone(),
            from: request.from.clone(),
            view: request.view,
            file_name: request.file_name.clone(),
            offset: None,
            limit: Some(u64::MAX),
            palette: request.palette.clone(),
        };
        let findings = self.check(&check, cx)?.findings;
        let (doc, hash) = self.drawing_file(&request.doc, cx)?;
        let drawing = doc.lift(cx).map_err(|_| Failed)?;
        let target = zumen_review::Target {
            id: request.target.clone(),
            format: doc.format().to_string(),
            hash: hash.to_string(),
            name: Some(request.doc.clone()),
            extra: Vec::new(),
        };
        let review = match &request.review {
            Some(name) => Some(self.input(name, "/review", cx)?),
            None => None,
        };
        let plan = review::Plan::drawing(&drawing, target.hash.clone(), cx)?;
        let findings: Vec<(String, &messages::Diagnostic)> = findings
            .iter()
            .map(|f| (f.rule.clone(), &f.diagnostic))
            .collect();
        let (result, bytes) =
            review::adopt(&plan, target, review, &findings, &request.findings, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// 建物モデルを IDS で照合し、選んだ指摘をレビューの文書に取り込み、足した文書を blob で返す（決定 0219）
    pub(super) fn review_adopt_ids(
        &mut self,
        request: &ReviewAdoptIdsRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ReviewAdoptResult, Failed> {
        let model = self.model(&request.doc, "review_adopt_ids", cx)?;
        let ids = self.input(&request.ids, "/ids", cx)?;
        let express = match &request.express {
            Some(name) => Some(self.input(name, "/express", cx)?),
            None => None,
        };
        let (_, report) = building::check(model, ids, express, cx)?;
        let findings: Vec<IdsFinding> = report.findings.iter().map(building::finding).collect();
        let findings: Vec<(String, &messages::Diagnostic)> = findings
            .iter()
            .map(|f| (f.specification.clone(), &f.diagnostic))
            .collect();
        let plan = review::Plan {
            hash: model.hash.to_string(),
            elements: model.hashes(cx)?,
        };
        let target = zumen_review::Target {
            id: request.target.clone(),
            format: "ifc".to_string(),
            hash: plan.hash.clone(),
            name: Some(request.doc.clone()),
            extra: Vec::new(),
        };
        let review = match &request.review {
            Some(name) => Some(self.input(name, "/review", cx)?),
            None => None,
        };
        let (result, bytes) =
            review::adopt(&plan, target, review, &findings, &request.findings, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// bcfzip を読み、開いた建物モデルを対象にしたレビューの文書を blob で返す（決定 0220）
    pub(super) fn bcf_read(
        &mut self,
        request: &BcfReadRequest,
        cx: &mut Cx<'_>,
    ) -> Result<BcfReadResult, Failed> {
        let model = self.model(&request.doc, "bcf_read", cx)?;
        let bytes = self.input(&request.input, "/input", cx)?;
        let (result, bytes) = bcf::read(model, bytes, request, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }

    /// レビューの文書を bcfzip に書き、blob で返す（決定 0220）
    pub(super) fn bcf_write(
        &mut self,
        request: &BcfWriteRequest,
        cx: &mut Cx<'_>,
    ) -> Result<BcfWriteResult, Failed> {
        let review = self.input(&request.review, "/review", cx)?;
        let original = match &request.original {
            Some(name) => Some(self.input(name, "/original", cx)?),
            None => None,
        };
        let (result, bytes) = bcf::write(review, original, request, cx)?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(result)
    }
}
