//! 実寸の空間と公開 ID で 2 要素を選び、交点・接点・重なりと近似・失敗の理由を返す。

use super::*;
use crate::{
    intersection::{self, Overlap},
    measurement,
    messages::{
        IntersectRequest, IntersectResult, IntersectionOverlap, IntersectionOverlapKind,
        IntersectionPoint, MeasureElement, Point2,
    },
};
use zumen_drawing::{ModelPoint, ModelSpace};

struct Target<'a> {
    id: &'a str,
    value: &'a Element<ModelPoint>,
}

impl Session {
    pub(super) fn intersect(
        &self,
        request: &IntersectRequest,
        cx: &mut Cx<'_>,
    ) -> Result<IntersectResult, Failed> {
        let tolerance = request.tolerance.unwrap_or(0.001);
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(fail(cx, invalid_value("/tolerance")));
        }
        if request.first.id == request.second.id {
            return Err(fail(cx, invalid_value("/second")));
        }
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
        let first = resolve(&request.first, space, &request.from, cx)?;
        let second = resolve(&request.second, space, &request.from, cx)?;
        let a = measurement::path(&first.value.shape, space.id, tolerance, cx)
            .map_err(|error| failed(&first, error, cx))?;
        let b = measurement::path(&second.value.shape, space.id, tolerance, cx)
            .map_err(|error| failed(&second, error, cx))?;
        let grid =
            intersection::grid(&a, &b, tolerance).map_err(|error| failed(&first, error, cx))?;
        let curves_a =
            intersection::curves(&a, grid, cx).map_err(|error| failed(&first, error, cx))?;
        let curves_b =
            intersection::curves(&b, grid, cx).map_err(|error| failed(&second, error, cx))?;
        let found = intersection::intersect(&curves_a, &curves_b, grid, cx)
            .map_err(|error| failed(&first, error, cx))?;
        for (target, path) in [(&first, &a), (&second, &b)] {
            if path.approximate {
                approximated(target, tolerance, cx)?;
            }
        }
        let count = (found.points.len() + found.overlaps.len()) as u64;
        cx.check(
            Kind::DecodedBytes,
            1024 + request.from.len() as u64 + count.saturating_mul(256),
        )
        .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::Work, count)
            .map_err(|stop| stopped(cx, stop))?;
        let point = |[x, y]: [f64; 2]| Point2 { x, y };
        Ok(IntersectResult {
            hash,
            from: request.from.clone(),
            unit: unit_of(space.unit),
            tolerance,
            approximate: a.approximate || b.approximate,
            points: found
                .points
                .into_iter()
                .map(|value| IntersectionPoint {
                    position: point(value.position),
                    tangent: value.tangent,
                    rounded: value.rounded,
                })
                .collect(),
            overlaps: found
                .overlaps
                .into_iter()
                .map(|value| match value {
                    Overlap::Segment { start, end } => IntersectionOverlap {
                        kind: IntersectionOverlapKind::Segment,
                        start: Some(point(start)),
                        end: Some(point(end)),
                        center: None,
                        radius: None,
                    },
                    Overlap::Arc {
                        center,
                        radius,
                        start,
                        end,
                    } => IntersectionOverlap {
                        kind: IntersectionOverlapKind::Arc,
                        start: Some(point(start)),
                        end: Some(point(end)),
                        center: Some(point(center)),
                        radius: Some(radius),
                    },
                    Overlap::Circle { center, radius } => IntersectionOverlap {
                        kind: IntersectionOverlapKind::Circle,
                        start: None,
                        end: None,
                        center: Some(point(center)),
                        radius: Some(radius),
                    },
                })
                .collect(),
        })
    }
}

fn resolve<'a>(
    wanted: &'a MeasureElement,
    space: &'a ModelSpace,
    place: &str,
    cx: &mut Cx<'_>,
) -> Result<Target<'a>, Failed> {
    let mut namer = id::Namer::new(place);
    for (index, value) in space.elements.iter().enumerate() {
        let bytes = value.source.id().map_or(0, |id| id.len() as u64) + place.len() as u64;
        cx.check(Kind::Work, 8 + bytes.div_ceil(16))
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, 1024 + bytes.saturating_mul(4))
            .map_err(|stop| stopped(cx, stop))?;
        if namer.next(index, value) == wanted.id {
            return Ok(Target {
                id: &wanted.id,
                value,
            });
        }
    }
    Err(report_failed(&wanted.id, None, "id-not-found", cx))
}

fn diagnostic_budget(id: &str, source: Option<&Ref>, cx: &mut Cx<'_>) -> Result<(), Failed> {
    let bytes = id.len() as u64 + source.and_then(Ref::id).map_or(0, |id| id.len() as u64);
    cx.check(Kind::Work, 4 + bytes.div_ceil(16))
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, 2048 + bytes.saturating_mul(2))
        .map_err(|stop| stopped(cx, stop))
}

fn report_failed(id: &str, source: Option<&Ref>, reason: &str, cx: &mut Cx<'_>) -> Failed {
    if let Err(failed) = diagnostic_budget(id, source, cx) {
        return failed;
    }
    let mut diagnostic = Diagnostic::new(codes::DRAWING_ELEMENT_INTERSECTION_FAILED)
        .with("id", id)
        .with("reason", reason);
    if let Some(source) = source {
        diagnostic = diagnostic.at(source.clone());
    }
    fail(cx, diagnostic)
}

fn failed(target: &Target<'_>, error: measurement::Error, cx: &mut Cx<'_>) -> Failed {
    match error {
        measurement::Error::Stopped(stop) => {
            fail(cx, stop.to_diagnostic().at(target.value.source.clone()))
        }
        measurement::Error::Geometry(reason) => {
            report_failed(target.id, Some(&target.value.source), reason, cx)
        }
    }
}

fn approximated(target: &Target<'_>, tolerance: f64, cx: &mut Cx<'_>) -> Result<(), Failed> {
    diagnostic_budget(target.id, Some(&target.value.source), cx)?;
    cx.report(
        Diagnostic::new(codes::DRAWING_ELEMENT_INTERSECTION_APPROXIMATED)
            .with("id", target.id)
            .with("tolerance", tolerance)
            .at(target.value.source.clone()),
    );
    Ok(())
}
