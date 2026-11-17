//! 実寸の空間と公開 ID で要素を選び、オフセット・トリム・延長・面取り・フィレットの形と、
//! 切断点・近似・失敗の理由を返す。文書は変更しない。

use super::*;
use crate::{
    construction::{self, Built, Operation, Source},
    intersection, measurement,
    messages::{
        ArcGeometry, CircleGeometry, ConstructCut, ConstructEnd, ConstructOperation,
        ConstructReplacement, ConstructRequest, ConstructResult, Geometry, LineGeometry, Point2,
    },
};
use element_set::{Family, approximated, failed, report_failed, reserve, resolve};
use zumen_drawing::{Edge, ModelPoint};
use zumen_geom2d::topology as t;

const FAMILY: Family = Family {
    failed: codes::DRAWING_ELEMENT_CONSTRUCTION_FAILED,
    approximated: codes::DRAWING_ELEMENT_CONSTRUCTION_APPROXIMATED,
};

/// 操作ごとに必要なフィールドと、ほかの操作のフィールドを書かないことを確かめる。失敗はそのフィールドを指す。
fn fields(request: &ConstructRequest) -> Result<(), String> {
    let operation = request.operation;
    let corner = matches!(
        operation,
        ConstructOperation::Chamfer | ConstructOperation::Fillet
    );
    let edit = matches!(
        operation,
        ConstructOperation::Trim | ConstructOperation::Extend
    );
    for (present, wanted, name) in [
        (request.second.is_some(), corner, "/second"),
        (request.boundaries.is_some(), edit, "/boundaries"),
        (
            request.pick.is_some(),
            operation == ConstructOperation::Trim,
            "/pick",
        ),
        (
            request.end.is_some(),
            operation == ConstructOperation::Extend,
            "/end",
        ),
        (
            request.distance.is_some(),
            operation == ConstructOperation::Offset,
            "/distance",
        ),
        (
            request.distances.is_some(),
            operation == ConstructOperation::Chamfer,
            "/distances",
        ),
        (
            request.radius.is_some(),
            operation == ConstructOperation::Fillet,
            "/radius",
        ),
    ] {
        if present != wanted {
            return Err(name.to_owned());
        }
    }
    if let Some(second) = &request.second
        && second.id == request.target.id
    {
        return Err("/second".to_owned());
    }
    if let Some(distance) = request.distance
        && (!distance.is_finite() || distance == 0.0)
    {
        return Err("/distance".to_owned());
    }
    if let Some(distances) = &request.distances {
        if distances.len() != 2 {
            return Err("/distances".to_owned());
        }
        for (index, value) in distances.iter().enumerate() {
            if !value.is_finite() || *value <= 0.0 {
                return Err(format!("/distances/{index}"));
            }
        }
    }
    if let Some(radius) = request.radius
        && (!radius.is_finite() || radius <= 0.0)
    {
        return Err("/radius".to_owned());
    }
    if let Some(pick) = &request.pick
        && (!pick.x.is_finite() || !pick.y.is_finite())
    {
        return Err("/pick".to_owned());
    }
    Ok(())
}

fn geometry(shape: construction::Shape) -> Geometry {
    match shape {
        construction::Shape::Line(start, end) => Geometry::Line(LineGeometry {
            start: start.to_vec(),
            end: end.to_vec(),
        }),
        construction::Shape::Circle(center, radius) => Geometry::Circle(CircleGeometry {
            center: center.to_vec(),
            radius,
        }),
        construction::Shape::Arc {
            center,
            radius,
            start,
            end,
        } => Geometry::Arc(ArcGeometry {
            center: center.to_vec(),
            radius,
            start_angle: start,
            end_angle: end,
        }),
    }
}

/// 対象にできる形。線分・円・円弧に限り、面取りとフィレットは線分だけにする。
fn supported(shape: &Shape<ModelPoint>, line_only: bool) -> bool {
    match shape {
        Shape::Line { .. } | Shape::Curve(Edge::Line { .. }) => true,
        Shape::Circle { .. } | Shape::Curve(Edge::Arc { .. }) => !line_only,
        _ => false,
    }
}

impl Session {
    pub(super) fn construct(
        &self,
        request: &ConstructRequest,
        cx: &mut Cx<'_>,
    ) -> Result<ConstructResult, Failed> {
        let tolerance = request.tolerance.unwrap_or(0.001);
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(fail(cx, invalid_value("/tolerance")));
        }
        fields(request).map_err(|pointer| fail(cx, invalid_value(pointer)))?;
        let boundaries = request.boundaries.as_deref().unwrap_or_default();
        if request.boundaries.is_some() && boundaries.is_empty() {
            return Err(fail(cx, invalid_value("/boundaries")));
        }
        if boundaries.len() > t::MAX_REGION_EDGES {
            return Err(fail(cx, invalid_value("/boundaries")));
        }
        cx.check(
            Kind::Work,
            (boundaries.len() as u64).saturating_mul(boundaries.len() as u64 + 1),
        )
        .map_err(|stop| stopped(cx, stop))?;
        for (index, item) in boundaries.iter().enumerate() {
            if item.id == request.target.id
                || request
                    .second
                    .as_ref()
                    .is_some_and(|second| second.id == item.id)
                || boundaries
                    .iter()
                    .take(index)
                    .any(|other| other.id == item.id)
            {
                return Err(fail(cx, invalid_value(format!("/boundaries/{index}"))));
            }
        }
        let head = request.target.id.as_str();
        let (_, hash) = self.drawing_file(&request.doc, cx)?;
        cx.check(Kind::Work, 4).map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, 256)
            .map_err(|stop| stopped(cx, stop))?;
        let hash = hash.to_string();
        if request
            .expected_hash
            .as_ref()
            .is_some_and(|expected| expected != &hash)
        {
            return Err(fail(cx, invalid_value("/expected_hash")));
        }
        let drawing = self.lift(&request.doc, cx)?;
        let mut selected = None;
        for space in &drawing.spaces {
            cx.check(Kind::Work, 4).map_err(|stop| stopped(cx, stop))?;
            cx.check(Kind::DecodedBytes, 64)
                .map_err(|stop| stopped(cx, stop))?;
            if id::space(space.id) == request.from {
                selected = Some(space);
                break;
            }
        }
        let space = selected.ok_or_else(|| fail(cx, invalid_value("/from")))?;
        let wanted = std::iter::once(&request.target)
            .chain(&request.second)
            .chain(boundaries)
            .collect::<Vec<_>>();
        let targets = resolve(FAMILY, &wanted, space, &request.from, head, cx)?;
        let owned = 1 + usize::from(request.second.is_some());
        let line_only = request.second.is_some();
        for target in targets.iter().take(owned) {
            if !supported(&target.value.shape, line_only) {
                return Err(report_failed(
                    FAMILY,
                    target.id,
                    Some(&target.value.source),
                    "unsupported-shape",
                    cx,
                ));
            }
        }
        let mut paths = Vec::new();
        reserve(FAMILY, &mut paths, targets.len(), head, cx)?;
        for target in &targets {
            paths.push(
                measurement::path(&target.value.shape, space.id, tolerance, cx)
                    .map_err(|error| failed(FAMILY, target, error, cx))?,
            );
        }
        let Some(first) = targets.first() else {
            return Err(fail(cx, invalid_value("/target")));
        };
        let grid = intersection::shared_grid(&paths, tolerance)
            .map_err(|error| failed(FAMILY, first, error, cx))?;
        let mut sources = Vec::new();
        reserve(FAMILY, &mut sources, owned, head, cx)?;
        for (target, path) in targets.iter().zip(&paths).take(owned) {
            let [piece] = path.pieces.as_slice() else {
                return Err(report_failed(
                    FAMILY,
                    target.id,
                    Some(&target.value.source),
                    "unsupported-shape",
                    cx,
                ));
            };
            let curve = intersection::curve(piece, grid)
                .map_err(|error| failed(FAMILY, target, error, cx))?;
            sources.push(Source { piece, curve });
        }
        let mut edges = Vec::new();
        let mut owners = Vec::new();
        for (index, (target, path)) in targets.iter().zip(&paths).enumerate().skip(owned) {
            let curves = intersection::curves(path, grid, cx)
                .map_err(|error| failed(FAMILY, target, error, cx))?;
            reserve(FAMILY, &mut edges, curves.len(), head, cx)?;
            reserve(FAMILY, &mut owners, curves.len(), head, cx)?;
            for curve in curves {
                edges.push(match curve {
                    intersection::Curve::Segment(value) => t::Boundary::Segment(value),
                    intersection::Curve::Circle(value) => t::Boundary::Circle(value),
                    intersection::Curve::Arc(value) => t::Boundary::Arc(value),
                });
                owners.push(index);
            }
        }
        let operation = match request.operation {
            ConstructOperation::Offset => Operation::Offset(request.distance.unwrap_or_default()),
            ConstructOperation::Trim => {
                let pick = request.pick.as_ref().map_or([0.0, 0.0], |p| [p.x, p.y]);
                let point =
                    intersection::snap(grid, pick).map_err(|_| fail(cx, invalid_value("/pick")))?;
                Operation::Trim(point)
            }
            ConstructOperation::Extend => Operation::Extend(match request.end {
                Some(ConstructEnd::Start) => t::End::Start,
                _ => t::End::End,
            }),
            ConstructOperation::Chamfer => {
                let [a, b] = request
                    .distances
                    .as_deref()
                    .and_then(|list| <[f64; 2]>::try_from(list).ok())
                    .unwrap_or_default();
                Operation::Chamfer([a, b])
            }
            ConstructOperation::Fillet => Operation::Fillet(request.radius.unwrap_or_default()),
        };
        let mut sources = sources.into_iter();
        let Some(source) = sources.next() else {
            return Err(fail(cx, invalid_value("/target")));
        };
        let built = construction::construct(source, sources.next(), &edges, operation, grid, cx)
            .map_err(|error| failed(FAMILY, first, error, cx))?;
        let mut approximate = false;
        for (target, path) in targets.iter().zip(&paths).skip(owned) {
            if path.approximate {
                approximated(FAMILY, target, tolerance, cx)?;
                approximate = true;
            }
        }
        let Built { replace, add, cuts } = built;
        let count = replace.iter().flatten().map(Vec::len).sum::<usize>()
            + usize::from(add.is_some())
            + cuts.len();
        cx.check(
            Kind::DecodedBytes,
            1024 + request.from.len() as u64 + (count as u64).saturating_mul(256),
        )
        .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::Work, count as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let name = |index: usize| {
            targets
                .get(index)
                .map_or_else(String::new, |target| target.id.to_owned())
        };
        Ok(ConstructResult {
            hash,
            from: request.from.clone(),
            unit: unit_of(space.unit),
            tolerance,
            approximate,
            replace: replace
                .into_iter()
                .enumerate()
                .filter_map(|(index, shapes)| {
                    shapes.map(|shapes| ConstructReplacement {
                        id: name(index),
                        geometries: shapes.into_iter().map(geometry).collect(),
                    })
                })
                .collect(),
            add: add.into_iter().map(geometry).collect(),
            cuts: cuts
                .into_iter()
                .map(|value| ConstructCut {
                    boundary: usize::try_from(value.boundary)
                        .ok()
                        .and_then(|edge| owners.get(edge))
                        .map_or_else(String::new, |&owner| name(owner)),
                    position: Point2 {
                        x: value.position[0],
                        y: value.position[1],
                    },
                    rounded: value.rounded,
                    adjusted: value.adjusted,
                })
                .collect(),
        })
    }
}
