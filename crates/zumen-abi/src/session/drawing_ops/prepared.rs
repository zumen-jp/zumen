//! 形式ごとの作業用の文書。表への参照をすべて解き、書き出しも成功してから Session に返す。

use super::*;
use zumen_acad::{DwgFile, DxfFile};
use zumen_jww::JwwFile;
use zumen_sxf::SxfFile;

enum Prepared {
    Dxf(DxfFile),
    Dwg(DwgFile),
    Sxf(SxfFile),
    Jww(JwwFile),
}

impl Session {
    pub(super) fn apply_with_tables(
        &mut self,
        request: &ApplyRequest,
        drawing: &Drawing2D,
        cx: &mut Cx<'_>,
    ) -> Result<ApplyResult, Failed> {
        let (original, original_hash) = self.drawing_file(&request.doc, cx)?;
        let mut bytes = Vec::new();
        let (prepared, applied, shifted) = match original.file(cx).map_err(|_| Failed)? {
            DrawingFile::Dxf(file) => {
                let (updated, applied, shifted) = batch::run(
                    request,
                    drawing,
                    original_hash,
                    file,
                    |file, changes, cx| {
                        let mut updated = file.snapshot(cx).map_err(|_| Failed)?;
                        let applied = updated.apply(changes, cx).map_err(|_| Failed)?;
                        Ok((updated, applied, false))
                    },
                    |file, cx| quiet_lift(cx, |cx| file.lift(cx)),
                    cx,
                )?;
                Dxf::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                (Prepared::Dxf(updated), applied, shifted)
            }
            DrawingFile::Dwg(file) => {
                let (updated, applied, shifted) = batch::run(
                    request,
                    drawing,
                    original_hash,
                    file,
                    |file, changes, cx| {
                        let mut updated = file.snapshot(cx).map_err(|_| Failed)?;
                        let applied = updated.apply(changes, cx).map_err(|_| Failed)?;
                        Ok((updated, applied, false))
                    },
                    |file, cx| quiet_lift(cx, |cx| file.lift(cx)),
                    cx,
                )?;
                Dwg::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                (Prepared::Dwg(updated), applied, shifted)
            }
            DrawingFile::Sxf(file, archive, attributes) => {
                let (updated, applied, shifted) = batch::run(
                    request,
                    drawing,
                    original_hash,
                    file,
                    |file, changes, cx| {
                        let (updated, applied) = file.applied(changes, cx).map_err(|_| Failed)?;
                        Ok((updated, applied, false))
                    },
                    |file, cx| quiet_lift(cx, |cx| file.lift_with(attributes.as_deref(), cx)),
                    cx,
                )?;
                Sxf::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                if let Some(archive) = archive {
                    bytes = archive
                        .write(&bytes)
                        .ok_or_else(|| fail(cx, Diagnostic::new(codes::SXF_ARCHIVE_UNSUPPORTED)))?;
                }
                (Prepared::Sxf(updated), applied, shifted)
            }
            DrawingFile::Jww(file) => {
                let (updated, applied, shifted) = batch::run(
                    request,
                    drawing,
                    original_hash,
                    file,
                    |file, changes, cx| {
                        // 元のバイト列から作業用の写しを読み、複製も読み取りの予算で制限する。
                        let mut bytes = Vec::new();
                        Jww::write(file, &mut bytes, cx).map_err(|_| Failed)?;
                        let mut updated = Jww::read(&bytes, cx).map_err(|_| Failed)?;
                        let (applied, shifted) = updated.apply(changes, cx).map_err(|_| Failed)?;
                        Ok((updated, applied, shifted))
                    },
                    |file, cx| quiet_lift(cx, |cx| file.lift(cx)),
                    cx,
                )?;
                Jww::write(&updated, &mut bytes, cx).map_err(|_| Failed)?;
                (Prepared::Jww(updated), applied, shifted)
            }
            _ => return Err(fail(cx, unsupported(&request.doc, self.op))),
        };
        let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
        let Some(Document::Drawing {
            file,
            hash: current_hash,
        }) = self.docs.get_mut(&request.doc)
        else {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_DOCUMENT_MISSING).with("name", request.doc.as_str()),
            ));
        };
        match (file.file_mut(cx).map_err(|_| Failed)?, prepared) {
            (DrawingFile::Dxf(file), Prepared::Dxf(updated)) => *file = updated,
            (DrawingFile::Dwg(file), Prepared::Dwg(updated)) => *file = updated,
            (DrawingFile::Sxf(file, ..), Prepared::Sxf(updated)) => *file = updated,
            (DrawingFile::Jww(file), Prepared::Jww(updated)) => *file = updated,
            _ => return Err(fail(cx, unsupported(&request.doc, self.op))),
        }
        *current_hash = hash;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(ApplyResult {
            applied: applied
                .into_iter()
                .map(|Applied { id, hash }| messages::Applied {
                    id,
                    hash: hash.map(|hash| hash.to_string()),
                })
                .collect(),
            hash: hash.to_string(),
            ids_shifted: shifted.then_some(true),
        })
    }
}

/// 読み直しの診断は既存の lift と重ねない。停止・失敗したときだけ、その理由を返す。
fn quiet_lift(
    cx: &mut Cx<'_>,
    lift: impl FnOnce(&mut Cx<'_>) -> Result<Drawing2D, zumen_sdk::Error>,
) -> Result<Drawing2D, Failed> {
    let mut quiet = Diagnostics::new();
    match lift(&mut Cx::new(cx.budget, &mut quiet, cx.cancel)) {
        Ok(drawing) => Ok(drawing),
        Err(zumen_sdk::Error::Stopped(stop)) => Err(stopped(cx, stop)),
        Err(_) => {
            for diagnostic in quiet.iter() {
                cx.report(diagnostic.clone());
            }
            Err(Failed)
        }
    }
}
