//! 地物の面の頂点を先に投影し、既存の幾何カーネルで三角形へ分ける。
use super::*;
use crate::Location;
use zumen_geom3d::{Face, MeshChanges, Shape, Transform};

pub(super) fn evaluate(
    form: &Form,
    projection: Projection,
    cx: &mut Cx,
) -> Result<(Mesh, MeshChanges), Failure> {
    if let Form::Line(points) = form {
        if points.len() < 2 {
            return Err(Failure::Invalid("points"));
        }
        let positions = locations(points, projection, cx)?;
        let mut edges = Vec::new();
        visual::reserve(&mut edges, positions.len() - 1, 8, cx)?;
        cx.check(Kind::Segments, positions.len() as u64 - 1)?;
        for i in 1..positions.len() {
            edges.push([
                u32::try_from(i - 1).map_err(|_| Failure::Invalid("size"))?,
                u32::try_from(i).map_err(|_| Failure::Invalid("size"))?,
            ]);
        }
        return Ok((
            Mesh {
                positions,
                edges,
                triangles: Vec::new(),
            },
            MeshChanges::default(),
        ));
    }
    let shape = match form {
        Form::Solid(shape) => project(shape, Transform::IDENTITY, projection, 0, cx)?,
        Form::Area { outer, holes } => {
            let outer = locations(outer, projection, cx)?;
            let mut copied = Vec::new();
            visual::reserve(&mut copied, holes.len(), 32, cx)?;
            for ring in holes {
                copied.push(locations(ring, projection, cx)?);
            }
            cx.check(Kind::DecodedBytes, 64)?;
            Shape::FaceSet {
                faces: vec![Face {
                    outer,
                    holes: copied,
                }],
                closed: false,
            }
        }
        Form::Point(_) => return Err(Failure::Unsupported("point_primitive")),
        Form::Line(_) => return Err(Failure::Invalid("line")),
    };
    // 地物の読み手が持つ面集合・三角形だけを扱う。曲線の分割を度の単位で評価しない。
    let evaluated = ManifoldKernel.evaluate_with_changes(
        &shape,
        Tolerance {
            chord: 0.001,
            length: 1e-9,
        },
        cx,
    )?;
    Ok((evaluated.mesh, evaluated.changes))
}

fn locations(
    points: &[Location],
    projection: Projection,
    cx: &mut Cx,
) -> Result<Vec<[f64; 3]>, Failure> {
    cx.check(Kind::Work, 1)?;
    let mut output = Vec::new();
    visual::reserve(&mut output, points.len(), 24, cx)?;
    cx.check(Kind::Segments, points.len() as u64)?;
    for p in points {
        let height = p
            .height
            .filter(|n| n.is_finite())
            .ok_or(Failure::Invalid("height_missing"))?;
        let [x, y] = projection.xy(p.position, cx)?;
        output.push([x, y, height]);
    }
    Ok(output)
}

fn points(
    points: &[[f64; 3]],
    transform: Transform,
    projection: Projection,
    cx: &mut Cx,
) -> Result<Vec<[f64; 3]>, Failure> {
    let mut output = Vec::new();
    visual::reserve(&mut output, points.len(), 24, cx)?;
    cx.check(Kind::Segments, points.len() as u64)?;
    for &p in points {
        output.push(projection.xyz(transform.point(p), cx)?);
    }
    Ok(output)
}

fn project(
    shape: &Shape,
    transform: Transform,
    projection: Projection,
    depth: u32,
    cx: &mut Cx,
) -> Result<Shape, Failure> {
    cx.check(Kind::Work, 1)?;
    if depth >= 64 || !transform.is_finite() {
        return Err(Failure::Invalid("transform_depth"));
    }
    cx.enter()?;
    let result = (|| {
        Ok(match shape {
            Shape::FaceSet { faces, closed } => {
                let mut output = Vec::new();
                visual::reserve(&mut output, faces.len(), 64, cx)?;
                for face in faces {
                    cx.check(Kind::Work, 1)?;
                    let outer = points(&face.outer, transform, projection, cx)?;
                    let mut holes = Vec::new();
                    visual::reserve(&mut holes, face.holes.len(), 32, cx)?;
                    for ring in &face.holes {
                        cx.check(Kind::Work, 1)?;
                        holes.push(points(ring, transform, projection, cx)?);
                    }
                    output.push(Face { outer, holes });
                }
                Shape::FaceSet {
                    faces: output,
                    closed: *closed,
                }
            }
            Shape::Triangles {
                positions,
                triangles,
                closed,
            } => {
                let positions = points(positions, transform, projection, cx)?;
                cx.check(Kind::Work, triangles.len() as u64)?;
                let mut copied = Vec::new();
                visual::reserve(&mut copied, triangles.len(), 12, cx)?;
                copied.extend_from_slice(triangles);
                Shape::Triangles {
                    positions,
                    triangles: copied,
                    closed: *closed,
                }
            }
            Shape::Mapped {
                transform: local,
                shape,
            } => project(shape, local.then(&transform), projection, depth + 1, cx)?,
            Shape::Group(shapes) => {
                let mut output = Vec::new();
                visual::reserve(&mut output, shapes.len(), 256, cx)?;
                for shape in shapes {
                    output.push(project(shape, transform, projection, depth + 1, cx)?);
                }
                Shape::Group(output)
            }
            _ => return Err(Failure::Unsupported(shape.kind())),
        })
    })();
    cx.leave();
    result
}
