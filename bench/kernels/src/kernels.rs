//! 候補のカーネル。ブーリアンの前の形は [`crate::primitives`] で作り、ブーリアンだけを候補に任せる

use zumen_base::{Cx, Kind};
use zumen_geom3d::{
    BooleanOp, Failure, Kernel, Mesh, Plane, Shape, Tolerance, Transform, Xy, cross, dot, normalize,
};

use crate::primitives;

/// 候補が持つブーリアン
pub trait Booleans {
    /// 名前
    fn name(&self) -> &'static str;
    /// メッシュのブーリアン
    fn boolean(&self, op: BooleanOp, first: &Mesh, second: &Mesh) -> Result<Mesh, Failure>;
    /// 半空間との差（平面の表を残す）か積（裏を残す）
    fn clip(&self, first: &Mesh, plane: &Plane, keep_front: bool) -> Result<Mesh, Failure> {
        // 既定は、メッシュを囲む大きな箱を平面の裏に置いて、ブーリアンにする
        let [lo, hi] = first.bounds().ok_or(Failure::Invalid("empty"))?;
        let size = zumen_geom3d::length(zumen_geom3d::sub(hi, lo)) * 2.0 + 1.0;
        let frame = Transform::from_axes(plane.origin, Some(plane.normal), None)
            .ok_or(Failure::Invalid("plane"))?;
        let square = vec![vec![
            [-size, -size],
            [size, -size],
            [size, size],
            [-size, size],
        ]];
        let back = primitives::prism(&square, [0.0, 0.0, size])?
            .transformed(&Transform::translation([0.0, 0.0, -size]).then(&frame));
        self.boolean(
            if keep_front {
                BooleanOp::Difference
            } else {
                BooleanOp::Intersection
            },
            first,
            &back,
        )
    }
    /// 形のままブーリアンにできれば、その結果（自前の候補が使う）
    fn shaped(
        &self,
        _op: BooleanOp,
        _first: &Shape,
        _second: &Shape,
        _tolerance: Tolerance,
    ) -> Option<Result<Mesh, Failure>> {
        None
    }
}

/// ブーリアンを持たない基準。wasm の大きさの差を測るのに使う
pub struct Nothing;

impl Booleans for Nothing {
    fn name(&self) -> &'static str {
        "none"
    }

    fn boolean(&self, _op: BooleanOp, _first: &Mesh, _second: &Mesh) -> Result<Mesh, Failure> {
        Err(Failure::Unsupported("boolean"))
    }
}

/// 候補を、[`Kernel`] の形で使う
pub struct Candidate<B>(pub B);

impl<B: Booleans> Kernel for Candidate<B> {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn evaluate(&self, shape: &Shape, tolerance: Tolerance, cx: &mut Cx) -> Result<Mesh, Failure> {
        cx.check(Kind::Work, 1)?;
        let mesh = match shape {
            Shape::Boolean { op, first, second } => {
                if let Some(result) = self.0.shaped(*op, first, second, tolerance) {
                    result?
                } else {
                    let a = self.evaluate(first, tolerance, cx)?;
                    match (op, second.as_ref()) {
                        (BooleanOp::Difference, Shape::HalfSpace { plane }) => {
                            self.0.clip(&a, plane, true)?
                        }
                        (BooleanOp::Intersection, Shape::HalfSpace { plane }) => {
                            self.0.clip(&a, plane, false)?
                        }
                        (_, Shape::HalfSpace { .. }) => {
                            return Err(Failure::Unsupported("half_space"));
                        }
                        _ => {
                            let b = self.evaluate(second, tolerance, cx)?;
                            self.0.boolean(*op, &a, &b)?
                        }
                    }
                }
            }
            Shape::Mapped { transform, shape } => {
                if !transform.is_finite() {
                    return Err(Failure::Invalid("transform"));
                }
                self.evaluate(shape, tolerance, cx)?.transformed(transform)
            }
            Shape::Group(shapes) => {
                let mut mesh = Mesh::default();
                for shape in shapes {
                    mesh.append(&self.evaluate(shape, tolerance, cx)?);
                }
                mesh
            }
            Shape::HalfSpace { .. } => return Err(Failure::Unsupported("half_space")),
            other => primitives::evaluate(other, tolerance)
                .unwrap_or(Err(Failure::Unsupported("shape")))?,
        };
        mesh.validate()?;
        cx.check(Kind::Segments, mesh.triangles.len() as u64)?;
        Ok(mesh)
    }
}

/// 自前: 同じ向きに押し出した断面どうしで、2 つめが 1 つめの厚さを覆うときだけ、断面の 2D の
/// ブーリアン（i_overlay）にする。ほかのブーリアンはしない
#[cfg(feature = "own")]
pub struct Own;

#[cfg(feature = "own")]
/// 断面に垂直な押し出し: 断面の平面の配置、断面（穴を持つ多角形の集まり）、押し出す長さ
struct Prism {
    frame: Transform,
    shapes: Vec<Vec<Vec<Xy>>>,
    depth: f64,
}

#[cfg(feature = "own")]
/// 押し出しと、押し出しから押し出しを引いた形を、断面の 2D のブーリアンで求める
fn as_prism(shape: &Shape, tolerance: Tolerance) -> Option<Prism> {
    use i_overlay::core::fill_rule::FillRule;
    use i_overlay::core::overlay_rule::OverlayRule;
    use i_overlay::float::single::SingleFloatOverlay;
    match shape {
        Shape::Extrusion {
            profile,
            position,
            direction,
            depth,
        } => {
            let d = normalize(*direction)?;
            if d[0] != 0.0 || d[1] != 0.0 || d[2] <= 0.0 {
                return None;
            }
            Some(Prism {
                frame: *position,
                shapes: vec![primitives::rings(profile, tolerance).ok()?],
                depth: *depth,
            })
        }
        Shape::Boolean {
            op: BooleanOp::Difference,
            first,
            second,
        } => {
            let a = as_prism(first, tolerance)?;
            let b = as_prism(second, tolerance)?;
            let (za, zb) = (a.frame.column(2), b.frame.column(2));
            if dot(cross(za, zb), cross(za, zb)) > 1e-24 || dot(za, zb) <= 0.0 {
                return None;
            }
            // 2 つめが、1 つめの厚さを覆うときだけ
            let offset = dot(zumen_geom3d::sub(b.frame.column(3), a.frame.column(3)), za);
            if !(offset <= 0.0 && offset + b.depth >= a.depth) {
                return None;
            }
            // 2 つめの断面を、1 つめの断面の平面に写す
            let to_a =
                |p: &Xy| zumen_geom3d::Plane::project(&a.frame, b.frame.point([p[0], p[1], 0.0]));
            let clip: Vec<Vec<Vec<[f64; 2]>>> = b
                .shapes
                .iter()
                .map(|s| s.iter().map(|r| r.iter().map(to_a).collect()).collect())
                .collect();
            let shapes = a
                .shapes
                .overlay(&clip, OverlayRule::Difference, FillRule::EvenOdd);
            Some(Prism {
                frame: a.frame,
                shapes,
                depth: a.depth,
            })
        }
        _ => None,
    }
}

#[cfg(feature = "own")]
impl Booleans for Own {
    fn name(&self) -> &'static str {
        "own (i_overlay)"
    }

    fn boolean(&self, _op: BooleanOp, _first: &Mesh, _second: &Mesh) -> Result<Mesh, Failure> {
        Err(Failure::Unsupported("boolean"))
    }

    fn clip(&self, _first: &Mesh, _plane: &Plane, _keep_front: bool) -> Result<Mesh, Failure> {
        Err(Failure::Unsupported("half_space"))
    }

    fn shaped(
        &self,
        op: BooleanOp,
        first: &Shape,
        second: &Shape,
        tolerance: Tolerance,
    ) -> Option<Result<Mesh, Failure>> {
        let shape = Shape::Boolean {
            op,
            first: Box::new(first.clone()),
            second: Box::new(second.clone()),
        };
        let prism = as_prism(&shape, tolerance)?;
        let mut mesh = Mesh::default();
        for rings in &prism.shapes {
            match primitives::prism(rings, [0.0, 0.0, prism.depth]) {
                Ok(part) => mesh.append(&part.transformed(&prism.frame)),
                Err(error) => return Some(Err(error)),
            }
        }
        Some(Ok(mesh))
    }
}

#[cfg(feature = "manifold")]
/// manifold-rust（Manifold の Rust への移植）
pub struct ManifoldRust;

#[cfg(feature = "manifold")]
fn to_manifold(mesh: &Mesh) -> manifold_rust::manifold::Manifold {
    let mut gl = manifold_rust::types::MeshGL64::default();
    gl.num_prop = 3;
    gl.vert_properties = mesh.positions.iter().flatten().copied().collect();
    gl.tri_verts = mesh
        .triangles
        .iter()
        .flatten()
        .map(|i| u64::from(*i))
        .collect();
    manifold_rust::manifold::Manifold::from_mesh_gl64(&gl)
}

#[cfg(feature = "manifold")]
fn from_manifold(m: &manifold_rust::manifold::Manifold) -> Result<Mesh, Failure> {
    if m.status() != manifold_rust::types::Error::NoError {
        return Err(Failure::Failed(format!("{:?}", m.status())));
    }
    let gl = m.get_mesh_gl64(-1);
    let stride = gl.num_prop as usize;
    Ok(Mesh {
        positions: gl
            .vert_properties
            .chunks_exact(stride)
            .map(|p| [p[0], p[1], p[2]])
            .collect(),
        triangles: gl
            .tri_verts
            .chunks_exact(3)
            .map(|t| [t[0] as u32, t[1] as u32, t[2] as u32])
            .collect(),
        edges: Vec::new(),
    })
}

#[cfg(feature = "manifold")]
impl Booleans for ManifoldRust {
    fn name(&self) -> &'static str {
        "manifold-rust"
    }

    fn boolean(&self, op: BooleanOp, first: &Mesh, second: &Mesh) -> Result<Mesh, Failure> {
        let (a, b) = (to_manifold(&first.welded()), to_manifold(&second.welded()));
        for m in [&a, &b] {
            if m.status() != manifold_rust::types::Error::NoError {
                return Err(Failure::Failed(format!("input {:?}", m.status())));
            }
        }
        let r = match op {
            BooleanOp::Union => a.union(&b),
            BooleanOp::Difference => a.difference(&b),
            BooleanOp::Intersection => a.intersection(&b),
        };
        from_manifold(&r)
    }

    fn clip(&self, first: &Mesh, plane: &Plane, keep_front: bool) -> Result<Mesh, Failure> {
        let n = normalize(plane.normal).ok_or(Failure::Invalid("plane"))?;
        let n = if keep_front {
            n
        } else {
            zumen_geom3d::scale(n, -1.0)
        };
        let a = to_manifold(&first.welded());
        from_manifold(&a.trim_by_plane(n.into(), dot(n, plane.origin)))
    }
}

#[cfg(feature = "csgrs")]
/// csgrs（BSP の木）
pub struct Csgrs;

#[cfg(feature = "csgrs")]
type CsgMesh = csgrs::mesh::Mesh<()>;

#[cfg(feature = "csgrs")]
fn to_csg(mesh: &Mesh) -> CsgMesh {
    use csgrs::mesh::{polygon::Polygon, vertex::Vertex};
    use nalgebra::{Point3, Vector3};
    let polygons: Vec<Polygon<()>> = mesh
        .triangles
        .iter()
        .filter_map(|t| mesh.triangle(*t))
        .map(|[a, b, c]| {
            let n = normalize(cross(zumen_geom3d::sub(b, a), zumen_geom3d::sub(c, a)))
                .unwrap_or([0.0, 0.0, 1.0]);
            let v = |p: [f64; 3]| Vertex {
                pos: Point3::new(p[0], p[1], p[2]),
                normal: Vector3::new(n[0], n[1], n[2]),
            };
            Polygon::new(vec![v(a), v(b), v(c)], None)
        })
        .collect();
    CsgMesh::from_polygons(&polygons, None)
}

#[cfg(feature = "csgrs")]
impl Booleans for Csgrs {
    fn name(&self) -> &'static str {
        "csgrs"
    }

    fn boolean(&self, op: BooleanOp, first: &Mesh, second: &Mesh) -> Result<Mesh, Failure> {
        use csgrs::traits::CSG;
        let (a, b) = (to_csg(first), to_csg(second));
        let r = match op {
            BooleanOp::Union => a.union(&b),
            BooleanOp::Difference => a.difference(&b),
            BooleanOp::Intersection => a.intersection(&b),
        };
        let mut mesh = Mesh::default();
        for polygon in &r.polygons {
            for tri in polygon.triangulate() {
                let base = mesh.positions.len() as u32;
                mesh.positions
                    .extend(tri.iter().map(|v| [v.pos.x, v.pos.y, v.pos.z]));
                mesh.triangles.push([base, base + 1, base + 2]);
            }
        }
        Ok(mesh)
    }
}
