//! 開いた IFC の階から、元の要素へ戻れる図面を作る。

use super::*;
use messages::{BuildingDrawingRequest, BuildingDrawingResult};
use zumen_building::{Building, SpatialKind, drawing, evaluation};
use zumen_crs::ProjectPoint;
use zumen_geom3d::Failure;

impl Session {
    pub(in crate::session) fn building_drawing(
        &mut self,
        request: &BuildingDrawingRequest,
        cx: &mut Cx<'_>,
    ) -> Result<BuildingDrawingResult, Failed> {
        if !name::is_safe(&request.to) {
            return Err(fail(cx, invalid_value("/to")));
        }
        self.check_new_document(&request.to, cx)?;
        let model = self.model(&request.doc, "building_drawing", cx)?;
        let file = model
            .file
            .as_ref()
            .ok_or_else(|| fail(cx, unsupported(&request.doc, "building_drawing")))?;
        let source = binary::Source {
            format: model.format.to_owned(),
            family: "ifc".to_owned(),
            hash: Hash(model.hash.0),
        };
        let (output, bytes) = in_input_order(cx, |cx| {
            let options = options(&model.building, request, cx)?;
            let building = file.lift_geometry(cx).map_err(|_| Failed)?;
            let output = drawing::cut(&building, options, cx).map_err(|e| cut_error(cx, e))?;
            output
                .drawing
                .finish_model_bytes(0, cx)
                .map_err(|s| stopped(cx, s))?;
            let bytes =
                binary::write(&source, &output.drawing, cx).map_err(|error| match error {
                    binary::WriteError::Stopped(s) => stopped(cx, s),
                    binary::WriteError::TooLarge => {
                        fail(cx, Diagnostic::new(codes::DRAWING_BINARY_SIZE_EXCEEDED))
                    }
                })?;
            Ok((output, bytes))
        })?;
        cx.check(Kind::DecodedBytes, 256 + 2 * request.to.len() as u64)
            .and_then(|()| cx.check(Kind::Work, 1))
            .map_err(|s| stopped(cx, s))?;
        let result = BuildingDrawingResult {
            doc: request.to.clone(),
            blob: self.blobs.len() as u64,
            lines: output.statistics.lines,
            complete: output.complete,
            evaluation_failures: output.statistics.evaluation_failures,
            projection_failures: output.statistics.projection_failures,
            collapsed: output.statistics.collapsed,
        };
        self.docs.insert(
            request.to.clone(),
            Document::Drawing {
                hash: source.hash,
                file: Resident::new(Box::new(DrawingFile::Lifted(source, output.drawing))),
            },
        );
        self.blobs.push(Some(Blob {
            kind: BlobKind::Drawing,
            bytes,
        }));
        Ok(result)
    }
}

fn cut_error(cx: &mut Cx<'_>, error: Failure) -> Failed {
    let reason = match error {
        Failure::Stopped(s) => return stopped(cx, s),
        Failure::Unsupported(reason) | Failure::Invalid(reason) => reason.to_owned(),
        Failure::Failed(reason) => reason,
    };
    fail(
        cx,
        Diagnostic::new(codes::BUILDING_DRAWING_FAILED).with("reason", reason),
    )
}

fn options(
    building: &Building,
    request: &BuildingDrawingRequest,
    cx: &mut Cx<'_>,
) -> Result<drawing::Options<'static>, Failed> {
    let storey = request
        .storey
        .parse()
        .ok()
        .and_then(|id| building.find(id))
        .ok_or_else(|| fail(cx, invalid_value("/storey")))?;
    let spatial = building
        .components
        .spatial
        .get(storey)
        .filter(|s| s.kind == SpatialKind::Storey)
        .ok_or_else(|| fail(cx, invalid_value("/storey")))?;
    cx.check(Kind::Work, building.document.units.len() as u64)
        .map_err(|s| stopped(cx, s))?;
    let unit = building
        .document
        .units
        .iter()
        .find(|u| u.kind == "LENGTHUNIT")
        .ok_or_else(|| cut_error(cx, Failure::Invalid("drawing_unit")))?;
    let meters = evaluation::length_scale(unit).map_err(|e| cut_error(cx, e))?;
    let input = match request.unit {
        messages::Unit::Millimeter => Unit::Millimeter,
        messages::Unit::Centimeter => Unit::Centimeter,
        messages::Unit::Meter => Unit::Meter,
        messages::Unit::Inch => Unit::Inch,
        messages::Unit::Foot => Unit::Foot,
        messages::Unit::Unknown => return Err(fail(cx, invalid_value("/unit"))),
    };
    let ratio = input
        .millimeters()
        .ok_or_else(|| fail(cx, invalid_value("/unit")))?
        / 1000.0
        / meters;
    let z = if building.components.placement.get(storey).is_some() {
        building
            .world_placement(storey, cx)
            .map_err(|s| stopped(cx, s))?
            .ok_or_else(|| cut_error(cx, Failure::Invalid("storey_placement")))?
            .0[2][3]
    } else {
        let level = spatial
            .elevation
            .as_ref()
            .ok_or_else(|| cut_error(cx, Failure::Invalid("storey_elevation")))?;
        let unit = level
            .unit
            .as_ref()
            .ok_or_else(|| cut_error(cx, Failure::Invalid("drawing_unit")))?;
        level.value * (evaluation::length_scale(unit).map_err(|e| cut_error(cx, e))? / meters)
    };
    Ok(drawing::Options {
        view: drawing::View::Plan {
            floor: ProjectPoint { x: 0.0, y: 0.0, z },
            height: request.height * ratio,
            depth: request.depth * ratio,
        },
        tolerance: request.tolerance * ratio,
        chord: request.chord * ratio,
        paper: drawing::Paper {
            denominator: request.scale,
            origin: [zumen_drawing::PaperLength(0.0); 2],
            size: None,
        },
        symbols: &[],
    })
}
