//! `check` の op（決定 0173・0179）。プロファイルの入力を集め、`zumen-profile` で照合して、指摘を返す。

use zumen_base::{Cx, Ref};
use zumen_drawing::Drawing2D;
use zumen_profile::{Error, Options};
use zumen_scene::Palette;

use crate::messages::{self, CheckFinding, CheckRequest, CheckResult, CheckRule, ProfileIdentity};
use crate::session::{Failed, fail, invalid_value, message_of};

/// limit を書かなかったときに返す指摘の数
const LIMIT: u64 = 1000;

/// 照合して、指摘の一部を返す。`profiles` は入力の名前とバイト列
pub(crate) fn check(
    drawing: &Drawing2D,
    request: &CheckRequest,
    profiles: &[(&str, &[u8])],
    palette: &Palette,
    family: &str,
    cx: &mut Cx<'_>,
) -> Result<CheckResult, Failed> {
    let origins = zumen_profile::origins(drawing, request.from.as_deref(), request.view)
        .map_err(|pointer| fail(cx, invalid_value(pointer)))?;
    let options = Options {
        origins,
        palette,
        family,
    };
    let report = zumen_profile::check(
        drawing,
        profiles,
        &request.profile.id,
        &request.profile.edition,
        &options,
        cx,
    )
    .map_err(|error| match error {
        Error::NotFound => fail(cx, invalid_value("/profile")),
        Error::Profile(diagnostic) => fail(cx, *diagnostic),
        Error::Stop(stop, at) if *at == Ref::default() => fail(cx, stop.to_diagnostic()),
        Error::Stop(stop, at) => fail(cx, stop.to_diagnostic().at(*at)),
    })?;
    let total = report.findings.len() as u64;
    let start = request.offset.unwrap_or(0);
    let end = start.saturating_add(request.limit.unwrap_or(LIMIT));
    let findings = report
        .findings
        .iter()
        .skip(usize::try_from(start).unwrap_or(usize::MAX))
        .take(usize::try_from(end - start).unwrap_or(usize::MAX))
        .map(|finding| {
            let mut diagnostic = message_of(&finding.diagnostic);
            diagnostic.severity = match finding.level {
                zumen_base::Severity::Info => messages::Severity::Info,
                zumen_base::Severity::Warning => messages::Severity::Warning,
                zumen_base::Severity::Error => messages::Severity::Error,
            };
            CheckFinding {
                rule: finding.rule.clone(),
                diagnostic,
            }
        })
        .collect();
    Ok(CheckResult {
        chain: report
            .chain
            .into_iter()
            .map(|identity| ProfileIdentity {
                id: identity.id,
                edition: identity.edition,
                revision: identity.revision,
            })
            .collect(),
        rules: report
            .rules
            .into_iter()
            .map(|rule| CheckRule {
                id: rule.id,
                check: rule.check,
                findings: rule.findings,
                unsupported: rule.unsupported,
            })
            .collect(),
        findings,
        total,
        next_offset: (end < total).then_some(end),
    })
}
