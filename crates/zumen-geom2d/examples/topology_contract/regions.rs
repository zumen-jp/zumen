//! 閉領域・丸め・破棄した部分と、固定の入力上限・予算停止を全環境で照らす。

use super::{charge, error, number, point};
use zumen_geom2d::topology::*;

fn location(out: &mut Vec<u8>, source: RegionLocation) {
    out.push(match source.operand {
        RegionOperand::Subject => 0,
        RegionOperand::Clip => 1,
        RegionOperand::Lines => 2,
    });
    number(out, source.contour);
    number(out, source.edge.unwrap_or(u64::MAX));
}
fn path(out: &mut Vec<u8>, points: &[Point]) {
    number(out, points.len() as u64);
    for &p in points {
        point(out, p);
    }
}
fn result(out: &mut Vec<u8>, value: RegionResult) {
    number(out, value.regions.len() as u64);
    for region in value.regions {
        path(out, &region.outer);
        number(out, region.holes.len() as u64);
        for hole in region.holes {
            path(out, &hole);
        }
    }
    number(out, value.adjustments.len() as u64);
    for adjustment in value.adjustments {
        location(out, adjustment.source);
        point(out, adjustment.point);
    }
    number(out, value.discarded.len() as u64);
    for discard in value.discarded {
        match discard {
            RegionDiscard::Degenerate(source) => {
                out.push(0);
                location(out, source);
            }
            RegionDiscard::Open { source, segment } => {
                out.push(1);
                location(out, source);
                point(out, segment.start);
                point(out, segment.end);
            }
        }
    }
}

fn run(
    out: &mut Vec<u8>,
    stop: Option<u64>,
    execute: impl FnOnce(
        &mut dyn FnMut(Charge) -> Result<(), u64>,
    ) -> Result<RegionResult, Failure<u64>>,
) -> u64 {
    let mut calls = 0;
    let mut check = |amount| {
        charge(out, amount);
        let at = calls;
        calls += 1;
        if stop == Some(at) { Err(at) } else { Ok(()) }
    };
    match execute(&mut check) {
        Ok(value) => {
            out.push(0);
            result(out, value);
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
    calls
}

fn p([x, y]: [i64; 2]) -> Point {
    Point::new(x, y).unwrap()
}
fn rect(x: i64, y: i64, w: i64, h: i64) -> Vec<Point> {
    [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]
        .map(p)
        .to_vec()
}
fn lines(paths: &[Vec<Point>]) -> Vec<Segment> {
    paths
        .iter()
        .flat_map(|path| {
            path.iter()
                .zip(path.iter().cycle().skip(1))
                .map(|(&start, &end)| Segment { start, end })
        })
        .collect()
}

pub(super) fn record(out: &mut Vec<u8>) {
    out.extend_from_slice(b"regions-v1");
    let m = MAX_COORDINATE;
    let cases = [
        (vec![rect(0, 0, 10, 10)], vec![rect(5, 5, 10, 10)]),
        (vec![rect(-10, -10, 20, 20)], vec![rect(-5, -5, 10, 10)]),
        (vec![rect(-10, -10, 10, 10)], vec![rect(0, 0, 10, 10)]),
        (vec![rect(-10, -10, 10, 10)], vec![rect(0, -10, 10, 10)]),
        (vec![rect(0, 0, 10, 10)], vec![rect(0, 0, 10, 10)]),
        (vec![rect(-m, -m, 2 * m, 2 * m)], vec![rect(-5, -5, 10, 10)]),
        (
            vec![[[0, 0], [10, 10], [0, 10], [10, 0]].map(p).to_vec()],
            vec![],
        ),
        (
            vec![vec![], [[0, 0], [1, 1], [2, 2]].map(p).to_vec()],
            vec![],
        ),
    ];
    for (subject, clip) in &cases {
        let subject: Vec<_> = subject.iter().map(Vec::as_slice).collect();
        let clip: Vec<_> = clip.iter().map(Vec::as_slice).collect();
        for fill in [RegionFill::EvenOdd, RegionFill::NonZero] {
            for op in [
                RegionOperation::Union,
                RegionOperation::Intersection,
                RegionOperation::Difference,
                RegionOperation::Xor,
            ] {
                run(out, None, |check| {
                    boolean_regions(&subject, &clip, op, fill, check)
                });
            }
        }
    }
    let mut nested = lines(&[rect(-10, -10, 20, 20), rect(-5, -5, 10, 10)]);
    nested.extend(
        [[p([0, 5]), p([0, 10])], [p([0, 10]), p([0, 20])]]
            .map(|[start, end]| Segment { start, end }),
    );
    let mut rounded = lines(&[rect(0, 0, 10, 10)]);
    rounded.extend(
        [[p([0, 3]), p([10, 4])], [p([0, 8]), p([10, 0])]]
            .map(|[start, end]| Segment { start, end }),
    );
    for lines in [
        nested,
        rounded,
        vec![Segment {
            start: p([0, 0]),
            end: p([0, 0]),
        }],
    ] {
        run(out, None, |check| detect_regions(&lines, check));
    }
    let square = rect(0, 0, 10, 10);
    let line_set = lines(std::slice::from_ref(&square));
    let calls = run(out, None, |check| detect_regions(&line_set, check));
    for stop in 0..calls {
        run(out, Some(stop), |check| detect_regions(&line_set, check));
    }
    let other = rect(5, 5, 10, 10);
    let calls = run(out, None, |check| {
        boolean_regions(
            &[&square],
            &[&other],
            RegionOperation::Xor,
            RegionFill::EvenOdd,
            check,
        )
    });
    for stop in 0..calls {
        run(out, Some(stop), |check| {
            boolean_regions(
                &[&square],
                &[&other],
                RegionOperation::Xor,
                RegionFill::EvenOdd,
                check,
            )
        });
    }
    let too_many = vec![
        Segment {
            start: p([0, 0]),
            end: p([1, 0])
        };
        MAX_REGION_EDGES + 1
    ];
    run(out, None, |check| detect_regions(&too_many, check));
}
