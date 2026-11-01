//! 合成の TIN と点群で、標高の差と区分ごとの集計を求める（決定 0234）。

use zumen_asbuilt::{Report, deviation, rules};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, Ref, codes};
use zumen_civil::{SurfaceRole, Tin};
use zumen_crs::MapPoint;
use zumen_pointcloud::Points;

const PROFILE: &[u8] = include_bytes!("../../../profiles/mlit-earthwork/2026-03/profile.json");

/// 東 0〜10 m が水平の平場、東 10〜20 m が 2 割の法面（東へ 5 m 上がる）の TIN
#[cfg(test)]
fn tin() -> Tin {
    let point = |easting: f64, northing: f64, height: f64| (MapPoint { northing, easting }, height);
    Tin {
        id: "design".into(),
        name: None,
        role: SurfaceRole::Design,
        points: vec![
            point(0.0, 0.0, 0.0),
            point(10.0, 0.0, 0.0),
            point(10.0, 10.0, 0.0),
            point(0.0, 10.0, 0.0),
            point(20.0, 0.0, 5.0),
            point(20.0, 10.0, 5.0),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3], [1, 4, 5], [1, 5, 2]],
        boundaries: Vec::new(),
        breaklines: Vec::new(),
        source: Ref::default(),
    }
}

/// 0.5 m の間隔の格子の点。標高は設計面に `offset` の差を足す
#[cfg(test)]
fn grid(offset: impl Fn(f64, f64) -> f64) -> Points {
    let mut points = Points::default();
    for row in 0..20 {
        for column in 0..40 {
            let (e, n) = (0.25 + f64::from(column) * 0.5, 0.25 + f64::from(row) * 0.5);
            let ground = if e <= 10.0 { 0.0 } else { (e - 10.0) / 2.0 };
            points.easting.push(e);
            points.northing.push(n);
            points.height.push(ground + offset(e, n));
        }
    }
    // TIN の外の点
    points.easting.push(30.0);
    points.northing.push(5.0);
    points.height.push(0.0);
    points
}

#[cfg(test)]
fn run(points: &Points) -> (Report, Vec<Code>) {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let (chain, values) = zumen_profile::rules(
        &[("profile.json", PROFILE)],
        "mlit-earthwork",
        "2026-03",
        &mut cx,
    )
    .unwrap();
    assert_eq!(chain.len(), 1);
    let found = rules(&values, "road-excavation", &mut cx);
    assert_eq!(found.len(), 2);
    let report = deviation(points, &tin(), &found, &mut cx).unwrap();
    (report, diag.iter().map(|d| d.code()).collect())
}

#[test]
fn summarizes_differences_by_part() {
    let points = grid(|e, _| if e < 10.0 { 0.02 } else { -0.01 });
    let (report, found) = run(&points);
    assert_eq!(found, []);
    assert_eq!(report.outside, 1);
    assert_eq!(report.unmatched, 0);
    let [flat, slope] = report.summaries.as_slice() else {
        panic!("{:?}", report.summaries);
    };
    assert_eq!(flat.rule, "road-excavation-flat");
    assert_eq!(flat.points, 400);
    assert_eq!(flat.area, 100.0);
    assert_eq!(flat.density, Some(4.0));
    assert!((flat.mean.unwrap() - 0.02).abs() < 1e-12);
    assert_eq!(slope.rule, "road-excavation-slope");
    assert_eq!(slope.points, 400);
    assert!((slope.max.unwrap() + 0.01).abs() < 1e-9);
    assert!(report.findings.is_empty());
    assert_eq!(report.evaluated.len(), 800);
}

#[test]
fn reports_values_outside_the_limits_and_excludes_the_part_border() {
    // 平場の 1 点だけが 200 mm 高い。法面は 100 mm 低い
    let points = grid(|e, n| {
        if e < 10.0 {
            if (e, n) == (0.25, 0.25) { 0.2 } else { 0.0 }
        } else {
            -0.1
        }
    });
    let mut moved = points.clone();
    // 平場と法面の境（東 10 m）から 3 cm の点を足す
    moved.easting.push(9.97);
    moved.northing.push(5.0);
    moved.height.push(1.0);
    let (report, _) = run(&moved);
    assert_eq!(report.excluded, 1);
    let codes: Vec<(String, Code)> = report
        .findings
        .iter()
        .map(|finding| (finding.rule.clone(), finding.diagnostic.code()))
        .collect();
    assert_eq!(
        codes,
        [
            (
                "road-excavation-flat".to_string(),
                codes::PROFILE_SURFACE_ELEVATION_INVALID
            ),
            (
                "road-excavation-slope".to_string(),
                codes::PROFILE_SURFACE_ELEVATION_INVALID
            ),
        ]
    );
    let [flat, slope] = report.summaries.as_slice() else {
        panic!();
    };
    assert_eq!(flat.findings, 1);
    assert_eq!(slope.findings, 1);
}

#[test]
fn reports_too_few_points_as_unsupported() {
    let mut points = Points::default();
    points.easting.push(5.0);
    points.northing.push(5.0);
    points.height.push(0.0);
    let (report, _) = run(&points);
    let levels: Vec<Code> = report
        .findings
        .iter()
        .map(|f| f.diagnostic.code())
        .collect();
    assert_eq!(
        levels,
        [
            codes::PROFILE_SURFACE_ELEVATION_UNSUPPORTED,
            codes::PROFILE_SURFACE_ELEVATION_UNSUPPORTED
        ]
    );
    assert_eq!(
        report.summaries.iter().map(|s| s.unsupported).sum::<u64>(),
        2
    );
}
