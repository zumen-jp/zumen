//! 合成の整数格子・交点・予算停止の記録を、ネイティブと wasm で同じバイト列にする。
#![allow(clippy::unwrap_used)] // 固定した試験入力。失敗したら比較用の実行も失敗させる。

use std::io::{self, Write};
use zumen_geom2d::topology::{
    Charge, Error, Failure, Grid, MAX_COORDINATE, Point, Segment, SegmentIntersection,
    segment_intersection, segment_intersections,
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
    io::stdout().lock().write_all(&out)
}
