//! `deviation` の op（決定 0234）。点群と土木の文書の TIN の標高の差を、プロファイルの規則の区分ごとに集計する。

use zumen_base::{Cx, Diagnostic, codes};
use zumen_civil::{CivilModel, Tin};
use zumen_crs::{HeightDatum, Realization};
use zumen_pointcloud::PointCloud;

use crate::check::{finding_of, identity_of, profile_failed};
use zumen_asbuilt::bands::Step;
use zumen_asbuilt::{Report, Rule};

use crate::messages::{
    DeviationBand, DeviationBandKind, DeviationPoint, DeviationRequest, DeviationResult,
    DeviationRule,
};
use crate::session::{Failed, fail, invalid_value, stopped};

/// 点群の高さから差し引く前に設計面へ足す値。診断は点ごとに重ねず、比較する文書の組に一度返す。
fn surface_height_offset(
    cloud: &PointCloud,
    model: &CivilModel,
    tin: &Tin,
    cx: &mut Cx<'_>,
) -> Result<f64, Failed> {
    let reference = model.height_reference.as_ref();
    let name = reference
        .and_then(|r| r.name.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let surface_name = name
        .or(model.height_datum.map(HeightDatum::key))
        .unwrap_or("unknown");
    let source = reference.map_or(&tin.source, |r| &r.source);
    if let Some(code) = cloud.unsupported_vertical_epsg {
        return Err(fail(
            cx,
            Diagnostic::new(codes::CRS_HEIGHT_DATUM_UNSUPPORTED)
                .with("first", format!("EPSG:{code}"))
                .with("second", surface_name)
                .at(cloud.source.clone()),
        ));
    }

    let offset = reference.and_then(|r| r.tokyo_bay_offset);
    let authored_surface = model.height_datum.is_some() || name.is_some() || offset.is_some();
    // 基準名のない点群は設計面と同じ元の現場基準と仮定する。T.P. への片側だけの換算は行わない。
    let surface_offset = match (cloud.vertical, authored_surface) {
        (Some(_), true) => match offset {
            Some(value) => value,
            None if reference.is_none() && model.height_datum == Some(HeightDatum::TokyoBay) => 0.0,
            None => {
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CRS_HEIGHT_DATUM_UNSUPPORTED)
                        .with("first", HeightDatum::TokyoBay.key())
                        .with("second", surface_name)
                        .at(source.clone()),
                ));
            }
        },
        _ => {
            cx.report(Diagnostic::new(codes::DEVIATION_HEIGHT_DATUM_ASSUMED).at(
                if cloud.vertical.is_none() {
                    cloud.source.clone()
                } else {
                    source.clone()
                },
            ));
            0.0
        }
    };
    let point_realization = cloud.vertical.map(|v| v.realization());
    if let Some(known) = point_realization.or(model.realization) {
        let first = Realization::or_assume(point_realization, known, cloud.source.clone(), cx);
        let second = Realization::or_assume(model.realization, known, source.clone(), cx);
        if first != second {
            cx.report(
                Diagnostic::new(codes::CRS_HEIGHT_REALIZATION_MIXED)
                    .with("first", first.key())
                    .with("second", second.key())
                    .at(source.clone()),
            );
        }
    }
    Ok(surface_offset)
}

/// 差を求めて、集計と、評価した点の一部を返す。`profiles` は入力の名前とバイト列。色分けに使う報告と規則も返す
pub(crate) fn deviation(
    request: &DeviationRequest,
    cloud: &PointCloud,
    model: &CivilModel,
    profiles: &[(&str, &[u8])],
    cx: &mut Cx<'_>,
) -> Result<(DeviationResult, Report, Vec<Rule>), Failed> {
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
    let offset = surface_height_offset(cloud, model, tin, cx)?;
    let mut report =
        zumen_asbuilt::deviation_with_height_offset(&cloud.points, tin, offset, &rules, cx)
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
    let result = DeviationResult {
        chain: chain.into_iter().map(identity_of).collect(),
        points: points.len() as u64,
        outside: report.outside,
        unmatched: report.unmatched,
        excluded: report.excluded,
        rules: std::mem::take(&mut report.summaries)
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
        scene3d: None,
        bands: None,
    };
    Ok((result, report, rules))
}

fn band_kind(step: Step) -> DeviationBandKind {
    match step {
        Step::Below => DeviationBandKind::Below,
        Step::LowOuter => DeviationBandKind::LowOuter,
        Step::LowInner => DeviationBandKind::LowInner,
        Step::HighInner => DeviationBandKind::HighInner,
        Step::HighOuter => DeviationBandKind::HighOuter,
        Step::Above => DeviationBandKind::Above,
        Step::NotEvaluated => DeviationBandKind::NotEvaluated,
    }
}

/// 規則の順の色の段と、最後に評価しなかった点の段（#476）
pub(crate) fn bands(rules: &[Rule]) -> Vec<DeviationBand> {
    let color = |step| zumen_asbuilt::bands::color(step).map(u64::from).to_vec();
    let mut out: Vec<DeviationBand> = rules
        .iter()
        .flat_map(|rule| {
            zumen_asbuilt::bands::bands(rule)
                .into_iter()
                .map(|band| DeviationBand {
                    rule: Some(rule.id.clone()),
                    band: band_kind(band.kind),
                    lower: band.lower,
                    upper: band.upper,
                    color: color(band.kind),
                })
        })
        .collect();
    out.push(DeviationBand {
        rule: None,
        band: DeviationBandKind::NotEvaluated,
        lower: None,
        upper: None,
        color: color(Step::NotEvaluated),
    });
    out
}
