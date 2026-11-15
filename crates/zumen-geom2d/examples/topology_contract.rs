//! 合成の整数格子・交点・予算停止の記録を、ネイティブと wasm で同じバイト列にする。
#![allow(clippy::unwrap_used)] // 固定した試験入力。失敗したら比較用の実行も失敗させる。

use std::io::{self, Write};
#[path = "topology_contract/construct.rs"]
mod construct;
#[path = "topology_contract/edit.rs"]
mod edit;
use zumen_geom2d::topology::{
    Arc, ArcIntersection, Charge, Circle, CurveIntersection, Direction, Error, Failure, Grid,
    MAX_COORDINATE, Point, Ray, Segment, SegmentIntersection, arc_intersection,
    circle_arc_intersection, circle_intersection, segment_arc_intersection,
    segment_circle_intersection, segment_intersection, segment_intersections,
};

fn number(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn point(out: &mut Vec<u8>, value: Point) {
    for coordinate in value.coordinates() {
        out.extend_from_slice(&coordinate.to_le_bytes());
    }
}

fn charge(out: &mut Vec<u8>, value: Charge) {
    let (kind, amount) = match value {
        Charge::Work(n) => (0, n),
        Charge::Points(n) => (1, n),
        Charge::Bytes(n) => (2, n),
        _ => (255, 0),
    };
    out.push(kind);
    number(out, amount);
}

fn error(out: &mut Vec<u8>, value: Error) {
    out.push(match value {
        Error::Tolerance => 0,
        Error::NonFinite => 1,
        Error::Range => 2,
        Error::Allocation => 3,
        Error::Degenerate => 4,
        Error::Ambiguous => 5,
        Error::Precision => 6,
        Error::NoSolution => 7,
        _ => 255,
    });
}

fn intersection(out: &mut Vec<u8>, value: SegmentIntersection) {
    match value {
        SegmentIntersection::None => out.push(0),
        SegmentIntersection::Point(hit) => {
            out.push(1);
            point(out, hit.point);
            out.push(u8::from(hit.rounded));
        }
        SegmentIntersection::Overlap(overlap) => {
            out.push(2);
            point(out, overlap.start);
            point(out, overlap.end);
        }
    }
}

fn curve(out: &mut Vec<u8>, value: CurveIntersection) {
    let points: &[zumen_geom2d::topology::Hit] = match &value {
        CurveIntersection::None => {
            out.push(0);
            &[]
        }
        CurveIntersection::Point(hit) => {
            out.push(1);
            std::slice::from_ref(hit)
        }
        CurveIntersection::Tangent(hit) => {
            out.push(4);
            std::slice::from_ref(hit)
        }
        CurveIntersection::Two(hits) => {
            out.push(2);
            hits
        }
        CurveIntersection::Coincident => {
            out.push(3);
            &[]
        }
    };
    for hit in points {
        point(out, hit.point);
        out.push(u8::from(hit.rounded));
    }
}

fn arc(out: &mut Vec<u8>, value: Arc) {
    point(out, value.circle().center());
    number(out, value.circle().radius() as u64);
    for ray in [value.start(), value.end()] {
        for component in ray.components() {
            out.extend_from_slice(&component.to_le_bytes());
        }
    }
    out.push(u8::from(value.direction() == Direction::Clockwise));
    out.push(u8::from(value.is_full()));
}

fn arc_result(out: &mut Vec<u8>, value: ArcIntersection) {
    let mut hit = |hit: zumen_geom2d::topology::Hit| {
        point(out, hit.point);
        out.push(u8::from(hit.rounded));
    };
    match value {
        ArcIntersection::None => out.push(0),
        ArcIntersection::Point(found) => {
            hit(found);
            out.push(1);
        }
        ArcIntersection::Tangent(found) => {
            hit(found);
            out.push(2);
        }
        ArcIntersection::Two(found) => {
            for found in found {
                hit(found);
            }
            out.push(3);
        }
        ArcIntersection::Overlap(found) => {
            arc(out, found);
            out.push(4);
        }
        ArcIntersection::OverlapAndPoint(found, point) => {
            hit(point);
            arc(out, found);
            out.push(5);
        }
        ArcIntersection::TwoOverlaps(found) => {
            for found in found {
                arc(out, found);
            }
            out.push(6);
        }
    }
}

fn arcs(out: &mut Vec<u8>, segments: &[Segment]) {
    out.extend_from_slice(b"arcs 1\n");
    let rays = [
        [1, 0],
        [3, 4],
        [0, 1],
        [-1, 1],
        [-1, 0],
        [-1, -1],
        [0, -1],
        [1, -1],
    ]
    .map(|[x, y]| Ray::new(x, y).unwrap());
    let main = Circle::new(Point::new(0, 0).unwrap(), 5).unwrap();
    let mut inputs = Vec::new();
    for start in rays {
        for end in rays {
            inputs.push(Arc::new(main, start, end, Direction::CounterClockwise).unwrap());
        }
    }
    for circle in [
        main,
        Circle::new(Point::new(6, 0).unwrap(), 5).unwrap(),
        Circle::new(Point::new(1, 0).unwrap(), 1).unwrap(),
        Circle::new(Point::new(0, 0).unwrap(), MAX_COORDINATE).unwrap(),
    ] {
        inputs.push(Arc::full(circle, Ray::X, Direction::Clockwise).unwrap());
        inputs.push(
            Arc::new(
                circle,
                Ray::new(MAX_COORDINATE, MAX_COORDINATE - 1).unwrap(),
                Ray::new(-MAX_COORDINATE, MAX_COORDINATE - 1).unwrap(),
                Direction::CounterClockwise,
            )
            .unwrap(),
        );
    }
    for &a in &inputs {
        arc(out, a);
        for endpoint in a
            .endpoints(|value| {
                charge(out, value);
                Ok::<_, ()>(())
            })
            .unwrap()
        {
            point(out, endpoint.point);
            out.push(u8::from(endpoint.rounded));
        }
        for &segment in segments {
            let found = segment_arc_intersection(segment, a, |value| {
                charge(out, value);
                Ok::<_, ()>(())
            })
            .unwrap();
            curve(out, found);
        }
        let found = circle_arc_intersection(main, a, |value| {
            charge(out, value);
            Ok::<_, ()>(())
        })
        .unwrap();
        arc_result(out, found);
        for &b in &inputs {
            let found = arc_intersection(a, b, |value| {
                charge(out, value);
                Ok::<_, ()>(())
            })
            .unwrap();
            assert_eq!(
                found,
                arc_intersection(b.reversed(), a.reversed(), |_| Ok::<_, ()>(())).unwrap()
            );
            arc_result(out, found);
        }
    }
    for b in inputs.iter().copied().step_by(7) {
        let mut calls = 0;
        let a = Arc::new(
            main,
            Ray::X,
            Ray::new(0, -1).unwrap(),
            Direction::CounterClockwise,
        )
        .unwrap();
        arc_intersection(a, b, |_| {
            calls += 1;
            Ok::<_, ()>(())
        })
        .unwrap();
        for stop in 0..=calls {
            number(out, stop);
            let mut count = 0;
            let found = arc_intersection(a, b, |value| {
                charge(out, value);
                let at = count;
                count += 1;
                if at == stop { Err(at) } else { Ok(()) }
            });
            match found {
                Ok(found) => {
                    out.push(0);
                    arc_result(out, found);
                }
                Err(Failure::Stopped(at)) => {
                    out.push(1);
                    number(out, at);
                }
                Err(Failure::Geometry(found)) => {
                    out.push(2);
                    error(out, found);
                }
            }
        }
    }
}

fn main() -> io::Result<()> {
    let mut out = b"zumen-geom2d topology 1\n".to_vec();
    for origin in [[0.0, 0.0], [100.0, -20.0], [1e100, 0.0]] {
        for tolerance in [1.0, 0.001, 0.0, f64::from_bits(1)] {
            match Grid::new(origin, tolerance) {
                Err(found) => {
                    out.push(0);
                    error(&mut out, found);
                }
                Ok(grid) => {
                    out.push(1);
                    number(&mut out, grid.spacing().to_bits());
                    for input in [
                        [-0.0, 0.0],
                        [0.125, -0.125],
                        [100.0, -20.0],
                        [1e100, 0.0],
                        [f64::INFINITY, 0.0],
                        [f64::NAN, 0.0],
                    ] {
                        match grid.snap(input) {
                            Err(found) => {
                                out.push(0);
                                error(&mut out, found);
                            }
                            Ok(snap) => {
                                out.push(1);
                                point(&mut out, snap.point);
                                for value in snap.offset {
                                    number(&mut out, value.to_bits());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let inputs = [
        [[0, 0], [10, 10]],
        [[0, 10], [10, 0]],
        [[0, 5], [10, 5]],
        [[3, 0], [3, 0]],
        [[0, 0], [6, 0]],
        [[3, 0], [9, 0]],
        [[9, 0], [12, 0]],
        [[0, 0], [3, 3]],
        [[0, 3], [3, 0]],
        [[-3, -3], [0, 0]],
        [[-3, 0], [0, -3]],
        [
            [-MAX_COORDINATE, -MAX_COORDINATE],
            [MAX_COORDINATE, MAX_COORDINATE],
        ],
        [
            [-MAX_COORDINATE, MAX_COORDINATE],
            [MAX_COORDINATE, -MAX_COORDINATE],
        ],
        [
            [MAX_COORDINATE, MAX_COORDINATE - 1],
            [-MAX_COORDINATE, -MAX_COORDINATE],
        ],
    ];
    let segments: Vec<_> = inputs
        .into_iter()
        .map(|[[x, y], [u, v]]| Segment {
            start: Point::new(x, y).unwrap(),
            end: Point::new(u, v).unwrap(),
        })
        .collect();
    for &a in &segments {
        for &b in &segments {
            let found = segment_intersection(a, b, |amount| {
                charge(&mut out, amount);
                Ok::<_, ()>(())
            })
            .unwrap();
            intersection(&mut out, found);
        }
    }
    // 最初から各段階で停止した記録と、最後まで終えた記録を、出力全体で照らす。
    let mut calls = 0_u64;
    segment_intersections(&segments, |_| {
        calls += 1;
        Ok::<_, ()>(())
    })
    .unwrap();
    for stop in 0..=calls {
        number(&mut out, stop);
        let mut count = 0;
        let found = segment_intersections(&segments, |amount| {
            charge(&mut out, amount);
            let at = count;
            count += 1;
            if at == stop { Err(at) } else { Ok(()) }
        });
        match found {
            Ok(pairs) => {
                out.push(0);
                number(&mut out, pairs.len() as u64);
                for found in pairs {
                    for index in found.pair {
                        number(&mut out, index);
                    }
                    intersection(&mut out, found.intersection);
                }
            }
            Err(Failure::Stopped(at)) => {
                out.push(1);
                number(&mut out, at);
            }
            Err(Failure::Geometry(found)) => {
                out.push(2);
                error(&mut out, found);
            }
        }
    }
    let large = vec![segments.first().copied().unwrap(); 8193];
    let found = segment_intersections(&large, |amount| {
        charge(&mut out, amount);
        Ok::<_, ()>(())
    });
    assert_eq!(found, Err(Failure::Geometry(Error::Range)));
    error(&mut out, Error::Range);

    out.extend_from_slice(b"circles 1\n");
    let circles: Vec<_> = [
        ([0, 0], 0),
        ([0, 0], 1),
        ([0, 0], 2),
        ([3, 4], 3),
        ([0, 0], 5),
        ([1, 0], 5),
        ([-1, 0], 5),
        ([6, 0], 5),
        ([10, 0], 5),
        ([2, 0], 3),
        ([5, 0], 0),
        ([1, 1], 1),
        ([0, 0], MAX_COORDINATE),
        ([MAX_COORDINATE - 1, 0], 1),
        ([-MAX_COORDINATE / 2, 0], MAX_COORDINATE / 2),
        ([MAX_COORDINATE / 2, 0], MAX_COORDINATE / 2),
    ]
    .into_iter()
    .map(|([x, y], radius)| Circle::new(Point::new(x, y).unwrap(), radius).unwrap())
    .collect();
    for &circle in &circles {
        for &segment in &segments {
            let found = segment_circle_intersection(segment, circle, |amount| {
                charge(&mut out, amount);
                Ok::<_, ()>(())
            })
            .unwrap();
            curve(&mut out, found);
        }
        for &other in &circles {
            let found = circle_intersection(circle, other, |amount| {
                charge(&mut out, amount);
                Ok::<_, ()>(())
            })
            .unwrap();
            curve(&mut out, found);
        }
    }
    let first = Circle::new(Point::new(0, 0).unwrap(), 5).unwrap();
    let second = Circle::new(Point::new(1, 0).unwrap(), 5).unwrap();
    for stop in 0..=4_u64 {
        number(&mut out, stop);
        let mut calls = 0;
        let found = circle_intersection(first, second, |amount| {
            charge(&mut out, amount);
            let at = calls;
            calls += 1;
            if at == stop { Err(at) } else { Ok(()) }
        });
        match found {
            Ok(found) => {
                out.push(0);
                curve(&mut out, found);
            }
            Err(Failure::Stopped(at)) => {
                out.push(1);
                number(&mut out, at);
            }
            Err(Failure::Geometry(found)) => {
                out.push(2);
                error(&mut out, found);
            }
        }
    }
    arcs(&mut out, &segments);
    edit::record(&mut out);
    construct::record(&mut out);
    io::stdout().lock().write_all(&out)
}
