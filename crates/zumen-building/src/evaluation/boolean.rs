//! 部品の差は順に取り、和は両側を 1 つにする。失敗時には演算前の部品を保つ。

use super::*;
use crate::BooleanOperation;
use zumen_geom3d::{EvaluatedMesh, MeshChanges};

fn merge<'a>(
    parts: impl IntoIterator<Item = &'a LocalPart>,
    tol: Tolerance,
    cx: &mut Cx,
) -> Result<EvaluatedMesh, Failure> {
    let mut parts = parts.into_iter();
    let first = parts.next().ok_or(Failure::Invalid("operand_missing"))?;
    let mut mesh = transformed(&first.mesh, first.transform, cx)?;
    let mut changes = MeshChanges::default();
    for part in parts {
        let other = transformed(&part.mesh, part.transform, cx)?;
        let next = ManifoldKernel.boolean(BooleanOp::Union, &mesh, &other, tol, cx)?;
        changes.dropped_faces += next.changes.dropped_faces;
        changes.split_edges += next.changes.split_edges;
        changes.snapped_vertices += next.changes.snapped_vertices;
        changes.approximated_curves += next.changes.approximated_curves;
        mesh = next.mesh;
    }
    Ok(EvaluatedMesh {
        mesh,
        tolerance: tol,
        changes,
    })
}

impl Walker<'_> {
    fn replace(
        &mut self,
        part: &mut LocalPart,
        result: Result<EvaluatedMesh, Failure>,
        at: GeometryRef,
        kind: &str,
        cx: &mut Cx,
    ) -> Result<(), Failure> {
        match result {
            Ok(out) => {
                out.changes
                    .report(Some(&self.place(at)), out.tolerance, cx)?;
                cx.check(Kind::DecodedBytes, 128)?;
                part.mesh = Arc::new(out.mesh);
                part.transform = Transform::IDENTITY;
            }
            Err(error) => {
                failed(error, kind, self.place(at), true, cx)?;
                self.complete = false;
            }
        }
        Ok(())
    }

    pub(super) fn boolean(
        &mut self,
        at: GeometryRef,
        operation: BooleanOperation,
        [first, second]: [GeometryRef; 2],
        tol: Tolerance,
        cx: &mut Cx,
    ) -> Result<Vec<LocalPart>, Failure> {
        let op = match operation {
            BooleanOperation::Union => BooleanOp::Union,
            BooleanOperation::Intersection => BooleanOp::Intersection,
            BooleanOperation::Difference => BooleanOp::Difference,
        };
        let mut a = self.node(first, tol, cx)?;
        if let Some(GeometryDefinition::HalfSpace {
            definition,
            boundary,
            length_unit,
        }) = self
            .data
            .nodes
            .get(second.0 as usize)
            .map(|n| &n.definition)
        {
            for part in &mut a {
                let result = (|| {
                    let mesh = transformed(&part.mesh, part.transform, cx)?;
                    cut_half_space(
                        op,
                        &mesh,
                        definition,
                        boundary.as_ref(),
                        unit(length_unit.as_ref(), "LENGTHUNIT")?,
                        tol,
                        cx,
                    )
                })();
                self.replace(part, result, at, "half_space", cx)?;
            }
            return Ok(a);
        }
        let b = self.node(second, tol, cx)?;
        if b.is_empty() || a.is_empty() {
            failed(
                Failure::Invalid("operand_missing"),
                "boolean",
                self.place(at),
                true,
                cx,
            )?;
            self.complete = false;
            return Ok(a);
        }
        if op == BooleanOp::Union {
            // A の各部品へ B を足すと B が重複するため、和だけは全体をまとめる。
            match merge(a.iter().chain(&b), tol, cx) {
                Ok(out) => {
                    if let Some(first) = a.first_mut() {
                        self.replace(first, Ok(out), at, "boolean", cx)?;
                        a.truncate(1);
                    }
                }
                Err(error) => {
                    failed(error, "boolean", self.place(at), true, cx)?;
                    self.complete = false;
                }
            }
        } else if op == BooleanOp::Intersection {
            match merge(&b, tol, cx) {
                Ok(rhs) => {
                    rhs.changes.report(Some(&self.place(at)), tol, cx)?;
                    for part in &mut a {
                        let result = (|| {
                            let mesh = transformed(&part.mesh, part.transform, cx)?;
                            ManifoldKernel.boolean(op, &mesh, &rhs.mesh, tol, cx)
                        })();
                        self.replace(part, result, at, "boolean", cx)?;
                    }
                }
                Err(error) => {
                    failed(error, "boolean", self.place(at), true, cx)?;
                    self.complete = false;
                }
            }
        } else {
            for part in &mut a {
                for other in &b {
                    let result = (|| {
                        let mesh = transformed(&part.mesh, part.transform, cx)?;
                        let tool = transformed(&other.mesh, other.transform, cx)?;
                        ManifoldKernel.boolean(op, &mesh, &tool, tol, cx)
                    })();
                    self.replace(part, result, at, "boolean", cx)?;
                }
            }
        }
        Ok(a)
    }
}
