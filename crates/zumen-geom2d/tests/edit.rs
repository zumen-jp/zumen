//! トリムと延長で、元の線・円・向き・切断位置・予算停止を保つことを確かめる。
#![allow(clippy::unwrap_used)] // 合成した固定入力と結果を検査する。

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
fn circle(r: i64) -> Circle {
    Circle::new(p([0, 0]), r).unwrap()
}
fn arc(start: [i64; 2], end: [i64; 2]) -> Arc {
    Arc::new(circle(5), ray(start), ray(end), Direction::CounterClockwise).unwrap()
}
fn edge(x: i64) -> Boundary {
    Boundary::Segment(line([x, -10], [x, 10]))
}
fn free(_: Charge) -> Result<(), ()> {
    Ok(())
}
fn ends(piece: SegmentPiece) -> [[i64; 2]; 2] {
    piece
        .endpoints(free)
        .unwrap()
        .map(|hit| hit.point.coordinates())
}

#[test]
fn segment_trim_keeps_both_sides_and_original_direction() {
    let source = line([0, 0], [10, 0]);
    let boundaries = [edge(7), edge(3)];
    let found = trim_segment(source, &boundaries, p([5, 8]), free).unwrap();
    assert_eq!(ends(found.removed.unwrap()), [[3, 0], [7, 0]]);
    assert_eq!(ends(found.kept[0].unwrap()), [[0, 0], [3, 0]]);
    assert_eq!(ends(found.kept[1].unwrap()), [[7, 0], [10, 0]]);
    assert_eq!(found.cuts.map(|cut| cut.unwrap().boundary), [1, 0]);
    assert!(found.cuts.into_iter().all(|cut| !cut.unwrap().adjusted));
    let reverse = trim_segment(line([10, 0], [0, 0]), &boundaries, p([5, 8]), free).unwrap();
    assert_eq!(ends(reverse.removed.unwrap()), [[7, 0], [3, 0]]);
    assert_eq!(ends(reverse.kept[0].unwrap()), [[10, 0], [7, 0]]);
    assert_eq!(ends(reverse.kept[1].unwrap()), [[3, 0], [0, 0]]);
    let outer = trim_segment(source, &boundaries, p([0, 0]), free).unwrap();
    assert_eq!(ends(outer.removed.unwrap()), [[0, 0], [3, 0]]);
    assert_eq!(ends(outer.kept[0].unwrap()), [[3, 0], [10, 0]]);
    assert!(outer.kept[1].is_none());
    let no_cut = trim_segment(
        source,
        &[Boundary::Segment(line([11, 0], [20, 0]))],
        p([5, 0]),
        free,
    )
    .unwrap();
    assert!(no_cut.removed.is_none());
    assert_eq!(ends(no_cut.kept[0].unwrap()), [[0, 0], [10, 0]]);
}

#[test]
fn projected_cuts_preserve_the_carrier_instead_of_tilting_the_line() {
    let source = line([0, 0], [10, 3]);
    let found = trim_segment(source, &[edge(5)], p([1, 0]), free).unwrap();
    let kept = found.kept[0].unwrap();
    assert_eq!(kept.source(), source);
    assert_eq!(kept.parameters(), [[56, 109], [109, 109]]);
    assert_eq!(found.cuts[1].unwrap().intersection.point, p([5, 2]));
    assert!(found.cuts[1].unwrap().adjusted);
    let positions = kept
        .positions(Grid::new([0.0, 0.0], 4.0).unwrap(), free)
        .unwrap();
    let [x, y] = positions[0];
    assert!((x - 560.0 / 109.0).abs() < 1e-12);
    assert!((y - 168.0 / 109.0).abs() < 1e-12);
    assert!((3.0 * x - 10.0 * y).abs() < 1e-12);
    assert_eq!(positions[1], [10.0, 3.0]);
    assert_eq!(
        trim_segment(line([0, 0], [3, 1]), &[edge(1)], p([1, 0]), free),
        Err(Failure::Geometry(Error::Precision))
    );
}

#[test]
fn segment_extension_chooses_the_nearest_finite_boundary_on_either_side() {
    let source = line([0, 0], [3, 0]);
    let boundaries = [edge(7), edge(5), edge(-7), edge(-5)];
    let end = extend_segment(source, End::End, &boundaries, free).unwrap();
    assert_eq!(ends(end.curve), [[0, 0], [5, 0]]);
    assert_eq!(end.cut.unwrap().boundary, 1);
    let start = extend_segment(source, End::Start, &boundaries, free).unwrap();
    assert_eq!(ends(start.curve), [[-5, 0], [3, 0]]);
    assert_eq!(start.cut.unwrap().boundary, 3);
    let reverse = extend_segment(line([3, 0], [0, 0]), End::Start, &boundaries, free).unwrap();
    assert_eq!(ends(reverse.curve), [[5, 0], [0, 0]]);
    let missed = extend_segment(
        source,
        End::End,
        &[Boundary::Segment(line([5, 1], [5, 3]))],
        free,
    )
    .unwrap();
    assert_eq!(ends(missed.curve), [[0, 0], [3, 0]]);
    assert!(missed.cut.is_none());
    let round = extend_segment(source, End::End, &[Boundary::Circle(circle(5))], free).unwrap();
    assert_eq!(ends(round.curve), [[0, 0], [5, 0]]);
    let input = line([-10, 0], [10, 0]);
    let trim = trim_segment(
        input,
        &[Boundary::Arc(arc([1, 0], [-1, 0]))],
        p([0, 0]),
        free,
    )
    .unwrap();
    assert_eq!(ends(trim.removed.unwrap()), [[-5, 0], [5, 0]]);
}

#[test]
fn arc_trim_keeps_the_circle_and_handles_a_full_turn() {
    let source = arc([1, 0], [-1, 0]);
    let found = trim_arc(source, &[edge(3), edge(-3)], p([0, 5]), free).unwrap();
    assert_eq!(found.removed, Some(arc([3, 4], [-3, 4])));
    assert_eq!(
        found.kept,
        [Some(arc([1, 0], [3, 4])), Some(arc([-3, 4], [-1, 0]))]
    );
    let reverse = trim_arc(source.reversed(), &[edge(3), edge(-3)], p([0, 5]), free).unwrap();
    assert_eq!(reverse.removed, found.removed.map(Arc::reversed));
    assert_eq!(
        reverse.kept,
        [
            found.kept[1].map(Arc::reversed),
            found.kept[0].map(Arc::reversed)
        ]
    );
    let full = Arc::full(circle(5), ray([1, 1]), Direction::CounterClockwise).unwrap();
    let found = trim_arc(
        full,
        &[Boundary::Segment(line([-10, 0], [10, 0]))],
        p([0, 5]),
        free,
    )
    .unwrap();
    assert_eq!(found.removed, Some(arc([1, 0], [-1, 0])));
    assert_eq!(found.kept, [Some(arc([-1, 0], [1, 0])), None]);
    let one = trim_arc(
        full,
        &[Boundary::Segment(line([5, 0], [5, 0]))],
        p([0, 5]),
        free,
    )
    .unwrap();
    assert!(one.removed.is_none());
    assert_eq!(one.kept[0], Some(full));
    let unit = Arc::new(circle(1), Ray::X, ray([0, 1]), Direction::CounterClockwise).unwrap();
    assert_eq!(
        trim_arc(
            unit,
            &[Boundary::Circle(Circle::new(p([1, 0]), 1).unwrap())],
            p([1, 1]),
            free
        ),
        Err(Failure::Geometry(Error::Precision))
    );
}

#[test]
fn arc_extension_stops_at_the_first_contact_or_a_full_turn() {
    let source = arc([1, 0], [0, 1]);
    let found = extend_arc(source, End::End, &[edge(-3)], free).unwrap();
    assert_eq!(found.curve, arc([1, 0], [-3, 4]));
    let reverse = extend_arc(source.reversed(), End::Start, &[edge(-3)], free).unwrap();
    assert_eq!(reverse.curve, found.curve.reversed());
    let start = extend_arc(
        source,
        End::Start,
        &[Boundary::Segment(line([-10, -3], [10, -3]))],
        free,
    )
    .unwrap();
    assert_eq!(start.curve, arc([4, -3], [0, 1]));
    let full = extend_arc(
        source,
        End::End,
        &[Boundary::Segment(line([5, 0], [5, 0]))],
        free,
    )
    .unwrap();
    assert!(full.curve.is_full());
    assert_eq!(full.curve.start(), source.start());
    let full_start = extend_arc(
        source,
        End::Start,
        &[Boundary::Segment(line([0, 5], [0, 5]))],
        free,
    )
    .unwrap();
    assert!(full_start.curve.is_full());
    assert_eq!(full_start.curve.end(), source.end());
    assert_eq!(full_start.curve.start(), source.end());
    let none = extend_arc(source, End::End, &[edge(3)], free).unwrap();
    assert_eq!(none.curve, arc([1, 0], [3, -4]));
    let same_circle_point = Boundary::Arc(arc([-3, 4], [-3, 4]));
    assert_eq!(
        extend_arc(source, End::End, &[same_circle_point], free)
            .unwrap()
            .curve,
        arc([1, 0], [-3, 4])
    );
    let disjoint = trim_arc(
        source,
        &[Boundary::Arc(arc([-1, 0], [0, -1]))],
        p([1, 1]),
        free,
    )
    .unwrap();
    assert!(disjoint.removed.is_none());
}

#[test]
fn ambiguous_and_degenerate_inputs_do_not_produce_an_edit() {
    assert_eq!(
        SegmentPiece::new(line([0, 0], [0, 0])),
        Err(Error::Degenerate)
    );
    assert_eq!(
        trim_segment(line([0, 0], [10, 0]), &[edge(3)], p([3, 0]), free),
        Err(Failure::Geometry(Error::Ambiguous))
    );
    assert_eq!(
        trim_segment(
            line([0, 0], [10, 0]),
            &[Boundary::Segment(line([2, 0], [5, 0]))],
            p([6, 0]),
            free
        ),
        Err(Failure::Geometry(Error::Ambiguous))
    );
    assert_eq!(
        trim_arc(arc([1, 0], [1, 0]), &[], p([1, 0]), free),
        Err(Failure::Geometry(Error::Degenerate))
    );
    assert_eq!(
        trim_arc(
            arc([1, 0], [0, 1]),
            &[Boundary::Circle(circle(5))],
            p([1, 1]),
            free
        ),
        Err(Failure::Geometry(Error::Ambiguous))
    );
}

#[test]
fn all_edit_budget_checkpoints_abort_before_returning_a_result() {
    for operation in 0..4 {
        let mut amounts = Vec::new();
        let run = |check: &mut dyn FnMut(Charge) -> Result<(), usize>| match operation {
            0 => trim_segment(line([0, 0], [10, 0]), &[edge(3), edge(7)], p([5, 0]), check)
                .map(|_| ()),
            1 => extend_segment(line([0, 0], [3, 0]), End::End, &[edge(5), edge(7)], check)
                .map(|_| ()),
            2 => trim_arc(arc([1, 0], [-1, 0]), &[edge(3), edge(-3)], p([0, 5]), check).map(|_| ()),
            _ => extend_arc(arc([1, 0], [0, 1]), End::End, &[edge(-3)], check).map(|_| ()),
        };
        run(&mut |value| {
            amounts.push(value);
            Ok(())
        })
        .unwrap();
        assert!(
            !amounts
                .iter()
                .any(|amount| matches!(amount, Charge::Bytes(_)))
        );
        for stop in 0..amounts.len() {
            let mut calls = 0;
            let result = run(&mut |_| {
                let at = calls;
                calls += 1;
                if at == stop { Err(at) } else { Ok(()) }
            });
            assert_eq!(result, Err(Failure::Stopped(stop)));
            assert_eq!(calls, stop + 1);
        }
    }
}
