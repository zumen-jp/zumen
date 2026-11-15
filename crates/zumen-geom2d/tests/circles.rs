//! 円の接触・重なり・端点と、整数式での交点の丸めを確かめる。

use zumen_geom2d::topology::{
    Charge, Circle, CurveIntersection, Error, Failure, Hit, MAX_COORDINATE, Point, Segment,
    circle_intersection, segment_circle_intersection,
};

#[cfg(test)]
fn point([x, y]: [i64; 2]) -> Point {
    Point::new(x, y).unwrap()
}

#[cfg(test)]
fn circle(center: [i64; 2], radius: i64) -> Circle {
    Circle::new(point(center), radius).unwrap()
}

fn segment(start: [i64; 2], end: [i64; 2]) -> Segment {
    Segment {
        start: point(start),
        end: point(end),
    }
}

fn hit(at: [i64; 2], rounded: bool) -> Hit {
    Hit {
        point: point(at),
        rounded,
    }
}

fn one(at: [i64; 2], rounded: bool) -> CurveIntersection {
    CurveIntersection::Point(hit(at, rounded))
}

fn tangent(at: [i64; 2]) -> CurveIntersection {
    CurveIntersection::Tangent(hit(at, false))
}

fn two(a: [i64; 2], b: [i64; 2], rounded: bool) -> CurveIntersection {
    CurveIntersection::Two([hit(a, rounded), hit(b, rounded)])
}

#[test]
fn segments_find_exact_tangencies_endpoints_and_two_roots() {
    let cases = [
        ([-10, 0], [10, 0], [0, 0], 5, two([-5, 0], [5, 0], false)),
        ([-10, 5], [10, 5], [0, 0], 5, tangent([0, 5])),
        (
            [-2, 4],
            [2, 1],
            [0, 0],
            2,
            CurveIntersection::Tangent(hit([1, 2], true)),
        ),
        ([-10, 6], [10, 6], [0, 0], 5, CurveIntersection::None),
        ([-10, 3], [10, 3], [0, 0], 5, two([-4, 3], [4, 3], false)),
        ([-10, -10], [10, 10], [0, 0], 5, two([-4, -4], [4, 4], true)),
        ([5, 0], [10, 0], [0, 0], 5, one([5, 0], false)),
        ([0, 0], [3, 0], [0, 0], 5, CurveIntersection::None),
        ([0, 0], [10, 0], [0, 0], 5, one([5, 0], false)),
        ([0, 5], [0, 5], [0, 0], 5, one([0, 5], false)),
        ([0, 0], [0, 0], [0, 0], 5, CurveIntersection::None),
        ([-10, -10], [10, 10], [0, 0], 0, one([0, 0], false)),
        (
            [990, -1997],
            [1010, -1997],
            [1000, -2000],
            5,
            two([996, -1997], [1004, -1997], false),
        ),
        (
            [-MAX_COORDINATE, 0],
            [MAX_COORDINATE, 0],
            [0, 0],
            MAX_COORDINATE,
            two([-MAX_COORDINATE, 0], [MAX_COORDINATE, 0], false),
        ),
        (
            [MAX_COORDINATE - 10, MAX_COORDINATE - 2],
            [MAX_COORDINATE, MAX_COORDINATE - 2],
            [MAX_COORDINATE - 5, MAX_COORDINATE - 5],
            5,
            two(
                [MAX_COORDINATE - 9, MAX_COORDINATE - 2],
                [MAX_COORDINATE - 1, MAX_COORDINATE - 2],
                false,
            ),
        ),
        // 根は (0, -1) と (20/101, -99/101)。同じ格子点に丸められても 2 点を保つ。
        (
            [0, -1],
            [10, 0],
            [0, 0],
            1,
            CurveIntersection::Two([hit([0, -1], false), hit([0, -1], true)]),
        ),
    ];
    for (start, end, center, radius, expected) in cases {
        let circle = circle(center, radius);
        for line in [segment(start, end), segment(end, start)] {
            assert_eq!(
                segment_circle_intersection(line, circle, |_| Ok::<_, ()>(())),
                Ok(expected),
                "{line:?} {circle:?}"
            );
        }
    }
}

#[test]
fn circles_find_outer_inner_and_concentric_contacts() {
    let cases = [
        ([0, 0], 5, [6, 0], 5, two([3, -4], [3, 4], false)),
        ([0, 0], 5, [10, 0], 5, tangent([5, 0])),
        (
            [0, 0],
            2,
            [3, 4],
            3,
            CurveIntersection::Tangent(hit([1, 2], true)),
        ),
        ([0, 0], 5, [11, 0], 5, CurveIntersection::None),
        ([0, 0], 5, [2, 0], 3, tangent([5, 0])),
        ([0, 0], 5, [1, 0], 3, CurveIntersection::None),
        ([0, 0], 5, [0, 0], 3, CurveIntersection::None),
        ([0, 0], 5, [0, 0], 5, CurveIntersection::Coincident),
        ([0, 0], 0, [0, 0], 0, one([0, 0], false)),
        ([0, 0], 5, [5, 0], 0, one([5, 0], false)),
        ([0, 0], 5, [1, 0], 5, two([1, -5], [1, 5], true)),
        ([0, 0], 5, [-1, 0], 5, two([-1, -5], [-1, 5], true)),
        ([0, 0], 1, [1, 1], 1, two([0, 1], [1, 0], false)),
        (
            [-MAX_COORDINATE / 2, 0],
            MAX_COORDINATE / 2,
            [MAX_COORDINATE / 2, 0],
            MAX_COORDINATE / 2,
            tangent([0, 0]),
        ),
        (
            [0, 0],
            MAX_COORDINATE,
            [MAX_COORDINATE - 1, 0],
            1,
            tangent([MAX_COORDINATE, 0]),
        ),
    ];
    for (c1, r1, c2, r2, expected) in cases {
        let (a, b) = (circle(c1, r1), circle(c2, r2));
        for (a, b) in [(a, b), (b, a)] {
            assert_eq!(
                circle_intersection(a, b, |_| Ok::<_, ()>(())),
                Ok(expected),
                "{a:?} {b:?}"
            );
        }
    }
    assert_eq!(Circle::new(point([0, 0]), -1), Err(Error::Range));
    assert_eq!(
        Circle::new(point([MAX_COORDINATE, 0]), 1),
        Err(Error::Range)
    );
}

#[test]
fn rounded_roots_are_the_same_after_reversing_small_and_large_inputs() {
    for scale in [1, MAX_COORDINATE / 8] {
        let circles = [
            circle([0, 0], 3 * scale),
            circle([scale, scale], 4 * scale),
            circle([-scale, 0], 2 * scale),
        ];
        for x in -5..=5 {
            for y in -5..=5 {
                let forward = segment([x * scale, y * scale], [y * scale, -x * scale]);
                let reverse = Segment {
                    start: forward.end,
                    end: forward.start,
                };
                for circle in circles {
                    assert_eq!(
                        segment_circle_intersection(forward, circle, |_| Ok::<_, ()>(())),
                        segment_circle_intersection(reverse, circle, |_| Ok::<_, ()>(()))
                    );
                }
            }
        }
        for a in circles {
            for b in circles {
                assert_eq!(
                    circle_intersection(a, b, |_| Ok::<_, ()>(())),
                    circle_intersection(b, a, |_| Ok::<_, ()>(()))
                );
            }
        }
    }
}

#[test]
fn root_rounding_and_output_stop_before_returning_a_partial_result() {
    let line = segment([-10, -10], [10, 10]);
    let circle = circle([0, 0], 5);
    let mut charges = Vec::new();
    segment_circle_intersection(line, circle, |charge| {
        charges.push(charge);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(
        charges,
        [
            Charge::Work(1),
            Charge::Work(86),
            Charge::Work(86),
            Charge::Points(2)
        ]
    );
    for stop_at in 0..charges.len() {
        let mut calls = 0;
        let found = segment_circle_intersection(line, circle, |_| {
            let at = calls;
            calls += 1;
            if at == stop_at { Err(at) } else { Ok(()) }
        });
        assert_eq!(found, Err(Failure::Stopped(stop_at)));
        assert_eq!(calls, stop_at + 1);
    }
}
