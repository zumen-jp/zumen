//! 作図の距離・接線・向き、成立しない条件と予算を検査する。
#![allow(clippy::unwrap_used)] // 合成入力と数学的な期待値を使う。

use zumen_geom2d::topology::*;

fn p([x, y]: [i64; 2]) -> Point {
    Point::new(x, y).unwrap()
}
fn ray([x, y]: [i64; 2]) -> Ray {
    Ray::new(x, y).unwrap()
}
fn line(a: [i64; 2], b: [i64; 2]) -> Segment {
    Segment {
        start: p(a),
        end: p(b),
    }
}
fn corner(a: [i64; 2], b: [i64; 2]) -> Corner {
    Corner::new(p([0, 0]), p(a), p(b)).unwrap()
}
fn free(_: Charge) -> Result<(), ()> {
    Ok(())
}
fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}

#[test]
fn line_offsets_keep_length_direction_and_signed_distance() {
    let input = line([0, 0], [3, 4]);
    let found = offset_segment(input, 5.0, free).unwrap();
    assert_eq!(found.start.coordinates, [-4.0, 3.0]);
    assert_eq!(found.end.coordinates, [-1.0, 7.0]);
    assert!(!found.start.nearest.rounded);
    let reverse = offset_segment(line([3, 4], [0, 0]), -5.0, free).unwrap();
    assert_eq!(reverse.start, found.end);
    assert_eq!(reverse.end, found.start);
    let diagonal = offset_segment(line([0, 0], [10, 10]), 1.0, free).unwrap();
    let [x, y] = diagonal.start.coordinates;
    near(libm::hypot(x, y), 1.0);
    near(x + y, 0.0);
    assert!(diagonal.start.nearest.rounded);
    assert_eq!(diagonal.start.nearest.point, p([-1, 1]));
    let grid = Grid::new([100.0, -100.0], 2.0).unwrap();
    let [wx, wy] = diagonal.start.position(grid).unwrap();
    near(wx, 100.0 + x / 2.0);
    near(wy, -100.0 + y / 2.0);
}

#[test]
fn circle_and_arc_offsets_keep_the_center_and_angular_range() {
    let circle = Circle::new(p([0, 0]), 5).unwrap();
    assert_eq!(offset_circle(circle, -5.0, free).unwrap().radius, 0.0);
    assert_eq!(
        offset_circle(circle, -6.0, free),
        Err(Failure::Geometry(Error::NoSolution))
    );
    let input = Arc::new(circle, Ray::X, ray([0, 1]), Direction::CounterClockwise).unwrap();
    let found = offset_arc(input, 1.0, free).unwrap();
    assert_eq!(found.circle.radius, 4.0);
    assert_eq!(found.start.coordinates, [4.0, 0.0]);
    assert_eq!(found.end.coordinates, [0.0, 4.0]);
    assert_eq!(found.start_degrees, 0.0);
    assert_eq!(found.sweep_degrees, 90.0);
    let reverse = offset_arc(input.reversed(), -1.0, free).unwrap();
    assert_eq!(reverse.circle, found.circle);
    assert_eq!(reverse.start, found.end);
    assert_eq!(reverse.end, found.start);
    assert_eq!(reverse.sweep_degrees, -90.0);
    let full = Arc::full(circle, ray([3, 4]), Direction::Clockwise).unwrap();
    let found = offset_arc(full, 0.0, free).unwrap();
    assert_eq!(found.sweep_degrees, -360.0);
    assert_eq!(found.start, found.end);
    assert_eq!(found.start.coordinates, [3.0, 4.0]);
    assert_eq!(
        offset_arc(input, 5.0, free),
        Err(Failure::Geometry(Error::NoSolution))
    );
}

#[test]
fn chamfer_keeps_the_far_ends_and_removes_exhausted_segments() {
    let input = corner([10, 0], [0, 10]);
    let found = chamfer(input, [3.0, 4.0], free).unwrap();
    assert_eq!(found.join.start.coordinates, [3.0, 0.0]);
    assert_eq!(found.join.end.coordinates, [0.0, 4.0]);
    assert_eq!(found.kept[0].unwrap().start.coordinates, [10.0, 0.0]);
    assert_eq!(found.kept[1].unwrap().end.coordinates, [0.0, 10.0]);
    let exhausted = chamfer(input, [10.0, 10.0], free).unwrap();
    assert_eq!(exhausted.kept, [None, None]);
    assert_eq!(
        chamfer(input, [10.01, 1.0], free),
        Err(Failure::Geometry(Error::NoSolution))
    );
    assert_eq!(
        chamfer(input, [0.0, 1.0], free),
        Err(Failure::Geometry(Error::NoSolution))
    );
    assert_eq!(
        chamfer(input, [0.1, 0.1], free),
        Err(Failure::Geometry(Error::Precision))
    );
}

#[test]
fn fillet_is_tangent_to_both_lines_and_reverses_with_the_corner() {
    let input = corner([10, 0], [0, 10]);
    let found = fillet(input, 2.0, free).unwrap();
    assert_eq!(found.arc.circle.center.coordinates, [2.0, 2.0]);
    assert_eq!(found.arc.circle.radius, 2.0);
    assert_eq!(found.arc.start.coordinates, [2.0, 0.0]);
    assert_eq!(found.arc.end.coordinates, [0.0, 2.0]);
    assert_eq!(found.arc.start_degrees, -90.0);
    assert_eq!(found.arc.sweep_degrees, -90.0);
    let reverse = fillet(corner([0, 10], [10, 0]), 2.0, free).unwrap();
    assert_eq!(reverse.arc.circle, found.arc.circle);
    assert_eq!(reverse.arc.start, found.arc.end);
    assert_eq!(reverse.arc.end, found.arc.start);
    assert_eq!(reverse.arc.sweep_degrees, 90.0);
    let oblique = fillet(corner([5, 0], [3, 4]), 1.0, free).unwrap();
    assert_eq!(oblique.arc.circle.center.coordinates, [2.0, 1.0]);
    assert_eq!(oblique.arc.start.coordinates, [2.0, 0.0]);
    assert_eq!(oblique.arc.end.coordinates, [1.2, 1.6]);
    near(
        (oblique.arc.end.coordinates[0] - 2.0) * 0.6 + (oblique.arc.end.coordinates[1] - 1.0) * 0.8,
        0.0,
    );
    let rotated = fillet(corner([30, 40], [-40, 30]), 5.0, free).unwrap();
    assert_eq!(rotated.arc.circle.center.coordinates, [-1.0, 7.0]);
    assert_eq!(rotated.arc.start.coordinates, [3.0, 4.0]);
    assert_eq!(rotated.arc.end.coordinates, [-4.0, 3.0]);
    let all = fillet(input, 10.0, free).unwrap();
    assert_eq!(all.kept, [None, None]);
    assert_eq!(
        fillet(input, 10.01, free),
        Err(Failure::Geometry(Error::NoSolution))
    );
}

#[test]
fn nearly_opposite_rays_avoid_subtracting_near_equal_values() {
    let n = MAX_COORDINATE;
    let input = corner([n, 1], [-n, 1]);
    let found = fillet(input, n as f64 / 2.0, free).unwrap();
    assert!(found.arc.sweep_degrees < 0.0);
    assert!(found.arc.sweep_degrees.abs() < 1e-6);
    near(found.arc.start.coordinates[0], 0.5);
    near(found.arc.end.coordinates[0], -0.5);
    assert_ne!(found.arc.start.nearest.point, found.arc.end.nearest.point);
    assert_eq!(
        fillet(corner([n, 1], [n, 2]), 1.0, free),
        Err(Failure::Geometry(Error::NoSolution))
    );
}

#[test]
fn invalid_geometry_and_every_budget_stop_are_explicit() {
    assert_eq!(
        Corner::new(p([0, 0]), p([0, 0]), p([1, 1])),
        Err(Error::Degenerate)
    );
    assert_eq!(
        Corner::new(p([0, 0]), p([1, 0]), p([-1, 0])),
        Err(Error::NoSolution)
    );
    assert_eq!(
        offset_segment(line([0, 0], [1, 0]), f64::NAN, free),
        Err(Failure::Geometry(Error::NonFinite))
    );
    assert_eq!(
        offset_segment(line([MAX_COORDINATE, 0], [MAX_COORDINATE, 1]), -1.0, free),
        Err(Failure::Geometry(Error::Range))
    );
    assert_eq!(
        fillet(corner([10, 0], [0, 10]), 0.0, free),
        Err(Failure::Geometry(Error::NoSolution))
    );
    for op in 0..5 {
        let run = |check: &mut dyn FnMut(Charge) -> Result<(), usize>| {
            let c = Circle::new(p([0, 0]), 5).unwrap();
            match op {
                0 => offset_segment(line([0, 0], [3, 4]), 1.0, check).map(|_| ()),
                1 => offset_circle(c, 1.0, check).map(|_| ()),
                2 => offset_arc(
                    Arc::new(c, Ray::X, ray([0, 1]), Direction::Clockwise).unwrap(),
                    1.0,
                    check,
                )
                .map(|_| ()),
                3 => chamfer(corner([10, 0], [0, 10]), [2.0, 3.0], check).map(|_| ()),
                _ => fillet(corner([10, 0], [0, 10]), 2.0, check).map(|_| ()),
            }
        };
        let mut amounts = Vec::new();
        run(&mut |amount| {
            amounts.push(amount);
            Ok(())
        })
        .unwrap();
        assert_eq!(amounts.len(), 2);
        assert!(matches!(amounts[0], Charge::Work(_)));
        assert!(matches!(amounts[1], Charge::Points(_)));
        for stop in 0..amounts.len() {
            let mut count = 0;
            assert_eq!(
                run(&mut |_| {
                    let at = count;
                    count += 1;
                    if at == stop { Err(at) } else { Ok(()) }
                }),
                Err(Failure::Stopped(stop))
            );
        }
    }
}
