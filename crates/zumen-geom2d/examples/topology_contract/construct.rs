//! 作図した座標・角度・格子への丸めと予算停止を照らす。

use super::{charge, error, number, point};
use zumen_geom2d::topology::*;

fn position(out: &mut Vec<u8>, value: ConstructionPoint) {
    for coordinate in value.coordinates {
        number(out, coordinate.to_bits());
    }
    point(out, value.nearest.point);
    out.push(u8::from(value.nearest.rounded));
    for coordinate in value
        .position(Grid::new([50.0, -20.0], 0.001).unwrap())
        .unwrap()
    {
        number(out, coordinate.to_bits());
    }
}
fn line(out: &mut Vec<u8>, value: LineGeometry) {
    position(out, value.start);
    position(out, value.end);
}
fn circle(out: &mut Vec<u8>, value: CircleGeometry) {
    position(out, value.center);
    number(out, value.radius.to_bits());
}
fn arc(out: &mut Vec<u8>, value: ArcGeometry) {
    circle(out, value.circle);
    position(out, value.start);
    position(out, value.end);
    number(out, value.start_degrees.to_bits());
    number(out, value.sweep_degrees.to_bits());
}
fn kept(out: &mut Vec<u8>, values: [Option<LineGeometry>; 2]) {
    for value in values {
        if let Some(value) = value {
            out.push(1);
            line(out, value);
        } else {
            out.push(0);
        }
    }
}

#[derive(Clone, Copy)]
enum Case {
    Line(Segment, f64),
    Circle(Circle, f64),
    Arc(Arc, f64),
    Chamfer(Corner, [f64; 2]),
    Fillet(Corner, f64),
}

fn run(case: Case, stop: Option<u64>, out: &mut Vec<u8>) {
    let mut count = 0;
    let mut check = |amount| {
        charge(out, amount);
        let at = count;
        count += 1;
        if Some(at) == stop { Err(at) } else { Ok(()) }
    };
    let result = match case {
        Case::Line(source, distance) => {
            offset_segment(source, distance, &mut check).map(|value| line(out, value))
        }
        Case::Circle(source, distance) => {
            offset_circle(source, distance, &mut check).map(|value| circle(out, value))
        }
        Case::Arc(source, distance) => {
            offset_arc(source, distance, &mut check).map(|value| arc(out, value))
        }
        Case::Chamfer(source, distances) => chamfer(source, distances, &mut check).map(|value| {
            line(out, value.join);
            kept(out, value.kept);
        }),
        Case::Fillet(source, radius) => fillet(source, radius, &mut check).map(|value| {
            arc(out, value.arc);
            kept(out, value.kept);
        }),
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
}

pub(super) fn record(out: &mut Vec<u8>) {
    out.extend_from_slice(b"construction 1\n");
    let p = |[x, y]: [i64; 2]| Point::new(x, y).unwrap();
    let ray = |[x, y]: [i64; 2]| Ray::new(x, y).unwrap();
    let circle = Circle::new(p([0, 0]), 5).unwrap();
    let distances = [
        -10.0,
        -5.0,
        -1.0,
        -0.0,
        0.01,
        1.0,
        2.5,
        5.0,
        10.0,
        f64::NAN,
        f64::INFINITY,
    ];
    for distance in distances {
        for [a, b] in [
            [[0, 0], [3, 4]],
            [[0, 0], [10, 10]],
            [[10, 10], [0, 0]],
            [[0, 0], [0, 0]],
            [
                [-MAX_COORDINATE, -MAX_COORDINATE],
                [MAX_COORDINATE, MAX_COORDINATE],
            ],
        ] {
            run(
                Case::Line(
                    Segment {
                        start: p(a),
                        end: p(b),
                    },
                    distance,
                ),
                None,
                out,
            );
        }
        run(Case::Circle(circle, distance), None, out);
        for [a, b] in [
            [[1, 0], [0, 1]],
            [[3, 4], [-1, -1]],
            [[1, 1], [1, 1]],
            [[MAX_COORDINATE, 1], [-MAX_COORDINATE, 1]],
        ] {
            let input = Arc::new(circle, ray(a), ray(b), Direction::CounterClockwise).unwrap();
            run(Case::Arc(input, distance), None, out);
            run(Case::Arc(input.reversed(), distance), None, out);
        }
        run(
            Case::Arc(
                Arc::full(circle, ray([3, 4]), Direction::Clockwise).unwrap(),
                distance,
            ),
            None,
            out,
        );
    }
    for [vertex, first, second] in [
        [[0, 0], [10, 0], [0, 10]],
        [[0, 0], [0, 10], [10, 0]],
        [[0, 0], [5, 0], [3, 4]],
        [[0, 0], [30, 40], [-40, 30]],
        [[100, 100], [130, 140], [60, 130]],
        [[0, 0], [MAX_COORDINATE, 1], [-MAX_COORDINATE, 1]],
        [[0, 0], [MAX_COORDINATE, 1], [MAX_COORDINATE, 2]],
    ] {
        let corner = Corner::new(p(vertex), p(first), p(second)).unwrap();
        for distance in distances.into_iter().chain([MAX_COORDINATE as f64 / 2.0]) {
            run(Case::Fillet(corner, distance), None, out);
            for second in [distance, 1.0, 10.0] {
                run(Case::Chamfer(corner, [distance, second]), None, out);
            }
        }
    }
    let corner = Corner::new(p([0, 0]), p([10, 0]), p([0, 10])).unwrap();
    for case in [
        Case::Line(
            Segment {
                start: p([0, 0]),
                end: p([3, 4]),
            },
            1.0,
        ),
        Case::Circle(circle, 1.0),
        Case::Arc(
            Arc::new(circle, Ray::X, ray([0, 1]), Direction::Clockwise).unwrap(),
            1.0,
        ),
        Case::Fillet(corner, 2.0),
        Case::Chamfer(corner, [2.0, 3.0]),
    ] {
        for stop in 0..=2 {
            run(case, Some(stop), out);
        }
    }
}
