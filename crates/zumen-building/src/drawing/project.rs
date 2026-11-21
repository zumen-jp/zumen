//! メッシュを図面の近くの座標へ移し、見る深さで切って投影する。

use zumen_base::{Cx, Kind};
use zumen_geom2d::topology::{Grid, Point, RegionFill, RegionOperation, boolean_regions};
use zumen_geom3d::{Failure, Mesh, MeshSection, Plane, Xyz, add, scale, sub};

use super::{
    Frame,
    budget::{charge, push, reserve, topology, work},
    visibility::{Surface, Vertex},
};
use crate::evaluation::MeshPart;

#[derive(Clone, Debug)]
pub(super) struct Edge {
    pub vertices: [Vertex; 2],
    pub source: usize,
    pub cut: bool,
}

#[derive(Default)]
pub(super) struct Projected {
    pub surfaces: Vec<Surface>,
    pub edges: Vec<Edge>,
    pub snapped: u64,
    pub collapsed_edges: u64,
    pub collapsed_contours: u64,
}

impl Projected {
    fn vertex(&mut self, p: Xyz, grid: Grid) -> Result<Vertex, Failure> {
        let snap = grid
            .snap([p[0], p[1]])
            .map_err(|_| Failure::Invalid("drawing_grid"))?;
        self.snapped += u64::from(snap.offset != [0.0, 0.0]);
        Ok(Vertex {
            point: snap.point,
            depth: p[2],
        })
    }

    fn edge(
        &mut self,
        ends: [Xyz; 2],
        source: usize,
        cut: bool,
        grid: Grid,
        cx: &mut Cx<'_>,
    ) -> Result<(), Failure> {
        if ends[0][0] == ends[1][0] && ends[0][1] == ends[1][1] {
            return Ok(());
        }
        let vertices = [self.vertex(ends[0], grid)?, self.vertex(ends[1], grid)?];
        if vertices[0].point == vertices[1].point {
            self.collapsed_edges += 1;
            return Ok(());
        }
        cx.check(Kind::Segments, 1)?;
        push(
            &mut self.edges,
            Edge {
                vertices,
                source,
                cut,
            },
            64,
            cx,
        )
    }
}

pub(super) fn part(
    part: &MeshPart,
    frame: Frame,
    units: f64,
    grid: Grid,
    source: usize,
    cx: &mut Cx<'_>,
) -> Result<Projected, Failure> {
    let mut result = Projected::default();
    let original = &part.mesh;
    work(
        cx,
        (original.positions.len() + original.triangles.len() + original.edges.len()) as u64,
    )?;
    let mut mesh = Mesh::default();
    reserve(&mut mesh.positions, original.positions.len(), 24, cx)?;
    reserve(&mut mesh.triangles, original.triangles.len(), 12, cx)?;
    let origin = frame.origin();
    let offset = sub(
        part.placement.column(3),
        scale([origin.x, origin.y, origin.z], units),
    );
    for p in &original.positions {
        // 局所の点を絶対座標へ足す前に、配置と図面の原点の差を求める。
        let relative = scale(add(part.placement.vector(*p), offset), 1.0 / units);
        mesh.positions.push(frame.project_offset(relative)?);
    }
    let mirrored = part.placement.determinant() < 0.0;
    mesh.triangles.extend(
        original
            .triangles
            .iter()
            .map(|t| if mirrored { [t[2], t[1], t[0]] } else { *t }),
    );
    mesh.validate()?;
    if frame.is_section() {
        let paths = MeshSection.paths(
            &mesh,
            &Plane {
                origin: [0.0; 3],
                normal: [0.0, 0.0, 1.0],
            },
            cx,
        )?;
        let mut rings = Vec::new();
        for path in paths {
            work(cx, path.points.len() as u64 * 8)?;
            let mut points = Vec::new();
            reserve(&mut points, path.points.len(), 16, cx)?;
            for p in path.points {
                points.push(result.vertex([p[0], p[1], 0.0], grid)?.point);
            }
            points.dedup();
            let points = simplify(points, path.closed)?;
            let n = points.len();
            if n < if path.closed { 3 } else { 2 } {
                result.collapsed_contours += 1;
                continue;
            }
            let edges = if path.closed { n } else { n.saturating_sub(1) };
            for (&a, &b) in points.iter().zip(points.iter().cycle().skip(1)).take(edges) {
                let vertices = [a, b].map(|point| Vertex { point, depth: 0.0 });
                if vertices[0].point == vertices[1].point {
                    result.collapsed_contours += 1;
                    continue;
                }
                cx.check(Kind::Segments, 1)?;
                push(
                    &mut result.edges,
                    Edge {
                        vertices,
                        source,
                        cut: true,
                    },
                    64,
                    cx,
                )?;
            }
            if path.closed {
                push(&mut rings, points, 24, cx)?;
            }
        }
        if !rings.is_empty() {
            let mut slices = Vec::new();
            reserve(&mut slices, rings.len(), 16, cx)?;
            slices.extend(rings.iter().map(Vec::as_slice));
            let regions = boolean_regions(
                &slices,
                &[],
                RegionOperation::Union,
                RegionFill::NonZero,
                |c| charge(cx, c),
            )
            .map_err(topology)?;
            result.snapped += regions.adjustments.len() as u64;
            result.collapsed_contours += regions.discarded.len() as u64;
            for region in regions.regions {
                push(&mut result.surfaces, Surface::Cap(region), 80, cx)?;
            }
        }
    }
    for triangle in &mesh.triangles {
        work(cx, 16)?;
        let points = mesh
            .triangle(*triangle)
            .ok_or(Failure::Invalid("drawing_index"))?;
        let mut buffer = [[0.0; 3]; 8];
        buffer[..3].copy_from_slice(&points);
        let (buffer, n) = clip_polygon(buffer, 3, 0.0, true)?;
        let (buffer, n) = clip_polygon(buffer, n, -frame.depth(), false)?;
        if n < 3 {
            continue;
        }
        let first = result.vertex(buffer[0], grid)?;
        for pair in buffer
            .get(1..n)
            .ok_or(Failure::Invalid("drawing_polygon"))?
            .windows(2)
        {
            let [a, b] = pair else {
                return Err(Failure::Invalid("drawing_polygon"));
            };
            let triangle = [first, result.vertex(*a, grid)?, result.vertex(*b, grid)?];
            push(&mut result.surfaces, Surface::Triangle(triangle), 80, cx)?;
        }
    }
    for edge in &original.edges {
        work(cx, 8)?;
        let a = *mesh
            .positions
            .get(edge[0] as usize)
            .ok_or(Failure::Invalid("drawing_index"))?;
        let b = *mesh
            .positions
            .get(edge[1] as usize)
            .ok_or(Failure::Invalid("drawing_index"))?;
        if let Some(ends) = clip_edge([a, b], frame.depth())? {
            result.edge(ends, source, false, grid, cx)?;
        }
    }
    Ok(result)
}

// 三角形に分けた辺との交点を除き、同じ直線上の中間点で ID の順を変えない。
fn simplify(mut points: Vec<Point>, closed: bool) -> Result<Vec<Point>, Failure> {
    fn at(points: &[Point], index: usize) -> Result<Point, Failure> {
        points
            .get(index)
            .copied()
            .ok_or(Failure::Invalid("drawing_contour"))
    }
    fn middle(a: Point, b: Point, c: Point) -> bool {
        let [ax, ay] = a.coordinates();
        let [bx, by] = b.coordinates();
        let [cx, cy] = c.coordinates();
        let [x, y, u, v] = [bx - ax, by - ay, cx - bx, cy - by].map(i128::from);
        x * v == y * u && x * u + y * v >= 0
    }
    let mut used = 0;
    for i in 0..points.len() {
        let p = at(&points, i)?;
        while used >= 2 && middle(at(&points, used - 2)?, at(&points, used - 1)?, p) {
            used -= 1;
        }
        *points
            .get_mut(used)
            .ok_or(Failure::Invalid("drawing_contour"))? = p;
        used += 1;
    }
    points.truncate(used);
    let mut first = 0;
    if closed {
        loop {
            let n = points.len();
            if n - first < 3 {
                break;
            }
            if middle(
                at(&points, n - 2)?,
                at(&points, n - 1)?,
                at(&points, first)?,
            ) {
                points.pop();
            } else if middle(
                at(&points, n - 1)?,
                at(&points, first)?,
                at(&points, first + 1)?,
            ) {
                first += 1;
            } else {
                break;
            }
        }
    }
    points.drain(..first);
    Ok(points)
}

fn between(a: Xyz, b: Xyz, t: f64) -> Result<Xyz, Failure> {
    // 座標が等しい軸はその値を保つ。垂直辺を深さで切ったときに投影上の長さを作らない。
    let coordinate = |a, b| if a == b { a } else { a * (1.0 - t) + b * t };
    let p = [
        coordinate(a[0], b[0]),
        coordinate(a[1], b[1]),
        coordinate(a[2], b[2]),
    ];
    if p.into_iter().all(f64::is_finite) {
        Ok(p)
    } else {
        Err(Failure::Invalid("drawing_coordinate"))
    }
}

fn clip_polygon(
    input: [Xyz; 8],
    count: usize,
    limit: f64,
    less: bool,
) -> Result<([Xyz; 8], usize), Failure> {
    let mut out = [[0.0; 3]; 8];
    let mut n = 0;
    let input = input
        .get(..count)
        .ok_or(Failure::Invalid("drawing_polygon"))?;
    for (&a, &b) in input.iter().zip(input.iter().cycle().skip(1)).take(count) {
        let inside = |p: Xyz| if less { p[2] <= limit } else { p[2] >= limit };
        let mut append = |p| -> Result<(), Failure> {
            let slot = out.get_mut(n).ok_or(Failure::Invalid("drawing_polygon"))?;
            *slot = p;
            n += 1;
            Ok(())
        };
        if inside(a) {
            append(a)?;
        }
        if inside(a) != inside(b) {
            let denominator = b[2] - a[2];
            if !denominator.is_finite() {
                return Err(Failure::Invalid("drawing_depth"));
            }
            let mut p = between(a, b, (limit - a[2]) / denominator)?;
            p[2] = limit;
            append(p)?;
        }
    }
    Ok((out, n))
}

pub(super) fn clip_edge([a, b]: [Xyz; 2], depth: f64) -> Result<Option<[Xyz; 2]>, Failure> {
    let mut range: [f64; 2] = [0.0, 1.0];
    for (limit, less) in [(0.0, true), (-depth, false)] {
        let inside = |v: f64| if less { v <= limit } else { v >= limit };
        if !inside(a[2]) && !inside(b[2]) {
            return Ok(None);
        }
        if inside(a[2]) != inside(b[2]) {
            let denominator = b[2] - a[2];
            if !denominator.is_finite() {
                return Err(Failure::Invalid("drawing_depth"));
            }
            let t = (limit - a[2]) / denominator;
            if !inside(a[2]) {
                range[0] = range[0].max(t);
            } else {
                range[1] = range[1].min(t);
            }
        }
    }
    if range[0] >= range[1] {
        return Ok(None);
    }
    Ok(Some([between(a, b, range[0])?, between(a, b, range[1])?]))
}
