//! 整数格子の交点・接触・重なりと、丸め・予算・範囲の契約。

use zumen_geom2d::topology::{
    Charge, Error, Failure, Grid, Hit, MAX_COORDINATE, PairIntersection, Point, Segment,
    SegmentIntersection, segment_intersection, segment_intersections,
};

#[cfg(test)]
fn point(x: i64, y: i64) -> Point {
    Point::new(x, y).unwrap()
}

fn segment(a: [i64; 2], b: [i64; 2]) -> Segment {
    let [x, y] = a;
    let [u, v] = b;
    Segment {
        start: point(x, y),
        end: point(u, v),
    }
}

fn hit(x: i64, y: i64, rounded: bool) -> SegmentIntersection {
    SegmentIntersection::Point(Hit {
        point: point(x, y),
        rounded,
    })
}

fn reverse(s: Segment) -> Segment {
    Segment {
        start: s.end,
        end: s.start,
    }
}

#[test]
fn crossings_contacts_overlaps_and_points_have_fixed_results() {
    let cases = [
        ([0, 0], [8, 8], [0, 8], [8, 0], hit(4, 4, false)),
        ([0, 0], [3, 3], [0, 3], [3, 0], hit(2, 2, true)),
        ([-3, -3], [0, 0], [-3, 0], [0, -3], hit(-2, -2, true)),
        ([0, 0], [6, 0], [3, 0], [3, 5], hit(3, 0, false)),
        ([0, 0], [6, 0], [6, 0], [9, 0], hit(6, 0, false)),
        (
            [0, 0],
            [6, 0],
            [3, 0],
            [9, 0],
            SegmentIntersection::Overlap(segment([3, 0], [6, 0])),
        ),
        (
            [2, 8],
            [2, -8],
            [2, 3],
            [2, -3],
            SegmentIntersection::Overlap(segment([2, -3], [2, 3])),
        ),
        ([0, 0], [6, 0], [7, 0], [9, 0], SegmentIntersection::None),
        ([0, 0], [6, 0], [0, 1], [6, 1], SegmentIntersection::None),
        ([0, 0], [1, 1], [2, 0], [2, 2], SegmentIntersection::None),
        ([3, 0], [3, 0], [0, 0], [6, 0], hit(3, 0, false)),
        ([3, 1], [3, 1], [0, 0], [6, 0], SegmentIntersection::None),
        ([3, 1], [3, 1], [3, 1], [3, 1], hit(3, 1, false)),
        ([3, 1], [3, 1], [3, 2], [3, 2], SegmentIntersection::None),
        (
            [-MAX_COORDINATE, -MAX_COORDINATE],
            [MAX_COORDINATE, MAX_COORDINATE],
            [-MAX_COORDINATE, MAX_COORDINATE],
            [MAX_COORDINATE, -MAX_COORDINATE],
            hit(0, 0, false),
        ),
        (
            [-MAX_COORDINATE, -MAX_COORDINATE],
            [MAX_COORDINATE, MAX_COORDINATE - 1],
            [-MAX_COORDINATE, MAX_COORDINATE],
            [MAX_COORDINATE, -MAX_COORDINATE],
            hit(0, 0, true),
        ),
        (
            [MAX_COORDINATE - 3, MAX_COORDINATE - 3],
            [MAX_COORDINATE, MAX_COORDINATE],
            [MAX_COORDINATE - 3, MAX_COORDINATE],
            [MAX_COORDINATE, MAX_COORDINATE - 3],
            hit(MAX_COORDINATE - 1, MAX_COORDINATE - 1, true),
        ),
        (
            [-MAX_COORDINATE, -MAX_COORDINATE],
            [-MAX_COORDINATE + 3, -MAX_COORDINATE + 3],
            [-MAX_COORDINATE, -MAX_COORDINATE + 3],
            [-MAX_COORDINATE + 3, -MAX_COORDINATE],
            hit(-MAX_COORDINATE + 1, -MAX_COORDINATE + 1, true),
        ),
    ];
    for (a, b, c, d, expected) in cases {
        let (first, second) = (segment(a, b), segment(c, d));
        for a in [first, reverse(first)] {
            for b in [second, reverse(second)] {
                for (a, b) in [(a, b), (b, a)] {
                    let mut charges = Vec::new();
                    let result = segment_intersection(a, b, |charge| {
                        charges.push(charge);
                        Ok::<_, ()>(())
                    });
                    assert_eq!(result, Ok(expected), "{a:?} {b:?}");
                    assert_eq!(charges.first(), Some(&Charge::Work(1)));
                    assert_eq!(charges.len(), 2);
                }
            }
        }
    }
}

#[test]
fn grid_reports_displacement_and_rejects_unrepresentable_coordinates() {
    let grid = Grid::new([100.0, -20.0], 1.0).unwrap();
    assert_eq!(grid.spacing(), 0.25);
    assert_eq!(grid.origin(), [100.0, -20.0]);
    assert_eq!(grid.tolerance(), 1.0);
    let positive = grid.snap([100.125, -19.875]).unwrap();
    assert_eq!(positive.point, point(1, 1));
    assert_eq!(positive.offset, [0.125, 0.125]);
    let negative = grid.snap([99.875, -20.125]).unwrap();
    assert_eq!(negative.point, point(-1, -1));
    assert_eq!(negative.offset, [-0.125, -0.125]);
    assert_eq!(grid.snap([100.0, -20.0]).unwrap().offset, [0.0, 0.0]);
    for tolerance in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::from_bits(1)] {
        assert_eq!(Grid::new([0.0, 0.0], tolerance), Err(Error::Tolerance));
    }
    assert_eq!(Grid::new([f64::NAN, 0.0], 1.0), Err(Error::NonFinite));
    assert_eq!(grid.snap([f64::INFINITY, 0.0]), Err(Error::NonFinite));
    assert_eq!(grid.snap([f64::MAX, 0.0]), Err(Error::Range));
    assert_eq!(Point::new(i64::MIN, 0), Err(Error::Range));
    assert_eq!(Point::new(MAX_COORDINATE + 1, 0), Err(Error::Range));
    let too_fine = Grid::new([1e100, 0.0], 1.0).unwrap();
    assert_eq!(too_fine.position(point(1, 0)), Err(Error::Range));
    let too_large = Grid::new([f64::MAX, 0.0], f64::MAX).unwrap();
    assert_eq!(too_large.position(point(1, 0)), Err(Error::Range));
    let zero = Grid::new([-0.0, -0.0], 1.0)
        .unwrap()
        .position(point(0, 0))
        .unwrap();
    assert_eq!(zero.map(f64::to_bits), [0, 0]);
}

#[test]
fn grid_crossings_are_symmetric_even_when_rounded() {
    let mut segments = Vec::new();
    for x in -2..=2 {
        for y in -2..=2 {
            segments.push(segment([x, y], [-x, -y]));
            segments.push(segment([x, y], [y, x]));
        }
    }
    for &a in &segments {
        for &b in &segments {
            let direct = segment_intersection(a, b, |_| Ok::<_, ()>(())).unwrap();
            for (a, b) in [(b, a), (reverse(a), b), (a, reverse(b))] {
                assert_eq!(
                    segment_intersection(a, b, |_| Ok::<_, ()>(())).unwrap(),
                    direct
                );
            }
        }
    }
}

#[test]
fn batch_checks_work_and_memory_before_reserving_and_has_no_partial_result() {
    let segments = [
        segment([0, 0], [10, 10]),
        segment([0, 10], [10, 0]),
        segment([0, 5], [10, 5]),
    ];
    let expected = (0..3)
        .flat_map(|a| {
            ((a + 1)..3).map(move |b| PairIntersection {
                pair: [a, b],
                intersection: hit(5, 5, false),
            })
        })
        .collect::<Vec<_>>();
    let mut charges = Vec::new();
    assert_eq!(
        segment_intersections(&segments, |charge| {
            charges.push(charge);
            Ok::<_, ()>(())
        }),
        Ok(expected)
    );
    assert_eq!(
        charges,
        [
            Charge::Work(6),
            Charge::Bytes(192),
            Charge::Work(0),
            Charge::Points(1),
            Charge::Work(0),
            Charge::Points(1),
            Charge::Work(0),
            Charge::Points(1)
        ]
    );
    assert!(size_of::<PairIntersection>() <= 64);
    for stop_at in 0..charges.len() {
        let mut calls = 0;
        let result = segment_intersections(&segments, |_| {
            let current = calls;
            calls += 1;
            if current == stop_at {
                Err(stop_at)
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(Failure::Stopped(stop_at)));
        assert_eq!(calls, stop_at + 1);
    }
    assert_eq!(
        segment_intersections(&[], |_| Ok::<_, ()>(())),
        Ok(Vec::new())
    );
}

#[test]
fn batch_allocation_range_is_the_same_on_32_and_64_bit_targets() {
    let segments = vec![segment([0, 0], [1, 1]); 8193];
    let mut charges = Vec::new();
    assert_eq!(
        segment_intersections(&segments, |charge| {
            charges.push(charge);
            Ok::<_, ()>(())
        }),
        Err(Failure::Geometry(Error::Range))
    );
    // 全組のバイト数が 32 bit の上限を超えたら、どの環境でも確保を要求する前に止める。
    assert_eq!(charges, [Charge::Work(8193 + 8193 * 8192 / 2)]);
}
