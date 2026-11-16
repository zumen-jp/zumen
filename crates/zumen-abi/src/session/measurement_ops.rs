//! 実寸の空間と公開 ID を選び、計測の値・近似・未対応の理由を返す。

use super::*;
use crate::{
    measurement,
    messages::{MeasureQuantity, MeasureRequest, MeasureResult, MeasureTarget, Point2},
};
use zumen_drawing::{ModelPoint, ModelSpace};
use zumen_geom2d::measure::Metric;

enum Target<'a> {
    Element {
        id: &'a str,
        value: &'a Element<ModelPoint>,
    },
    Point([f64; 2]),
}

impl Target<'_> {
    fn id(&self) -> &str {
        match self {
            Self::Element { id, .. } => id,
            Self::Point(_) => "point",
        }
    }
    fn source(&self) -> Option<&Ref> {
        match self {
            Self::Element { value, .. } => Some(&value.source),
            Self::Point(_) => None,
        }
    }
    fn path(
        &self,
        space: &ModelSpace,
        tolerance: f64,
        cx: &mut Cx<'_>,
    ) -> Result<measurement::Path, measurement::Error> {
        match self {
            Self::Element { value, .. } => measurement::path(&value.shape, space.id, tolerance, cx),
            Self::Point(point) => measurement::point_path(*point, cx),
        }
    }
}

impl Session {
    pub(super) fn measure(
        &self,
        request: &MeasureRequest,
        cx: &mut Cx<'_>,
    ) -> Result<MeasureResult, Failed> {
        let tolerance = request.tolerance.unwrap_or(0.001);
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(fail(cx, invalid_value("/tolerance")));
        }
        if (request.quantity == MeasureQuantity::Distance) != request.second.is_some() {
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
        let first = resolve(&request.first, space, &request.from, request.quantity, cx)?;
        let (value, approximate, nearest_first, nearest_second) =
            if let Some(second) = &request.second {
                let second = resolve(second, space, &request.from, request.quantity, cx)?;
                let a = first
                    .path(space, tolerance, cx)
                    .map_err(|error| failed(&first, request.quantity, error, cx))?;
                let b = second
                    .path(space, tolerance, cx)
                    .map_err(|error| failed(&second, request.quantity, error, cx))?;
                let found = measurement::closest(&a, &b, cx)
                    .map_err(|error| failed(&first, request.quantity, error, cx))?;
                if a.approximate {
                    approximated(&first, request.quantity, tolerance, cx)?;
                }
                if b.approximate {
                    approximated(&second, request.quantity, tolerance, cx)?;
                }
                let point = |[x, y]: [f64; 2]| Point2 { x, y };
                (
                    found.distance,
                    a.approximate || b.approximate,
                    Some(point(found.first)),
                    Some(point(found.second)),
                )
            } else {
                let Target::Element { value, .. } = &first else {
                    return Err(failed(
                        &first,
                        request.quantity,
                        measurement::Error::Geometry("point-only-distance"),
                        cx,
                    ));
                };
                let metric = match request.quantity {
                    MeasureQuantity::Length => Metric::Length,
                    MeasureQuantity::Perimeter => Metric::Perimeter,
                    MeasureQuantity::Area => Metric::Area,
                    MeasureQuantity::Distance => return Err(fail(cx, invalid_value("/second"))),
                };
                let found = measurement::scalar(&value.shape, space.id, metric, tolerance, cx)
                    .map_err(|error| failed(&first, request.quantity, error, cx))?;
                if found.approximate {
                    approximated(&first, request.quantity, tolerance, cx)?;
                }
                (found.value, found.approximate, None, None)
            };
        cx.check(Kind::DecodedBytes, 1024 + request.from.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
        Ok(MeasureResult {
            hash,
            from: request.from.clone(),
            quantity: request.quantity,
            value,
            unit: unit_of(space.unit),
            unit_power: if request.quantity == MeasureQuantity::Area {
                2
            } else {
                1
            },
            approximate,
            tolerance,
            nearest_first,
            nearest_second,
        })
    }
}

fn resolve<'a>(
    wanted: &'a MeasureTarget,
    space: &'a ModelSpace,
    place: &str,
    quantity: MeasureQuantity,
    cx: &mut Cx<'_>,
) -> Result<Target<'a>, Failed> {
    let MeasureTarget::Element(wanted) = wanted else {
        let MeasureTarget::Point(value) = wanted else {
            return Err(fail(cx, invalid_value("/first")));
        };
        return Ok(Target::Point([value.position.x, value.position.y]));
    };
    let mut namer = id::Namer::new(place);
    for (index, value) in space.elements.iter().enumerate() {
        let bytes = value.source.id().map_or(0, |id| id.len() as u64) + place.len() as u64;
        cx.check(Kind::Work, 8 + bytes.div_ceil(16))
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, 1024 + bytes.saturating_mul(4))
            .map_err(|stop| stopped(cx, stop))?;
        if namer.next(index, value) == wanted.id {
            return Ok(Target::Element {
                id: &wanted.id,
                value,
            });
        }
    }
    Err(report_failed(
        &wanted.id,
        None,
        quantity,
        "id-not-found",
        cx,
    ))
}

fn diagnostic_budget(id: &str, source: Option<&Ref>, cx: &mut Cx<'_>) -> Result<(), Failed> {
    let bytes = id.len() as u64 + source.and_then(Ref::id).map_or(0, |id| id.len() as u64);
    cx.check(Kind::Work, 4 + bytes.div_ceil(16))
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, 2048 + bytes.saturating_mul(2))
        .map_err(|stop| stopped(cx, stop))
}

fn report_failed(
    id: &str,
    source: Option<&Ref>,
    quantity: MeasureQuantity,
    reason: &str,
    cx: &mut Cx<'_>,
) -> Failed {
    if let Err(failed) = diagnostic_budget(id, source, cx) {
        return failed;
    }
    let mut diagnostic = Diagnostic::new(codes::DRAWING_ELEMENT_MEASUREMENT_FAILED)
        .with("id", id)
        .with("quantity", quantity.name())
        .with("reason", reason);
    if let Some(source) = source {
        diagnostic = diagnostic.at(source.clone());
    }
    fail(cx, diagnostic)
}

fn failed(
    target: &Target<'_>,
    quantity: MeasureQuantity,
    error: measurement::Error,
    cx: &mut Cx<'_>,
) -> Failed {
    match error {
        measurement::Error::Stopped(stop) => {
            let mut diagnostic = stop.to_diagnostic();
            if let Some(source) = target.source() {
                diagnostic = diagnostic.at(source.clone());
            }
            fail(cx, diagnostic)
        }
        measurement::Error::Geometry(reason) => {
            report_failed(target.id(), target.source(), quantity, reason, cx)
        }
    }
}

fn approximated(
    target: &Target<'_>,
    quantity: MeasureQuantity,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<(), Failed> {
    diagnostic_budget(target.id(), target.source(), cx)?;
    let mut diagnostic = Diagnostic::new(codes::DRAWING_ELEMENT_MEASUREMENT_APPROXIMATED)
        .with("id", target.id())
        .with("quantity", quantity.name())
        .with("tolerance", tolerance);
    if let Some(source) = target.source() {
        diagnostic = diagnostic.at(source.clone());
    }
    cx.report(diagnostic);
    Ok(())
}
