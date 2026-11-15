//! 円弧の角度境界・重なる区間・固定長の根の比較と予算停止を検査する。
#![allow(clippy::unwrap_used)] // 期待値を持つ固定入力と、その結果を検査するため。

use zumen_geom2d::topology::{
    Arc, ArcIntersection as A, Charge, Circle, CurveIntersection as C, Direction as D, Error,
    Failure, Hit, MAX_COORDINATE, Point, Ray, Segment, arc_intersection, circle_arc_intersection,
    segment_arc_intersection,
};

fn point([x, y]: [i64; 2]) -> Point {
    Point::new(x, y).unwrap()
}
fn ray([x, y]: [i64; 2]) -> Ray {
    Ray::new(x, y).unwrap()
}
fn circle(center: [i64; 2], radius: i64) -> Circle {
    Circle::new(point(center), radius).unwrap()
}
fn hit(xy: [i64; 2], rounded: bool) -> Hit {
    Hit {
        point: point(xy),
        rounded,
    }
}
fn arc(circle: Circle, start: [i64; 2], end: [i64; 2]) -> Arc {
    Arc::new(circle, ray(start), ray(end), D::CounterClockwise).unwrap()
}
fn full(circle: Circle) -> Arc {
    Arc::full(circle, Ray::X, D::CounterClockwise).unwrap()
}
fn free(_: Charge) -> Result<(), ()> {
    Ok(())
}

#[test]
fn directions_normalize_and_endpoints_preserve_the_circle() {
    assert_eq!(ray([3, 4]), ray([30, 40]));
    assert_eq!(ray([0, -100]).components(), [0, -1]);
    assert_eq!(Ray::new(0, 0), Err(Error::Degenerate));
    assert_eq!(Ray::new(i64::MIN, 1), Err(Error::Range));
    let mut rays = [
        [-1, 1],
        [0, -1],
        [1, -1],
        [0, 1],
        [-1, -1],
        [1, 0],
        [1, 1],
        [-1, 0],
    ]
    .map(ray);
    rays.sort_unstable();
    assert_eq!(
        rays,
        [
            [1, 0],
            [1, 1],
            [0, 1],
            [-1, 1],
            [-1, 0],
            [-1, -1],
            [0, -1],
            [1, -1]
        ]
        .map(ray)
    );
    assert_eq!(
        Arc::new(circle([0, 0], 0), Ray::X, Ray::X, D::Clockwise),
        Err(Error::Degenerate)
    );
    let input = arc(circle([0, 0], 5), [3, 4], [-1, -1]);
    assert_eq!(
        input.endpoints(free).unwrap(),
        [hit([3, 4], false), hit([-4, -4], true)]
    );
    assert_eq!(
        input.reversed().endpoints(free).unwrap(),
        [hit([-4, -4], true), hit([3, 4], false)]
    );
    assert_eq!(input.circle(), circle([0, 0], 5));
    assert_eq!(
        full(circle([0, 0], MAX_COORDINATE))
            .endpoints(free)
            .unwrap(),
        [hit([MAX_COORDINATE, 0], false); 2]
    );
}

#[test]
fn segment_roots_are_filtered_before_rounding_to_the_grid() {
    let input = arc(circle([0, 0], 5), [1, 0], [0, 1]);
    let cases = [
        ([[-10, 0], [10, 0]], C::Point(hit([5, 0], false))),
        ([[3, -10], [3, 10]], C::Point(hit([3, 4], false))),
        ([[-10, 5], [10, 5]], C::Tangent(hit([0, 5], false))),
        ([[-10, -5], [10, -5]], C::None),
        ([[3, 4], [3, 4]], C::Point(hit([3, 4], false))),
    ];
    for (ends, expected) in cases {
        let [start, end] = ends.map(point);
        for segment in [
            Segment { start, end },
            Segment {
                start: end,
                end: start,
            },
        ] {
            for input in [input, input.reversed()] {
                assert_eq!(
                    segment_arc_intersection(segment, input, free).unwrap(),
                    expected
                );
            }
        }
    }
    let unit = circle([0, 0], 1);
    // (1/2, sqrt(3)/2) は (1, 1) に丸められるが、45 度までの円弧には含まれない。
    let other = circle([1, 0], 1);
    assert_eq!(
        circle_arc_intersection(other, arc(unit, [1, 0], [1, 1]), free).unwrap(),
        A::None
    );
    assert_eq!(
        circle_arc_intersection(other, arc(unit, [1, 1], [0, 1]), free).unwrap(),
        A::Point(hit([1, 1], true))
    );
    assert_eq!(
        circle_arc_intersection(other, arc(unit, [1, 2], [0, 1]), free).unwrap(),
        A::None
    );
    assert_eq!(
        circle_arc_intersection(circle([5, 0], 0), input, free).unwrap(),
        A::Point(hit([5, 0], false))
    );
}

#[test]
fn coincident_arcs_keep_separate_spans_and_isolated_contacts() {
    let c = circle([0, 0], 5);
    let cases = [
        (arc(c, [1, 0], [0, 1]), arc(c, [-1, 0], [0, -1]), A::None),
        (
            arc(c, [1, 0], [0, 1]),
            arc(c, [0, 1], [-1, 0]),
            A::Point(hit([0, 5], false)),
        ),
        (
            arc(c, [1, 0], [-1, 0]),
            arc(c, [-1, 0], [1, 0]),
            A::Two([hit([-5, 0], false), hit([5, 0], false)]),
        ),
        (
            arc(c, [1, 0], [0, -1]),
            arc(c, [-1, 0], [0, 1]),
            A::TwoOverlaps([arc(c, [1, 0], [0, 1]), arc(c, [-1, 0], [0, -1])]),
        ),
        (
            arc(c, [1, 0], [0, -1]),
            arc(c, [-1, 0], [1, 0]),
            A::OverlapAndPoint(arc(c, [-1, 0], [0, -1]), hit([5, 0], false)),
        ),
        (
            full(c),
            arc(c, [0, -1], [0, 1]),
            A::Overlap(arc(c, [0, -1], [0, 1])),
        ),
        (full(c), arc(c, [1, 1], [1, 1]), A::Point(hit([4, 4], true))),
        (arc(c, [1, 1], [1, 1]), arc(c, [-1, -1], [-1, -1]), A::None),
        (
            Arc::full(c, ray([1, 1]), D::Clockwise).unwrap(),
            full(c),
            A::Overlap(full(c)),
        ),
    ];
    for (a, b, expected) in cases {
        for a in [a, a.reversed()] {
            for b in [b, b.reversed()] {
                assert_eq!(
                    arc_intersection(a, b, free).unwrap(),
                    expected,
                    "{a:?}, {b:?}"
                );
                assert_eq!(arc_intersection(b, a, free).unwrap(), expected);
            }
        }
    }
}

#[test]
fn distinct_circles_keep_exact_boundaries_and_large_angles() {
    let c = circle([0, 0], 5);
    let a = arc(c, [3, 4], [0, -1]);
    let b = full(circle([6, 0], 5));
    assert_eq!(
        arc_intersection(a, b, free).unwrap(),
        A::Point(hit([3, 4], false))
    );
    assert_eq!(
        arc_intersection(full(c), full(circle([10, 0], 5)), free).unwrap(),
        A::Tangent(hit([5, 0], false))
    );
    let scale = MAX_COORDINATE / 8;
    let large = [
        arc(
            circle([0, 0], 5 * scale),
            [3 * scale, 4 * scale - 1],
            [0, -1],
        ),
        arc(
            circle([6 * scale, 0], scale),
            [-MAX_COORDINATE, MAX_COORDINATE - 1],
            [1, -1],
        ),
        arc(
            circle([-scale, scale], 5 * scale),
            [-1, 1],
            [MAX_COORDINATE, MAX_COORDINATE - 1],
        ),
        full(circle([0, 0], MAX_COORDINATE)),
    ];
    for a in large {
        for b in large {
            let expected = arc_intersection(a, b, free).unwrap();
            assert_eq!(arc_intersection(a.reversed(), b, free).unwrap(), expected);
            assert_eq!(arc_intersection(b, a.reversed(), free).unwrap(), expected);
        }
    }
}

#[test]
fn each_budget_checkpoint_stops_without_a_partial_intersection() {
    let c = circle([0, 0], 5);
    for (a, b) in [
        (full(c), full(circle([1, 0], 5))),
        (arc(c, [1, 0], [0, -1]), arc(c, [-1, 0], [1, 0])),
        (arc(c, [1, 0], [-1, 0]), arc(c, [-1, 0], [1, 0])),
        (full(c), full(c)),
    ] {
        let mut amounts = Vec::new();
        arc_intersection(a, b, |amount| {
            amounts.push(amount);
            Ok::<_, usize>(())
        })
        .unwrap();
        assert_eq!(amounts[0], Charge::Work(1));
        for stop in 0..amounts.len() {
            let mut calls = 0;
            let result = arc_intersection(a, b, |_| {
                let at = calls;
                calls += 1;
                if at == stop { Err(at) } else { Ok(()) }
            });
            assert_eq!(result, Err(Failure::Stopped(stop)));
            assert_eq!(calls, stop + 1);
        }
    }
    assert_eq!(full(c).endpoints(|_| Err(7)), Err(Failure::Stopped(7)));
}
