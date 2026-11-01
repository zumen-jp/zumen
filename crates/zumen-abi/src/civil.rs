//! `civil` の op（決定 0230）。要求に書いた線形の上の所を、追加距離か測点の表記から求める。

use zumen_base::{Cx, Kind};
use zumen_civil::station::{self, Notation};
use zumen_civil::{
    Alignment, Horizontal, HorizontalKind, StationBreak, Stationing, TransitionCurve, Vertical,
    VerticalKind,
};
use zumen_crs::MapPoint;

use crate::messages::{
    CivilAlignment, CivilHorizontalKind, CivilPosition, CivilRequest, CivilResult,
    CivilTransitionCurve, CivilVerticalKind,
};
use crate::session::{Failed, fail, invalid_value, stopped};

/// 線形の上の所を、distances・stations・interval の順に求める
pub(crate) fn civil(request: &CivilRequest, cx: &mut Cx<'_>) -> Result<CivilResult, Failed> {
    let alignment = alignment(&request.alignment, cx)?;
    let notation = Notation {
        pitch: request.pitch.unwrap_or(20.0),
        digits: request
            .digits
            .map_or(3, |digits| u8::try_from(digits.min(9)).unwrap_or(9)),
    };
    if !(notation.pitch.is_finite() && notation.pitch > 0.0) {
        return Err(fail(cx, invalid_value("/pitch")));
    }
    let length = zumen_civil::length(&alignment);
    let mut distances: Vec<f64> = request.distances.clone().unwrap_or_default();
    for text in request.stations.iter().flatten() {
        match station::distance(&alignment, text, &notation, None, cx) {
            Ok(distance) => distances.push(distance),
            Err(diagnostic) => cx.report(*diagnostic),
        }
    }
    if let Some(interval) = request.interval {
        if !(interval.is_finite() && interval > 0.0) {
            return Err(fail(cx, invalid_value("/interval")));
        }
        let steps = libm::floor(length / interval);
        let steps = if steps.is_finite() { steps as u64 } else { 0 };
        cx.check(Kind::Elements, steps.saturating_add(1))
            .map_err(|stop| stopped(cx, stop))?;
        distances.extend((0..=steps).map(|step| step as f64 * interval));
        if steps as f64 * interval < length {
            distances.push(length);
        }
    }
    cx.check(Kind::Elements, distances.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut positions = Vec::with_capacity(distances.len());
    for distance in distances {
        match zumen_civil::place(&alignment, distance, cx) {
            Ok(placed) => {
                let value = alignment.stations.value(distance);
                positions.push(CivilPosition {
                    distance,
                    station: value,
                    label: notation.format(value),
                    northing: placed.point.northing,
                    easting: placed.point.easting,
                    direction: placed.direction,
                    height: placed.height,
                    gradient: placed.gradient,
                });
            }
            Err(diagnostic) => cx.report(*diagnostic),
        }
    }
    Ok(CivilResult { length, positions })
}

/// 要求の線形をモデルの線形にする。区間の種類に要る欄がなければ、失敗にする
fn alignment(request: &CivilAlignment, cx: &mut Cx<'_>) -> Result<Alignment, Failed> {
    let mut horizontal = Vec::with_capacity(request.horizontal.len());
    for (index, segment) in request.horizontal.iter().enumerate() {
        let kind = match segment.kind {
            CivilHorizontalKind::Line => HorizontalKind::Line,
            CivilHorizontalKind::Arc => HorizontalKind::Arc {
                radius: segment.radius.ok_or_else(|| {
                    fail(
                        cx,
                        invalid_value(format!("/alignment/horizontal/{index}/radius")),
                    )
                })?,
            },
            CivilHorizontalKind::Transition => HorizontalKind::Transition {
                start_radius: segment.start_radius,
                end_radius: segment.end_radius,
                curve: match segment.curve.unwrap_or(CivilTransitionCurve::Clothoid) {
                    CivilTransitionCurve::Clothoid => TransitionCurve::Clothoid,
                    CivilTransitionCurve::CubicParabola => TransitionCurve::CubicParabola,
                    CivilTransitionCurve::SineHalfWave => TransitionCurve::SineHalfWave,
                    CivilTransitionCurve::Cosine => TransitionCurve::Cosine,
                    CivilTransitionCurve::Bloss => TransitionCurve::Bloss,
                    CivilTransitionCurve::Helmert => TransitionCurve::Helmert,
                    CivilTransitionCurve::Viennese => TransitionCurve::Viennese,
                },
            },
        };
        horizontal.push(Horizontal {
            start: MapPoint {
                northing: segment.northing,
                easting: segment.easting,
            },
            direction: segment.direction,
            length: segment.length,
            kind,
        });
    }
    let mut vertical = Vec::new();
    for (index, segment) in request.vertical.iter().flatten().enumerate() {
        let missing = |field: &str| invalid_value(format!("/alignment/vertical/{index}/{field}"));
        let kind = match segment.kind {
            CivilVerticalKind::ConstantGradient => VerticalKind::ConstantGradient,
            CivilVerticalKind::Parabola => VerticalKind::Parabola {
                end_gradient: segment
                    .end_gradient
                    .ok_or_else(|| fail(cx, missing("end_gradient")))?,
            },
            CivilVerticalKind::CircularArc => VerticalKind::CircularArc {
                radius: segment.radius.ok_or_else(|| fail(cx, missing("radius")))?,
            },
        };
        vertical.push(Vertical {
            start_distance: segment.start_distance,
            start_height: segment.start_height,
            start_gradient: segment.start_gradient,
            length: segment.length,
            kind,
        });
    }
    let alignment = Alignment {
        horizontal,
        vertical,
        stations: Stationing {
            start: request.start_station.unwrap_or(0.0),
            breaks: request
                .breaks
                .iter()
                .flatten()
                .map(|at| StationBreak {
                    distance: at.distance,
                    back: at.back,
                    ahead: at.ahead,
                })
                .collect(),
        },
        ..Alignment::default()
    };
    // 区間の初めの点の食い違いは、計算をそのまま続けて知らせる
    zumen_civil::check(&alignment, 1e-3, cx);
    Ok(alignment)
}
