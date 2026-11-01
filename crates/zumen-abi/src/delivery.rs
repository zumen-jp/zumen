//! 電子納品の成果品（決定 0241）の境界の処理。成果品を読み、管理ファイルの要約と、プロファイルの規則と照らした指摘を返す。

use zumen_base::Cx;
use zumen_delivery::{Package, Record};

use crate::check::{LIMIT, finding_of, identity_of, profile_failed};
use crate::messages::{CheckRule, DeliveryCheckRequest, DeliveryCheckResult, DeliveryManaged};
use crate::session::{Failed, stopped};

/// 成果品の入力
pub(crate) enum Source<'a> {
    /// 起点からの相対の名前とバイト列
    Files(Vec<(&'a str, &'a [u8])>),
    /// ZIP
    Archive(&'a [u8]),
}

pub(crate) fn delivery_check(
    request: &DeliveryCheckRequest,
    source: Source<'_>,
    profiles: &[(&str, &[u8])],
    cx: &mut Cx<'_>,
) -> Result<DeliveryCheckResult, Failed> {
    let package = match source {
        Source::Files(files) => zumen_delivery::read(&files, cx),
        Source::Archive(bytes) => zumen_delivery::read_zip(bytes, cx),
    };
    let package: Package = package.map_err(|error| match error {
        zumen_sdk::Error::Stopped(stop) => stopped(cx, stop),
        // ほかの理由は、読む処理が診断として足してある
        _ => Failed,
    })?;
    let (chain, report) = match &request.profile {
        None => (Vec::new(), zumen_delivery::Report::default()),
        Some(profile) => {
            let (chain, values) = zumen_profile::rules(profiles, &profile.id, &profile.edition, cx)
                .map_err(|error| profile_failed(cx, error))?;
            let report =
                zumen_delivery::check(&package, &values, cx).map_err(|stop| stopped(cx, stop))?;
            (chain, report)
        }
    };
    let total = report.findings.len() as u64;
    let start = request.offset.unwrap_or(0);
    let end = start.saturating_add(request.limit.unwrap_or(LIMIT));
    let findings = report
        .findings
        .iter()
        .skip(usize::try_from(start).unwrap_or(usize::MAX))
        .take(usize::try_from(end - start).unwrap_or(usize::MAX))
        .map(finding_of)
        .collect();
    Ok(DeliveryCheckResult {
        files: package.files.len() as u64,
        managed: package
            .managed
            .iter()
            .map(|managed| {
                let mut summary = Summary::default();
                summary.add(&managed.root);
                DeliveryManaged {
                    name: managed.name.clone(),
                    doctype: managed.doctype.clone(),
                    root: managed.root.name.clone(),
                    records: summary.records,
                    links: summary.links,
                    missing: summary.missing,
                }
            })
            .collect(),
        chain: chain.into_iter().map(identity_of).collect(),
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

#[derive(Default)]
struct Summary {
    records: u64,
    links: u64,
    missing: Vec<String>,
}

impl Summary {
    fn add(&mut self, record: &Record) {
        self.records += 1;
        if let Some(link) = &record.file {
            self.links += 1;
            if !link.found {
                self.missing.push(link.name.clone());
            }
        }
        for child in &record.children {
            self.add(child);
        }
    }
}
