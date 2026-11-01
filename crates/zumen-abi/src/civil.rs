//! `civil` の op（決定 0230）。要求に書くか、J-LandXML・IFC4.3 から開いた線形の上の所を、追加距離か測点の表記から求める。

use zumen_base::{Cx, Kind};
use zumen_civil::station::{self, Notation};
use zumen_civil::{
    Alignment, CivilModel, Horizontal, HorizontalKind, StationBreak, Stationing, SurfaceRole, Tin,
    TransitionCurve, Vertical, VerticalKind,
};
use zumen_crs::MapPoint;
#[cfg(feature = "bim")]
use zumen_ifc::IfcFile;
use zumen_landxml::LandXml;
#[cfg(feature = "bim")]
use zumen_sdk::Error;
use zumen_sdk::{Codec, Lift};

use crate::messages::{
    CivilAlignment, CivilAlignmentSummary, CivilHorizontalKind, CivilPosition, CivilRequest,
    CivilResult, CivilSurfaceSummary, CivilTransitionCurve, CivilVerticalKind,
};
use crate::session::{Failed, fail, invalid_value, stopped};

/// J-LandXML なら読み、土木の領域モデルに移す。ほかの形式なら `None`
pub(crate) fn read(bytes: &[u8], cx: &mut Cx<'_>) -> Option<Result<CivilModel, Failed>> {
    if !LandXml::SIGNATURES.iter().any(|rule| rule.matches(bytes)) {
        return None;
    }
    // 失敗の理由は、コーデックと移す処理が診断として足してある
    Some(
        LandXml::read(bytes, cx)
            .and_then(|file| file.lift(cx))
            .map_err(|_| Failed),
    )
}

/// IFC4.3 の線形があれば、土木の領域モデルに移す（決定 0235）。線形がなければ `None`。移せないときは、
/// 理由を診断に足して `None` にし、建物モデルとしては開く
#[cfg(feature = "bim")]
pub(crate) fn from_ifc(file: &IfcFile, cx: &mut Cx<'_>) -> Result<Option<CivilModel>, Failed> {
    let has_alignment = file.ids().any(|id| {
        file.instance(id)
            .is_some_and(|record| file.kind(record).eq_ignore_ascii_case(b"IFCALIGNMENT"))
    });
    if !has_alignment {
        return Ok(None);
    }
    match Lift::<CivilModel>::lift(file, cx) {
        Ok(model) => Ok(Some(model)),
        Err(Error::Stopped(_)) => Err(Failed),
        Err(_) => Ok(None),
    }
}

/// 線形の上の所を、distances・stations・interval の順に求める。線形は、要求に書くか、開いた土木の領域モデル
/// `model` から ID で選ぶ
pub(crate) fn civil(
    request: &CivilRequest,
    model: Option<&CivilModel>,
    cx: &mut Cx<'_>,
) -> Result<CivilResult, Failed> {
    let asks =
        request.distances.is_some() || request.stations.is_some() || request.interval.is_some();
    let (alignment, summary) = match (&request.alignment, model) {
        (Some(written), None) => (Some(alignment(written, cx)?), None),
        (None, Some(model)) => {
            let chosen = match &request.id {
                Some(id) => Some(
                    model
                        .alignments
                        .iter()
                        .find(|alignment| &alignment.id == id)
                        .ok_or_else(|| fail(cx, invalid_value("/id")))?,
                ),
                None => match model.alignments.as_slice() {
                    [only] => Some(only),
                    [] => None,
                    _ if asks => return Err(fail(cx, invalid_value("/id"))),
                    _ => None,
                },
            };
            (chosen.cloned(), Some(model))
        }
        // 線形の書き方は、要求に書くか doc で選ぶかのどちらか 1 つ
        _ => return Err(fail(cx, invalid_value("/alignment"))),
    };
    let (length, positions) = match &alignment {
        Some(alignment) => positions(request, alignment, cx)?,
        None => (0.0, Vec::new()),
    };
    Ok(CivilResult {
        length,
        positions,
        alignment: summary
            .and(alignment.as_ref())
            .map(|alignment| alignment.id.clone()),
        alignments: summary.map(|model| model.alignments.iter().map(describe).collect()),
        surfaces: summary.map(|model| model.surfaces.iter().map(surface).collect()),
        crs: summary
            .and_then(|model| model.crs)
            .map(|crs| u64::from(crs.epsg())),
    })
}

fn describe(alignment: &Alignment) -> CivilAlignmentSummary {
    CivilAlignmentSummary {
        id: alignment.id.clone(),
        name: alignment.name.clone(),
        length: zumen_civil::length(alignment),
        start_station: alignment.stations.start,
        horizontal: alignment.horizontal.len() as u64,
        vertical: alignment.vertical.len() as u64,
        breaks: alignment.stations.breaks.len() as u64,
        cross_sections: alignment.cross_sections.len() as u64,
    }
}

fn surface(tin: &Tin) -> CivilSurfaceSummary {
    CivilSurfaceSummary {
        id: tin.id.clone(),
        name: tin.name.clone(),
        role: match &tin.role {
            SurfaceRole::Existing => "existing".to_string(),
            SurfaceRole::Design => "design".to_string(),
            SurfaceRole::Other(role) => role.clone(),
        },
        points: tin.points.len() as u64,
        triangles: tin.triangles.len() as u64,
    }
}

/// 線形の長さと、求めた所
fn positions(
    request: &CivilRequest,
    alignment: &Alignment,
    cx: &mut Cx<'_>,
) -> Result<(f64, Vec<CivilPosition>), Failed> {
    let notation = Notation {
        pitch: request
            .pitch
            .or(alignment.stations.pitch)
            .unwrap_or(Notation::default().pitch),
        digits: request
            .digits
            .map_or(3, |digits| u8::try_from(digits.min(9)).unwrap_or(9)),
    };
    if !(notation.pitch.is_finite() && notation.pitch > 0.0) {
        return Err(fail(cx, invalid_value("/pitch")));
    }
    let length = zumen_civil::length(alignment);
    let mut distances: Vec<f64> = request.distances.clone().unwrap_or_default();
    for text in request.stations.iter().flatten() {
        match station::distance(alignment, text, &notation, None, cx) {
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
        match zumen_civil::place(alignment, distance, cx) {
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
    Ok((length, positions))
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
            source: Default::default(),
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
            source: Default::default(),
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
                    source: Default::default(),
                })
                .collect(),
            pitch: None,
        },
        ..Alignment::default()
    };
    // 区間の初めの点の食い違いは、計算をそのまま続けて知らせる
    zumen_civil::check(&alignment, 1e-3, cx);
    Ok(alignment)
}
