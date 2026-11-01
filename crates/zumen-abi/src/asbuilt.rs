//! `deviation` の op（決定 0234）。点群と土木の文書の TIN の標高の差を、プロファイルの規則の区分ごとに集計する。

use zumen_base::{Cx, Diagnostic, codes};
use zumen_civil::{CivilModel, Tin};
use zumen_pointcloud::PointCloud;

use crate::check::{finding_of, identity_of, profile_failed};
use crate::messages::{DeviationPoint, DeviationRequest, DeviationResult, DeviationRule};
use crate::session::{Failed, fail, invalid_value, stopped};

/// 差を求めて、集計と、評価した点の一部を返す。`profiles` は入力の名前とバイト列
pub(crate) fn deviation(
    request: &DeviationRequest,
    cloud: &PointCloud,
    model: &CivilModel,
    profiles: &[(&str, &[u8])],
    cx: &mut Cx<'_>,
) -> Result<DeviationResult, Failed> {
    let Some(tin) = model
        .surfaces
        .iter()
        .find(|tin: &&Tin| tin.id == request.surface.id)
    else {
        return Err(fail(cx, invalid_value("/surface/id")));
    };
    // 座標系がどちらも分かって違えば、差は意味を持たない
    if let (Some(points), Some(surface)) = (cloud.crs, model.crs)
        && points != surface
    {
        return Err(fail(
            cx,
            Diagnostic::new(codes::DEVIATION_CRS_MIXED)
                .with("points", u64::from(points.epsg()))
                .with("surface", u64::from(surface.epsg())),
        ));
    }
    let (chain, values) =
        zumen_profile::rules(profiles, &request.profile.id, &request.profile.edition, cx)
            .map_err(|error| profile_failed(cx, error))?;
    let rules = zumen_asbuilt::rules(&values, &request.work, cx);
    if rules.is_empty() {
        return Err(fail(cx, invalid_value("/work")));
    }
    let report = zumen_asbuilt::deviation(&cloud.points, tin, &rules, cx)
        .map_err(|stop| stopped(cx, stop))?;
    let total = report.evaluated.len() as u64;
    let start = request.offset.unwrap_or(0);
    let end = start.saturating_add(request.limit.unwrap_or(0));
    let points = &cloud.points;
    let differences = report
        .evaluated
        .iter()
        .skip(usize::try_from(start).unwrap_or(usize::MAX))
        .take(usize::try_from(end - start).unwrap_or(usize::MAX))
        .filter_map(|point| {
            let at = point.index;
            Some(DeviationPoint {
                index: at as u64,
                position: vec![
                    *points.northing.get(at)?,
                    *points.easting.get(at)?,
                    *points.height.get(at)?,
                ],
                difference: point.difference,
                rule: rules.get(point.rule)?.id.clone(),
            })
        })
        .collect();
    Ok(DeviationResult {
        chain: chain.into_iter().map(identity_of).collect(),
        points: points.len() as u64,
        outside: report.outside,
        unmatched: report.unmatched,
        excluded: report.excluded,
        rules: report
            .summaries
            .into_iter()
            .map(|summary| DeviationRule {
                rule: summary.rule,
                part: summary.part,
                points: summary.points,
                area: summary.area,
                density: summary.density,
                mean: summary.mean,
                min: summary.min,
                max: summary.max,
                findings: summary.findings,
                unsupported: summary.unsupported,
            })
            .collect(),
        findings: report.findings.iter().map(finding_of).collect(),
        evaluated: total,
        differences,
        next_offset: (end < total && request.limit.is_some()).then_some(end),
    })
}
