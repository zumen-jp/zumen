//! セッションの成果品の照合。

use super::*;

impl Session {
    /// 管理 XML の変更を検査し、元の入力を変えずにファイルを返す。
    #[cfg(feature = "drawing")]
    pub(super) fn delivery_apply(
        &mut self,
        request: &messages::DeliveryApplyRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::DeliveryApplyResult, Failed> {
        use messages::DeliveryEdit;
        use zumen_delivery::{Edit, PathStep};

        let source = self.input(&request.input, "/input", cx)?;
        if let Some(value) = &request.hash {
            let expected = value
                .parse::<Hash>()
                .map_err(|_| fail(cx, invalid_value("/hash")))?;
            let actual = Hash::document_checked(source, cx).map_err(|stop| stopped(cx, stop))?;
            if actual != expected {
                return Err(fail(cx, Diagnostic::new(codes::DELIVERY_EDIT_HASH_STALE)));
            }
        }
        cx.check(Kind::Work, request.edits.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(
            Kind::DecodedBytes,
            (request.edits.len() as u64)
                .saturating_mul(192)
                .saturating_add(request.input.len() as u64)
                .saturating_add(32),
        )
        .map_err(|stop| stopped(cx, stop))?;
        let mut prepared = Vec::with_capacity(request.edits.len());
        for edit in &request.edits {
            let path = match edit {
                DeliveryEdit::SetText(edit) => &edit.path,
                DeliveryEdit::Append(edit) => &edit.path,
            };
            cx.check(Kind::Work, path.len() as u64)
                .map_err(|stop| stopped(cx, stop))?;
            cx.check(Kind::DecodedBytes, (path.len() as u64).saturating_mul(32))
                .map_err(|stop| stopped(cx, stop))?;
            let path = path
                .iter()
                .map(|step| {
                    cx.check(Kind::Work, step.name.len() as u64)
                        .map_err(|stop| stopped(cx, stop))?;
                    cx.check(Kind::DecodedBytes, step.name.len() as u64)
                        .map_err(|stop| stopped(cx, stop))?;
                    Ok(PathStep {
                        name: step.name.clone(),
                        index: step.index.unwrap_or(0),
                    })
                })
                .collect::<Result<Vec<_>, Failed>>()?;
            let child = match edit {
                DeliveryEdit::SetText(_) => None,
                DeliveryEdit::Append(edit) => Some(new_element(&edit.child, cx)?),
            };
            prepared.push((path, child));
        }
        let edits = request
            .edits
            .iter()
            .zip(&prepared)
            .map(|(edit, (path, child))| {
                Ok(match edit {
                    DeliveryEdit::SetText(edit) => Edit::SetText {
                        path,
                        value: &edit.value,
                    },
                    DeliveryEdit::Append(_) => Edit::Append {
                        path,
                        child: child.as_ref().ok_or(Failed)?,
                    },
                })
            })
            .collect::<Result<Vec<_>, Failed>>()?;
        let bytes = zumen_delivery::write_xml(source, &edits, cx).map_err(|_| Failed)?;
        let hash = Hash::document_checked(&bytes, cx).map_err(|stop| stopped(cx, stop))?;
        self.blobs.push(Some(Blob {
            kind: BlobKind::File,
            bytes,
        }));
        Ok(messages::DeliveryApplyResult {
            input: request.input.clone(),
            hash: hash.to_string(),
            applied: request.edits.len() as u64,
        })
    }

    /// 電子納品の成果品を読み、管理ファイルの要約を返す（決定 0241）
    #[cfg(feature = "drawing")]
    pub(super) fn delivery_check(
        &self,
        request: &messages::DeliveryCheckRequest,
        cx: &mut Cx<'_>,
    ) -> Result<messages::DeliveryCheckResult, Failed> {
        let source = match (&request.root, &request.archive) {
            (Some(root), None) => {
                let root = match root.strip_suffix('/') {
                    Some(stem) if !stem.is_empty() && name::is_safe(stem) => root.as_str(),
                    _ => return Err(fail(cx, invalid_value("/root"))),
                };
                let files: Vec<(&str, &[u8])> = self
                    .inputs
                    .iter()
                    .filter_map(|(name, bytes)| {
                        name.strip_prefix(root).map(|rest| (rest, bytes.as_slice()))
                    })
                    .collect();
                if files.is_empty() {
                    return Err(fail(cx, invalid_value("/root")));
                }
                delivery::Source::Files(files)
            }
            (None, Some(archive)) => {
                delivery::Source::Archive(self.input(archive, "/archive", cx)?)
            }
            _ => return Err(fail(cx, invalid_value("/root"))),
        };
        let names = request.profiles.as_deref().unwrap_or_default();
        let mut profiles = Vec::with_capacity(names.len());
        for (index, name) in names.iter().enumerate() {
            let bytes = self.input(name, &format!("/profiles/{index}"), cx)?;
            profiles.push((name.as_str(), bytes));
        }
        delivery::delivery_check(request, source, &profiles, cx)
    }
}

/// 要求の木を、深さ・作業量・確保量を検査してから書き手の値へ移す。
#[cfg(feature = "drawing")]
fn new_element(
    value: &messages::DeliveryNewElement,
    cx: &mut Cx<'_>,
) -> Result<zumen_delivery::NewElement, Failed> {
    cx.check(Kind::Depth, 1).map_err(|stop| stopped(cx, stop))?;
    let result = (|| {
        let children = value.children.as_deref().unwrap_or_default();
        let text = value.value.as_ref().map_or(0, |value| value.len() as u64);
        let size = (value.name.len() as u64)
            .saturating_add(text)
            .saturating_add((children.len() as u64).saturating_mul(80))
            .saturating_add(80);
        cx.check(Kind::Work, size)
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, size)
            .map_err(|stop| stopped(cx, stop))?;
        let children = children
            .iter()
            .map(|child| new_element(child, cx))
            .collect::<Result<Vec<_>, Failed>>()?;
        Ok(zumen_delivery::NewElement {
            name: value.name.clone(),
            value: value.value.clone(),
            children,
        })
    })();
    cx.leave();
    result
}
