//! 編集の区間・出所・丸めと、予算停止を同じ記録へ加える。

use super::{arc, charge, error, number, point};
use zumen_geom2d::topology::*;

fn segment(out: &mut Vec<u8>, value: SegmentPiece) {
    point(out, value.source().start);
    point(out, value.source().end);
    for pair in value.parameters() {
        for value in pair {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    for hit in value
        .endpoints(|value| {
            charge(out, value);
            Ok::<_, ()>(())
        })
        .unwrap()
    {
        point(out, hit.point);
        out.push(u8::from(hit.rounded));
    }
    for pair in value
        .positions(Grid::new([0.0, 0.0], 4.0).unwrap(), |value| {
            charge(out, value);
            Ok::<_, ()>(())
        })
        .unwrap()
    {
        for value in pair {
            number(out, value.to_bits());
        }
    }
}

fn cut(out: &mut Vec<u8>, value: Option<Cut>) {
    if let Some(cut) = value {
        out.push(1);
        number(out, cut.boundary);
        point(out, cut.intersection.point);
        out.push(u8::from(cut.intersection.rounded));
        out.push(u8::from(cut.adjusted));
    } else {
        out.push(0);
    }
}

fn trim<T>(out: &mut Vec<u8>, value: Trimmed<T>, encode: impl Fn(&mut Vec<u8>, T)) {
    for value in value.kept.into_iter().chain([value.removed]) {
        if let Some(value) = value {
            out.push(1);
            encode(out, value);
        } else {
            out.push(0);
        }
    }
    for value in value.cuts {
        cut(out, value);
    }
}

#[derive(Clone, Copy)]
enum Case {
    TrimSegment(Segment, Point),
    ExtendSegment(Segment, End),
    TrimArc(Arc, Point),
    ExtendArc(Arc, End),
}

fn run(case: Case, boundaries: &[Boundary], stop: Option<u64>, out: &mut Vec<u8>) -> u64 {
    let mut count = 0;
    let mut check = |value| {
        charge(out, value);
        let at = count;
        count += 1;
        if Some(at) == stop { Err(at) } else { Ok(()) }
    };
    let result = match case {
        Case::TrimSegment(source, pick) => trim_segment(source, boundaries, pick, &mut check)
            .map(|value| trim(out, value, segment)),
        Case::ExtendSegment(source, end) => extend_segment(source, end, boundaries, &mut check)
            .map(|value| {
                segment(out, value.curve);
                cut(out, value.cut);
            }),
        Case::TrimArc(source, pick) => {
            trim_arc(source, boundaries, pick, &mut check).map(|value| trim(out, value, arc))
        }
        Case::ExtendArc(source, end) => {
            extend_arc(source, end, boundaries, &mut check).map(|value| {
                arc(out, value.curve);
                cut(out, value.cut);
            })
        }
    };
    match result {
        Ok(()) => out.push(0),
        Err(Failure::Geometry(value)) => {
            out.push(1);
            error(out, value);
        }
        Err(Failure::Stopped(at)) => {
            out.push(2);
            number(out, at);
        }
    }
    count
}

pub(super) fn record(out: &mut Vec<u8>) {
    out.extend_from_slice(b"edits 1\n");
    let p = |[x, y]: [i64; 2]| Point::new(x, y).unwrap();
    let line = |a, b| Segment {
        start: p(a),
        end: p(b),
    };
    let ray = |[x, y]: [i64; 2]| Ray::new(x, y).unwrap();
    let circle = |r| Circle::new(p([0, 0]), r).unwrap();
    let arc = |start, end| {
        Arc::new(circle(5), ray(start), ray(end), Direction::CounterClockwise).unwrap()
    };
    let boundaries = [
        vec![],
        vec![
            Boundary::Segment(line([3, -10], [3, 10])),
            Boundary::Segment(line([7, -10], [7, 10])),
        ],
        vec![Boundary::Circle(circle(5))],
        vec![
            Boundary::Circle(Circle::new(p([6, 0]), 5).unwrap()),
            Boundary::Arc(arc([-3, 4], [-3, 4])),
        ],
        vec![Boundary::Segment(line([2, 0], [5, 0]))],
        vec![Boundary::Circle(Circle::new(p([1, 0]), 1).unwrap())],
        vec![Boundary::Segment(line(
            [0, -MAX_COORDINATE],
            [0, MAX_COORDINATE],
        ))],
    ];
    let segments = [
        line([0, 0], [10, 0]),
        line([10, 0], [0, 0]),
        line([0, 0], [3, 0]),
        line([0, 0], [10, 3]),
        line([0, 0], [3, 1]),
        line([-10, 0], [10, 0]),
        line([0, 0], [0, 0]),
        line(
            [-MAX_COORDINATE, -MAX_COORDINATE],
            [MAX_COORDINATE, MAX_COORDINATE],
        ),
    ];
    for boundaries in &boundaries {
        for &source in &segments {
            for pick in [[0, 0], [3, 0], [5, 0], [1, 1]] {
                run(Case::TrimSegment(source, p(pick)), boundaries, None, out);
            }
            for end in [End::Start, End::End] {
                run(Case::ExtendSegment(source, end), boundaries, None, out);
            }
        }
        for source in [
            arc([1, 0], [-1, 0]),
            arc([1, 0], [0, 1]),
            arc([1, 0], [0, 1]).reversed(),
            Arc::full(circle(5), ray([1, 1]), Direction::Clockwise).unwrap(),
            arc([1, 0], [1, 0]),
            Arc::new(circle(1), Ray::X, ray([0, 1]), Direction::CounterClockwise).unwrap(),
            Arc::full(circle(MAX_COORDINATE), Ray::X, Direction::CounterClockwise).unwrap(),
        ] {
            for pick in [[1, 1], [0, 5], [0, -1], [0, 0]] {
                run(Case::TrimArc(source, p(pick)), boundaries, None, out);
            }
            for end in [End::Start, End::End] {
                run(Case::ExtendArc(source, end), boundaries, None, out);
            }
        }
    }
    let boundaries = [
        Boundary::Segment(line([-3, -10], [-3, 10])),
        Boundary::Segment(line([3, -10], [3, 10])),
    ];
    for case in [
        Case::TrimSegment(line([-10, 0], [10, 0]), p([0, 0])),
        Case::ExtendSegment(line([0, 0], [1, 0]), End::End),
        Case::TrimArc(arc([1, 0], [-1, 0]), p([0, 5])),
        Case::ExtendArc(arc([1, 0], [0, 1]), End::End),
    ] {
        let calls = run(case, &boundaries, None, out);
        for stop in 0..=calls {
            number(out, stop);
            run(case, &boundaries, Some(stop), out);
        }
    }
}
