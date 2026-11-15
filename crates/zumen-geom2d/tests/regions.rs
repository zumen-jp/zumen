//! 閉領域の面積・穴・接触と、格子の丸め・入力位置・停止の契約を検査する。
#![allow(clippy::unwrap_used)] // 自作の合成入力と期待値。

use zumen_geom2d::topology::*;

fn p([x, y]: [i64; 2]) -> Point {
    Point::new(x, y).unwrap()
}
fn rect(x: i64, y: i64, w: i64, h: i64) -> Vec<Point> {
    [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
        .map(p)
        .to_vec()
}
fn line(a: [i64; 2], b: [i64; 2]) -> Segment {
    Segment {
        start: p(a),
        end: p(b),
    }
}
fn lines(path: &[Point]) -> Vec<Segment> {
    path.iter()
        .zip(path.iter().cycle().skip(1))
        .map(|(&start, &end)| Segment { start, end })
        .collect()
}
fn free(_: Charge) -> Result<(), ()> {
    Ok(())
}
fn signed_area(path: &[Point]) -> i128 {
    path.iter()
        .zip(path.iter().cycle().skip(1))
        .map(|(&a, &b)| {
            let [x, y] = a.coordinates().map(i128::from);
            let [u, v] = b.coordinates().map(i128::from);
            x * v - y * u
        })
        .sum()
}
fn area(result: &RegionResult) -> i128 {
    result
        .regions
        .iter()
        .map(|r| {
            assert!(signed_area(&r.outer) > 0);
            assert_eq!(r.outer.first(), r.outer.iter().min());
            assert!(
                r.holes
                    .iter()
                    .all(|p| signed_area(p) < 0 && p.first() == p.iter().min())
            );
            signed_area(&r.outer) + r.holes.iter().map(|p| signed_area(p)).sum::<i128>()
        })
        .sum::<i128>()
        / 2
}

#[test]
fn boolean_rectangles_have_expected_area_and_canonical_order() {
    let a = rect(0, 0, 10, 10);
    let b = rect(5, 5, 10, 10);
    for (op, expected) in [
        (RegionOperation::Union, 175),
        (RegionOperation::Intersection, 25),
        (RegionOperation::Difference, 75),
        (RegionOperation::Xor, 150),
    ] {
        let result = boolean_regions(&[&a], &[&b], op, RegionFill::NonZero, free).unwrap();
        assert_eq!(area(&result), expected);
        assert!(result.adjustments.is_empty());
        assert!(result.discarded.is_empty());
        for rotation in 0..4 {
            let mut reverse_a = a.clone();
            reverse_a.reverse();
            reverse_a.rotate_left(rotation);
            let mut reverse_b = b.clone();
            reverse_b.reverse();
            reverse_b.rotate_left(rotation);
            assert_eq!(
                boolean_regions(&[&reverse_a], &[&reverse_b], op, RegionFill::NonZero, free)
                    .unwrap(),
                result
            );
        }
    }
}

#[test]
fn holes_winding_touching_and_coincident_boundaries_are_explicit() {
    let outer = rect(-10, -10, 20, 20);
    let inner = rect(-5, -5, 10, 10);
    let difference = boolean_regions(
        &[&outer],
        &[&inner],
        RegionOperation::Difference,
        RegionFill::NonZero,
        free,
    )
    .unwrap();
    assert_eq!(area(&difference), 300);
    assert_eq!(difference.regions[0].holes.len(), 1);
    assert_eq!(
        difference,
        boolean_regions(
            &[&outer, &inner],
            &[],
            RegionOperation::Union,
            RegionFill::EvenOdd,
            free
        )
        .unwrap()
    );
    let filled = boolean_regions(
        &[&outer, &inner],
        &[],
        RegionOperation::Union,
        RegionFill::NonZero,
        free,
    )
    .unwrap();
    assert_eq!(area(&filled), 400);
    let reversed: Vec<_> = inner.iter().copied().rev().collect();
    assert_eq!(
        difference,
        boolean_regions(
            &[&outer, &reversed],
            &[],
            RegionOperation::Union,
            RegionFill::NonZero,
            free
        )
        .unwrap()
    );
    for b in [rect(10, -10, 20, 20), rect(10, 10, 20, 20), outer.clone()] {
        let intersection = boolean_regions(
            &[&outer],
            &[&b],
            RegionOperation::Intersection,
            RegionFill::EvenOdd,
            free,
        )
        .unwrap();
        assert_eq!(area(&intersection), if b == outer { 400 } else { 0 });
    }
    assert!(
        boolean_regions(
            &[&outer],
            &[&outer],
            RegionOperation::Xor,
            RegionFill::EvenOdd,
            free
        )
        .unwrap()
        .regions
        .is_empty()
    );
}

#[test]
fn detect_nested_faces_and_report_bridges_and_open_parts() {
    let outer = rect(-10, -10, 20, 20);
    let inner = rect(-5, -5, 10, 10);
    let mut input = lines(&outer);
    input.extend(lines(&inner));
    input.push(line([0, 5], [0, 10])); // 2 つの境界をつなぐ橋。
    input.push(line([0, 10], [0, 20])); // 外側の枝。
    let result = detect_regions(&input, free).unwrap();
    assert_eq!(area(&result), 400);
    assert_eq!(result.regions.len(), 2);
    assert_eq!(
        result.regions.iter().map(|r| r.holes.len()).sum::<usize>(),
        1
    );
    assert_eq!(
        result.discarded,
        [8, 9].map(|index| RegionDiscard::Open {
            source: RegionLocation {
                operand: RegionOperand::Lines,
                contour: 0,
                edge: Some(index)
            },
            segment: input[index as usize],
        })
    );
    let mut reverse = input
        .iter()
        .map(|s| Segment {
            start: s.end,
            end: s.start,
        })
        .collect::<Vec<_>>();
    reverse.reverse();
    assert_eq!(
        detect_regions(&reverse, free).unwrap().regions,
        result.regions
    );

    let mut split = lines(&rect(0, 0, 10, 10));
    split.push(line([-5, 5], [15, 5]));
    let result = detect_regions(&split, free).unwrap();
    assert_eq!(result.regions.len(), 2);
    assert_eq!(area(&result), 100);
    let discarded: Vec<_> = result
        .discarded
        .iter()
        .filter_map(|d| match d {
            RegionDiscard::Open { segment, .. } => Some(*segment),
            _ => None,
        })
        .collect();
    assert_eq!(discarded, [line([-5, 5], [0, 5]), line([10, 5], [15, 5])]);
}

#[test]
fn point_contacts_crosses_duplicates_and_self_crossings_keep_faces() {
    let mut input = lines(&rect(-10, -10, 10, 10));
    input.extend(lines(&rect(0, 0, 10, 10)));
    let result = detect_regions(&input, free).unwrap();
    assert_eq!(result.regions.len(), 2);
    assert_eq!(area(&result), 200);
    input.extend(input.clone());
    assert_eq!(
        detect_regions(&input, free).unwrap().regions,
        result.regions
    );
    let bow = [[0, 0], [10, 10], [0, 10], [10, 0]].map(p);
    let result = boolean_regions(
        &[&bow],
        &[],
        RegionOperation::Union,
        RegionFill::EvenOdd,
        free,
    )
    .unwrap();
    assert_eq!(result.regions.len(), 2);
    assert_eq!(area(&result), 50);
    assert!(result.discarded.is_empty());
    let mut input = lines(&rect(-10, -10, 20, 20));
    input.extend([line([-10, 0], [10, 0]), line([0, -10], [0, 10])]);
    let result = detect_regions(&input, free).unwrap();
    assert_eq!(result.regions.len(), 4);
    assert_eq!(area(&result), 400);
    assert!(result.discarded.is_empty());
}

#[test]
fn rounded_crossings_record_each_source_and_keep_the_total_area() {
    let mut input = lines(&rect(0, 0, 10, 10));
    input.extend([line([0, 3], [10, 4]), line([0, 8], [10, 0])]);
    let result = detect_regions(&input, free).unwrap();
    assert_eq!(result.regions.len(), 4);
    assert_eq!(area(&result), 100);
    assert_eq!(
        result.adjustments,
        [4, 5].map(|edge| RegionAdjustment {
            source: RegionLocation {
                operand: RegionOperand::Lines,
                contour: 0,
                edge: Some(edge)
            },
            point: p([6, 4]),
        })
    );
    assert!(result.discarded.is_empty());
}

#[test]
fn degenerate_inputs_and_extreme_coordinates_do_not_disappear_silently() {
    let flat = [[0, 0], [1, 1], [2, 2]].map(p);
    let result = boolean_regions(
        &[&[], &flat],
        &[],
        RegionOperation::Union,
        RegionFill::EvenOdd,
        free,
    )
    .unwrap();
    assert_eq!(
        result.discarded,
        [0, 1].map(|contour| RegionDiscard::Degenerate(RegionLocation {
            operand: RegionOperand::Subject,
            contour,
            edge: None
        }))
    );
    let mut closed = rect(0, 0, 10, 10);
    closed.push(closed[0]);
    assert!(
        boolean_regions(
            &[&closed],
            &[],
            RegionOperation::Union,
            RegionFill::NonZero,
            free
        )
        .unwrap()
        .discarded
        .is_empty()
    );
    let result = detect_regions(&[line([0, 0], [0, 0]), line([0, 0], [10, 0])], free).unwrap();
    assert_eq!(result.discarded.len(), 2);
    assert!(result.regions.is_empty());
    let limit = MAX_COORDINATE;
    let large = rect(-limit, -limit, 2 * limit, 2 * limit);
    let result = detect_regions(&lines(&large), free).unwrap();
    assert_eq!(result.regions[0].outer, large);
    assert_eq!(area(&result), 4 * i128::from(limit).pow(2));
}

#[test]
fn counts_normalized_edges_and_every_budget_stop_are_bounded() {
    assert_eq!(
        detect_regions(&vec![line([0, 0], [1, 0]); MAX_REGION_EDGES + 1], free),
        Err(Failure::Geometry(Error::Range))
    );
    assert_eq!(
        boolean_regions(
            &vec![&[] as &[Point]; MAX_REGION_EDGES + 1],
            &[],
            RegionOperation::Union,
            RegionFill::EvenOdd,
            free
        ),
        Err(Failure::Geometry(Error::Range))
    );
    let grid: Vec<_> = (0..64)
        .flat_map(|i| [line([i, -1], [i, 64]), line([-1, i], [64, i])])
        .collect();
    assert_eq!(
        detect_regions(&grid, free),
        Err(Failure::Geometry(Error::Range))
    );
    let input = lines(&rect(0, 0, 10, 10));
    let mut calls = 0;
    detect_regions(&input, |_| {
        calls += 1;
        Ok::<_, usize>(())
    })
    .unwrap();
    for stop in 0..calls {
        let mut at = 0;
        let result = detect_regions(&input, |_| {
            let current = at;
            at += 1;
            if current == stop { Err(stop) } else { Ok(()) }
        });
        assert_eq!(result, Err(Failure::Stopped(stop)));
        assert_eq!(at, stop + 1);
    }
    let a = rect(0, 0, 10, 10);
    let b = rect(5, 5, 10, 10);
    calls = 0;
    boolean_regions(
        &[&a],
        &[&b],
        RegionOperation::Xor,
        RegionFill::EvenOdd,
        |_| {
            calls += 1;
            Ok::<_, usize>(())
        },
    )
    .unwrap();
    for stop in 0..calls {
        let mut at = 0;
        assert_eq!(
            boolean_regions(
                &[&a],
                &[&b],
                RegionOperation::Xor,
                RegionFill::EvenOdd,
                |_| {
                    let current = at;
                    at += 1;
                    if current == stop { Err(stop) } else { Ok(()) }
                }
            ),
            Err(Failure::Stopped(stop))
        );
    }
}
