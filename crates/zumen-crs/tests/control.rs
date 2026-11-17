//! 図面と地図の対応点から、図面から地図への相似の変換を求める（#685）。

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, codes};
use zumen_crs::{ControlPoint, MapConversion, MapPoint, ProjectPoint, fit_map_conversion};

#[cfg(test)]
fn with_diagnostics<T>(limits: Limits, f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Diagnostic>) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::new(limits);
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let out = f(&mut cx);
    (out, diag.into_vec())
}

#[cfg(test)]
fn fit(points: &[ControlPoint]) -> (Option<zumen_crs::Fitted>, Vec<Diagnostic>) {
    with_diagnostics(Limits::default(), |cx| fit_map_conversion(points, cx))
}

/// mm で描いた図面を、30 度回して IX 系の地図（m）に置く変換
const KNOWN: MapConversion = MapConversion {
    eastings: 30_000.0,
    northings: -190_000.0,
    orthogonal_height: 0.0,
    x_axis_abscissa: 0.866_025_403_784_438_6,
    x_axis_ordinate: 0.5,
    scale: 0.001,
};

const DRAWING: [(f64, f64); 5] = [
    (0.0, 0.0),
    (120_000.0, 0.0),
    (120_000.0, 80_000.0),
    (0.0, 80_000.0),
    (45_000.0, 30_000.0),
];

#[cfg(test)]
fn controls(points: &[(f64, f64)]) -> Vec<ControlPoint> {
    points
        .iter()
        .map(|&(x, y)| {
            let drawing = ProjectPoint { x, y, z: 0.0 };
            ControlPoint {
                drawing,
                map: KNOWN.to_map(drawing).unwrap().point,
            }
        })
        .collect()
}

#[cfg(test)]
fn length(p: MapPoint) -> f64 {
    libm::hypot(p.northing, p.easting)
}

#[test]
fn points_mapped_by_a_known_conversion_give_it_back() {
    let points = controls(&DRAWING);
    let (found, diag) = fit(&points);
    assert!(diag.is_empty(), "{diag:?}");
    let found = found.unwrap();
    let c = found.conversion;
    assert!((c.scale - KNOWN.scale).abs() < 1e-15, "{c:?}");
    assert!(
        (c.x_axis_abscissa - KNOWN.x_axis_abscissa).abs() < 1e-12,
        "{c:?}"
    );
    assert!(
        (c.x_axis_ordinate - KNOWN.x_axis_ordinate).abs() < 1e-12,
        "{c:?}"
    );
    assert!((c.eastings - KNOWN.eastings).abs() < 1e-6, "{c:?}");
    assert!((c.northings - KNOWN.northings).abs() < 1e-6, "{c:?}");
    assert_eq!(c.orthogonal_height, 0.0);
    assert_eq!(found.residuals.len(), points.len());
    assert!(
        found.residuals.iter().all(|r| length(*r) < 1e-6),
        "{found:?}"
    );
    // 同じ入力なら、同じビットの値を返す
    assert_eq!(fit(&points).0, Some(found));
}

#[test]
fn the_moved_point_has_the_largest_residual() {
    for moved in 0..DRAWING.len() {
        let mut points = controls(&DRAWING);
        points[moved].map.northing += 0.5;
        points[moved].map.easting -= 0.3;
        let (found, diag) = fit(&points);
        assert!(diag.is_empty(), "{diag:?}");
        let residuals: Vec<f64> = found.unwrap().residuals.into_iter().map(length).collect();
        let largest = (0..residuals.len())
            .max_by(|a, b| residuals[*a].total_cmp(&residuals[*b]))
            .unwrap();
        assert_eq!(largest, moved, "{residuals:?}");
    }
}

/// 相似の変換は、離れた 2 点や一直線に並ぶ点でも決まる
#[test]
fn two_points_or_points_on_a_line_are_enough() {
    for drawing in [
        &[(0.0, 0.0), (120_000.0, 80_000.0)][..],
        &[(0.0, 0.0), (60_000.0, 40_000.0), (120_000.0, 80_000.0)][..],
    ] {
        let (found, diag) = fit(&controls(drawing));
        assert!(diag.is_empty(), "{diag:?}");
        let c = found.unwrap().conversion;
        let probe = ProjectPoint {
            x: 10_000.0,
            y: 70_000.0,
            z: 0.0,
        };
        let (a, b) = (c.to_map(probe).unwrap(), KNOWN.to_map(probe).unwrap());
        assert!(
            length(MapPoint {
                northing: a.point.northing - b.point.northing,
                easting: a.point.easting - b.point.easting,
            }) < 1e-6
        );
    }
}

#[test]
fn unusable_points_are_reported() {
    let failed = |points: &[ControlPoint], given: u64| {
        let (found, diag) = fit(points);
        assert_eq!(found, None);
        assert_eq!(diag.len(), 1, "{diag:?}");
        assert_eq!(diag[0].code(), codes::CRS_CONTROL_POINTS_FAILED);
        assert_eq!(diag[0].param("points"), Some(&given.into()));
    };
    // 組が足りない
    failed(&[], 0);
    failed(&controls(&DRAWING[..1]), 1);
    // 図面の点が重なる
    let mut same = controls(&DRAWING[..2]);
    same[1].drawing = same[0].drawing;
    failed(&same, 2);
    // 有限でない値は、組の番号を書く
    let mut broken = controls(&DRAWING);
    broken[3].map.easting = f64::NAN;
    broken[4].drawing.x = f64::INFINITY;
    let (found, diag) = fit(&broken);
    assert_eq!(found, None);
    assert_eq!(diag.len(), 1, "{diag:?}");
    assert_eq!(diag[0].code(), codes::CRS_CONTROL_POINT_INVALID);
    assert_eq!(diag[0].param("index"), Some(&3u64.into()));
}

#[test]
fn control_points_are_counted_in_the_budget() {
    let points = controls(&DRAWING);
    let limits = Limits::default().with(Kind::Work, Limit::Finite(4));
    let (found, diag) = with_diagnostics(limits, |cx| fit_map_conversion(&points, cx));
    assert_eq!(found, None);
    assert_eq!(
        diag.iter().map(Diagnostic::code).collect::<Vec<_>>(),
        [codes::BUDGET_WORK_EXCEEDED]
    );
}
