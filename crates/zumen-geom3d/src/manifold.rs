//! manifold-rust 0.19.1 の Exact 経路。平面での切断も有限の箱との積・差で求める。

use manifold_rust::{
    manifold::Manifold,
    types::{BooleanEngine, Error, MeshGL64, OpType},
};
use zumen_base::{Cx, Kind};

use crate::{BooleanOp, Failure, Kernel, Mesh, Plane, Shape, Tolerance, Transform, dot, sub};

mod budget;
mod primitives;
mod topology;

/// 評価時に形を整えた件数。元の形・ID は変更しない。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeshChanges {
    /// 面積のない多角形・三角形を除いた数
    pub dropped_faces: u64,
    /// T 字の継ぎ目で辺を分けた数
    pub split_edges: u64,
    /// ブーリアンの前に格子へ移した点の数
    pub snapped_vertices: u64,
    /// 曲線・回転面を弦の誤差で分割した数
    pub approximated_curves: u64,
}

impl MeshChanges {
    /// 元の要素・項目の場所を付け、カタログの診断へ写す。
    pub fn report(
        &self,
        place: Option<&zumen_base::Ref>,
        tolerance: Tolerance,
        cx: &mut Cx,
    ) -> Result<(), Failure> {
        use zumen_base::{Diagnostic, codes};
        for (count, code) in [
            (self.dropped_faces, codes::GEOM3D_FACE_DROPPED),
            (self.split_edges, codes::GEOM3D_EDGE_EQUIVALENT),
            (self.snapped_vertices, codes::GEOM3D_GRID_APPROXIMATED),
            (self.approximated_curves, codes::GEOM3D_CURVE_APPROXIMATED),
        ] {
            for _ in 0..count {
                budget::work(cx, 1)?;
                let mut diagnostic = Diagnostic::new(code);
                if code == codes::GEOM3D_GRID_APPROXIMATED {
                    diagnostic = diagnostic.with("spacing", tolerance.length);
                }
                if code == codes::GEOM3D_CURVE_APPROXIMATED {
                    diagnostic = diagnostic.with("chord", tolerance.chord);
                }
                if let Some(place) = place {
                    diagnostic = diagnostic.at(place.clone());
                }
                cx.report(diagnostic);
            }
        }
        budget::work(cx, 0)
    }
}

/// 形の評価結果と、使った許容差・手当ての件数
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedMesh {
    /// 評価した形
    pub mesh: Mesh,
    /// 形と同じ単位の許容差
    pub tolerance: Tolerance,
    /// 変更の明細。利用側で元の要素・形の位置へ結び付ける
    pub changes: MeshChanges,
}

/// 選定した 3D カーネル。既定のブーリアンエンジンや並列実行の設定に依存しない。
#[derive(Clone, Copy, Debug, Default)]
pub struct ManifoldKernel;

impl ManifoldKernel {
    /// 評価済みの形と平面の裏の半空間の積・差。結果は有限の箱を使って求める。
    pub fn half_space(
        &self,
        op: BooleanOp,
        mesh: &Mesh,
        plane: &Plane,
        tolerance: Tolerance,
        cx: &mut Cx,
    ) -> Result<EvaluatedMesh, Failure> {
        validate_tolerance(tolerance)?;
        let mut changes = MeshChanges::default();
        let mut input = budget::mesh_copy(mesh, cx)?;
        topology::clean(&mut input, &mut changes, cx)?;
        let mut mesh = self.clip(op, &mut input, plane, tolerance, &mut changes, cx)?;
        topology::edges(&mut mesh, cx)?;
        Ok(EvaluatedMesh {
            mesh,
            tolerance,
            changes,
        })
    }
    /// 向きのそろった閉じた殻で、正の体積を持つか。辺で接する殻も許す。
    pub fn is_solid(&self, mesh: &Mesh, cx: &mut Cx) -> Result<bool, Failure> {
        budget::work(
            cx,
            (mesh.positions.len() + mesh.triangles.len() + mesh.edges.len()) as u64,
        )?;
        mesh.validate()?;
        Ok(topology::watertight(mesh, cx)? && topology::positive_volume(mesh, cx)?)
    }

    /// 変更の件数も返す。診断の出所が分かる利用側は、この値へ場所を付けて報告する。
    pub fn evaluate_with_changes(
        &self,
        shape: &Shape,
        tolerance: Tolerance,
        cx: &mut Cx,
    ) -> Result<EvaluatedMesh, Failure> {
        validate_tolerance(tolerance)?;
        let mut changes = MeshChanges::default();
        let mut mesh = self.evaluate_inner(shape, tolerance, &mut changes, cx)?;
        topology::edges(&mut mesh, cx)?;
        Ok(EvaluatedMesh {
            mesh,
            tolerance,
            changes,
        })
    }

    /// 評価済みの 2 つの形を演算する。開いた殻と体積のない形は受け付けない。
    pub fn boolean(
        &self,
        op: BooleanOp,
        first: &Mesh,
        second: &Mesh,
        tolerance: Tolerance,
        cx: &mut Cx,
    ) -> Result<EvaluatedMesh, Failure> {
        validate_tolerance(tolerance)?;
        let mut changes = MeshChanges::default();
        let mut a = budget::mesh_copy(first, cx)?;
        let mut b = budget::mesh_copy(second, cx)?;
        let mut mesh = self.boolean_inner(op, &mut a, &mut b, tolerance, &mut changes, cx)?;
        topology::edges(&mut mesh, cx)?;
        Ok(EvaluatedMesh {
            mesh,
            tolerance,
            changes,
        })
    }

    fn evaluate_inner(
        &self,
        shape: &Shape,
        tol: Tolerance,
        changes: &mut MeshChanges,
        cx: &mut Cx,
    ) -> Result<Mesh, Failure> {
        cx.enter()?;
        let result = (|| {
            budget::work(cx, 1)?;
            let mut mesh = match shape {
                Shape::Boolean { op, first, second } => {
                    let mut a = self.evaluate_inner(first, tol, changes, cx)?;
                    if let Shape::HalfSpace { plane } = second.as_ref() {
                        self.clip(*op, &mut a, plane, tol, changes, cx)?
                    } else {
                        let mut b = self.evaluate_inner(second, tol, changes, cx)?;
                        self.boolean_inner(*op, &mut a, &mut b, tol, changes, cx)?
                    }
                }
                Shape::Mapped { transform, shape } => {
                    let mut mesh = self.evaluate_inner(shape, tol, changes, cx)?;
                    primitives::transform(&mut mesh, transform, cx)?;
                    mesh
                }
                Shape::Group(shapes) => {
                    let mut mesh = Mesh::default();
                    for shape in shapes {
                        budget::append(
                            &mut mesh,
                            &self.evaluate_inner(shape, tol, changes, cx)?,
                            cx,
                        )?;
                    }
                    mesh
                }
                shape => primitives::evaluate(shape, tol, changes, cx)?,
            };
            topology::clean(&mut mesh, changes, cx)?;
            topology::repair(&mut mesh, changes, cx)?;
            Ok(mesh)
        })();
        cx.leave();
        result
    }

    fn boolean_inner(
        &self,
        op: BooleanOp,
        a: &mut Mesh,
        b: &mut Mesh,
        tol: Tolerance,
        changes: &mut MeshChanges,
        cx: &mut Cx,
    ) -> Result<Mesh, Failure> {
        for mesh in [&mut *a, &mut *b] {
            topology::snapped(mesh, tol.length, changes, cx)?;
            topology::clean(mesh, changes, cx)?;
            topology::repair(mesh, changes, cx)?;
            if mesh.triangles.is_empty() {
                continue;
            }
            if !topology::watertight(mesh, cx)? {
                return Err(Failure::Invalid("open_shell"));
            }
            if !topology::positive_volume(mesh, cx)? {
                return Err(Failure::Invalid("nonpositive_volume"));
            }
            if mesh.positions.iter().flatten().any(|v| v.abs() > 1e12) {
                return Err(Failure::Invalid("coordinate_range"));
            }
        }
        // カーネル内部の確保・反復には Cx を渡せない。入力上限と三角形の全組で先に制限する。
        // 正確な実使用メモリではなく、同じ入力から決まる作業領域の予約量。
        let n = a.triangles.len() as u64;
        let m = b.triangles.len() as u64;
        if n + m > 32_768 || n.saturating_mul(m) > 4_194_304 {
            return Err(Failure::Invalid("boolean_size"));
        }
        budget::work(
            cx,
            (n + m)
                .saturating_mul(32)
                .saturating_add(n.saturating_mul(m).saturating_mul(16)),
        )?;
        budget::bytes(
            cx,
            (n + m)
                .saturating_mul(4096)
                .saturating_add(n.saturating_mul(m).saturating_mul(128)),
        )?;
        let a = to_manifold(a, cx)?;
        let b = to_manifold(b, cx)?;
        let op = match op {
            BooleanOp::Union => OpType::Add,
            BooleanOp::Difference => OpType::Subtract,
            BooleanOp::Intersection => OpType::Intersect,
        };
        let result = a.boolean_with_engine(&b, op, BooleanEngine::Exact);
        budget::work(cx, 0)?;
        let mut mesh = from_manifold(&result, cx)?;
        topology::clean(&mut mesh, changes, cx)?;
        Ok(mesh)
    }

    fn clip(
        &self,
        op: BooleanOp,
        mesh: &mut Mesh,
        plane: &Plane,
        tol: Tolerance,
        changes: &mut MeshChanges,
        cx: &mut Cx,
    ) -> Result<Mesh, Failure> {
        if op == BooleanOp::Union {
            return Err(Failure::Unsupported("half_space_union"));
        }
        if !mesh.triangles.is_empty() && !self.is_solid(mesh, cx)? {
            return Err(Failure::Invalid("non_solid_half_space"));
        }
        let frame = plane.frame().ok_or(Failure::Invalid("plane"))?;
        if mesh.positions.is_empty() {
            return Ok(Mesh::default());
        }
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        budget::work(cx, mesh.positions.len() as u64)?;
        for &p in &mesh.positions {
            let d = sub(p, plane.origin);
            let p = [
                dot(d, frame.column(0)),
                dot(d, frame.column(1)),
                dot(d, frame.column(2)),
            ];
            for ((lo, hi), p) in lo.iter_mut().zip(hi.iter_mut()).zip(p) {
                *lo = lo.min(p);
                *hi = hi.max(p);
            }
        }
        if !lo.iter().chain(&hi).all(|v| v.is_finite()) {
            return Err(Failure::Invalid("plane_range"));
        }
        if hi[2] < 0.0 {
            return if op == BooleanOp::Difference {
                Ok(Mesh::default())
            } else {
                budget::mesh_copy(mesh, cx)
            };
        }
        if lo[2] > 0.0 {
            return if op == BooleanOp::Intersection {
                Ok(Mesh::default())
            } else {
                budget::mesh_copy(mesh, cx)
            };
        }
        let margin = (hi[0] - lo[0])
            .max(hi[1] - lo[1])
            .max(hi[2] - lo[2])
            .max(tol.length * 4.0);
        let bottom = lo[2] - margin;
        let left = lo[0] - margin;
        let right = hi[0] + margin;
        let back = lo[1] - margin;
        let front = hi[1] + margin;
        budget::bytes(cx, 24 + 4 * 16)?;
        let rings = vec![vec![
            [left, back],
            [right, back],
            [right, front],
            [left, front],
        ]];
        let mut box_mesh = primitives::prism(&rings, [0.0, 0.0, -bottom], changes, cx)?;
        primitives::transform(
            &mut box_mesh,
            &Transform::translation([0.0, 0.0, bottom]).then(&frame),
            cx,
        )?;
        self.boolean_inner(op, mesh, &mut box_mesh, tol, changes, cx)
    }
}

impl Kernel for ManifoldKernel {
    fn name(&self) -> &'static str {
        "manifold-rust-0.19.1-exact"
    }

    fn evaluate(&self, shape: &Shape, tolerance: Tolerance, cx: &mut Cx) -> Result<Mesh, Failure> {
        let evaluated = self.evaluate_with_changes(shape, tolerance, cx)?;
        evaluated.changes.report(None, tolerance, cx)?;
        budget::work(cx, 0)?;
        Ok(evaluated.mesh)
    }
}

fn validate_tolerance(tolerance: Tolerance) -> Result<(), Failure> {
    if !(tolerance.chord.is_finite()
        && tolerance.chord > 0.0
        && tolerance.length.is_finite()
        && tolerance.length > 0.0)
    {
        return Err(Failure::Invalid("tolerance"));
    }
    Ok(())
}

fn to_manifold(mesh: &Mesh, cx: &mut Cx) -> Result<Manifold, Failure> {
    let mut gl = MeshGL64 {
        num_prop: 3,
        ..Default::default()
    };
    budget::reserve(&mut gl.vert_properties, mesh.positions.len() * 3, 8, cx)?;
    budget::reserve(&mut gl.tri_verts, mesh.triangles.len() * 3, 8, cx)?;
    gl.vert_properties
        .extend(mesh.positions.iter().flatten().copied());
    gl.tri_verts
        .extend(mesh.triangles.iter().flatten().map(|&i| u64::from(i)));
    let result = Manifold::from_mesh_gl64(&gl);
    budget::work(cx, 0)?;
    status(&result)?;
    Ok(result)
}

fn status(mesh: &Manifold) -> Result<(), Failure> {
    if mesh.status() != Error::NoError {
        return Err(Failure::Failed(format!("manifold:{:?}", mesh.status())));
    }
    Ok(())
}

fn from_manifold(result: &Manifold, cx: &mut Cx) -> Result<Mesh, Failure> {
    status(result)?;
    if result.num_vert() > budget::MAX_POINTS || result.num_tri() > budget::MAX_TRIANGLES {
        return Err(Failure::Invalid("result_size"));
    }
    budget::work(cx, (result.num_vert() + result.num_tri()) as u64)?;
    budget::bytes(
        cx,
        result.num_vert() as u64 * 128 + result.num_tri() as u64 * 128,
    )?;
    cx.check(Kind::Segments, result.num_tri() as u64)?;
    let gl = result.get_mesh_gl64(-1);
    if gl.num_prop != 3
        || !gl.tri_verts.len().is_multiple_of(3)
        || !gl.vert_properties.len().is_multiple_of(3)
    {
        return Err(Failure::Invalid("mesh_layout"));
    }
    let mut mesh = Mesh::default();
    budget::reserve(&mut mesh.positions, gl.vert_properties.len() / 3, 24, cx)?;
    budget::reserve(&mut mesh.triangles, gl.tri_verts.len() / 3, 12, cx)?;
    mesh.positions
        .extend(gl.vert_properties.as_chunks::<3>().0.iter().copied());
    for &t in gl.tri_verts.as_chunks::<3>().0 {
        if t.iter().any(|&i| i >= mesh.positions.len() as u64) {
            return Err(Failure::Invalid("index"));
        }
        mesh.triangles.push(t.map(|i| i as u32));
    }
    Ok(mesh)
}
