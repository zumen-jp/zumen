//! 合成の線形で、追加距離と測点から位置・方向角・高さを求め、曲線の式から求めた値と比べる（決定 0230）。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_civil::station::{self, Notation};
use zumen_civil::{
    Alignment, Horizontal, HorizontalKind, StationBreak, Stationing, TransitionCurve, Vertical,
    VerticalKind,
};
use zumen_crs::MapPoint;

const A: f64 = 100.0;
const R: f64 = 200.0;

/// クロソイドの原点からの長さ s の点（接線の向きを x、右を y とする）。級数の 4 項まで
fn clothoid(s: f64) -> (f64, f64) {
    let a2 = A * A;
    let x = s - libm::pow(s, 5.0) / (40.0 * a2 * a2)
        + libm::pow(s, 9.0) / (3456.0 * libm::pow(a2, 4.0))
        - libm::pow(s, 13.0) / (599040.0 * libm::pow(a2, 6.0));
    let y = libm::pow(s, 3.0) / (6.0 * a2) - libm::pow(s, 7.0) / (336.0 * libm::pow(a2, 3.0))
        + libm::pow(s, 11.0) / (42240.0 * libm::pow(a2, 5.0));
    (x, y)
}

/// 北へ 50 m の直線、半径 200 m へ右に曲がるクロソイド（A = 100、長さ 50 m）、半径 200 m の円弧 40 m。
/// 縦断は、60 m まで 2 % の勾配と、100 m まで 2 % から −1 % への放物線。測点は 70 m で 10 m 戻る
fn alignment() -> Alignment {
    let length = A * A / R;
    let (x, y) = clothoid(length);
    Alignment {
        id: "a".into(),
        horizontal: vec![
            Horizontal {
                start: MapPoint {
                    northing: 0.0,
                    easting: 0.0,
                },
                direction: 0.0,
                length: 50.0,
                kind: HorizontalKind::Line,
            },
            Horizontal {
                start: MapPoint {
                    northing: 50.0,
                    easting: 0.0,
                },
                direction: 0.0,
                length,
                kind: HorizontalKind::Transition {
                    start_radius: None,
                    end_radius: Some(R),
                    curve: TransitionCurve::Clothoid,
                },
            },
            Horizontal {
                start: MapPoint {
                    northing: 50.0 + x,
                    easting: y,
                },
                direction: length * length / (2.0 * A * A),
                length: 40.0,
                kind: HorizontalKind::Arc { radius: R },
            },
        ],
        vertical: vec![
            Vertical {
                start_distance: 0.0,
                start_height: 10.0,
                start_gradient: 0.02,
                length: 60.0,
                kind: VerticalKind::ConstantGradient,
            },
            Vertical {
                start_distance: 60.0,
                start_height: 11.2,
                start_gradient: 0.02,
                length: 40.0,
                kind: VerticalKind::Parabola {
                    end_gradient: -0.01,
                },
            },
        ],
        stations: Stationing {
            start: 0.0,
            breaks: vec![StationBreak {
                distance: 70.0,
                back: 70.0,
                ahead: 60.0,
            }],
        },
        ..Alignment::default()
    }
}

fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let out = f(&mut cx);
    (out, diag)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * a.abs().max(1.0)
}

/// 緩和曲線の点は数値の積分で求めるので、0.1 µm まで合えばよい
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-7
}

#[test]
fn positions_on_the_line_clothoid_and_arc_match_the_curve_formulas() {
    let alignment = alignment();
    let ((), diag) = with_cx(|cx| zumen_civil::check(&alignment, 1e-6, cx));
    assert!(diag.is_empty(), "{diag:?}");
    // 直線
    let (placed, _) = with_cx(|cx| zumen_civil::place(&alignment, 30.0, cx).unwrap());
    assert!(near(placed.point.northing, 30.0) && near(placed.point.easting, 0.0));
    // クロソイド
    for s in [10.0, 25.0, 50.0] {
        let (placed, _) = with_cx(|cx| zumen_civil::place(&alignment, 50.0 + s, cx).unwrap());
        let (x, y) = clothoid(s);
        assert!(close(placed.point.northing, 50.0 + x), "{s} {placed:?} {x}");
        assert!(close(placed.point.easting, y), "{s} {placed:?} {y}");
        assert!(near(placed.direction, s * s / (2.0 * A * A)), "{s}");
    }
    // 円弧。中心は、クロソイドの終わりから右の法線の向きに R
    let start = &alignment.horizontal[2];
    let tau = start.direction;
    let center = [
        start.start.northing - R * libm::sin(tau),
        start.start.easting + R * libm::cos(tau),
    ];
    for u in [0.0, 15.0, 40.0] {
        let (placed, _) = with_cx(|cx| zumen_civil::place(&alignment, 100.0 + u, cx).unwrap());
        let angle = tau + u / R;
        assert!(
            close(placed.point.northing, center[0] + R * libm::sin(angle)),
            "{u}"
        );
        assert!(
            close(placed.point.easting, center[1] - R * libm::cos(angle)),
            "{u}"
        );
        assert!(near(placed.direction, angle), "{u}");
    }
}

#[test]
fn heights_follow_the_gradient_and_the_parabola() {
    let alignment = alignment();
    for (distance, height, gradient) in [
        (30.0, 10.6, 0.02),
        (60.0, 11.2, 0.02),
        (80.0, 11.45, 0.005),
        (100.0, 11.4, -0.01),
    ] {
        let (placed, _) = with_cx(|cx| zumen_civil::place(&alignment, distance, cx).unwrap());
        assert!(
            near(placed.height.unwrap(), height),
            "{distance} {placed:?}"
        );
        assert!(
            near(placed.gradient.unwrap(), gradient),
            "{distance} {placed:?}"
        );
    }
    // 縦断線形の外は、高さを返さずに知らせる
    let (placed, diag) = with_cx(|cx| zumen_civil::place(&alignment, 120.0, cx).unwrap());
    assert_eq!(placed.height, None);
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CIVIL_PROFILE_MISSING)
    );
}

#[test]
fn stations_are_counted_again_after_a_break() {
    let alignment = alignment();
    let notation = Notation::default();
    assert_eq!(alignment.stations.value(65.0), 65.0);
    assert_eq!(alignment.stations.value(75.0), 65.0);
    assert_eq!(notation.format(65.0).unwrap(), "No.3+5.000");
    assert_eq!(notation.format(39.9999).unwrap(), "No.2+0.000");
    assert_eq!(notation.parse("No.3+5.000"), Some(65.0));
    assert_eq!(notation.parse("No.12"), Some(240.0));
    assert_eq!(notation.parse("No.3+x"), None);
    // 戻りの不連続の前後の 2 か所にある値は、指定がなければ最初の所にして知らせる
    let (found, diag) =
        with_cx(|cx| station::distance(&alignment, "No.3+5", &notation, None, cx).unwrap());
    assert_eq!(found, 65.0);
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CIVIL_STATION_ASSUMED)
    );
    let (found, _) =
        with_cx(|cx| station::distance(&alignment, "No.3+5", &notation, Some(1), cx).unwrap());
    assert_eq!(found, 75.0);
    let (missing, _) = with_cx(|cx| station::distance(&alignment, "No.9", &notation, None, cx));
    assert_eq!(missing.unwrap_err().code(), codes::CIVIL_STATION_MISSING);
}

/// どの緩和曲線も、終わりの向きは初めの向きに平均の曲率 × 長さを足したものになる
#[test]
fn every_ramp_turns_by_the_mean_curvature() {
    use zumen_geom2d::{Ramp, along};
    for ramp in [
        Ramp::Clothoid,
        Ramp::Bloss,
        Ramp::Cosine,
        Ramp::SineHalfWave,
        Ramp::Helmert,
    ] {
        let (_, direction) = along([0.0, 0.0], 0.3, [0.0, 1.0 / R], 60.0, Some(ramp), 60.0);
        assert!(
            near(direction, 0.3 + 60.0 / R / 2.0),
            "{ramp:?} {direction}"
        );
    }
}

#[test]
fn outside_distances_and_uncomputable_curves_are_reported() {
    let mut alignment = alignment();
    let (outside, _) = with_cx(|cx| zumen_civil::place(&alignment, 200.0, cx));
    assert_eq!(outside.unwrap_err().code(), codes::CIVIL_DISTANCE_MISSING);
    alignment.horizontal[1].kind = HorizontalKind::Transition {
        start_radius: None,
        end_radius: Some(R),
        curve: TransitionCurve::CubicParabola,
    };
    let (unsupported, _) = with_cx(|cx| zumen_civil::place(&alignment, 60.0, cx));
    assert_eq!(
        unsupported.unwrap_err().code(),
        codes::CIVIL_CURVE_UNSUPPORTED
    );
    alignment.horizontal[2].kind = HorizontalKind::Arc { radius: 0.0 };
    let (invalid, _) = with_cx(|cx| zumen_civil::place(&alignment, 110.0, cx));
    assert_eq!(invalid.unwrap_err().code(), codes::CIVIL_SEGMENT_INVALID);
}
