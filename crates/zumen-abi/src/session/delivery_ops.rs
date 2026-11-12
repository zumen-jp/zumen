//! セッションの成果品の照合。

use super::*;

impl Session {
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
