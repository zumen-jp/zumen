//! 曲線を線分にしない計測の解析的な期待値と、予算の停止を確かめる。
#![allow(clippy::unwrap_used)] // 自作の有限の入力と期待値。

use std::f64::consts::{PI, TAU};
use zumen_geom2d::{
    measure::*,
    topology::{Charge, Error, Failure},
};
fn free(_: Charge) -> Result<(), ()> {
    Ok(())
}
fn line(start: [f64; 2], end: [f64; 2]) -> Piece {
    Piece::Line { start, end }
}
fn circle(center: [f64; 2], radius: f64) -> Piece {
    Piece::Arc {
        center,
        radius,
        start: 0.0,
        sweep: TAU,
    }
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-10 * (1.0 + b.abs()), "{a} != {b}");
}
fn distance(a: Piece, b: Piece, expected: f64) {
    let found = closest_paths(&[a], &[b], free).unwrap();
    near(found.distance, expected);
    near(
        libm::hypot(
            found.second[0] - found.first[0],
            found.second[1] - found.first[1],
        ),
        expected,
    );
    near(closest_paths(&[b], &[a], free).unwrap().distance, expected);
}

#[test]
fn lengths_and_closed_areas_keep_circle_and_bulge_definitions() {
    assert_eq!(
        path_metric(&[line([0.0, 0.0], [3.0, 4.0])], false, Metric::Length, free),
        Ok(5.0)
    );
    assert_eq!(
        path_metric(&[circle([0.0, 0.0], 2.0)], true, Metric::Length, free),
        Ok(4.0 * PI)
    );
    assert_eq!(
        path_metric(&[circle([1e12, -1e12], 2.0)], true, Metric::Area, free),
        Ok(4.0 * PI)
    );
    let semicircle = bulge_piece([-2.0, 0.0], [2.0, 0.0], 1.0).unwrap();
    near(
        path_metric(&[semicircle], true, Metric::Area, free).unwrap(),
        2.0 * PI,
    );
    near(
        path_metric(&[semicircle], true, Metric::Perimeter, free).unwrap(),
        2.0 * PI + 4.0,
    );
    let reverse = bulge_piece([2.0, 0.0], [-2.0, 0.0], -1.0).unwrap();
    near(
        path_metric(&[reverse], true, Metric::Area, free).unwrap(),
        2.0 * PI,
    );
    assert_eq!(
        path_metric(&[semicircle], false, Metric::Area, free),
        Err(Failure::Geometry(Error::NoSolution))
    );
    let shifted = [
        line([1e12, 1e12], [1e12 + 10.0, 1e12]),
        line([1e12 + 10.0, 1e12], [1e12 + 10.0, 1e12 + 20.0]),
        line([1e12 + 10.0, 1e12 + 20.0], [1e12, 1e12 + 20.0]),
    ];
    assert_eq!(path_metric(&shifted, true, Metric::Area, free), Ok(200.0));
    assert_eq!(
        path_metric(&shifted, true, Metric::Perimeter, free),
        Ok(60.0)
    );
}

#[test]
fn closest_lines_circles_and_points_use_the_boundaries() {
    distance(
        line([0.0, 0.0], [10.0, 0.0]),
        line([5.0, -5.0], [5.0, 5.0]),
        0.0,
    );
    distance(
        line([0.0, 0.0], [10.0, 0.0]),
        line([2.0, 3.0], [8.0, 3.0]),
        3.0,
    );
    distance(circle([0.0, 0.0], 10.0), circle([0.0, 0.0], 2.0), 8.0);
    distance(circle([0.0, 0.0], 10.0), circle([3.0, 0.0], 2.0), 5.0);
    distance(circle([0.0, 0.0], 2.0), circle([10.0, 0.0], 3.0), 5.0);
    distance(circle([0.0, 0.0], 5.0), circle([6.0, 0.0], 5.0), 0.0);
    distance(circle([0.0, 0.0], 5.0), line([-2.0, 0.0], [2.0, 0.0]), 3.0);
    distance(
        circle([0.0, 0.0], 5.0),
        line([-10.0, 5.0], [10.0, 5.0]),
        0.0,
    );
    distance(
        circle([0.0, 0.0], 5.0),
        line([-10.0, 7.0], [10.0, 7.0]),
        2.0,
    );
    distance(circle([0.0, 0.0], 5.0), line([0.0, 0.0], [0.0, 0.0]), 5.0);
}

#[test]
fn arc_ranges_limit_intersections_and_normal_candidates() {
    let quarter = Piece::Arc {
        center: [0.0, 0.0],
        radius: 5.0,
        start: 0.0,
        sweep: PI / 2.0,
    };
    distance(quarter, line([-10.0, -3.0], [10.0, -3.0]), 3.0);
    distance(quarter, line([-10.0, 3.0], [10.0, 3.0]), 0.0);
    distance(quarter, line([-10.0, 0.0], [-10.0, 0.0]), libm::sqrt(125.0));
    let upper = Piece::Arc {
        center: [0.0, 0.0],
        radius: 5.0,
        start: PI / 4.0,
        sweep: PI / 2.0,
    };
    distance(quarter, upper, 0.0);
    let lower = Piece::Arc {
        center: [0.0, 0.0],
        radius: 5.0,
        start: PI,
        sweep: PI / 2.0,
    };
    distance(quarter, lower, libm::sqrt(50.0));
}

#[test]
fn invalid_values_and_all_budget_checkpoints_stop() {
    assert_eq!(
        closest_paths(&[], &[], free),
        Err(Failure::Geometry(Error::Degenerate))
    );
    assert_eq!(
        path_metric(
            &[circle([0.0, 0.0], f64::INFINITY)],
            true,
            Metric::Length,
            free
        ),
        Err(Failure::Geometry(Error::NonFinite))
    );
    assert_eq!(
        bulge_piece([0.0, 0.0], [1.0, 1.0], f64::NAN),
        Err(Error::NonFinite)
    );
    assert_eq!(
        closest_paths(
            &[line([-1e308, 0.0], [1e308, 0.0])],
            &[line([0.0, 0.0], [0.0, 0.0])],
            free
        ),
        Err(Failure::Geometry(Error::Range))
    );
    assert_eq!(
        closest_paths(
            &[circle([-1e308, 0.0], 1e308)],
            &[line([0.0, 0.0], [0.0, 0.0])],
            free
        ),
        Err(Failure::Geometry(Error::Range))
    );
    let a = [line([0.0, 0.0], [3.0, 4.0]), circle([10.0, 0.0], 2.0)];
    let b = [circle([0.0, 0.0], 1.0)];
    let mut calls = 0;
    closest_paths(&a, &b, |_| {
        calls += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    for stop in 0..calls {
        let mut at = 0;
        assert_eq!(
            closest_paths(&a, &b, |_| {
                let current = at;
                at += 1;
                if current == stop { Err(stop) } else { Ok(()) }
            }),
            Err(Failure::Stopped(stop))
        );
    }
}
