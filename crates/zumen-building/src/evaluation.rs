//! 所有する建物モデルを、要素の ID と出所を保った開口付きメッシュへ評価する。

use crate::{
    Building, ContextPlacement, ElementRef, GeometryData, GeometryDefinition, GeometryLinks,
    GeometryRef, HalfSpaceLimit, Id, Opening,
};
use std::{collections::BTreeMap, sync::Arc};
use zumen_base::{Cx, Diagnostic, Key, Kind, Ref, codes};
use zumen_geom3d::{
    BooleanOp, Curve, Failure, ManifoldKernel, Mesh, MeshChanges, Profile, Shape, Tolerance,
    Transform,
};

mod boolean;
pub(crate) mod curves;
mod openings;
mod shape;
pub(crate) mod values;
use values::{inverse, mapping, placement, reserve, unit};

/// 建物の形を評価する設定。結果の長さの単位はメートル。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// 曲線を分ける弦の誤差（m）。既定は 1 mm
    pub chord: f64,
    /// 母体から開口を引くか
    pub openings: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            chord: 0.001,
            openings: true,
        }
    }
}

/// 文書の長さ 1 単位をメートルへ換算する倍率。単位不明をメートルとはみなさない。
pub fn length_scale(unit: &crate::Unit) -> Result<f64, Failure> {
    values::unit(Some(unit), "LENGTHUNIT")
}

/// 1 つの部品。写像だけが違う同じ項目はメッシュを共有する。
#[derive(Clone, Debug, PartialEq)]
pub struct MeshPart {
    /// 元の GeometryData の項目。永続 ID は所属する要素の ID
    pub item: GeometryRef,
    /// メートルの局所座標で評価した形
    pub mesh: Arc<Mesh>,
    /// 局所座標からプロジェクトの座標（m）への変換
    pub placement: Transform,
    /// 評価に使った実寸の許容差
    pub tolerance: Tolerance,
    frame: Transform,
}

/// 1 要素の評価結果。開口の要素自体も結果に残す。
#[derive(Clone, Debug, PartialEq)]
pub struct ElementMesh {
    /// 建物の要素の参照
    pub element: ElementRef,
    /// 元の要素の ID
    pub id: Id,
    /// 元の項目順の部品
    pub parts: Vec<MeshPart>,
    /// 未対応の項目や失敗による代替がなかったか
    pub complete: bool,
}

#[derive(Clone)]
struct LocalPart {
    item: GeometryRef,
    mesh: Arc<Mesh>,
    transform: Transform,
}

fn copy_parts(parts: &[LocalPart], cx: &mut Cx) -> Result<Vec<LocalPart>, Failure> {
    let mut out = Vec::new();
    reserve(&mut out, parts.len(), 128, cx)?;
    cx.check(Kind::Work, parts.len() as u64)?;
    out.extend_from_slice(parts);
    Ok(out)
}

fn transformed(mesh: &Mesh, t: Transform, cx: &mut Cx) -> Result<Mesh, Failure> {
    if !t.is_finite() || !t.determinant().is_finite() || t.determinant() == 0.0 {
        return Err(Failure::Invalid("placement"));
    }
    cx.check(
        Kind::Work,
        (mesh.positions.len() + mesh.triangles.len() + mesh.edges.len()) as u64,
    )?;
    cx.check(
        Kind::DecodedBytes,
        mesh.positions.len() as u64 * 24
            + mesh.triangles.len() as u64 * 12
            + mesh.edges.len() as u64 * 8,
    )?;
    let mesh = mesh.transformed(&t);
    mesh.validate()?;
    Ok(mesh)
}

fn error_reason(error: &Failure) -> &str {
    match error {
        Failure::Unsupported(s) | Failure::Invalid(s) => s,
        Failure::Failed(s) => s,
        Failure::Stopped(_) => "stopped",
    }
}

fn failed(
    error: Failure,
    kind: &str,
    place: Ref,
    fallback: bool,
    cx: &mut Cx,
) -> Result<(), Failure> {
    if matches!(error, Failure::Stopped(_)) {
        return Err(error);
    }
    cx.report(
        Diagnostic::new(if fallback {
            codes::BUILDING_BOOLEAN_FAILED
        } else {
            codes::BUILDING_SHAPE_DROPPED
        })
        .with("kind", kind)
        .with("reason", error_reason(&error))
        .at(place),
    );
    cx.check(Kind::Work, 0)?;
    Ok(())
}

struct Walker<'a> {
    data: &'a GeometryData,
    place: Ref,
    cache: BTreeMap<(u32, u64, u64), Vec<LocalPart>>,
    complete: bool,
}

impl Walker<'_> {
    fn place(&self, at: GeometryRef) -> Ref {
        let place = self.place.clone();
        match self
            .data
            .nodes
            .get(at.0 as usize)
            .and_then(|n| n.source.instance)
        {
            Some(instance) => place.with_key(Key::Instance(instance)),
            None => place,
        }
    }

    fn node(
        &mut self,
        at: GeometryRef,
        tol: Tolerance,
        cx: &mut Cx,
    ) -> Result<Vec<LocalPart>, Failure> {
        cx.check(Kind::Work, 1)?;
        let key = (at.0, tol.chord.to_bits(), tol.length.to_bits());
        if let Some(parts) = self.cache.get(&key) {
            return copy_parts(parts, cx);
        }
        cx.enter()?;
        let result = self.node_inner(at, tol, cx);
        cx.leave();
        let parts = match result {
            Ok(parts) => parts,
            Err(error) => {
                let kind = self
                    .data
                    .nodes
                    .get(at.0 as usize)
                    .map(|n| shape::kind(&n.definition))
                    .unwrap_or("reference");
                failed(error, kind, self.place(at), false, cx)?;
                self.complete = false;
                Vec::new()
            }
        };
        cx.check(Kind::DecodedBytes, 128)?;
        self.cache.insert(key, copy_parts(&parts, cx)?);
        Ok(parts)
    }

    fn node_inner(
        &mut self,
        at: GeometryRef,
        tol: Tolerance,
        cx: &mut Cx,
    ) -> Result<Vec<LocalPart>, Failure> {
        let data = self.data;
        let node = data
            .nodes
            .get(at.0 as usize)
            .ok_or(Failure::Invalid("geometry_reference"))?;
        match &node.links {
            GeometryLinks::Definition(items)
            | GeometryLinks::Representation { items, .. }
            | GeometryLinks::Set { items, .. } => {
                let mut out = Vec::new();
                for &item in items {
                    let parts = self.node(item, tol, cx)?;
                    reserve(&mut out, parts.len(), 128, cx)?;
                    out.extend(parts);
                }
                Ok(out)
            }
            GeometryLinks::Map(child) => self.node(*child, tol, cx),
            GeometryLinks::Mapped(child) => {
                let GeometryDefinition::Mapping {
                    definition,
                    length_unit,
                } = &node.definition
                else {
                    return Err(Failure::Invalid("mapping_definition"));
                };
                let transform = mapping(definition, unit(length_unit.as_ref(), "LENGTHUNIT")?)?;
                let mut parts = self.node(*child, tol, cx)?;
                cx.check(Kind::Work, parts.len() as u64)?;
                for part in &mut parts {
                    part.transform = part.transform.then(&transform);
                }
                Ok(parts)
            }
            GeometryLinks::Boolean {
                operation,
                first,
                second,
            } => self.boolean(at, *operation, [*first, *second], tol, cx),
            GeometryLinks::Leaf => {
                let mut changes = MeshChanges::default();
                let shape = shape::evaluate(&node.definition, tol, &mut changes, cx)?;
                let mut out = Vec::new();
                let shapes = if let Shape::Group(shapes) = shape {
                    shapes
                } else {
                    cx.check(Kind::DecodedBytes, 256)?;
                    vec![shape]
                };
                reserve(&mut out, shapes.len(), 128, cx)?;
                for shape in shapes {
                    let evaluated = match ManifoldKernel.evaluate_with_changes(&shape, tol, cx) {
                        Ok(value) => value,
                        Err(error) => {
                            failed(error, shape.kind(), self.place(at), false, cx)?;
                            self.complete = false;
                            continue;
                        }
                    };
                    evaluated.changes.report(Some(&self.place(at)), tol, cx)?;
                    changes.report(Some(&self.place(at)), tol, cx)?;
                    cx.check(Kind::DecodedBytes, 128)?;
                    out.push(LocalPart {
                        item: at,
                        mesh: Arc::new(evaluated.mesh),
                        transform: Transform::IDENTITY,
                    });
                }
                Ok(out)
            }
        }
    }
}

fn cut_half_space(
    op: BooleanOp,
    mesh: &Mesh,
    value: &crate::PlaneHalfSpace,
    boundary: Option<&crate::CurveGraph>,
    factor: f64,
    tol: Tolerance,
    cx: &mut Cx,
) -> Result<zumen_geom3d::EvaluatedMesh, Failure> {
    let plane = shape::half_space(value, factor)?;
    let HalfSpaceLimit::Polygonal {
        position,
        boundary: at,
    } = &value.limit
    else {
        return ManifoldKernel.half_space(op, mesh, &plane, tol, cx);
    };
    let graph = boundary.ok_or(Failure::Invalid("half_space_boundary"))?;
    let mut changes = MeshChanges::default();
    let points = curves::evaluate(graph, *at, tol.chord, &mut changes, cx)?;
    let frame = placement(Some(position), factor)?;
    let back = inverse(frame)?;
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    cx.check(Kind::Work, mesh.positions.len() as u64)?;
    for &p in &mesh.positions {
        let z = back.point(p)[2];
        min = min.min(z);
        max = max.max(z);
    }
    if !min.is_finite() || !max.is_finite() {
        return Err(Failure::Invalid("empty_half_space"));
    }
    let margin = (max - min).max(tol.length * 4.0);
    let mut ring = Vec::new();
    reserve(&mut ring, points.len(), 16, cx)?;
    ring.extend(points.into_iter().map(|p| [p[0], p[1]]));
    let shape = Shape::Extrusion {
        profile: Profile {
            outer: Curve::Polygon(ring),
            holes: Vec::new(),
        },
        position: Transform::translation([0.0, 0.0, min - margin]).then(&frame),
        direction: [0.0, 0.0, 1.0],
        depth: max - min + 2.0 * margin,
    };
    let prism = ManifoldKernel.evaluate_with_changes(&shape, tol, cx)?;
    let limited =
        ManifoldKernel.half_space(BooleanOp::Intersection, &prism.mesh, &plane, tol, cx)?;
    let mut out = ManifoldKernel.boolean(op, mesh, &limited.mesh, tol, cx)?;
    out.changes.dropped_faces += prism.changes.dropped_faces + limited.changes.dropped_faces;
    out.changes.split_edges += prism.changes.split_edges + limited.changes.split_edges;
    out.changes.snapped_vertices +=
        prism.changes.snapped_vertices + limited.changes.snapped_vertices;
    out.changes.approximated_curves += changes.approximated_curves
        + prism.changes.approximated_curves
        + limited.changes.approximated_curves;
    Ok(out)
}

fn context(node: &GeometryDefinition, options: Options) -> Result<(Tolerance, Transform), Failure> {
    let mut tolerance = Tolerance {
        chord: options.chord,
        length: 1e-9,
    };
    let mut world = Transform::IDENTITY;
    if let GeometryDefinition::Context {
        definition,
        length_unit,
    } = node
    {
        let factor = unit(length_unit.as_ref(), "LENGTHUNIT")?;
        if let Some(p) = definition.geometric.precision {
            tolerance.length = p * factor;
        }
        world = match &definition.geometric.world_coordinate_system {
            ContextPlacement::Space(value) => placement(Some(value), factor)?,
            ContextPlacement::Plane(_) => return Err(Failure::Unsupported("plan_context")),
        };
    }
    if !tolerance.length.is_finite() || tolerance.length <= 0.0 {
        return Err(Failure::Invalid("precision"));
    }
    Ok((tolerance, world))
}

fn element(
    building: &Building,
    at: ElementRef,
    options: Options,
    cx: &mut Cx,
) -> Result<ElementMesh, Failure> {
    let id = building.id(at).ok_or(Failure::Invalid("element"))?;
    let mut out = ElementMesh {
        element: at,
        id,
        parts: Vec::new(),
        complete: true,
    };
    let Some(geometry) = building.geometry(at) else {
        return Ok(out);
    };
    let data = geometry.data();
    let Some(root) = data.root else {
        return Ok(out);
    };
    let mut walker = Walker {
        data,
        place: building.place(at),
        cache: BTreeMap::new(),
        complete: true,
    };
    let root_node = data
        .nodes
        .get(root.0 as usize)
        .ok_or(Failure::Invalid("root"))?;
    let representations = match &root_node.links {
        GeometryLinks::Definition(items) => items.as_slice(),
        _ => core::slice::from_ref(&root),
    };
    let opening = matches!(building.components.opening.get(at), Some(Opening::Voids(_)));
    let named = |at: &GeometryRef, name: &str| {
        data.nodes.get(at.0 as usize).is_some_and(|n|matches!(&n.links,GeometryLinks::Representation{identifier:Some(id),..} if id==name))
    };
    let body = representations.iter().any(|r| named(r, "Body"));
    let world = match building.world_placement(at, cx)? {
        Some(value) => {
            let factor = unit(
                building
                    .document
                    .units
                    .iter()
                    .find(|u| u.kind == "LENGTHUNIT"),
                "LENGTHUNIT",
            )?;
            let mut m = Transform(value.0);
            for row in &mut m.0 {
                row[3] *= factor;
            }
            m
        }
        None if building.components.placement.get(at).is_none() => Transform::IDENTITY,
        None => return Err(Failure::Invalid("element_placement")),
    };
    for &representation in representations {
        let node = data
            .nodes
            .get(representation.0 as usize)
            .ok_or(Failure::Invalid("representation"))?;
        if matches!(node.links, GeometryLinks::Representation { .. })
            && !named(
                &representation,
                if body {
                    "Body"
                } else if opening {
                    "Reference"
                } else {
                    "Body"
                },
            )
        {
            continue;
        }
        let (tolerance, context) = match context(&node.definition, options) {
            Ok(value) => value,
            Err(error) => {
                failed(error, "context", walker.place(representation), false, cx)?;
                walker.complete = false;
                continue;
            }
        };
        let frame = world.then(&context);
        for part in walker.node(representation, tolerance, cx)? {
            reserve(&mut out.parts, 1, 256, cx)?;
            out.parts.push(MeshPart {
                item: part.item,
                mesh: part.mesh,
                placement: part.transform.then(&frame),
                tolerance,
                frame,
            });
        }
    }
    if out.parts.is_empty() && walker.complete {
        failed(
            Failure::Unsupported("representation_missing"),
            "representation",
            walker.place(root),
            false,
            cx,
        )?;
        walker.complete = false;
    }
    out.complete = walker.complete;
    Ok(out)
}

/// 建物を要素・項目の順に評価する。停止時は部分結果を返さず、形の失敗は診断と代替を返す。
pub fn evaluate(
    building: &Building,
    options: Options,
    cx: &mut Cx,
) -> Result<Vec<ElementMesh>, Failure> {
    evaluate_selected(building, options, |_| true, cx)
}

pub(crate) fn evaluate_selected(
    building: &Building,
    options: Options,
    include: impl Fn(ElementRef) -> bool,
    cx: &mut Cx,
) -> Result<Vec<ElementMesh>, Failure> {
    if !options.chord.is_finite() || options.chord <= 0.0 {
        return Err(Failure::Invalid("chord"));
    }
    let mut elements = Vec::new();
    reserve(&mut elements, building.elements().len(), 128, cx)?;
    for (index, _) in building.elements().iter().enumerate() {
        cx.check(Kind::Work, 1)?;
        let at = ElementRef(index as u32);
        if !include(at) {
            elements.push(ElementMesh {
                element: at,
                id: building.id(at).ok_or(Failure::Invalid("element"))?,
                parts: Vec::new(),
                complete: true,
            });
            continue;
        }
        let value = match element(building, at, options, cx) {
            Ok(value) => value,
            Err(error) => {
                failed(error, "element", building.place(at), false, cx)?;
                ElementMesh {
                    element: at,
                    id: building.id(at).ok_or(Failure::Invalid("element"))?,
                    parts: Vec::new(),
                    complete: false,
                }
            }
        };
        elements.push(value);
    }
    if options.openings {
        openings::apply(building, &mut elements, cx)?;
    }
    cx.check(Kind::Work, 0)?;
    Ok(elements)
}
