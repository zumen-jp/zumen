//! 格子上の投影領域と、面と線の深さの差で隠れる区間を求める。

use zumen_base::Cx;
use zumen_geom2d::topology::{Point, Region, Segment, SegmentIntersection, segment_intersection};
use zumen_geom3d::Failure;

use super::{
    budget::{charge, push, reserve, topology, work},
    bvh::{Bounds, Bvh},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Vertex {
    pub point: Point,
    pub depth: f64,
}

#[derive(Clone, Debug)]
pub(super) enum Surface {
    Triangle([Vertex; 3]),
    /// 切り口の領域は深さ 0。穴を別の塗りつぶしにしない。
    Cap(Region),
}

impl Surface {
    fn bounds(&self) -> Option<Bounds> {
        match self {
            Self::Triangle(points) => Bounds::points(points.iter().map(|v| v.point.coordinates())),
            Self::Cap(region) => Bounds::points(region.outer.iter().map(|v| v.coordinates())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Span {
    pub range: [f64; 2],
    pub hidden: bool,
}

pub(super) struct Split {
    pub spans: Vec<Span>,
    pub rounded: u64,
}

pub(super) struct Visibility {
    surfaces: Vec<Surface>,
    index: Bvh,
    tolerance: f64,
}

impl Visibility {
    pub fn new(surfaces: Vec<Surface>, tolerance: f64, cx: &mut Cx<'_>) -> Result<Self, Failure> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(Failure::Invalid("drawing_tolerance"));
        }
        let mut boxes = Vec::new();
        reserve(&mut boxes, surfaces.len(), 32, cx)?;
        work(cx, surfaces.len() as u64)?;
        for surface in &surfaces {
            boxes.push(
                surface
                    .bounds()
                    .ok_or(Failure::Invalid("drawing_surface"))?,
            );
        }
        let index = Bvh::new(&boxes, cx)?;
        Ok(Self {
            surfaces,
            index,
            tolerance,
        })
    }

    pub fn split(&self, edge: [Vertex; 2], cx: &mut Cx<'_>) -> Result<Split, Failure> {
        let bounds = Bounds::points(edge.map(|v| v.point.coordinates()))
            .ok_or(Failure::Invalid("drawing_edge"))?;
        let mut hidden = Vec::new();
        let mut rounded = 0;
        self.index.visit(bounds, cx, |at, cx| {
            match self
                .surfaces
                .get(at)
                .ok_or(Failure::Invalid("drawing_index"))?
            {
                Surface::Triangle(points) => {
                    work(cx, 32)?;
                    if let Some(range) = triangle_interval(edge, *points, self.tolerance)? {
                        push(&mut hidden, range, 16, cx)?;
                    }
                }
                Surface::Cap(region) => {
                    rounded += cap_intervals(edge, region, self.tolerance, &mut hidden, cx)?;
                }
            }
            Ok(())
        })?;
        let n = hidden.len() as u64;
        work(cx, n.saturating_mul(65 - n.leading_zeros() as u64))?;
        hidden.sort_unstable_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
        let mut result = Vec::new();
        let mut end = 0.0;
        let mut begin = None;
        for [lo, hi] in hidden {
            if lo > end {
                if let Some(start) = begin.take() {
                    push(
                        &mut result,
                        Span {
                            range: [start, end],
                            hidden: true,
                        },
                        24,
                        cx,
                    )?;
                }
                push(
                    &mut result,
                    Span {
                        range: [end, lo],
                        hidden: false,
                    },
                    24,
                    cx,
                )?;
            }
            if begin.is_none() {
                begin = Some(lo);
            }
            end = end.max(hi);
        }
        if let Some(start) = begin {
            push(
                &mut result,
                Span {
                    range: [start, end],
                    hidden: true,
                },
                24,
                cx,
            )?;
        }
        if end < 1.0 {
            push(
                &mut result,
                Span {
                    range: [end, 1.0],
                    hidden: false,
                },
                24,
                cx,
            )?;
        }
        Ok(Split {
            spans: result,
            rounded,
        })
    }
}

fn cross(a: Point, b: Point, p: Point) -> i128 {
    let [ax, ay] = a.coordinates();
    let [bx, by] = b.coordinates();
    let [px, py] = p.coordinates();
    i128::from(bx - ax) * i128::from(py - ay) - i128::from(by - ay) * i128::from(px - ax)
}

/// 線形な値が正の区間。端点の一致は区間の長さを持たない。
fn positive(range: &mut [f64; 2], a: f64, b: f64) -> bool {
    if a <= 0.0 && b <= 0.0 {
        return false;
    }
    let zero = || {
        let denominator = a - b;
        if denominator.is_finite() {
            a / denominator
        } else {
            let scale = a.abs().max(b.abs());
            (a / scale) / (a / scale - b / scale)
        }
    };
    if a < 0.0 {
        range[0] = range[0].max(zero());
    }
    if b < 0.0 {
        range[1] = range[1].min(zero());
    }
    range[0] < range[1]
}

fn triangle_interval(
    edge: [Vertex; 2],
    triangle: [Vertex; 3],
    tolerance: f64,
) -> Result<Option<[f64; 2]>, Failure> {
    let [a, b, c] = triangle;
    let area = cross(a.point, b.point, c.point);
    if area == 0 {
        return Ok(None);
    }
    let sign = area.signum();
    let mut range: [f64; 2] = [0.0, 1.0];
    for (p, q) in [(a, b), (b, c), (c, a)] {
        let x = cross(p.point, q.point, edge[0].point) * sign;
        let y = cross(p.point, q.point, edge[1].point) * sign;
        if x < 0 && y < 0 {
            return Ok(None);
        }
        if x < 0 {
            range[0] = range[0].max(x as f64 / (x - y) as f64);
        }
        if y < 0 {
            range[1] = range[1].min(x as f64 / (x - y) as f64);
        }
        if range[0] >= range[1] {
            return Ok(None);
        }
    }
    // 整数の面積比で奥行きを補間する。面の向きが逆でも同じ値になる。
    let delta = |v: Vertex| {
        let u = cross(c.point, a.point, v.point) as f64 / area as f64;
        let w = cross(a.point, b.point, v.point) as f64 / area as f64;
        a.depth + (b.depth - a.depth) * u + (c.depth - a.depth) * w - v.depth - tolerance
    };
    let [x, y] = edge.map(delta);
    if ![x, y].into_iter().all(f64::is_finite) {
        return Err(Failure::Invalid("drawing_depth"));
    }
    Ok(positive(&mut range, x, y).then_some(range))
}

fn parameter(edge: [Vertex; 2], point: Point) -> i64 {
    let a = edge[0].point.coordinates();
    let b = edge[1].point.coordinates();
    let p = point.coordinates();
    let (start, end, point) = if (b[1] - a[1]).abs() > (b[0] - a[0]).abs() {
        (a[1], b[1], p[1])
    } else {
        (a[0], b[0], p[0])
    };
    if end == start {
        0
    } else {
        ((point - start) * (end - start).signum()).clamp(0, (end - start).abs())
    }
}

// 中点を格子へ丸めると、格子幅 1 の穴が境界へ縮む。有理数のまま内外を調べる。
// 格子の上限 2^40、辺の差 2^41、分母 2^42 から、積和は i128 に収まる。
fn inside([x, y, denominator]: [i128; 3], contour: &[Point]) -> (bool, bool) {
    let mut winding = 0i64;
    for (&a, &b) in contour
        .iter()
        .zip(contour.iter().cycle().skip(1))
        .take(contour.len())
    {
        let [ax, ay] = a.coordinates();
        let [bx, by] = b.coordinates();
        let [ax, ay, bx, by] = [ax, ay, bx, by].map(i128::from);
        let direction = (bx - ax) * (y - ay * denominator) - (by - ay) * (x - ax * denominator);
        let [ax, ay, bx, by] = [ax, ay, bx, by].map(|v| v * denominator);
        if direction == 0
            && x >= ax.min(bx)
            && x <= ax.max(bx)
            && y >= ay.min(by)
            && y <= ay.max(by)
        {
            return (true, true);
        }
        if ay <= y && by > y && direction > 0 {
            winding += 1;
        }
        if by <= y && ay > y && direction < 0 {
            winding -= 1;
        }
    }
    (winding != 0, false)
}

fn cap_intervals(
    edge: [Vertex; 2],
    region: &Region,
    tolerance: f64,
    out: &mut Vec<[f64; 2]>,
    cx: &mut Cx<'_>,
) -> Result<u64, Failure> {
    let count = region
        .holes
        .iter()
        .fold(region.outer.len(), |n, h| n.saturating_add(h.len())) as u64;
    // 交点と、その間の内外判定は、輪郭ごとの辺数から上限を先に数える。
    work(cx, count.saturating_mul(count).saturating_mul(8))?;
    let mut events = Vec::new();
    let length = parameter(edge, edge[1].point);
    if length == 0 {
        return Err(Failure::Invalid("drawing_edge"));
    }
    let mut rounded = 0;
    push(&mut events, 0, 8, cx)?;
    push(&mut events, length, 8, cx)?;
    let segment = Segment {
        start: edge[0].point,
        end: edge[1].point,
    };
    for contour in std::iter::once(&region.outer).chain(&region.holes) {
        for (&start, &end) in contour
            .iter()
            .zip(contour.iter().cycle().skip(1))
            .take(contour.len())
        {
            let intersection =
                segment_intersection(segment, Segment { start, end }, |c| charge(cx, c))
                    .map_err(topology)?;
            match intersection {
                SegmentIntersection::None => {}
                SegmentIntersection::Point(hit) => {
                    rounded += u64::from(hit.rounded);
                    push(&mut events, parameter(edge, hit.point), 8, cx)?
                }
                SegmentIntersection::Overlap(line) => {
                    push(&mut events, parameter(edge, line.start), 8, cx)?;
                    push(&mut events, parameter(edge, line.end), 8, cx)?;
                }
            }
        }
    }
    events.sort_unstable();
    events.dedup();
    for pair in events.windows(2) {
        work(cx, 0)?;
        let [lo, hi] = pair else {
            return Err(Failure::Invalid("drawing_interval"));
        };
        let mut range = [*lo as f64 / length as f64, *hi as f64 / length as f64];
        let numerator = i128::from(lo + hi);
        let denominator = i128::from(length) * 2;
        let a = edge[0].point.coordinates();
        let b = edge[1].point.coordinates();
        let point = [
            i128::from(a[0]) * denominator + i128::from(b[0] - a[0]) * numerator,
            i128::from(a[1]) * denominator + i128::from(b[1] - a[1]) * numerator,
            denominator,
        ];
        let (in_outer, boundary) = inside(point, &region.outer);
        let in_hole = !boundary
            && region.holes.iter().any(|hole| {
                let (value, edge) = inside(point, hole);
                value && !edge
            });
        if in_outer
            && !in_hole
            && positive(
                &mut range,
                -edge[0].depth - tolerance,
                -edge[1].depth - tolerance,
            )
        {
            push(out, range, 16, cx)?;
        }
    }
    Ok(rounded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics};
    fn vertex(x: i64, y: i64, depth: f64) -> Vertex {
        Vertex {
            point: Point::new(x, y).unwrap(),
            depth,
        }
    }
    fn split(surfaces: Vec<Surface>, edge: [Vertex; 2]) -> Vec<Span> {
        let (mut budget, mut diag, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        Visibility::new(surfaces, 0.01, &mut cx)
            .unwrap()
            .split(edge, &mut cx)
            .unwrap()
            .spans
    }
    #[test]
    fn touching_a_triangle_vertex_leaves_no_hidden_interval() {
        let surface = Surface::Triangle([vertex(0, 0, 0.0), vertex(3, 0, 0.0), vertex(0, 3, 0.0)]);
        let spans = split(vec![surface], [vertex(-2, 2, -1.0), vertex(2, -2, -1.0)]);
        assert_eq!(
            spans,
            [Span {
                range: [0.0, 1.0],
                hidden: false
            }]
        );
    }

    #[test]
    fn a_sloped_surface_changes_visibility_at_the_depth_crossing() {
        let surface =
            Surface::Triangle([vertex(0, 0, -2.0), vertex(10, 0, 0.0), vertex(0, 10, -2.0)]);
        let spans = split(vec![surface], [vertex(0, 0, -1.0), vertex(10, 0, -1.0)]);
        assert_eq!(spans.len(), 2);
        assert!(!spans[0].hidden && spans[1].hidden);
        assert!((spans[1].range[0] - 0.505).abs() < 1e-12);
    }
    #[test]
    fn a_nonzero_depth_crossing_has_a_span_even_when_both_ends_round_to_one_point() {
        use zumen_geom2d::topology::Grid;
        // 許容差 0.01 mm の格子間隔は 0.0025 mm。辺は 40 格子、奥行きは長さの 1/128 で交差する。
        let coarse = Grid::new([0.; 2], 0.01).unwrap();
        let finer = Grid::new([0.; 2], 0.001).unwrap();
        for offset in [0, 400_000_000, -400_000_000] {
            for slope in [1., -1., 0.01, -0.01] {
                let triangle = [
                    vertex(offset, 0, 0.),
                    vertex(offset + 40, 0, 0.),
                    vertex(offset, 40, 0.),
                ];
                let edge = [
                    vertex(offset, 0, -0.01 - slope / 128.),
                    vertex(offset + 40, 0, -0.01 + slope * 127. / 128.),
                ];
                let spans = split(vec![Surface::Triangle(triangle)], edge);
                let [short, rest] = spans.as_slice() else {
                    panic!("可視性が切り替わる位置が必要")
                };
                assert_eq!(short.hidden, slope > 0.);
                assert_ne!(short.hidden, rest.hidden);
                assert_eq!(short.range[0], 0.);
                assert!((short.range[1] - 1. / 128.).abs() < 1e-12);
                let [a, b] = edge.map(|v| coarse.position(v.point).unwrap());
                let end = [a[0] + (b[0] - a[0]) * short.range[1], 0.];
                // 0.78125 µm は 0 ではない。丸めの格子を変えると端点を分けられる。
                assert!((end[0] - a[0] - 0.00078125).abs() < 1e-9);
                assert_eq!(
                    coarse.snap(a).unwrap().point,
                    coarse.snap(end).unwrap().point
                );
                assert_ne!(finer.snap(a).unwrap().point, finer.snap(end).unwrap().point);
            }
        }
    }

    #[test]
    fn a_cut_cap_keeps_its_hole_and_splits_both_sides() {
        let points = |p: &[[i64; 2]]| p.iter().map(|p| Point::new(p[0], p[1]).unwrap()).collect();
        let surface = Surface::Cap(Region {
            outer: points(&[[0, 0], [10, 0], [10, 10], [0, 10]]),
            holes: vec![points(&[[4, 4], [4, 6], [6, 6], [6, 4]])],
        });
        let spans = split(vec![surface], [vertex(-2, 5, -1.0), vertex(12, 5, -1.0)]);
        assert_eq!(
            spans.iter().map(|s| s.hidden).collect::<Vec<_>>(),
            [false, true, false, true, false]
        );
        assert_eq!(spans[2].range, [6.0 / 14.0, 8.0 / 14.0]);
    }
    #[test]
    fn coplanar_edges_remain_visible_and_reversing_faces_changes_nothing() {
        let t = [vertex(0, 0, -1.0), vertex(10, 0, -1.0), vertex(0, 10, -1.0)];
        let edge = [vertex(0, 0, -1.0), vertex(10, 0, -1.0)];
        assert_eq!(
            split(vec![Surface::Triangle(t)], edge),
            [Span {
                range: [0.0, 1.0],
                hidden: false
            }]
        );
        let behind = edge.map(|v| Vertex { depth: -2.0, ..v });
        assert_eq!(
            split(vec![Surface::Triangle(t)], behind),
            split(vec![Surface::Triangle([t[2], t[1], t[0]])], behind)
        );
    }

    #[test]
    fn a_hole_one_grid_unit_wide_keeps_an_interior() {
        let points = |p: &[[i64; 2]]| p.iter().map(|p| Point::new(p[0], p[1]).unwrap()).collect();
        let cap = Surface::Cap(Region {
            outer: points(&[[0, 0], [10, 0], [10, 10], [0, 10]]),
            holes: vec![points(&[[4, 4], [4, 6], [5, 6], [5, 4]])],
        });
        for edge in [
            [vertex(0, 5, -1.0), vertex(10, 5, -1.0)],
            [vertex(10, 5, -1.0), vertex(0, 5, -1.0)],
        ] {
            let spans = split(vec![cap.clone()], edge);
            assert_eq!(
                spans.iter().map(|s| s.hidden).collect::<Vec<_>>(),
                [true, false, true]
            );
            assert!((spans[1].range[1] - spans[1].range[0] - 0.1).abs() < 1e-12);
        }
    }
}
