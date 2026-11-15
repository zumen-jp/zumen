//! 入力位置を保った交点分割。丸めの反復を依存へ渡さず、辺数と移動量もここで制限する。

use super::{
    Charge, Error, Failure, MAX_REGION_EDGES, MAX_REGION_PASSES, Point, RegionAdjustment,
    RegionDiscard, RegionLocation, RegionOperand, RegionResult, Segment, SegmentIntersection,
};
use crate::topology::{cross, difference, intersect, wide::Wide};

#[derive(Clone, Copy)]
pub(super) struct Edge {
    pub segment: Segment,
    original: Segment,
    pub source: RegionLocation,
}

pub(super) struct Prepared {
    pub edges: Vec<Edge>,
    pub result: RegionResult,
}

#[derive(Clone, Copy)]
struct Mark {
    edge: usize,
    parameter: i128,
    point: Point,
}

pub(super) fn work<E>(
    amount: u64,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<(), Failure<E>> {
    check(Charge::Work(amount)).map_err(Failure::Stopped)
}

pub(super) fn reserve<T, E>(
    output: &mut Vec<T>,
    additional: usize,
    slot_bytes: u64,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<(), Failure<E>> {
    // 呼び出す側の個数の上限と、環境によらない型ごとの予約量を使う。
    check(Charge::Bytes(additional as u64 * slot_bytes)).map_err(Failure::Stopped)?;
    output
        .try_reserve_exact(additional)
        .map_err(|_| Failure::Geometry(Error::Allocation))
}

fn range<E>() -> Failure<E> {
    Failure::Geometry(Error::Range)
}
fn precision<E>() -> Failure<E> {
    Failure::Geometry(Error::Precision)
}

impl Prepared {
    fn new<E>(
        edges: usize,
        contours: usize,
        check: &mut impl FnMut(Charge) -> Result<(), E>,
    ) -> Result<Self, Failure<E>> {
        let mut result = Self {
            edges: Vec::new(),
            result: RegionResult::default(),
        };
        reserve(&mut result.edges, edges, 128, check)?;
        reserve(&mut result.result.discarded, edges + contours, 80, check)?;
        reserve(
            &mut result.result.adjustments,
            2 * MAX_REGION_EDGES,
            64,
            check,
        )?;
        Ok(result)
    }

    pub fn lines<E>(
        lines: &[Segment],
        check: &mut impl FnMut(Charge) -> Result<(), E>,
    ) -> Result<Self, Failure<E>> {
        if lines.len() > MAX_REGION_EDGES {
            return Err(range());
        }
        work(lines.len() as u64 + 1, check)?;
        let mut result = Self::new(lines.len(), 0, check)?;
        for (index, &segment) in lines.iter().enumerate() {
            work(0, check)?;
            let source = RegionLocation {
                operand: RegionOperand::Lines,
                contour: 0,
                edge: Some(index as u64),
            };
            result.push(segment, source);
        }
        Ok(result)
    }

    pub fn contours<E>(
        subject: &[&[Point]],
        clip: &[&[Point]],
        check: &mut impl FnMut(Charge) -> Result<(), E>,
    ) -> Result<Self, Failure<E>> {
        let contours = subject.len().checked_add(clip.len()).ok_or_else(range)?;
        if contours > MAX_REGION_EDGES {
            return Err(range());
        }
        work(contours as u64 + 1, check)?;
        let count = subject.iter().chain(clip).try_fold(0usize, |sum, path| {
            let count = sum.checked_add(path.len()).ok_or_else(range)?;
            if count > MAX_REGION_EDGES {
                Err(range())
            } else {
                Ok(count)
            }
        })?;
        work((count * 3) as u64, check)?;
        let mut result = Self::new(count, contours, check)?;
        for (paths, operand) in [
            (subject, RegionOperand::Subject),
            (clip, RegionOperand::Clip),
        ] {
            for (index, &path) in paths.iter().enumerate() {
                work(0, check)?;
                let source = RegionLocation {
                    operand,
                    contour: index as u64,
                    edge: None,
                };
                let mut path = path;
                if path.len() > 1 && path.first() == path.last() {
                    path = path.split_last().map_or(path, |(_, rest)| rest);
                }
                let line = path.first().and_then(|&first| {
                    path.iter()
                        .find(|&&p| p != first)
                        .map(|&end| Segment { start: first, end })
                });
                let degenerate = line.is_none_or(|line| {
                    path.iter().all(|&point| {
                        cross(
                            difference(line.end, line.start),
                            difference(point, line.start),
                        ) == 0
                    })
                });
                if degenerate {
                    result
                        .result
                        .discarded
                        .push(RegionDiscard::Degenerate(source));
                    continue;
                }
                for (edge, (&start, &end)) in
                    path.iter().zip(path.iter().cycle().skip(1)).enumerate()
                {
                    result.push(
                        Segment { start, end },
                        RegionLocation {
                            edge: Some(edge as u64),
                            ..source
                        },
                    );
                }
            }
        }
        Ok(result)
    }

    fn push(&mut self, segment: Segment, source: RegionLocation) {
        if segment.start == segment.end {
            self.result
                .discarded
                .push(RegionDiscard::Degenerate(source));
        } else {
            self.edges.push(Edge {
                segment,
                original: segment,
                source,
            });
        }
    }

    pub fn split<E>(
        &mut self,
        check: &mut impl FnMut(Charge) -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        for pass in 0..=MAX_REGION_PASSES {
            let n = self.edges.len();
            work((n * n.saturating_sub(1) / 2 + n) as u64, check)?;
            let mut marks = Vec::new();
            reserve(&mut marks, MAX_REGION_EDGES - n, 64, check)?;
            let mut rounded = false;
            for (a_index, &a) in self.edges.iter().enumerate() {
                for (b_index, &b) in self.edges.iter().enumerate().skip(a_index + 1) {
                    work(0, check)?;
                    match intersect(a.segment, b.segment).map_err(Failure::Geometry)? {
                        SegmentIntersection::None => {}
                        SegmentIntersection::Point(hit) => {
                            rounded |= hit.rounded;
                            for (index, edge) in [(a_index, a), (b_index, b)] {
                                add_mark(
                                    &mut marks,
                                    index,
                                    edge,
                                    hit.point,
                                    MAX_REGION_EDGES - n,
                                    check,
                                )?;
                                if hit.rounded {
                                    Self::adjust(
                                        &mut self.result.adjustments,
                                        edge.source,
                                        hit.point,
                                        check,
                                    )?;
                                }
                            }
                        }
                        SegmentIntersection::Overlap(overlap) => {
                            for (index, edge) in [(a_index, a), (b_index, b)] {
                                for point in [overlap.start, overlap.end] {
                                    add_mark(
                                        &mut marks,
                                        index,
                                        edge,
                                        point,
                                        MAX_REGION_EDGES - n,
                                        check,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
            if marks.is_empty() {
                return if rounded { Err(precision()) } else { Ok(()) };
            }
            if pass == MAX_REGION_PASSES {
                return Err(precision());
            }
            work((n + marks.len()) as u64, check)?;
            check(Charge::Points(marks.len() as u64)).map_err(Failure::Stopped)?;
            let mut next = Vec::new();
            reserve(&mut next, n + marks.len(), 128, check)?;
            let mut marks = marks.into_iter().peekable();
            for (index, &edge) in self.edges.iter().enumerate() {
                let mut start = edge.segment.start;
                while marks.peek().is_some_and(|mark| mark.edge == index) {
                    if let Some(mark) = marks.next() {
                        next.push(Edge {
                            segment: Segment {
                                start,
                                end: mark.point,
                            },
                            ..edge
                        });
                        start = mark.point;
                    }
                }
                next.push(Edge {
                    segment: Segment {
                        start,
                        end: edge.segment.end,
                    },
                    ..edge
                });
            }
            self.edges = next;
        }
        Err(precision())
    }

    fn adjust<E>(
        output: &mut Vec<RegionAdjustment>,
        source: RegionLocation,
        point: Point,
        check: &mut impl FnMut(Charge) -> Result<(), E>,
    ) -> Result<(), Failure<E>> {
        work(output.len() as u64 + 1, check)?;
        let value = RegionAdjustment { source, point };
        if let Err(at) = output.binary_search(&value) {
            if output.len() == 2 * MAX_REGION_EDGES {
                return Err(range());
            }
            check(Charge::Points(1)).map_err(Failure::Stopped)?;
            output.insert(at, value);
        }
        Ok(())
    }
}

fn parameter(segment: Segment, point: Point) -> (i128, i128) {
    let [x, y] = difference(segment.end, segment.start);
    let [u, v] = difference(point, segment.start);
    (x * u + y * v, x * x + y * y)
}

fn add_mark<E>(
    marks: &mut Vec<Mark>,
    index: usize,
    edge: Edge,
    point: Point,
    limit: usize,
    check: &mut impl FnMut(Charge) -> Result<(), E>,
) -> Result<(), Failure<E>> {
    work(marks.len() as u64 + 16, check)?;
    if point == edge.segment.start || point == edge.segment.end {
        return Ok(());
    }
    let (position, length) = parameter(edge.segment, point);
    let (original_position, original_length) = parameter(edge.original, point);
    let (original_start, _) = parameter(edge.original, edge.segment.start);
    let (original_end, _) = parameter(edge.original, edge.segment.end);
    let deviation = cross(
        difference(edge.original.end, edge.original.start),
        difference(point, edge.original.start),
    );
    // 元の線分への垂線距離を 1 格子以内に保ち、投影を元の端点間に収める。
    if !(1..length).contains(&position)
        || !(0..=original_length).contains(&original_position)
        || !(original_start + 1..original_end).contains(&original_position)
        || Wide::square(deviation) > Wide::product(original_length as u128, 1)
    {
        return Err(precision());
    }
    match marks.binary_search_by_key(&(index, position), |m| (m.edge, m.parameter)) {
        Ok(at) if marks.get(at).is_none_or(|mark| mark.point != point) => Err(precision()),
        Ok(_) => Ok(()),
        Err(at) => {
            if marks.len() == limit {
                return Err(range());
            }
            marks.insert(
                at,
                Mark {
                    edge: index,
                    parameter: position,
                    point,
                },
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reservations_cover_types_on_each_target() {
        assert!(size_of::<Edge>() <= 128);
        assert!(size_of::<Mark>() <= 64);
        assert!(size_of::<RegionAdjustment>() <= 64);
        assert!(size_of::<RegionDiscard>() <= 80);
        assert!(size_of::<super::super::Region>() <= 64);
    }

    #[test]
    fn repeated_splits_cannot_accumulate_drift_or_fold_the_original_edge() {
        let point = |x, y| Point::new(x, y).expect("範囲内の合成入力");
        let edge = Edge {
            original: Segment {
                start: point(0, 0),
                end: point(10, 0),
            },
            segment: Segment {
                start: point(0, 0),
                end: point(5, 1),
            },
            source: RegionLocation {
                operand: RegionOperand::Lines,
                contour: 0,
                edge: Some(0),
            },
        };
        let free = &mut |_| Ok::<_, ()>(());
        for invalid in [point(2, 2), point(5, 0)] {
            let mut marks = Vec::new();
            assert_eq!(
                add_mark(&mut marks, 0, edge, invalid, 10, free),
                Err(Failure::Geometry(Error::Precision))
            );
            assert!(marks.is_empty());
        }
        let mut marks = Vec::new();
        assert!(add_mark(&mut marks, 0, edge, point(2, 1), 10, free).is_ok());
        assert_eq!(marks.len(), 1);
    }
}
