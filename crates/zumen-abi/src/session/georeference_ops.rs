//! 図面の実寸から地図への相似の変換（GEODATA の読み取りと対応点からの最小二乗推定、#685）。

use super::*;
use crate::messages::{
    Georeference, GeoreferenceRequest, GeoreferenceResult, MapConversion, MapPoint2, Point2,
};
use zumen_crs::{ControlPoint, Crs, MapPoint, ProjectPoint, fit_map_conversion};

const POINT_BYTES: u64 = 64;

pub(crate) fn georeference_message(geo: zumen_crs::Georeference) -> Georeference {
    let c = geo.conversion;
    Georeference {
        crs: u64::from(geo.crs.epsg()),
        conversion: MapConversion {
            eastings: c.eastings,
            northings: c.northings,
            orthogonal_height: c.orthogonal_height,
            x_axis_abscissa: c.x_axis_abscissa,
            x_axis_ordinate: c.x_axis_ordinate,
            scale: c.scale,
        },
    }
}

fn map_points(
    conversion: zumen_crs::MapConversion,
    points: Option<&[Point2]>,
    cx: &mut Cx<'_>,
) -> Result<Option<Vec<MapPoint2>>, Failed> {
    let Some(points) = points else {
        return Ok(None);
    };
    cx.check(
        Kind::DecodedBytes,
        (points.len() as u64).saturating_mul(POINT_BYTES),
    )
    .map_err(|stop| stopped(cx, stop))?;
    let mut mapped = Vec::with_capacity(points.len());
    for (index, point) in points.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(fail(cx, invalid_value(format!("/points/{index}"))));
        }
        let Some(out) = conversion.to_map(ProjectPoint {
            x: point.x,
            y: point.y,
            z: 0.0,
        }) else {
            return Err(fail(cx, invalid_value(format!("/points/{index}"))));
        };
        mapped.push(MapPoint2 {
            easting: out.point.easting,
            northing: out.point.northing,
        });
    }
    Ok(Some(mapped))
}

fn check_points_finite(points: Option<&[Point2]>, cx: &mut Cx<'_>) -> Result<(), Failed> {
    let Some(points) = points else {
        return Ok(());
    };
    for (index, point) in points.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(fail(cx, invalid_value(format!("/points/{index}"))));
        }
    }
    Ok(())
}

impl Session {
    pub(super) fn georeference(
        &self,
        request: &GeoreferenceRequest,
        cx: &mut Cx<'_>,
    ) -> Result<GeoreferenceResult, Failed> {
        if let Some(doc) = &request.doc {
            if request.crs.is_some() {
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CALL_FIELD_UNSUPPORTED).with("pointer", "/crs"),
                ));
            }
            if request.control_points.is_some() {
                return Err(fail(
                    cx,
                    Diagnostic::new(codes::CALL_FIELD_UNSUPPORTED)
                        .with("pointer", "/control_points"),
                ));
            }
            #[cfg(feature = "drawing")]
            {
                let (file, hash) = self.drawing_file(doc, cx)?;
                let hash =
                    calibration_ops::checked_hash(hash, request.expected_hash.as_deref(), cx)?;
                check_points_finite(request.points.as_deref(), cx)?;
                let drawing = file.lift(cx).map_err(|_| Failed)?;
                let mapped = match drawing.georeference {
                    Some(geo) => map_points(geo.conversion, request.points.as_deref(), cx)?,
                    None => None,
                };
                return Ok(GeoreferenceResult {
                    hash: Some(hash),
                    georeference: drawing.georeference.map(georeference_message),
                    residuals: None,
                    mapped,
                });
            }
            #[cfg(not(feature = "drawing"))]
            {
                if !name::is_safe(doc) {
                    return Err(fail(cx, invalid_value("/doc")));
                }
                let _ = self.document(doc, cx)?;
                return Err(fail(cx, unsupported(doc, "georeference")));
            }
        }
        if request.expected_hash.is_some() {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_FIELD_UNSUPPORTED).with("pointer", "/expected_hash"),
            ));
        }
        let Some(crs_code) = request.crs else {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_FIELD_MISSING).with("pointer", "/crs"),
            ));
        };
        let Some(control_points) = &request.control_points else {
            return Err(fail(
                cx,
                Diagnostic::new(codes::CALL_FIELD_MISSING).with("pointer", "/control_points"),
            ));
        };
        let Some(crs) = u32::try_from(crs_code)
            .ok()
            .and_then(Crs::from_epsg)
            .filter(|crs| crs.zone().is_some())
        else {
            return Err(fail(cx, invalid_value("/crs")));
        };
        check_points_finite(request.points.as_deref(), cx)?;
        cx.check(
            Kind::DecodedBytes,
            (control_points.len() as u64).saturating_mul(POINT_BYTES),
        )
        .map_err(|stop| stopped(cx, stop))?;
        let mut pairs = Vec::with_capacity(control_points.len());
        for pair in control_points {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            pairs.push(ControlPoint {
                drawing: ProjectPoint {
                    x: pair.drawing.x,
                    y: pair.drawing.y,
                    z: 0.0,
                },
                map: MapPoint {
                    northing: pair.map.northing,
                    easting: pair.map.easting,
                },
            });
        }
        let fit = fit_map_conversion(&pairs, cx).ok_or(Failed)?;
        let mapped = map_points(fit.conversion, request.points.as_deref(), cx)?;
        let geo = zumen_crs::Georeference {
            crs,
            conversion: fit.conversion,
        };
        let residuals = fit
            .residuals
            .into_iter()
            .map(|r| MapPoint2 {
                easting: r.easting,
                northing: r.northing,
            })
            .collect();
        Ok(GeoreferenceResult {
            hash: None,
            georeference: Some(georeference_message(geo)),
            residuals: Some(residuals),
            mapped,
        })
    }
}
